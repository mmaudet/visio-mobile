package io.visio.mobile

import android.view.Surface
import java.nio.ByteBuffer

object NativeVideo {
    init {
        System.loadLibrary("visio_ffi")
    }

    external fun attachSurface(trackSid: String, surface: Surface)
    external fun detachSurface(trackSid: String)

    /**
     * Push a YUV_420_888 camera frame into the LiveKit NativeVideoSource.
     * Called from CameraCapture's ImageReader callback.
     *
     * The ByteBuffers must be direct buffers pointing to the Y, U, V planes.
     * pixelStride indicates the byte spacing between consecutive pixel values
     * in each plane (1 for planar I420, 2 for semi-planar NV12/NV21).
     */
    external fun nativePushCameraFrame(
        y: ByteBuffer, u: ByteBuffer, v: ByteBuffer,
        yStride: Int, uStride: Int, vStride: Int,
        uPixelStride: Int, vPixelStride: Int,
        width: Int, height: Int
    )

    /**
     * Clear the stored NativeVideoSource when camera capture stops.
     */
    external fun nativeStopCameraCapture()
}
