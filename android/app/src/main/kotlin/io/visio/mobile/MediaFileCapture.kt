package io.visio.mobile

import android.media.MediaCodec
import android.media.MediaCodecInfo
import android.media.MediaExtractor
import android.media.MediaFormat
import android.util.Log
import java.nio.ByteBuffer
import java.nio.ByteOrder

/**
 * Decodes audio and video from an MP4 file using MediaExtractor + MediaCodec,
 * and pushes frames through the existing JNI FFI for injection into LiveKit.
 *
 * Audio is resampled to 48kHz mono i16 PCM and pushed in 10ms frames.
 * Video is decoded to YUV_420_888 and pushed at ~15fps.
 * Both tracks loop when the file ends.
 */
class MediaFileCapture(private val filePath: String) {
    companion object {
        private const val TAG = "MediaFileCapture"

        // Audio output constants (must match LiveKit NativeAudioSource expectations)
        private const val TARGET_SAMPLE_RATE = 48000
        private const val TARGET_CHANNELS = 1
        private const val FRAME_SIZE_MS = 10
        private const val SAMPLES_PER_FRAME = TARGET_SAMPLE_RATE * FRAME_SIZE_MS / 1000 // 480

        // Video output constants
        private const val TARGET_FPS = 15
        private const val FRAME_INTERVAL_MS = 1000L / TARGET_FPS // ~66ms
        private const val TARGET_VIDEO_WIDTH = 640
        private const val TARGET_VIDEO_HEIGHT = 480

        private const val CODEC_TIMEOUT_US = 10_000L // 10ms
    }

    @Volatile private var audioRunning = false

    @Volatile private var videoRunning = false

    private var audioThread: Thread? = null
    private var videoThread: Thread? = null

    /**
     * Start decoding and pushing audio from the MP4 file.
     * Audio is resampled to 48kHz mono and pushed as 10ms PCM frames.
     */
    fun startAudio() {
        if (audioRunning) return
        audioRunning = true

        audioThread =
            Thread({
                android.os.Process.setThreadPriority(android.os.Process.THREAD_PRIORITY_URGENT_AUDIO)
                Log.i(TAG, "Audio decode thread started for: $filePath")

                while (audioRunning) {
                    try {
                        decodeAudioPass()
                    } catch (e: Exception) {
                        Log.e(TAG, "Audio decode error, stopping", e)
                        audioRunning = false
                    }
                }

                Log.i(TAG, "Audio decode thread stopped")
            }, "MediaFileCapture-Audio").also { it.start() }
    }

    /**
     * Start decoding and pushing video from the MP4 file.
     * Video is decoded to YUV and pushed at ~15fps.
     */
    fun startVideo() {
        if (videoRunning) return
        videoRunning = true

        videoThread =
            Thread({
                android.os.Process.setThreadPriority(android.os.Process.THREAD_PRIORITY_MORE_FAVORABLE)
                Log.i(TAG, "Video decode thread started for: $filePath")

                while (videoRunning) {
                    try {
                        decodeVideoPass()
                    } catch (e: Exception) {
                        Log.e(TAG, "Video decode error, stopping", e)
                        videoRunning = false
                    }
                }

                Log.i(TAG, "Video decode thread stopped")
            }, "MediaFileCapture-Video").also { it.start() }
    }

    fun stopAudio() {
        audioRunning = false
        audioThread?.let {
            it.join(2000)
            if (it.isAlive) {
                Log.w(TAG, "Audio thread did not stop in 2s, interrupting")
                it.interrupt()
            }
        }
        audioThread = null
    }

    fun stopVideo() {
        videoRunning = false
        videoThread?.let {
            it.join(2000)
            if (it.isAlive) {
                Log.w(TAG, "Video thread did not stop in 2s, interrupting")
                it.interrupt()
            }
        }
        videoThread = null
    }

    // ---- Audio decoding (one full pass of the file) ----

