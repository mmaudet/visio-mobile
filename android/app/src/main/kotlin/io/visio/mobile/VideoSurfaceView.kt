package io.visio.mobile

import android.content.Context
import android.view.SurfaceHolder
import android.view.SurfaceView

class VideoSurfaceView(
    context: Context,
    private val trackSid: String
) : SurfaceView(context), SurfaceHolder.Callback {

    init {
        holder.addCallback(this)
    }

    override fun surfaceCreated(holder: SurfaceHolder) {
        // Pass surface to Rust via JNI
        // The native method will call visio_video_attach_surface
        NativeVideo.attachSurface(trackSid, holder.surface)
    }

    override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) {}

    override fun surfaceDestroyed(holder: SurfaceHolder) {
        NativeVideo.detachSurface(trackSid)
    }
}
