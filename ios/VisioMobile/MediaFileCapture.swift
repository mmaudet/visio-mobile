import Foundation
import AVFoundation
import CoreMedia
import visioFFI

/// Decodes audio and video from an MP4 file and pushes frames through the C FFI,
/// allowing E2E testing without real camera/microphone hardware.
final class MediaFileCapture: @unchecked Sendable {
    private let filePath: String
    private let audioQueue = DispatchQueue(label: "io.visio.media-file.audio", qos: .userInitiated)
    private let videoQueue = DispatchQueue(label: "io.visio.media-file.video", qos: .userInitiated)
    private let lock = NSLock()
    private var _audioRunning = false
    private var _videoRunning = false

    private var audioRunning: Bool {
        get { lock.withLock { _audioRunning } }
        set { lock.withLock { _audioRunning = newValue } }
    }

    private var videoRunning: Bool {
        get { lock.withLock { _videoRunning } }
        set { lock.withLock { _videoRunning = newValue } }
    }

    private let sampleRate: Int = 48000
    private let numChannels: Int = 1
    private let frameDurationMs: Int = 10
    private let videoFps: Double = 15.0

    init(filePath: String) {
        self.filePath = filePath
    }

    // MARK: - Audio

    func startAudio() {
        guard !audioRunning else { return }
        audioRunning = true

        audioQueue.async { [self] in
            NSLog("MediaFileCapture: audio start, file=%@", filePath)

            let samplesPerFrame = sampleRate * frameDurationMs / 1000  // 480

            while audioRunning {
                guard let (asset, reader, output) = createReader(mediaType: .audio, outputSettings: [
                    AVFormatIDKey: kAudioFormatLinearPCM,
                    AVSampleRateKey: sampleRate,
                    AVNumberOfChannelsKey: numChannels,
                    AVLinearPCMBitDepthKey: 16,
                    AVLinearPCMIsFloatKey: false,
                    AVLinearPCMIsBigEndianKey: false,
                ]) else {
                    NSLog("MediaFileCapture: failed to create audio reader, retrying in 1s")
                    Thread.sleep(forTimeInterval: 1.0)
                    continue
                }
                _ = asset  // keep asset alive

                reader.startReading()

                var buffer = [Int16](repeating: 0, count: samplesPerFrame)
                var residual = [Int16]()
                var residualOffset = 0

                while audioRunning && reader.status == .reading {
                    guard let sampleBuffer = output.copyNextSampleBuffer() else { break }

                    guard let blockBuffer = CMSampleBufferGetDataBuffer(sampleBuffer) else { continue }

                    var lengthAtOffset: Int = 0
                    var totalLength: Int = 0
                    var dataPointer: UnsafeMutablePointer<Int8>?
                    let status = CMBlockBufferGetDataPointer(
                        blockBuffer,
                        atOffset: 0,
                        lengthAtOffsetOut: &lengthAtOffset,
                        totalLengthOut: &totalLength,
                        dataPointerOut: &dataPointer
                    )
                    guard status == kCMBlockBufferNoErr, let dataPointer else { continue }

                    let sampleCount = totalLength / MemoryLayout<Int16>.size
                    let samplesPtr = UnsafeRawPointer(dataPointer).bindMemory(to: Int16.self, capacity: sampleCount)

                    // Append decoded samples to residual buffer.
                    residual.append(contentsOf: UnsafeBufferPointer(start: samplesPtr, count: sampleCount))

                    // Push complete frames.
                    let available = residual.count - residualOffset
                    while audioRunning && available >= samplesPerFrame && (residual.count - residualOffset) >= samplesPerFrame {
                        for i in 0..<samplesPerFrame {
                            buffer[i] = residual[residualOffset + i]
                        }
                        residualOffset += samplesPerFrame

                        buffer.withUnsafeBufferPointer { ptr in
                            guard let base = ptr.baseAddress else { return }
                            visio_push_ios_audio_frame(
                                base,
                                UInt32(samplesPerFrame),
                                UInt32(sampleRate),
                                UInt32(numChannels)
                            )
                        }

                        Thread.sleep(forTimeInterval: Double(frameDurationMs) / 1000.0)
                    }

                    // Compact residual when offset grows large.
                    if residualOffset > 4096 {
                        residual.removeFirst(residualOffset)
                        residualOffset = 0
                    }
                }

                reader.cancelReading()
                NSLog("MediaFileCapture: audio reached end of file, looping")
            }

            NSLog("MediaFileCapture: audio stopped")
        }
    }