    private fun decodeAudioPass() {
        val extractor = MediaExtractor()
        extractor.setDataSource(filePath)

        val trackIndex = findTrack(extractor, "audio/")
        if (trackIndex < 0) {
            Log.e(TAG, "No audio track found in $filePath")
            audioRunning = false
            extractor.release()
            return
        }

        extractor.selectTrack(trackIndex)
        val format = extractor.getTrackFormat(trackIndex)
        val mime = format.getString(MediaFormat.KEY_MIME) ?: "audio/mp4a-latm"
        val srcSampleRate = format.getInteger(MediaFormat.KEY_SAMPLE_RATE)
        val srcChannels = format.getInteger(MediaFormat.KEY_CHANNEL_COUNT)
        Log.i(TAG, "Audio track: mime=$mime, rate=$srcSampleRate, channels=$srcChannels")

        val codec = MediaCodec.createDecoderByType(mime)
        codec.configure(format, null, null, 0)
        codec.start()

        val info = MediaCodec.BufferInfo()

        // Accumulation buffer for resampled mono samples.
        // We accumulate decoded PCM, resample, then push in 480-sample chunks.
        val accumBuffer = ShortArray(SAMPLES_PER_FRAME * 4) // room for accumulation
        var accumCount = 0

        // Direct ByteBuffer for JNI push (480 samples * 2 bytes)
        val pushBuffer = ByteBuffer.allocateDirect(SAMPLES_PER_FRAME * 2)
        pushBuffer.order(ByteOrder.nativeOrder())
        val pushShortBuffer = pushBuffer.asShortBuffer()

        var inputDone = false

        try {
            while (audioRunning) {
                // Feed input
                if (!inputDone) {
                    val inIdx = codec.dequeueInputBuffer(CODEC_TIMEOUT_US)
                    if (inIdx >= 0) {
                        val inBuf = codec.getInputBuffer(inIdx)!!
                        val sampleSize = extractor.readSampleData(inBuf, 0)
                        if (sampleSize < 0) {
                            codec.queueInputBuffer(inIdx, 0, 0, 0, MediaCodec.BUFFER_FLAG_END_OF_STREAM)
                            inputDone = true
                        } else {
                            codec.queueInputBuffer(inIdx, 0, sampleSize, extractor.sampleTime, 0)
                            extractor.advance()
                        }
                    }
                }

                // Drain output
                val outIdx = codec.dequeueOutputBuffer(info, CODEC_TIMEOUT_US)
                if (outIdx >= 0) {
                    if (info.size > 0) {
                        val outBuf = codec.getOutputBuffer(outIdx)!!
                        outBuf.position(info.offset)
                        outBuf.limit(info.offset + info.size)

                        // Output is PCM 16-bit, possibly multi-channel
                        val sampleCount = info.size / 2 // total i16 samples across all channels
                        val frameSamples = sampleCount / srcChannels // per-channel sample count

                        // Read decoded PCM into temp array
                        val pcm = ShortArray(sampleCount)
                        outBuf.asShortBuffer().get(pcm)

                        // Mix down to mono if stereo+
                        val mono =
                            if (srcChannels > 1) {
                                ShortArray(frameSamples) { i ->
                                    var sum = 0
                                    for (ch in 0 until srcChannels) {
                                        sum += pcm[i * srcChannels + ch].toInt()
                                    }
                                    (sum / srcChannels).toShort()
                                }
                            } else {
                                pcm
                            }

                        // Resample to 48kHz if needed (linear interpolation)
                        val resampled =
                            if (srcSampleRate != TARGET_SAMPLE_RATE) {
                                resampleLinear(mono, srcSampleRate, TARGET_SAMPLE_RATE)
                            } else {
                                mono
                            }

                        // Accumulate and push 480-sample frames
                        var srcOffset = 0
                        while (srcOffset < resampled.size && audioRunning) {
                            val toCopy = minOf(resampled.size - srcOffset, SAMPLES_PER_FRAME - accumCount)
                            System.arraycopy(resampled, srcOffset, accumBuffer, accumCount, toCopy)
                            accumCount += toCopy
                            srcOffset += toCopy

                            if (accumCount >= SAMPLES_PER_FRAME) {
                                pushBuffer.clear()
                                pushShortBuffer.clear()
                                pushShortBuffer.put(accumBuffer, 0, SAMPLES_PER_FRAME)
                                pushBuffer.position(0)
                                pushBuffer.limit(SAMPLES_PER_FRAME * 2)

                                NativeVideo.nativePushAudioFrame(
                                    pushBuffer,
                                    SAMPLES_PER_FRAME,
                                    TARGET_SAMPLE_RATE,
                                    TARGET_CHANNELS,
                                )

                                // Pace at real-time (10ms per frame)
                                Thread.sleep(FRAME_SIZE_MS.toLong())

                                accumCount = 0
                            }
                        }
                    }
                    codec.releaseOutputBuffer(outIdx, false)

                    if (info.flags and MediaCodec.BUFFER_FLAG_END_OF_STREAM != 0) {
                        Log.i(TAG, "Audio end of stream, looping")
                        break // exit inner loop to restart from beginning
                    }
                }
            }
        } finally {
            codec.stop()
            codec.release()
            extractor.release()
        }
    }

    // ---- Video decoding (one full pass of the file) ----

