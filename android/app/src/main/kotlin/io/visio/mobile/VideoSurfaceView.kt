package io.visio.mobile

import android.content.Context
import android.util.Log
import android.view.SurfaceHolder
import android.view.SurfaceView

class VideoSurfaceView(
    context: Context,
    private val trackSid: String
) : SurfaceView(context), SurfaceHolder.Callback {

    init {
        holder.addCallback(this)
        Log.d(TAG, "VideoSurfaceView created for track=$trackSid")
    }

    override fun surfaceCreated(holder: SurfaceHolder) {
        Log.d(TAG, "surfaceCreated track=$trackSid, attaching surface")
        NativeVideo.attachSurface(trackSid, holder.surface)
    }

    override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) {
        Log.d(TAG, "surfaceChanged track=$trackSid ${width}x${height} format=$format")
    }

    override fun surfaceDestroyed(holder: SurfaceHolder) {
        Log.d(TAG, "surfaceDestroyed track=$trackSid, detaching surface")
        NativeVideo.detachSurface(trackSid)
    }

    companion object {
        private const val TAG = "VideoSurfaceView"
    }
}
