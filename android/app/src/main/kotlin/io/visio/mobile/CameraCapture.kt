package io.visio.mobile

import android.annotation.SuppressLint
import android.content.Context
import android.graphics.ImageFormat
import android.hardware.camera2.CameraCaptureSession
import android.hardware.camera2.CameraCharacteristics
import android.hardware.camera2.CameraDevice
import android.hardware.camera2.CameraManager
import android.hardware.camera2.CaptureRequest
import android.hardware.display.DisplayManager
import android.media.ImageReader
import android.os.Handler
import android.os.HandlerThread
import android.util.Log
import android.view.Display

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

    private val lock = Any()
    private var cameraDevice: CameraDevice? = null
    private var captureSession: CameraCaptureSession? = null
    private var imageReader: ImageReader? = null
    private var handlerThread: HandlerThread? = null
    private var handler: Handler? = null

    @Volatile private var sensorOrientation: Int = 0

    @Volatile private var isFrontCamera: Boolean = false

    @Volatile private var running = false
    private val displayManager = context.getSystemService(Context.DISPLAY_SERVICE) as DisplayManager

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

        val chars = cameraManager.getCameraCharacteristics(cameraId)
        sensorOrientation = chars.get(CameraCharacteristics.SENSOR_ORIENTATION) ?: 0
        isFrontCamera = chars.get(CameraCharacteristics.LENS_FACING) == CameraCharacteristics.LENS_FACING_FRONT
        Log.i(TAG, "Camera $cameraId: sensorOrientation=$sensorOrientation, front=$isFrontCamera")

        // ImageReader receives YUV_420_888 frames
        val newReader =
            ImageReader.newInstance(WIDTH, HEIGHT, ImageFormat.YUV_420_888, MAX_IMAGES).apply {
                setOnImageAvailableListener({ reader ->
                    val image =
                        reader.acquireLatestImage() ?: run {
                            Log.w(TAG, "acquireLatestImage returned null")
                            return@setOnImageAvailableListener
                        }
                    try {
                        val yPlane = image.planes[0]
                        val uPlane = image.planes[1]
                        val vPlane = image.planes[2]

                        // Compensate for device orientation so the self-view
                        // is always upright. Display.rotation is 0/1/2/3.
                        val displayDegrees =
                            (
                                displayManager
                                    .getDisplay(Display.DEFAULT_DISPLAY)?.rotation ?: 0
                            ) * 90
                        val rotation =
                            if (isFrontCamera) {
                                (sensorOrientation + displayDegrees) % 360
                            } else {
                                (sensorOrientation - displayDegrees + 360) % 360
                            }

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
                            image.height,
                            rotation,
                        )
                    } finally {
                        image.close()
                    }
                }, handler)
            }
        synchronized(lock) { imageReader = newReader }

        cameraManager.openCamera(
            cameraId,
            object : CameraDevice.StateCallback() {
                override fun onOpened(camera: CameraDevice) {
                    Log.i(TAG, "Camera opened: ${camera.id}")
                    synchronized(lock) { cameraDevice = camera }
                    createCaptureSession(camera)
                }

                override fun onDisconnected(camera: CameraDevice) {
                    Log.w(TAG, "Camera disconnected")
                    camera.close()
                    synchronized(lock) { cameraDevice = null }
                }

                override fun onError(
                    camera: CameraDevice,
                    error: Int,
                ) {
                    Log.e(TAG, "Camera error: $error")
                    camera.close()
                    synchronized(lock) { cameraDevice = null }
                }
            },
            handler,
        )
    }

    fun stop() {
        if (!running) return
        running = false

        synchronized(lock) {
            captureSession?.close()
            captureSession = null

            cameraDevice?.close()
            cameraDevice = null

            imageReader?.close()
            imageReader = null
        }

        handlerThread?.quitSafely()
        handlerThread = null
        handler = null

        NativeVideo.nativeStopCameraCapture()
        Log.i(TAG, "Camera capture stopped")
    }

    /**
     * Switch to a different camera by ID. Stops current capture and restarts with new camera.
     */
    @SuppressLint("MissingPermission")
    fun switchCamera(useFront: Boolean) {
        if (!running) return

        val cameraManager = context.getSystemService(Context.CAMERA_SERVICE) as CameraManager
        val newId = if (useFront) findFrontCamera(cameraManager) else findBackCamera(cameraManager)
        if (newId == null) {
            Log.e(TAG, "Requested camera not found (front=$useFront)")
            return
        }

        // Stop current session
        synchronized(lock) {
            captureSession?.close()
            captureSession = null
            cameraDevice?.close()
            cameraDevice = null
            imageReader?.close()
            imageReader = null
        }

        // Update orientation info
        val chars = cameraManager.getCameraCharacteristics(newId)
        sensorOrientation = chars.get(CameraCharacteristics.SENSOR_ORIENTATION) ?: 0
        isFrontCamera = chars.get(CameraCharacteristics.LENS_FACING) == CameraCharacteristics.LENS_FACING_FRONT
        Log.i(TAG, "Switching to camera $newId: sensorOrientation=$sensorOrientation, front=$isFrontCamera")

        // Recreate ImageReader
        val newReader =
            ImageReader.newInstance(WIDTH, HEIGHT, ImageFormat.YUV_420_888, MAX_IMAGES).apply {
                setOnImageAvailableListener({ reader ->
                    val image = reader.acquireLatestImage() ?: return@setOnImageAvailableListener
                    try {
                        val yPlane = image.planes[0]
                        val uPlane = image.planes[1]
                        val vPlane = image.planes[2]
                        val displayDegrees =
                            (
                                displayManager
                                    .getDisplay(Display.DEFAULT_DISPLAY)?.rotation ?: 0
                            ) * 90
                        val rotation =
                            if (isFrontCamera) {
                                (sensorOrientation + displayDegrees) % 360
                            } else {
                                (sensorOrientation - displayDegrees + 360) % 360
                            }
                        NativeVideo.nativePushCameraFrame(
                            yPlane.buffer, uPlane.buffer, vPlane.buffer,
                            yPlane.rowStride, uPlane.rowStride, vPlane.rowStride,
                            uPlane.pixelStride, vPlane.pixelStride,
                            image.width, image.height, rotation,
                        )
                    } finally {
                        image.close()
                    }
                }, handler)
            }
        synchronized(lock) { imageReader = newReader }

        // Open new camera
        cameraManager.openCamera(
            newId,
            object : CameraDevice.StateCallback() {
                override fun onOpened(camera: CameraDevice) {
                    Log.i(TAG, "Switched camera opened: ${camera.id}")
                    synchronized(lock) { cameraDevice = camera }
                    createCaptureSession(camera)
                }

                override fun onDisconnected(camera: CameraDevice) {
                    camera.close()
                    synchronized(lock) { cameraDevice = null }
                }

                override fun onError(
                    camera: CameraDevice,
                    error: Int,
                ) {
                    Log.e(TAG, "Camera switch error: $error")
                    camera.close()
                    synchronized(lock) { cameraDevice = null }
                }
            },
            handler,
        )
    }

    /** Returns true if currently using front camera. */
    fun isFront(): Boolean = isFrontCamera

    @Suppress("DEPRECATION")
    private fun createCaptureSession(camera: CameraDevice) {
        val reader = synchronized(lock) { imageReader } ?: return
        val surface = reader.surface

        camera.createCaptureSession(
            listOf(surface),
            object : CameraCaptureSession.StateCallback() {
                override fun onConfigured(session: CameraCaptureSession) {
                    if (!running) {
                        session.close()
                        return
                    }
                    synchronized(lock) { captureSession = session }

                    val request =
                        camera.createCaptureRequest(CameraDevice.TEMPLATE_PREVIEW).apply {
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
            handler,
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
