package io.visio.mobile

import android.app.PictureInPictureParams
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.os.Build
import android.os.Bundle
import android.util.Log
import android.util.Rational
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.ui.Modifier
import io.visio.mobile.auth.OidcAuthManager
import io.visio.mobile.navigation.AppNavigation
import io.visio.mobile.ui.theme.VisioTheme
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import uniffi.visio.ConnectionState

class MainActivity : ComponentActivity() {
    private fun parseDeepLink(intent: Intent?): String? {
        val uri = intent?.data ?: return null
        if (uri.scheme != "visio") return null
        val host = uri.host ?: return null

        // Handle auth callback deep link: visio://auth-callback
        if (host == OidcAuthManager.AUTH_CALLBACK_HOST) {
            handleAuthCallback()
            return null
        }

        val slug = uri.path?.trimStart('/') ?: return null
        if (host.isBlank() || slug.isBlank()) return null

        val instances = VisioManager.client.getMeetInstances()
        return if (instances.contains(host)) {
            // Preserve the ?name= query parameter from deep links
            val nameParam = uri.getQueryParameter("name")
            if (!nameParam.isNullOrBlank()) {
                val encoded = java.net.URLEncoder.encode(nameParam, "UTF-8")
                "https://$host/$slug?name=$encoded"
            } else {
                "https://$host/$slug"
            }
        } else {
            null
        }
    }

    /**
     * Handle the OIDC auth callback from Chrome Custom Tab.
     * Extracts the sessionid cookie from CookieManager and completes authentication.
     */
    private fun handleAuthCallback() {
        val result = VisioManager.authManager.handleAuthCallback()
        if (result != null) {
            val (sessionId, meetInstance) = result
            Log.i(TAG, "Auth callback: session cookie received for $meetInstance")
            VisioManager.onAuthCookieReceived(sessionId, meetInstance)
        } else {
            Log.w(TAG, "Auth callback: failed to extract session cookie")
        }
    }

    /**
     * Parse a visio-test://connect?livekit_url=...&token=... deep link for E2E testing.
     * Only available in debug builds.
     * Returns true if a test deep link was handled.
     */
    private fun parseTestDeepLink(intent: Intent?): Boolean {
        if (!BuildConfig.DEBUG) return false
        val uri = intent?.data ?: return false
        if (uri.scheme != "visio-test" || uri.host != "connect") return false

        val livekitUrl = uri.getQueryParameter("livekit_url")
        val token = uri.getQueryParameter("token")
        val mediaFile = uri.getQueryParameter("media_file")
        if (livekitUrl.isNullOrBlank() || token.isNullOrBlank()) {
            Log.w(TAG, "visio-test://connect missing livekit_url or token parameters")
            return false
        }

        Log.i(TAG, "Test deep link: connecting to $livekitUrl, media_file=$mediaFile")
        VisioManager.pendingTestConnect = Triple(livekitUrl, token, mediaFile)
        return true
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
                            } catch (e: Exception) {
                                Log.e(TAG, "Failed to toggle microphone in PiP mode", e)
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

        if (!parseTestDeepLink(intent)) {
            parseDeepLink(intent)?.let { VisioManager.pendingDeepLink = it }
        }

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
        if (!parseTestDeepLink(intent)) {
            parseDeepLink(intent)?.let { VisioManager.pendingDeepLink = it }
        }
    }

    override fun onPause() {
        super.onPause()
        if (!isInPictureInPictureMode) {
            VisioManager.onAppBackgrounded()
        }
    }

    override fun onResume() {
        super.onResume()
        VisioManager.onAppForegrounded()
    }

    override fun onDestroy() {
        super.onDestroy()
        try {
            unregisterReceiver(pipActionReceiver)
        } catch (e: Exception) {
            Log.e(TAG, "Failed to unregister PiP broadcast receiver", e)
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
        private const val TAG = "MainActivity"
        const val ACTION_TOGGLE_MIC = "io.visio.mobile.TOGGLE_MIC"
        const val ACTION_HANGUP = "io.visio.mobile.HANGUP"
    }
}
