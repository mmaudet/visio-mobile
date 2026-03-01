import AVFoundation
import visioFFI

/// Pulls decoded remote audio from the Rust AudioPlayoutBuffer via C FFI
/// and plays it through AVAudioEngine using a pull-based AVAudioSourceNode.
///
/// Format: 48 kHz mono Int16 (matches the Rust playout buffer).
/// Audio session category: .playback (no mic needed on simulator).
final class AudioPlayout {
    private let engine = AVAudioEngine()
    private var sourceNode: AVAudioSourceNode?

    /// 48 kHz mono — matches Rust AudioPlayoutBuffer output.
    private let sampleRate: Double = 48_000
    private let channelCount: AVAudioChannelCount = 1

    func start() {
        configureSession()

        let format = AVAudioFormat(
            commonFormat: .pcmFormatFloat32,
            sampleRate: sampleRate,
            channels: channelCount,
            interleaved: false
        )!

        // AVAudioSourceNode: the render callback pulls samples on the audio IO thread.
        let node = AVAudioSourceNode { _, _, frameCount, bufferList -> OSStatus in
            let capacity = Int(frameCount)
            // Allocate stack-like temp buffer for i16 samples.
            var i16Buf = [Int16](repeating: 0, count: capacity)
            let pulled = visio_pull_audio_playback(&i16Buf, UInt32(capacity))
            _ = pulled // pulled count for diagnostics; buffer is zero-filled for silence

            // Convert i16 → Float32 for AVAudioEngine.
            let ablPointer = UnsafeMutableAudioBufferListPointer(bufferList)
            guard let floatPtr = ablPointer[0].mData?.assumingMemoryBound(to: Float32.self) else {
                return noErr
            }
            for i in 0..<capacity {
                floatPtr[i] = Float32(i16Buf[i]) / 32768.0
            }
            ablPointer[0].mDataByteSize = UInt32(capacity * MemoryLayout<Float32>.size)

            return noErr
        }

        sourceNode = node
        engine.attach(node)
        engine.connect(node, to: engine.mainMixerNode, format: format)

        do {
            try engine.start()
            print("AudioPlayout: engine started (48kHz mono)")
        } catch {
            print("AudioPlayout: failed to start engine: \(error)")
        }
    }

    func stop() {
        engine.stop()
        if let node = sourceNode {
            engine.detach(node)
            sourceNode = nil
        }
        print("AudioPlayout: stopped")
    }

    private func configureSession() {
        let session = AVAudioSession.sharedInstance()
        do {
            // .playback — no microphone access needed (simulator has no mic).
            try session.setCategory(.playback, mode: .voiceChat)
            try session.setPreferredSampleRate(sampleRate)
            try session.setActive(true)
        } catch {
            print("AudioPlayout: session config failed: \(error)")
        }
    }
}
