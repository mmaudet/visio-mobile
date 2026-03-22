package io.visio.mobile

import android.view.Surface
import java.nio.ByteBuffer

/**
 * YUV_420_888 frame planes and layout info, used for camera and preview FFI calls.
 *
 * The ByteBuffers must be direct buffers pointing to the Y, U, V planes.
 * pixelStride indicates the byte spacing between consecutive pixel values
 * in each plane (1 for planar I420, 2 for semi-planar NV12/NV21).
 */
data class YuvFrame(
    val y: ByteBuffer,
    val u: ByteBuffer,
    val v: ByteBuffer,
    val yStride: Int,
    val uStride: Int,
    val vStride: Int,
    val uPixelStride: Int,
    val vPixelStride: Int,
    val width: Int,
    val height: Int,
)

object NativeVideo {
    init {
        System.loadLibrary("visio_ffi")
    }

    external fun attachSurface(
        trackSid: String,
        surface: Surface,
    )

    external fun detachSurface(trackSid: String)

    /**
     * Push a YUV_420_888 camera frame into the LiveKit NativeVideoSource.
     * Called from CameraCapture's ImageReader callback.
     */
    fun nativePushCameraFrame(
        frame: YuvFrame,
        rotation: Int,
    ) = nativePushCameraFrameRaw(
        frame.y, frame.u, frame.v,
        frame.yStride, frame.uStride, frame.vStride,
        frame.uPixelStride, frame.vPixelStride,
        frame.width, frame.height,
        rotation,
    )

    @Suppress("LongParameterList")
    private external fun nativePushCameraFrameRaw(
        y: ByteBuffer,
        u: ByteBuffer,
        v: ByteBuffer,
        yStride: Int,
        uStride: Int,
        vStride: Int,
        uPixelStride: Int,
        vPixelStride: Int,
        width: Int,
        height: Int,
        rotation: Int,
    )

    /**
     * Clear the stored NativeVideoSource when camera capture stops.
     */
    external fun nativeStopCameraCapture()

    /**
     * Preview-only frame processing: applies blur and renders to the local preview
     * surface without feeding into a LiveKit NativeVideoSource.
     * Called from CameraCapture in preview mode (pre-join lobby).
     */
    fun nativeProcessPreviewFrame(
        frame: YuvFrame,
        rotationDegrees: Int,
    ) = nativeProcessPreviewFrameRaw(
        frame.y, frame.u, frame.v,
        frame.yStride, frame.uStride, frame.vStride,
        frame.uPixelStride, frame.vPixelStride,
        frame.width, frame.height,
        rotationDegrees,
    )

    @Suppress("LongParameterList")
    private external fun nativeProcessPreviewFrameRaw(
        yBuf: ByteBuffer,
        uBuf: ByteBuffer,
        vBuf: ByteBuffer,
        yStride: Int,
        uStride: Int,
        vStride: Int,
        uPixelStride: Int,
        vPixelStride: Int,
        width: Int,
        height: Int,
        rotationDegrees: Int,
    )

    /**
     * Push a PCM audio frame into the LiveKit NativeAudioSource.
     * Called from AudioCapture's recording thread.
     *
     * @param data Direct ByteBuffer containing 16-bit signed PCM samples
     * @param numSamples Total number of samples in the buffer
     * @param sampleRate Sample rate in Hz (48000)
     * @param numChannels Number of audio channels (1 = mono)
     */
    external fun nativePushAudioFrame(
        data: ByteBuffer,
        numSamples: Int,
        sampleRate: Int,
        numChannels: Int,
    )

    /**
     * Clear the stored NativeAudioSource when mic capture stops.
     */
    external fun nativeStopAudioCapture()

    /**
     * Pull decoded remote audio samples from the Rust playout buffer.
     * Called from AudioPlayout's polling thread.
     *
     * @param buffer ShortArray to fill with PCM samples
     * @return Number of samples actually available (rest is silence)
     */
    external fun nativePullAudioPlayback(buffer: ShortArray): Int
}
