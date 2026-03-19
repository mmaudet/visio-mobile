package io.visio.mobile

import android.content.Context
import android.graphics.PixelFormat
import android.util.Log
import android.view.SurfaceHolder
import android.view.SurfaceView

/**
 * SurfaceView-based video renderer.
 *
 * SurfaceView is used instead of TextureView because TextureView's internal
 * triple-buffered SurfaceTexture starts with uninitialized green (YUV 0,0,0)
 * content that cannot be fully overwritten before display. SurfaceView does
 * not have this issue — its surface starts with a transparent/black background.
 */
class VideoSurfaceView(
    context: Context,
    private val trackSid: String,
) : SurfaceView(context), SurfaceHolder.Callback {
    init {
        holder.addCallback(this)
        holder.setFormat(PixelFormat.RGBA_8888)
        Log.d(TAG, "VideoSurfaceView created for track=$trackSid")
    }

    override fun surfaceCreated(holder: SurfaceHolder) {
        Log.d(TAG, "surfaceCreated track=$trackSid, attaching surface")
        NativeVideo.attachSurface(trackSid, holder.surface)
    }

    override fun surfaceChanged(
        holder: SurfaceHolder,
        format: Int,
        width: Int,
        height: Int,
    ) {
        Log.d(TAG, "surfaceChanged track=$trackSid ${width}x$height")
    }

    override fun surfaceDestroyed(holder: SurfaceHolder) {
        Log.d(TAG, "surfaceDestroyed track=$trackSid, detaching surface")
        NativeVideo.detachSurface(trackSid)
    }

    companion object {
        private const val TAG = "VideoSurfaceView"
    }
}
