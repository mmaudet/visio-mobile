package io.visio.mobile

import android.view.Surface

object NativeVideo {
    init {
        System.loadLibrary("visio_video")
    }

    external fun attachSurface(trackSid: String, surface: Surface)
    external fun detachSurface(trackSid: String)
}
