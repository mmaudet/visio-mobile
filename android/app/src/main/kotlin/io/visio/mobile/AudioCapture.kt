package io.visio.mobile

import android.annotation.SuppressLint
import android.media.AudioDeviceInfo
import android.media.AudioFormat
import android.media.AudioRecord
import android.media.MediaRecorder
import android.util.Log
import java.nio.ByteBuffer
import java.nio.ByteOrder

/**
 * Captures microphone audio via AudioRecord and pushes PCM frames
 * into the Rust NativeAudioSource via JNI.
 *
 * Also supports synthetic audio mode (440Hz sine wave) for E2E testing
 * on emulators that have no real microphone.
 */
class AudioCapture {
    companion object {
        private const val TAG = "AudioCapture"
        private const val SAMPLE_RATE = 48000
        private const val CHANNELS = 1
        private const val FRAME_SIZE_MS = 10

        // 480 samples per 10ms frame at 48kHz mono
        private const val SAMPLES_PER_FRAME = SAMPLE_RATE * FRAME_SIZE_MS / 1000 * CHANNELS
        private const val SINE_FREQ = 440.0
        private const val SINE_AMPLITUDE = 3000.0
    }

    private val lock = Any()

    @Volatile
    private var running = false
    private var recordThread: Thread? = null
    private var recorder: AudioRecord? = null
    private var synthetic = false

    @SuppressLint("MissingPermission") // Caller must check RECORD_AUDIO permission
    fun start(device: AudioDeviceInfo? = null) {
        synchronized(lock) {
            if (running) return
            running = true
            synthetic = false

            val bufferSize =
                maxOf(
                    AudioRecord.getMinBufferSize(
                        SAMPLE_RATE,
                        AudioFormat.CHANNEL_IN_MONO,
                        AudioFormat.ENCODING_PCM_16BIT,
                    ),
                    // 2 bytes per i16 sample
                    SAMPLES_PER_FRAME * 2,
                )

            val rec =
                AudioRecord(
                    MediaRecorder.AudioSource.VOICE_COMMUNICATION,
                    SAMPLE_RATE,
                    AudioFormat.CHANNEL_IN_MONO,
                    AudioFormat.ENCODING_PCM_16BIT,
                    bufferSize,
                )

            if (rec.state != AudioRecord.STATE_INITIALIZED) {
                Log.e(TAG, "AudioRecord failed to initialize")
                running = false
                return
            }

            recorder = rec
            // Set preferred device BEFORE startRecording()
            if (device != null) {
                rec.setPreferredDevice(device)
                Log.i(TAG, "Audio capture preferred device set before recording: ${device.productName}")
            }
            rec.startRecording()
        }

        Log.i(TAG, "Audio capture started: ${SAMPLE_RATE}Hz mono, ${FRAME_SIZE_MS}ms frames")

        recordThread =
            Thread({
                val rec = synchronized(lock) { recorder } ?: return@Thread

                // Direct ByteBuffer for JNI zero-copy
                val buffer = ByteBuffer.allocateDirect(SAMPLES_PER_FRAME * 2)
                buffer.order(ByteOrder.nativeOrder())
                val shortBuffer = buffer.asShortBuffer()

                android.os.Process.setThreadPriority(android.os.Process.THREAD_PRIORITY_URGENT_AUDIO)

                val tempArray = ShortArray(SAMPLES_PER_FRAME)

                while (running) {
                    val read = rec.read(tempArray, 0, SAMPLES_PER_FRAME)
                    if (read > 0) {
                        buffer.clear()
                        shortBuffer.clear()
                        shortBuffer.put(tempArray, 0, read)
                        buffer.position(0)
                        buffer.limit(read * 2)

                        NativeVideo.nativePushAudioFrame(
                            buffer, read, SAMPLE_RATE, CHANNELS,
                        )
                    }
                }

                Log.i(TAG, "Audio capture stopped")
            }, "AudioCapture").also { it.start() }
    }

    /**
     * Start synthetic audio capture (440Hz sine wave).
     * Used for E2E testing on emulators with no real microphone.
     */
    fun startSynthetic() {
        synchronized(lock) {
            if (running) return
            running = true
            synthetic = true
        }

        Log.i(TAG, "Synthetic audio capture started: ${SINE_FREQ}Hz sine, ${SAMPLE_RATE}Hz, ${FRAME_SIZE_MS}ms frames")

        recordThread =
            Thread({
                val buffer = ByteBuffer.allocateDirect(SAMPLES_PER_FRAME * 2)
                buffer.order(ByteOrder.nativeOrder())
                val shortBuffer = buffer.asShortBuffer()

                android.os.Process.setThreadPriority(android.os.Process.THREAD_PRIORITY_URGENT_AUDIO)

                val tempArray = ShortArray(SAMPLES_PER_FRAME)
                var sampleOffset = 0L

                while (running) {
                    for (i in 0 until SAMPLES_PER_FRAME) {
                        val t = (sampleOffset + i) / SAMPLE_RATE.toDouble()
                        tempArray[i] = (Math.sin(t * SINE_FREQ * 2.0 * Math.PI) * SINE_AMPLITUDE).toInt().toShort()
                    }
                    sampleOffset += SAMPLES_PER_FRAME

                    buffer.clear()
                    shortBuffer.clear()
                    shortBuffer.put(tempArray, 0, SAMPLES_PER_FRAME)
                    buffer.position(0)
                    buffer.limit(SAMPLES_PER_FRAME * 2)

                    NativeVideo.nativePushAudioFrame(
                        buffer, SAMPLES_PER_FRAME, SAMPLE_RATE, CHANNELS,
                    )

                    Thread.sleep(FRAME_SIZE_MS.toLong())
                }

                Log.i(TAG, "Synthetic audio capture stopped")
            }, "SyntheticAudioCapture").also { it.start() }
    }

    fun setPreferredDevice(device: AudioDeviceInfo?) {
        recorder?.setPreferredDevice(device)
    }

    fun switchDevice(device: AudioDeviceInfo?) {
        val wasRunning: Boolean
        synchronized(lock) {
            wasRunning = running
        }
        if (wasRunning) {
            stop()
            start(device)
        }
    }

    fun stop() {
        val thread: Thread?
        val rec: AudioRecord?
        synchronized(lock) {
            if (!running) return
            running = false
            thread = recordThread
            recordThread = null
            rec = recorder
            recorder = null
        }
        thread?.let {
            it.join(1000)
            if (it.isAlive) {
                Log.w(TAG, "Capture thread did not stop within 1s, interrupting")
                it.interrupt()
            }
        }
        rec?.let {
            it.stop()
            it.release()
        }
        NativeVideo.nativeStopAudioCapture()
    }
}