    func stopAudio() {
        audioRunning = false
    }

    // MARK: - Video

    func startVideo() {
        guard !videoRunning else { return }
        videoRunning = true

        videoQueue.async { [self] in
            NSLog("MediaFileCapture: video start, file=%@", filePath)

            let frameDuration = 1.0 / videoFps

            // Pre-allocate U/V plane buffers (resized if resolution changes).
            var uPlane = [UInt8]()
            var vPlane = [UInt8]()

            while videoRunning {
                guard let (asset, reader, output) = createReader(mediaType: .video, outputSettings: [
                    kCVPixelBufferPixelFormatTypeKey as String: kCVPixelFormatType_420YpCbCr8BiPlanarFullRange,
                ]) else {
                    NSLog("MediaFileCapture: failed to create video reader, retrying in 1s")
                    Thread.sleep(forTimeInterval: 1.0)
                    continue
                }
                _ = asset  // keep asset alive

                reader.startReading()
                var frameCount: UInt64 = 0

                while videoRunning && reader.status == .reading {
                    let frameStart = CFAbsoluteTimeGetCurrent()

                    guard let sampleBuffer = output.copyNextSampleBuffer() else { break }
                    guard let pixelBuffer = CMSampleBufferGetImageBuffer(sampleBuffer) else { continue }

                    pushNV12FrameToRust(pixelBuffer, uPlane: &uPlane, vPlane: &vPlane)
                    frameCount += 1

                    if frameCount % 30 == 1 {
                        let w = CVPixelBufferGetWidth(pixelBuffer)
                        let h = CVPixelBufferGetHeight(pixelBuffer)
                        NSLog("MediaFileCapture: video frame #%llu, %dx%d", frameCount, w, h)
                    }

                    // Pace to target fps.
                    let elapsed = CFAbsoluteTimeGetCurrent() - frameStart
                    let sleepTime = frameDuration - elapsed
                    if sleepTime > 0 {
                        Thread.sleep(forTimeInterval: sleepTime)
                    }
                }

                reader.cancelReading()
                NSLog("MediaFileCapture: video reached end of file (%llu frames), looping", frameCount)
            }

            NSLog("MediaFileCapture: video stopped")
        }
    }

    func stopVideo() {
        videoRunning = false
    }

    // MARK: - Private helpers

    private func createReader(mediaType: AVMediaType, outputSettings: [String: Any]) -> (AVAsset, AVAssetReader, AVAssetReaderTrackOutput)? {
        let url = URL(fileURLWithPath: filePath)
        let asset = AVAsset(url: url)

        guard let track = asset.tracks(withMediaType: mediaType).first else {
            NSLog("MediaFileCapture: no %@ track in %@", mediaType.rawValue, filePath)
            return nil
        }

        let output = AVAssetReaderTrackOutput(track: track, outputSettings: outputSettings)

        guard let reader = try? AVAssetReader(asset: asset) else {
            NSLog("MediaFileCapture: failed to create AVAssetReader for %@", mediaType.rawValue)
            return nil
        }

        if reader.canAdd(output) {
            reader.add(output)
        } else {
            NSLog("MediaFileCapture: cannot add %@ output to reader", mediaType.rawValue)
            return nil
        }

        return (asset, reader, output)
    }
}
