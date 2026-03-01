package io.visio.mobile

import android.annotation.SuppressLint
import android.media.AudioFormat
import android.media.AudioRecord
import android.media.MediaRecorder
import android.util.Log
import java.nio.ByteBuffer
import java.nio.ByteOrder

/**
 * Captures microphone audio via AudioRecord and pushes PCM frames
 * into the Rust NativeAudioSource via JNI.
 */
class AudioCapture {

    companion object {
        private const val TAG = "AudioCapture"
        private const val SAMPLE_RATE = 48000
        private const val CHANNELS = 1
        private const val FRAME_SIZE_MS = 10
        // 480 samples per 10ms frame at 48kHz mono
        private const val SAMPLES_PER_FRAME = SAMPLE_RATE * FRAME_SIZE_MS / 1000 * CHANNELS
    }

    @Volatile
    private var running = false
    private var recordThread: Thread? = null

    @SuppressLint("MissingPermission") // Caller must check RECORD_AUDIO permission
    fun start() {
        if (running) return
        running = true

        recordThread = Thread({
            val bufferSize = maxOf(
                AudioRecord.getMinBufferSize(
                    SAMPLE_RATE,
                    AudioFormat.CHANNEL_IN_MONO,
                    AudioFormat.ENCODING_PCM_16BIT
                ),
                SAMPLES_PER_FRAME * 2 // 2 bytes per i16 sample
            )

            val recorder = AudioRecord(
                MediaRecorder.AudioSource.VOICE_COMMUNICATION,
                SAMPLE_RATE,
                AudioFormat.CHANNEL_IN_MONO,
                AudioFormat.ENCODING_PCM_16BIT,
                bufferSize
            )

            if (recorder.state != AudioRecord.STATE_INITIALIZED) {
                Log.e(TAG, "AudioRecord failed to initialize")
                running = false
                return@Thread
            }

            // Direct ByteBuffer for JNI zero-copy
            val buffer = ByteBuffer.allocateDirect(SAMPLES_PER_FRAME * 2)
            buffer.order(ByteOrder.nativeOrder())
            val shortBuffer = buffer.asShortBuffer()

            recorder.startRecording()
            Log.i(TAG, "Audio capture started: ${SAMPLE_RATE}Hz mono, ${FRAME_SIZE_MS}ms frames")

            val tempArray = ShortArray(SAMPLES_PER_FRAME)

            while (running) {
                val read = recorder.read(tempArray, 0, SAMPLES_PER_FRAME)
                if (read > 0) {
                    buffer.clear()
                    shortBuffer.clear()
                    shortBuffer.put(tempArray, 0, read)
                    buffer.position(0)
                    buffer.limit(read * 2)

                    NativeVideo.nativePushAudioFrame(
                        buffer, read, SAMPLE_RATE, CHANNELS
                    )
                }
            }

            recorder.stop()
            recorder.release()
            Log.i(TAG, "Audio capture stopped")
        }, "AudioCapture").also { it.start() }
    }

    fun stop() {
        if (!running) return
        running = false
        recordThread?.join(1000)
        recordThread = null
        NativeVideo.nativeStopAudioCapture()
    }
}
