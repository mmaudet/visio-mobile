package io.visio.mobile.auth

import android.content.Context
import android.net.Uri
import android.security.KeyStoreException
import android.util.Log
import androidx.browser.customtabs.CustomTabsIntent
import androidx.security.crypto.EncryptedSharedPreferences
import androidx.security.crypto.MasterKey
import io.visio.mobile.meetBaseUrl
import java.security.GeneralSecurityException
import uniffi.visio.PkceChallenge
import uniffi.visio.pkceGenerate

class OidcAuthManager(context: Context) {
    companion object {
        private const val TAG = "OidcAuthManager"
        const val AUTH_CALLBACK_HOST = "auth-callback"

        private const val KEY_ACCESS_TOKEN = "access_token"
        private const val KEY_REFRESH_TOKEN = "refresh_token"
        private const val KEY_MEET_INSTANCE = "meet_instance"
        private const val KEY_PKCE_VERIFIER = "pkce_verifier"
        private const val KEY_PKCE_STATE = "pkce_state"
    }

    private val masterKey =
        MasterKey.Builder(context)
            .setKeyScheme(MasterKey.KeyScheme.AES256_GCM)
            .build()

    /**
     * Encrypted shared-prefs handle. Null when the Android Keystore is
     * unavailable (e.g. lock-screen change invalidated the user-auth-bound
     * key, OEM ROM with broken Keystore) — callers MUST treat that as
     * "secure storage unavailable" and refuse to persist tokens. We
     * deliberately do **not** silently wipe & recreate: a blank recovery
     * looks like "I'm logged out" and pushes the user to re-auth on
     * whatever (potentially hostile) network they're on.
     */
    private val prefs =
        try {
            EncryptedSharedPreferences.create(
                context,
                "visio_auth",
                masterKey,
                EncryptedSharedPreferences.PrefKeyEncryptionScheme.AES256_SIV,
                EncryptedSharedPreferences.PrefValueEncryptionScheme.AES256_GCM,
            )
        } catch (e: GeneralSecurityException) {
            Log.e(TAG, "EncryptedSharedPreferences unavailable (Keystore error)", e)
            null
        } catch (e: KeyStoreException) {
            Log.e(TAG, "EncryptedSharedPreferences unavailable (KeyStore exception)", e)
            null
        }

    /** True when secure storage is available. Views can surface a warning. */
    val secureStorageAvailable: Boolean get() = prefs != null

    /** The meet instance currently being authenticated against. */
    var pendingAuthInstance: String? = null
        private set

    /**
     * Launch the OAuth2 + PKCE authentication flow in a Chrome Custom Tab.
     *
     * The mobile client follows RFC 8252:
     *   1. Generates a code_verifier + code_challenge + state via the Rust core.
     *   2. Stores verifier+state in EncryptedSharedPreferences.
     *   3. Opens the Meet `/authenticate/` URL with `response_type=code`,
     *      `code_challenge`, `code_challenge_method=S256`, `state`,
     *      and `returnTo=/mobile-login`.
     *   4. The server logs the user in via the configured OIDC provider, then
     *      redirects to `visio://auth-callback?code=...&state=...`.
     *   5. The intent filter on `visio://auth-callback` brings the activity back;
     *      we extract `code+state` in `handleAuthCallback` and exchange it
     *      via `VisioClient.exchangePkceCode(...)` to obtain JWT tokens.
     */
    fun launchOidcFlow(
        context: Context,
        meetInstance: String,
    ) {
        pendingAuthInstance = meetInstance

        val pkce: PkceChallenge = pkceGenerate()
        saveVerifier(pkce.verifier)
        saveState(pkce.state)

        val returnTo = "visio://$AUTH_CALLBACK_HOST"
        val authUrl = buildString {
            append("${meetBaseUrl(meetInstance)}/api/v1.0/authenticate/?")
            append("response_type=code")
            append("&code_challenge=").append(java.net.URLEncoder.encode(pkce.challenge, "UTF-8"))
            append("&code_challenge_method=S256")
            append("&state=").append(java.net.URLEncoder.encode(pkce.state, "UTF-8"))
            append("&returnTo=").append(java.net.URLEncoder.encode("/mobile-login", "UTF-8"))
            append("&prompt=login")
        }

        Log.d(TAG, "Starting PKCE OIDC flow via Custom Tab")

        val customTabsIntent =
            CustomTabsIntent.Builder()
                .setShowTitle(true)
                .build()
        customTabsIntent.launchUrl(context, Uri.parse(authUrl))
    }

