package io.visio.mobile.ui

import android.graphics.Bitmap
import android.util.Log
import android.webkit.CookieManager
import android.webkit.WebResourceRequest
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties

private const val TAG = "OidcWebViewDialog"

/**
 * Full-screen dialog containing a WebView for OIDC authentication.
 *
 * The WebView loads the Meet authenticate endpoint which redirects to the SSO.
 * After the user logs in, the server sets a session cookie and redirects back.
 * We detect auth completion by:
 * 1. Intercepting a redirect to the visio:// custom scheme, OR
 * 2. Detecting the session cookie once we navigate back to the Meet domain.
 */
@Composable
fun OidcWebViewDialog(
    meetInstance: String,
    onAuthenticated: (sessionId: String, meetInstance: String) -> Unit,
    onDismiss: () -> Unit,
) {
    val authUrl = "https://$meetInstance/api/v1.0/authenticate/"
    var loading by remember { mutableStateOf(true) }

    Dialog(
        onDismissRequest = onDismiss,
        properties =
            DialogProperties(
                usePlatformDefaultWidth = false,
                dismissOnClickOutside = false,
            ),
    ) {
        Box(
            modifier =
                Modifier
                    .fillMaxSize()
                    .statusBarsPadding(),
        ) {
            AndroidView(
                modifier = Modifier.fillMaxSize(),
                factory = { context ->
                    WebView(context).apply {
                        settings.javaScriptEnabled = true
                        settings.domStorageEnabled = true

                        // Ensure cookies are accepted so we can extract the session cookie
                        CookieManager.getInstance().setAcceptCookie(true)
                        CookieManager.getInstance().setAcceptThirdPartyCookies(this, true)

                        webViewClient =
                            object : WebViewClient() {
                                override fun shouldOverrideUrlLoading(
                                    view: WebView,
                                    request: WebResourceRequest,
                                ): Boolean {
                                    val url = request.url
                                    // Intercept the visio:// custom scheme redirect
                                    if (url.scheme == "visio") {
                                        Log.d(TAG, "Intercepted visio:// redirect")
                                        extractAndComplete(meetInstance, onAuthenticated, onDismiss)
                                        return true
                                    }
                                    return false
                                }

                                override fun onPageStarted(
                                    view: WebView,
                                    url: String?,
                                    favicon: Bitmap?,
                                ) {
                                    loading = true
                                    // Check if we've returned to the Meet domain after auth
                                    if (url != null && url.startsWith("https://$meetInstance/") &&
                                        !url.contains("/api/v1.0/authenticate") &&
                                        !url.contains("/api/v1.0/callback")
                                    ) {
                                        Log.d(TAG, "Navigated back to Meet domain: $url")
                                        extractAndComplete(meetInstance, onAuthenticated, onDismiss)
                                    }
                                }

                                override fun onPageFinished(
                                    view: WebView,
                                    url: String?,
                                ) {
                                    loading = false
                                }
                            }

                        loadUrl(authUrl)
                    }
                },
            )

            // Close button
            IconButton(
                onClick = onDismiss,
                modifier =
                    Modifier
                        .align(Alignment.TopEnd)
                        .padding(8.dp),
            ) {
                Icon(
                    Icons.Default.Close,
                    contentDescription = "Close",
                    tint = MaterialTheme.colorScheme.onSurface,
                    modifier = Modifier.size(24.dp),
                )
            }

            // Loading indicator
            if (loading) {
                CircularProgressIndicator(
                    modifier = Modifier.align(Alignment.Center),
                )
            }
        }
    }
}

private fun extractAndComplete(
    meetInstance: String,
    onAuthenticated: (String, String) -> Unit,
    onDismiss: () -> Unit,
) {
    val allCookies = CookieManager.getInstance().getCookie("https://$meetInstance")
    Log.d(TAG, "Cookies for $meetInstance: $allCookies")

    if (allCookies == null) {
        Log.w(TAG, "No cookies found after auth")
        onDismiss()
        return
    }

    val cookieNames = listOf("meet_sessionid", "sessionid")
    val sessionId =
        allCookies.split(";")
            .map { it.trim() }
            .firstOrNull { cookie -> cookieNames.any { cookie.startsWith("$it=") } }
            ?.substringAfter("=")

    if (sessionId != null) {
        Log.i(TAG, "Session cookie extracted successfully")
        onAuthenticated(sessionId, meetInstance)
    } else {
        Log.w(TAG, "sessionid not found in cookies: $allCookies")
        onDismiss()
    }
}
