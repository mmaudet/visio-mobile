package io.visio.mobile

import android.app.Application
import android.util.Log
import io.visio.mobile.ui.i18n.Strings

class VisioApplication : Application() {

    companion object {
        init {
            System.loadLibrary("visio_ffi")
        }

        @JvmStatic
        external fun nativeInitWebrtc()
    }

    override fun onCreate() {
        super.onCreate()
        try {
            nativeInitWebrtc()
            Log.i("Visio", "WebRTC initialized on main thread")
        } catch (e: UnsatisfiedLinkError) {
            Log.e("Visio", "nativeInitWebrtc failed: ${e.message}")
        }
        Strings.init(this)
        VisioManager.initialize(applicationContext)
    }
}
