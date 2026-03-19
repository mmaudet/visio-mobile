package io.visio.mobile.auth

import android.content.Context
import android.net.Uri
import android.util.Log
import android.webkit.CookieManager
import androidx.browser.customtabs.CustomTabsIntent
import androidx.security.crypto.EncryptedSharedPreferences
import androidx.security.crypto.MasterKey

class OidcAuthManager(context: Context) {
    companion object {
        private const val TAG = "OidcAuthManager"
        const val AUTH_CALLBACK_HOST = "auth-callback"
    }

    private val masterKey =
        MasterKey.Builder(context)
            .setKeyScheme(MasterKey.KeyScheme.AES256_GCM)
            .build()

    private val prefs =
        try {
            EncryptedSharedPreferences.create(
                context,
                "visio_auth",
                masterKey,
                EncryptedSharedPreferences.PrefKeyEncryptionScheme.AES256_SIV,
                EncryptedSharedPreferences.PrefValueEncryptionScheme.AES256_GCM,
            )
        } catch (_: Exception) {
            context.deleteSharedPreferences("visio_auth")
            EncryptedSharedPreferences.create(
                context,
                "visio_auth",
                masterKey,
                EncryptedSharedPreferences.PrefKeyEncryptionScheme.AES256_SIV,
                EncryptedSharedPreferences.PrefValueEncryptionScheme.AES256_GCM,
            )
        }

    /** The meet instance currently being authenticated against. */
    var pendingAuthInstance: String? = null
        private set

    /**
     * Launch the OIDC authentication flow in a Chrome Custom Tab.
     *
     * The returnTo parameter uses the visio://auth-callback deep link so that
     * after the OIDC flow completes, Chrome navigates to the custom scheme,
     * which Android handles via the intent filter, closing the Custom Tab
     * automatically and returning to the app.
     */
    fun launchOidcFlow(
        context: Context,
        meetInstance: String,
    ) {
        pendingAuthInstance = meetInstance

        val returnTo = "visio://$AUTH_CALLBACK_HOST"
        val authUrl = "https://$meetInstance/api/v1.0/authenticate/?returnTo=${
            java.net.URLEncoder.encode(returnTo, "UTF-8")
        }"

        Log.d(TAG, "Starting OIDC flow via Custom Tab: $authUrl")

        val customTabsIntent =
            CustomTabsIntent.Builder()
                .setShowTitle(true)
                .build()
        customTabsIntent.launchUrl(context, Uri.parse(authUrl))
    }

    /**
     * Called when the visio://auth-callback deep link is received.
     * Extracts the sessionid cookie from CookieManager (shared with Custom Tab / Chrome).
     *
     * @param consumeOnFailure If true (default, used by deep link callback), clears
     *   pendingAuthInstance even if no cookie is found. If false (used by onResume fallback),
     *   keeps pendingAuthInstance so the user can finish auth and return again.
     * @return Pair of (sessionid, meetInstance) if successful, null otherwise.
     */
    fun handleAuthCallback(consumeOnFailure: Boolean = true): Pair<String, String>? {
        val meetInstance = pendingAuthInstance
        if (meetInstance == null) {
            Log.w(TAG, "Auth callback received but no pending auth instance")
            return null
        }

        val allCookies = CookieManager.getInstance().getCookie("https://$meetInstance")
        Log.d(TAG, "Cookies for $meetInstance: $allCookies")

        if (allCookies == null) {
            Log.w(TAG, "No cookies found for $meetInstance")
            if (consumeOnFailure) pendingAuthInstance = null
            return null
        }

        // Meet uses "meet_sessionid", other instances may use "sessionid"
        val cookieNames = listOf("meet_sessionid", "sessionid")
        val sessionId =
            allCookies.split(";")
                .map { it.trim() }
                .firstOrNull { cookie -> cookieNames.any { cookie.startsWith("$it=") } }
                ?.substringAfter("=")

        if (sessionId != null) {
            pendingAuthInstance = null
            Log.d(TAG, "Session cookie extracted successfully from CookieManager")
            return Pair(sessionId, meetInstance)
        } else {
            Log.w(TAG, "sessionid not found in cookies")
            if (consumeOnFailure) pendingAuthInstance = null
            return null
        }
    }

    fun saveCookie(cookie: String) {
        prefs.edit().putString("sessionid", cookie).apply()
    }

    fun getSavedCookie(): String? {
        return prefs.getString("sessionid", null)
    }

    fun clearCookie() {
        prefs.edit().remove("sessionid").apply()
    }
}