    /**
     * Handle the `visio://auth-callback?code={...}&state={...}` deep link.
     *
     * Returns Triple(code, codeVerifier, meetInstance) on success, null if:
     *   - no pending auth instance,
     *   - missing code or state in the URI,
     *   - state mismatch (CSRF protection).
     *
     * The verifier is consumed: the caller MUST exchange it via
     * `VisioClient.exchangePkceCode(...)` promptly. We clear it from storage
     * before returning so a stale verifier cannot be reused.
     */
    fun handleAuthCallback(callbackUri: Uri?): Triple<String, String, String>? {
        val meetInstance = pendingAuthInstance
        if (meetInstance == null) {
            Log.w(TAG, "Auth callback received but no pending auth instance")
            return null
        }
        pendingAuthInstance = null

        val code = callbackUri?.getQueryParameter("code")
        val state = callbackUri?.getQueryParameter("state")
        if (code.isNullOrBlank() || state.isNullOrBlank()) {
            Log.w(TAG, "Missing code or state in callback URI")
            return null
        }

        val expectedState = getSavedState()
        val verifier = getSavedVerifier()
        clearVerifier()
        clearState()

        // Constant-time comparison: single-use makes timing leaks moot in
        // practice, but `MessageDigest.isEqual` removes any ambiguity.
        if (expectedState == null ||
            !java.security.MessageDigest.isEqual(
                expectedState.toByteArray(Charsets.UTF_8),
                state.toByteArray(Charsets.UTF_8),
            )
        ) {
            Log.w(TAG, "PKCE state mismatch — possible CSRF attempt")
            return null
        }
        if (verifier.isNullOrBlank()) {
            Log.w(TAG, "No PKCE verifier stored for this exchange")
            return null
        }

        Log.d(TAG, "PKCE callback OK: extracted code+verifier")
        return Triple(code, verifier, meetInstance)
    }

    // ── Secure storage helpers ──────────────────────────────────────────
    //
    // All writes are silent no-ops when `prefs == null` (Keystore unavailable);
    // reads return null. The caller must check `secureStorageAvailable` if it
    // wants to surface a UI warning. We deliberately do not throw — the user
    // can still join meetings as a guest without persistent auth.

    /** Persist tokens together with the meet instance they were minted for. */
    fun saveTokens(access: String, refresh: String, meetInstance: String) {
        prefs?.edit()
            ?.putString(KEY_ACCESS_TOKEN, access)
            ?.putString(KEY_REFRESH_TOKEN, refresh)
            ?.putString(KEY_MEET_INSTANCE, meetInstance)
            ?.apply()
    }

    fun getSavedAccessToken(): String? = prefs?.getString(KEY_ACCESS_TOKEN, null)

    fun getSavedRefreshToken(): String? = prefs?.getString(KEY_REFRESH_TOKEN, null)

    /** Returns the meet instance that the saved tokens were issued for. */
    fun getSavedMeetInstance(): String? = prefs?.getString(KEY_MEET_INSTANCE, null)

    fun clearTokens() {
        prefs?.edit()
            ?.remove(KEY_ACCESS_TOKEN)
            ?.remove(KEY_REFRESH_TOKEN)
            ?.remove(KEY_MEET_INSTANCE)
            ?.apply()
    }

    private fun saveVerifier(verifier: String) {
        prefs?.edit()?.putString(KEY_PKCE_VERIFIER, verifier)?.apply()
    }

    private fun getSavedVerifier(): String? = prefs?.getString(KEY_PKCE_VERIFIER, null)

    private fun clearVerifier() {
        prefs?.edit()?.remove(KEY_PKCE_VERIFIER)?.apply()
    }

    private fun saveState(state: String) {
        prefs?.edit()?.putString(KEY_PKCE_STATE, state)?.apply()
    }

    private fun getSavedState(): String? = prefs?.getString(KEY_PKCE_STATE, null)

    private fun clearState() {
        prefs?.edit()?.remove(KEY_PKCE_STATE)?.apply()
    }
}
