package io.visio.mobile

import android.app.PictureInPictureParams
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.os.Build
import android.os.Bundle
import android.util.Rational
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.Surface
import androidx.compose.ui.Modifier
import io.visio.mobile.navigation.AppNavigation
import io.visio.mobile.ui.theme.VisioColors
import io.visio.mobile.ui.theme.VisioTheme
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import uniffi.visio.ConnectionState

class MainActivity : ComponentActivity() {

    private val pipActionReceiver = object : BroadcastReceiver() {
        override fun onReceive(context: Context, intent: Intent) {
            when (intent.action) {
                ACTION_TOGGLE_MIC -> {
                    CoroutineScope(Dispatchers.IO).launch {
                        try {
                            val enabled = VisioManager.client.isMicrophoneEnabled()
                            if (enabled) {
                                VisioManager.stopAudioCapture()
                                VisioManager.client.setMicrophoneEnabled(false)
                            } else {
                                VisioManager.client.setMicrophoneEnabled(true)
                                VisioManager.startAudioCapture()
                            }
                        } catch (_: Exception) {}
                    }
                }
                ACTION_HANGUP -> {
                    VisioManager.stopCameraCapture()
                    VisioManager.stopAudioCapture()
                    VisioManager.stopAudioPlayout()
                    VisioManager.client.disconnect()
                    finish()
                }
            }
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        val filter = IntentFilter().apply {
            addAction(ACTION_TOGGLE_MIC)
            addAction(ACTION_HANGUP)
        }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            registerReceiver(pipActionReceiver, filter, RECEIVER_NOT_EXPORTED)
        } else {
            registerReceiver(pipActionReceiver, filter)
        }

        setContent {
            VisioTheme {
                Surface(
                    modifier = Modifier.fillMaxSize(),
                    color = VisioColors.PrimaryDark50
                ) {
                    AppNavigation()
                }
            }
        }
    }

    override fun onDestroy() {
        super.onDestroy()
        try {
            unregisterReceiver(pipActionReceiver)
        } catch (_: Exception) {}
    }

    override fun onUserLeaveHint() {
        super.onUserLeaveHint()
        val state = VisioManager.connectionState.value
        if (state is ConnectionState.Connected) {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                val params = PictureInPictureParams.Builder()
                    .setAspectRatio(Rational(16, 9))
                    .build()
                enterPictureInPictureMode(params)
            }
        }
    }

    companion object {
        const val ACTION_TOGGLE_MIC = "io.visio.mobile.TOGGLE_MIC"
        const val ACTION_HANGUP = "io.visio.mobile.HANGUP"
    }
}
