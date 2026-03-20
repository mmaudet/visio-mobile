package io.visio.mobile.auth

import android.content.Context
import android.net.Uri
import android.util.Log
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
        }&prompt=login"

        Log.d(TAG, "Starting OIDC flow via Custom Tab: $authUrl")

        val customTabsIntent =
            CustomTabsIntent.Builder()
                .setShowTitle(true)
                .build()
        customTabsIntent.launchUrl(context, Uri.parse(authUrl))
    }

    /**
     * Handle the visio://auth-callback?code={uuid} deep link.
     * Extracts the exchange code from the URI query parameter.
     *
     * @return Pair of (code, meetInstance) if successful, null otherwise.
     */
    fun handleAuthCallback(callbackUri: android.net.Uri?): Pair<String, String>? {
        val meetInstance = pendingAuthInstance
        if (meetInstance == null) {
            Log.w(TAG, "Auth callback received but no pending auth instance")
            return null
        }
        pendingAuthInstance = null

        val code = callbackUri?.getQueryParameter("code")
        if (code.isNullOrBlank()) {
            Log.w(TAG, "No code parameter in callback URI: $callbackUri")
            return null
        }

        Log.d(TAG, "Exchange code extracted from callback")
        return Pair(code, meetInstance)
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
