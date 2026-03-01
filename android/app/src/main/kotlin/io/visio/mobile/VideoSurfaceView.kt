package io.visio.mobile

import android.content.Context
import android.graphics.SurfaceTexture
import android.util.Log
import android.view.Surface
import android.view.TextureView

class VideoSurfaceView(
    context: Context,
    private val trackSid: String
) : TextureView(context), TextureView.SurfaceTextureListener {

    private var surface: Surface? = null

    init {
        surfaceTextureListener = this
        Log.d(TAG, "VideoSurfaceView created for track=$trackSid")
    }

    override fun onSurfaceTextureAvailable(texture: SurfaceTexture, width: Int, height: Int) {
        Log.d(TAG, "surfaceCreated track=$trackSid ${width}x${height}, attaching surface")
        surface = Surface(texture)
        NativeVideo.attachSurface(trackSid, surface!!)
    }

    override fun onSurfaceTextureSizeChanged(texture: SurfaceTexture, width: Int, height: Int) {
        Log.d(TAG, "surfaceChanged track=$trackSid ${width}x${height}")
    }

    override fun onSurfaceTextureDestroyed(texture: SurfaceTexture): Boolean {
        Log.d(TAG, "surfaceDestroyed track=$trackSid, detaching surface")
        NativeVideo.detachSurface(trackSid)
        surface?.release()
        surface = null
        return true
    }

    override fun onSurfaceTextureUpdated(texture: SurfaceTexture) {
        // Called after each frame is drawn to the texture
    }

    companion object {
        private const val TAG = "VideoSurfaceView"
    }
}
