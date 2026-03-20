import AVFoundation
import visioFFI

/// Thread-safe wrapper for AVAudioPCMBuffer used in @Sendable tap closures.
private struct SendableAudioBuffer: @unchecked Sendable {
    let buffer: AVAudioPCMBuffer
}

/// Captures microphone audio via AVAudioEngine, resamples to 48 kHz if the
/// hardware rate differs (e.g. Bluetooth HFP), and pushes Int16 PCM frames
/// to Rust via visio_push_ios_audio_frame().
///
/// Thread safety: `@unchecked Sendable` because `start()`/`stop()` are called
/// from MainActor (via VisioManager) and the audio tap closure runs on the
/// AVAudioEngine render thread. The `isRunning` flag is protected by `lock`.
final class AudioCapture: @unchecked Sendable {
    private let engine = AVAudioEngine()
    private let lock = NSLock()
    private var _isRunning = false

    private var isRunning: Bool {
        get { lock.withLock { _isRunning } }
        set { lock.withLock { _isRunning = newValue } }
    }

    private let outputSampleRate: Double = 48_000
    private let channels: UInt32 = 1
    private let outputSamplesPerFrame: Int = 480 // 10ms at 48 kHz

    func start() {
        guard !isRunning else { return }

        let inputNode = engine.inputNode
        let hwFormat = inputNode.inputFormat(forBus: 0)
        NSLog("AudioCapture: hardware format: %@", hwFormat.description)

        guard hwFormat.sampleRate > 0 else {
            NSLog("AudioCapture: no input device available (sample rate = 0)")
            return
        }

        guard let tapFormat = AVAudioFormat(
            commonFormat: .pcmFormatFloat32,
            sampleRate: hwFormat.sampleRate,
            channels: AVAudioChannelCount(channels),
            interleaved: false
        ) else {
            NSLog("AudioCapture: failed to create tap format")
            return
        }

        let needsResample = hwFormat.sampleRate != outputSampleRate
        var converter: AVAudioConverter?
        var outputFormat: AVAudioFormat?
        if needsResample {
            guard let outFmt = AVAudioFormat(
                commonFormat: .pcmFormatFloat32,
                sampleRate: outputSampleRate,
                channels: AVAudioChannelCount(channels),
                interleaved: false
            ) else {
                NSLog("AudioCapture: failed to create output format")
                return
            }
            outputFormat = outFmt
            converter = AVAudioConverter(from: tapFormat, to: outFmt)
            NSLog("AudioCapture: resampling from %.0f Hz to %.0f Hz", hwFormat.sampleRate, outputSampleRate)
        }

        let outputSamplesPerFrame = self.outputSamplesPerFrame
        let outputRate = UInt32(self.outputSampleRate)
        let channels = self.channels

        let tapBufferSize = AVAudioFrameCount(hwFormat.sampleRate * 0.01) // 10ms

        inputNode.installTap(onBus: 0, bufferSize: tapBufferSize, format: tapFormat) { buffer, _ in
            let floatBuffer: AVAudioPCMBuffer

            if needsResample, let converter = converter, let outputFormat = outputFormat {
                let ratio = outputFormat.sampleRate / tapFormat.sampleRate
                let outputCapacity = AVAudioFrameCount(Double(buffer.frameLength) * ratio) + 1
                guard let resampledBuffer = AVAudioPCMBuffer(pcmFormat: outputFormat, frameCapacity: outputCapacity) else {
                    return
                }
                var error: NSError?
                let inputBox = SendableAudioBuffer(buffer: buffer)
                final class ConsumedFlag: @unchecked Sendable { var value = false }
                let consumed = ConsumedFlag()
                converter.convert(to: resampledBuffer, error: &error) { _, outStatus in
                    if consumed.value {
                        outStatus.pointee = .noDataNow
                        return nil
                    }
                    consumed.value = true
                    outStatus.pointee = .haveData
                    return inputBox.buffer
                }
                if let error = error {
                    NSLog("AudioCapture: resample error: %@", error.localizedDescription)
                    return
                }
                floatBuffer = resampledBuffer
            } else {
                floatBuffer = buffer
            }

            guard let floatData = floatBuffer.floatChannelData?[0] else { return }
            let frameCount = Int(floatBuffer.frameLength)

            // Float32 → Int16, push in 10ms chunks
            var i16Buf = [Int16](repeating: 0, count: frameCount)
            for i in 0..<frameCount {
                i16Buf[i] = Int16(clamping: Int(floatData[i] * 32767.0))
            }

            var offset = 0
            while offset + outputSamplesPerFrame <= frameCount {
                i16Buf.withUnsafeBufferPointer { ptr in
                    guard let base = ptr.baseAddress else { return }
                    visio_push_ios_audio_frame(
                        base.advanced(by: offset),
                        UInt32(outputSamplesPerFrame),
                        outputRate,
                        channels
                    )
                }
                offset += outputSamplesPerFrame
            }
        }

        do {
            try engine.start()
            isRunning = true
            NSLog("AudioCapture: started (hw=%.0f Hz, output=48000 Hz, %d samples/frame)",
                  hwFormat.sampleRate, outputSamplesPerFrame)
        } catch {
            NSLog("AudioCapture: failed to start engine: %@", error.localizedDescription)
        }
    }

    func stop() {
        guard isRunning else { return }
        engine.inputNode.removeTap(onBus: 0)
        engine.stop()
        isRunning = false
        NSLog("AudioCapture: stopped")
    }
}
