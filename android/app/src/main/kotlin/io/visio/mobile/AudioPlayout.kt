package io.visio.mobile

import android.media.AudioAttributes
import android.media.AudioFormat
import android.media.AudioManager
import android.media.AudioTrack
import android.util.Log

/**
 * Plays decoded remote audio received from the Rust playout buffer.
 *
 * Polls NativeVideo.nativePullAudioPlayback() on a dedicated thread
 * and writes PCM samples to an Android AudioTrack.
 */
class AudioPlayout {

    companion object {
        private const val TAG = "AudioPlayout"
        private const val SAMPLE_RATE = 48000
        private const val CHANNELS = 1
        private const val FRAME_SIZE_MS = 10
        // 480 samples per 10ms frame at 48kHz mono
        private const val SAMPLES_PER_FRAME = SAMPLE_RATE * FRAME_SIZE_MS / 1000 * CHANNELS
    }

    @Volatile
    private var running = false
    private var playThread: Thread? = null
    private var audioTrack: AudioTrack? = null

    fun start() {
        if (running) return
        running = true

        val bufferSize = maxOf(
            AudioTrack.getMinBufferSize(
                SAMPLE_RATE,
                AudioFormat.CHANNEL_OUT_MONO,
                AudioFormat.ENCODING_PCM_16BIT
            ),
            SAMPLES_PER_FRAME * 2 // 2 bytes per i16 sample
        )

        val track = AudioTrack.Builder()
            .setAudioAttributes(
                AudioAttributes.Builder()
                    .setUsage(AudioAttributes.USAGE_VOICE_COMMUNICATION)
                    .setContentType(AudioAttributes.CONTENT_TYPE_SPEECH)
                    .build()
            )
            .setAudioFormat(
                AudioFormat.Builder()
                    .setSampleRate(SAMPLE_RATE)
                    .setChannelMask(AudioFormat.CHANNEL_OUT_MONO)
                    .setEncoding(AudioFormat.ENCODING_PCM_16BIT)
                    .build()
            )
            .setBufferSizeInBytes(bufferSize)
            .setTransferMode(AudioTrack.MODE_STREAM)
            .build()

        audioTrack = track
        track.play()
        Log.i(TAG, "Audio playout started: ${SAMPLE_RATE}Hz mono, ${FRAME_SIZE_MS}ms frames")

        playThread = Thread({
            val buffer = ShortArray(SAMPLES_PER_FRAME)

            while (running) {
                val pulled = NativeVideo.nativePullAudioPlayback(buffer)
                if (pulled > 0) {
                    track.write(buffer, 0, buffer.size)
                } else {
                    // No data available — sleep briefly to avoid busy-spin
                    Thread.sleep(5)
                }
            }

            track.stop()
            track.release()
            Log.i(TAG, "Audio playout stopped")
        }, "AudioPlayout").also { it.start() }
    }

    fun stop() {
        if (!running) return
        running = false
        playThread?.join(1000)
        playThread = null
        audioTrack = null
    }
}
