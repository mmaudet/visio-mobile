package io.visio.mobile

import android.annotation.SuppressLint
import android.content.Context
import android.graphics.ImageFormat
import android.hardware.camera2.CameraCaptureSession
import android.hardware.camera2.CameraCharacteristics
import android.hardware.camera2.CameraDevice
import android.hardware.camera2.CameraManager
import android.hardware.camera2.CaptureRequest
import android.media.ImageReader
import android.os.Handler
import android.os.HandlerThread
import android.util.Log

/**
 * Captures camera frames via Camera2 API and pushes them into the
 * Rust NativeVideoSource via JNI.
 *
 * Lifecycle: [start] → frames flow → [stop].
 */
class CameraCapture(private val context: Context) {

    companion object {
        private const val TAG = "CameraCapture"
        private const val WIDTH = 640
        private const val HEIGHT = 480
        private const val MAX_IMAGES = 2
    }

    private var cameraDevice: CameraDevice? = null
    private var captureSession: CameraCaptureSession? = null
    private var imageReader: ImageReader? = null
    private var handlerThread: HandlerThread? = null
    private var handler: Handler? = null
    private var running = false

    @SuppressLint("MissingPermission") // Caller must check CAMERA permission first
    fun start() {
        if (running) return
        running = true

        // Background thread for camera callbacks
        val thread = HandlerThread("CameraCapture").also { it.start() }
        handlerThread = thread
        handler = Handler(thread.looper)

        val cameraManager = context.getSystemService(Context.CAMERA_SERVICE) as CameraManager
        val cameraId = findFrontCamera(cameraManager) ?: findBackCamera(cameraManager)
        if (cameraId == null) {
            Log.e(TAG, "No camera found")
            running = false
            return
        }

        // ImageReader receives YUV_420_888 frames
        imageReader = ImageReader.newInstance(WIDTH, HEIGHT, ImageFormat.YUV_420_888, MAX_IMAGES).apply {
            setOnImageAvailableListener({ reader ->
                val image = reader.acquireLatestImage() ?: return@setOnImageAvailableListener
                try {
                    val yPlane = image.planes[0]
                    val uPlane = image.planes[1]
                    val vPlane = image.planes[2]

                    NativeVideo.nativePushCameraFrame(
                        yPlane.buffer,
                        uPlane.buffer,
                        vPlane.buffer,
                        yPlane.rowStride,
                        uPlane.rowStride,
                        vPlane.rowStride,
                        uPlane.pixelStride,
                        vPlane.pixelStride,
                        image.width,
                        image.height
                    )
                } finally {
                    image.close()
                }
            }, handler)
        }

        cameraManager.openCamera(cameraId, object : CameraDevice.StateCallback() {
            override fun onOpened(camera: CameraDevice) {
                Log.i(TAG, "Camera opened: ${camera.id}")
                cameraDevice = camera
                createCaptureSession(camera)
            }

            override fun onDisconnected(camera: CameraDevice) {
                Log.w(TAG, "Camera disconnected")
                camera.close()
                cameraDevice = null
            }

            override fun onError(camera: CameraDevice, error: Int) {
                Log.e(TAG, "Camera error: $error")
                camera.close()
                cameraDevice = null
            }
        }, handler)
    }

    fun stop() {
        if (!running) return
        running = false

        captureSession?.close()
        captureSession = null

        cameraDevice?.close()
        cameraDevice = null

        imageReader?.close()
        imageReader = null

        handlerThread?.quitSafely()
        handlerThread = null
        handler = null

        NativeVideo.nativeStopCameraCapture()
        Log.i(TAG, "Camera capture stopped")
    }

    @Suppress("DEPRECATION")
    private fun createCaptureSession(camera: CameraDevice) {
        val reader = imageReader ?: return
        val surface = reader.surface

        camera.createCaptureSession(
            listOf(surface),
            object : CameraCaptureSession.StateCallback() {
                override fun onConfigured(session: CameraCaptureSession) {
                    if (!running) {
                        session.close()
                        return
                    }
                    captureSession = session

                    val request = camera.createCaptureRequest(CameraDevice.TEMPLATE_PREVIEW).apply {
                        addTarget(surface)
                        set(CaptureRequest.CONTROL_AF_MODE, CaptureRequest.CONTROL_AF_MODE_CONTINUOUS_VIDEO)
                    }.build()

                    session.setRepeatingRequest(request, null, handler)
                    Log.i(TAG, "Camera capture session started")
                }

                override fun onConfigureFailed(session: CameraCaptureSession) {
                    Log.e(TAG, "Camera capture session configuration failed")
                }
            },
            handler
        )
    }

    private fun findFrontCamera(manager: CameraManager): String? {
        return manager.cameraIdList.firstOrNull { id ->
            val chars = manager.getCameraCharacteristics(id)
            chars.get(CameraCharacteristics.LENS_FACING) == CameraCharacteristics.LENS_FACING_FRONT
        }
    }

    private fun findBackCamera(manager: CameraManager): String? {
        return manager.cameraIdList.firstOrNull { id ->
            val chars = manager.getCameraCharacteristics(id)
            chars.get(CameraCharacteristics.LENS_FACING) == CameraCharacteristics.LENS_FACING_BACK
        }
    }
}
