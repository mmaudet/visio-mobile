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
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.ui.Modifier
import io.visio.mobile.navigation.AppNavigation
import io.visio.mobile.ui.theme.VisioTheme
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import uniffi.visio.ConnectionState

class MainActivity : ComponentActivity() {
    /**
     * Parse a deep link URL.
     * Handles two types:
     * - Room links: visio://host/room-slug -> returns room URL
     * - Auth callbacks: visio://auth-callback?... -> handled separately, returns null
     */
    private fun parseDeepLink(intent: Intent?): String? {
        val uri = intent?.data ?: return null
        if (uri.scheme != "visio") return null
        val host = uri.host ?: return null

        // Handle auth callback
        if (host == "auth-callback") {
            handleAuthCallback(uri.toString())
            return null
        }

        // Handle room deep link
        val slug = uri.path?.trimStart('/') ?: return null
        if (host.isBlank() || slug.isBlank()) return null

        val instances = VisioManager.client.getMeetInstances()
        return if (instances.contains(host)) {
            "https://$host/$slug"
        } else {
            null
        }
    }

    /**
     * Handle SSO OIDC auth callback.
     * Called when the app receives a visio://auth-callback?... deep link.
     */
    private fun handleAuthCallback(callbackUrl: String) {
        CoroutineScope(Dispatchers.IO).launch {
            try {
                val session = VisioManager.client.handleAuthCallback(callbackUrl)
                VisioManager.onAuthSuccess(session)
            } catch (e: Exception) {
                VisioManager.onAuthError(e.message ?: "Authentication failed")
            }
        }
    }

    private val pipActionReceiver =
        object : BroadcastReceiver() {
            override fun onReceive(
                context: Context,
                intent: Intent,
            ) {
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
                            } catch (_: Exception) {
                            }
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
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)

        parseDeepLink(intent)?.let { VisioManager.pendingDeepLink = it }

        val filter =
            IntentFilter().apply {
                addAction(ACTION_TOGGLE_MIC)
                addAction(ACTION_HANGUP)
            }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            registerReceiver(pipActionReceiver, filter, RECEIVER_NOT_EXPORTED)
        } else {
            registerReceiver(pipActionReceiver, filter)
        }

        setContent {
            val isDark = VisioManager.currentTheme == "dark"
            VisioTheme(darkTheme = isDark) {
                Surface(
                    modifier = Modifier.fillMaxSize(),
                    color = MaterialTheme.colorScheme.background,
                ) {
                    AppNavigation()
                }
            }
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        parseDeepLink(intent)?.let { VisioManager.pendingDeepLink = it }
    }

    override fun onDestroy() {
        super.onDestroy()
        try {
            unregisterReceiver(pipActionReceiver)
        } catch (_: Exception) {
        }
    }

    override fun onUserLeaveHint() {
        super.onUserLeaveHint()
        val state = VisioManager.connectionState.value
        if (state is ConnectionState.Connected) {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                val params =
                    PictureInPictureParams.Builder()
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