    private fun decodeVideoPass() {
        val extractor = MediaExtractor()
        extractor.setDataSource(filePath)

        val trackIndex = findTrack(extractor, "video/")
        if (trackIndex < 0) {
            Log.e(TAG, "No video track found in $filePath")
            videoRunning = false
            extractor.release()
            return
        }

        extractor.selectTrack(trackIndex)
        val format = extractor.getTrackFormat(trackIndex)
        val mime = format.getString(MediaFormat.KEY_MIME) ?: "video/avc"
        val width = format.getInteger(MediaFormat.KEY_WIDTH)
        val height = format.getInteger(MediaFormat.KEY_HEIGHT)
        Log.i(TAG, "Video track: mime=$mime, ${width}x$height")

        // Request YUV output
        format.setInteger(
            MediaFormat.KEY_COLOR_FORMAT,
            MediaCodecInfo.CodecCapabilities.COLOR_FormatYUV420Flexible,
        )

        val codec = MediaCodec.createDecoderByType(mime)
        codec.configure(format, null, null, 0)
        codec.start()

        val info = MediaCodec.BufferInfo()
        var inputDone = false

        try {
            while (videoRunning) {
                // Feed input
                if (!inputDone) {
                    val inIdx = codec.dequeueInputBuffer(CODEC_TIMEOUT_US)
                    if (inIdx >= 0) {
                        val inBuf = codec.getInputBuffer(inIdx)!!
                        val sampleSize = extractor.readSampleData(inBuf, 0)
                        if (sampleSize < 0) {
                            codec.queueInputBuffer(inIdx, 0, 0, 0, MediaCodec.BUFFER_FLAG_END_OF_STREAM)
                            inputDone = true
                        } else {
                            codec.queueInputBuffer(inIdx, 0, sampleSize, extractor.sampleTime, 0)
                            extractor.advance()
                        }
                    }
                }

                // Drain output
                val outIdx = codec.dequeueOutputBuffer(info, CODEC_TIMEOUT_US)
                if (outIdx >= 0) {
                    if (info.size > 0) {
                        val image = codec.getOutputImage(outIdx)
                        if (image != null) {
                            val yPlane = image.planes[0]
                            val uPlane = image.planes[1]
                            val vPlane = image.planes[2]

                            val outWidth = image.width
                            val outHeight = image.height

                            NativeVideo.nativePushCameraFrame(
                                yPlane.buffer,
                                uPlane.buffer,
                                vPlane.buffer,
                                yPlane.rowStride,
                                uPlane.rowStride,
                                vPlane.rowStride,
                                uPlane.pixelStride,
                                vPlane.pixelStride,
                                outWidth,
                                outHeight,
                                // no rotation for file playback
                                0,
                            )

                            image.close()

                            // Pace at target FPS
                            Thread.sleep(FRAME_INTERVAL_MS)
                        }
                    }
                    codec.releaseOutputBuffer(outIdx, false)

                    if (info.flags and MediaCodec.BUFFER_FLAG_END_OF_STREAM != 0) {
                        Log.i(TAG, "Video end of stream, looping")
                        break // exit inner loop to restart from beginning
                    }
                }
            }
        } finally {
            codec.stop()
            codec.release()
            extractor.release()
        }
    }

    // ---- Helpers ----

    /**
     * Find the first track in the extractor matching the given MIME prefix.
     */
    private fun findTrack(
        extractor: MediaExtractor,
        mimePrefix: String,
    ): Int {
        for (i in 0 until extractor.trackCount) {
            val mime = extractor.getTrackFormat(i).getString(MediaFormat.KEY_MIME) ?: continue
            if (mime.startsWith(mimePrefix)) return i
        }
        return -1
    }

    /**
     * Linear interpolation resampler. Converts mono PCM from srcRate to dstRate.
     */
    private fun resampleLinear(
        input: ShortArray,
        srcRate: Int,
        dstRate: Int,
    ): ShortArray {
        if (input.isEmpty()) return input
        val ratio = srcRate.toDouble() / dstRate.toDouble()
        val outLen = (input.size / ratio).toInt()
        if (outLen <= 0) return ShortArray(0)

        val output = ShortArray(outLen)
        for (i in 0 until outLen) {
            val srcPos = i * ratio
            val idx = srcPos.toInt()
            val frac = srcPos - idx

            val s0 = input[idx].toInt()
            val s1 = if (idx + 1 < input.size) input[idx + 1].toInt() else s0
            output[i] = (s0 + (s1 - s0) * frac).toInt().coerceIn(Short.MIN_VALUE.toInt(), Short.MAX_VALUE.toInt()).toShort()
        }
        return output
    }
}
