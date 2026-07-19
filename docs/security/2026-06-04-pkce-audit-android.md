# Visio Mobile Android — Hostile Security Audit

**Date:** 2026-06-04
**Scope:** working-tree Android sources, focused on the just-refactored PKCE OIDC flow, deep-link surface, token storage, manifest, and network config.
**Branch state:** `visio-mobile main`, uncommitted PKCE refactor in the working tree.
**Audit type:** hostile / adversarial review on the user's own beta product (authorized).

---

## Critical

### C1 — `visio://` is an unverified custom scheme: deep-link / authorization-code hijack
- **Location**: `AndroidManifest.xml:36-41`, `OidcAuthManager.kt:78` (`returnTo = "visio://$AUTH_CALLBACK_HOST"`)
- **Attack scenario**: Any installed app can declare an identical `<data android:scheme="visio">` intent filter. When the IdP redirects to `visio://auth-callback?code=...&state=...`, Android shows a disambiguation chooser (or, with `setAsDefault`, silently routes to the attacker on most OEM ROMs). The attacker app then POSTs the captured `code` to `…/api/v1.0/authenticate/exchange`. Because the verifier was generated **on the victim's device** and the IdP only checks `code_challenge=S256(verifier)`, the attacker cannot complete PKCE — but they **can** force-fail the victim's exchange and, more importantly, observe the auth `code` (still useful for log correlation / target identification). On Android 11- there is no per-app scheme isolation; on Android 12+ App Links would help **only if** the redirect URI were an `https://` URL with `android:autoVerify="true"`. There is no `autoVerify`.
- Worse: the second filter `visio://` with NO host restriction (line 40) means even an unrelated `visio://anything` from an attacker app reaches `MainActivity`. Combined with `launchMode="singleTask"`, the same task receives the spoofed intent.
- **PoC**: malicious app `AndroidManifest` declares `<intent-filter><data android:scheme="visio" android:host="auth-callback"/></intent-filter>` with `android:priority="999"` plus `BROWSABLE` + `DEFAULT`. On many ROMs the higher priority wins for new installs.
- **Recommended fix**: move to **App Links** with `https://<meet-host>/mobile-oidc-callback` + `android:autoVerify="true"` + matching `assetlinks.json`. Keep `visio://` only for room deep-links, not for the OIDC redirect URI. If custom scheme must stay short-term, use [Custom Tabs Connection.validateRelationship](https://developer.android.com/reference/androidx/browser/customtabs/CustomTabsSession#validateRelationship) and/or require the in-flight `state` to match — which today **mitigates token theft** (see M1) but not code interception.

### C2 — Cleartext network exemption ships in release builds
- **Location**: `network_security_config.xml:6-11`, `AndroidManifest.xml:22`, `build.gradle.kts` (no `debug`/`release` source-set split, no `manifestPlaceholders`)
- **Attack scenario**: The cleartext-permit config for `localhost`, `127.0.0.1`, `10.0.2.2`, `10.0.3.2` is referenced unconditionally from `AndroidManifest.xml` and therefore packaged into the **release APK/AAB**. While public hosts still require TLS, a rogue Wi-Fi captive portal that DNS-poisons `meet.example.com` won't help (TLS still applies), but a developer who once typed `127.0.0.1:8000` in the server picker, or a malicious deep link `visio://10.0.2.2/abc-defg-hij`, can drive the app to hit cleartext HTTP carrying the `Authorization: Bearer …` header attached by the Rust client (`schemeFor` returns `http` for those hosts at `VisioManager.kt:50-56`). On a hostile LAN, an attacker controlling `10.0.x.x` routes can read the access + refresh tokens in plaintext.
- **Recommended fix**: move `network_security_config.xml` into `src/debug/res/xml/` only, and in the `debug` source set add a `<application android:networkSecurityConfig=…>` overlay. In `release`, default policy (no cleartext) applies. Or gate via `manifestPlaceholders["networkSecurityConfig"]` per build type.

### C3 — `visio-test://connect` test deep link is in the production manifest
- **Location**: `AndroidManifest.xml:42-46`, handled at `MainActivity.kt:94-110`
- **Attack scenario**: The runtime handler at line 95 guards on `BuildConfig.DEBUG`, but the intent filter itself is in `main/AndroidManifest.xml` — therefore exposed in **release** too. While `parseTestDeepLink` early-returns `false` in release, the filter still makes `MainActivity` reachable via that URI, the activity is launched (raising it to foreground from background), and Android logs / lockscreen previews may leak the URL. More importantly, in any future build where `BuildConfig.DEBUG` is mis-evaluated (e.g. an internal QA flavor), an attacker can inject a LiveKit URL + `token` of their choosing — `VisioManager.pendingTestConnect` is then trusted blind to perform a join (likely subscribing the victim's mic/camera to an attacker-controlled room). Defense in depth missing.
- **Recommended fix**: move the second `<intent-filter>` into `src/debug/AndroidManifest.xml`, not `main/`.

## High

### H1 — `EncryptedSharedPreferences` fallback deletes the keystore entry and silently re-creates plaintext-equivalent storage on tamper
- **Location**: `OidcAuthManager.kt:38-47`
- **Attack scenario**: An attacker with brief device access (e.g. unlocked-phone "evil maid", or a malicious sibling app that triggers `MasterKey` corruption via Android Keystore key invalidation by changing screen lock — which **automatically invalidates** `AES256_GCM` keys bound to user auth) forces `EncryptedSharedPreferences.create` to throw. The `catch (_: Exception)` then calls `deleteSharedPreferences("visio_auth")` — **wiping all stored tokens** — and recreates fresh prefs. This is a permanent token-eraser DoS: every screen-lock change → user must re-auth. More subtly, if `MasterKey.Builder` ever succeeds but with a **different** key (e.g. after key rotation), the old ciphertext is unreadable and tokens evaporate silently. There is no user-visible signal — the app just behaves as logged out, encouraging the user to re-auth on whichever WiFi they're on (chains with C2).
- **Recommended fix**: don't swallow the exception. Log severity, surface a "secure storage unavailable" error, and refuse to write tokens at all — never fall back. Also catch the narrower `KeyStoreException` / `GeneralSecurityException` rather than blanket `Exception`.

### H2 — `adb backup` extracts tokens because `android:allowBackup` is unset (defaults to `true` on `targetSdk=35`)
- **Location**: `AndroidManifest.xml:16-23` (no `android:allowBackup="false"`, no `android:dataExtractionRules`)
- **Attack scenario**: With USB debugging temporarily enabled (lab, repair shop, border crossing), `adb backup -f out.ab io.visio.mobile` produces a tar of `/data/data/io.visio.mobile/shared_prefs/visio_auth.xml`. The values are AES-GCM-encrypted, but the keys are in Android Keystore — bound to the device, not the user. On rooted attacker devices this is decryptable; on non-rooted it is at least an offline target. Auto Backup to Google Drive (default on for cloud-backed devices) means encrypted token blobs end up on Google infrastructure, and Google's per-app E2E backup encryption only kicks in if the user has set a screen lock — many beta users haven't.
- **Recommended fix**: `android:allowBackup="false"` and `android:dataExtractionRules="@xml/data_extraction_rules"` excluding `shared_prefs/visio_auth.xml`.

### H3 — Refresh-path mishandles cleartext flag on token-restore failure
- **Location**: `VisioManager.kt:319-332` (`initAuth` catch-block)
- **Attack scenario**: On startup, `client.setTokens(meetBaseUrl(meetInstance), savedAccess, savedRefresh)` is called with whatever instance happens to be **first** in `getMeetInstances()`. If the user's actual session was against `meet.foo.gouv.fr` but the stored list begins with a previously-saved attacker-injected `10.0.2.2` (writable via `onTokensReceived` adding any instance from the OIDC callback path — see H4), the access token is shipped to `http://10.0.2.2` over cleartext (chains with C2). Even without that, the `firstOrNull()` semantics mean the tokens are bound to the wrong base URL: `refreshTokens(meetInstance)` then refreshes against the wrong host and returns failures the user can't diagnose.
- **Recommended fix**: persist the `meetInstance` alongside the tokens (it's already in `SessionState.Authenticated.meetInstance` and `authenticatedMeetInstance`), and use that exact value — not `firstOrNull()`.

### H4 — `onTokensReceived` auto-adds any meet instance to the trusted list, with no allow-list
- **Location**: `VisioManager.kt:378-383`
- **Attack scenario**: `meetInstance` here is whatever string `pendingAuthInstance` held when the callback arrived — which is set by `launchOidcFlow` from the **server picker UI** (`HomeScreen.kt:531`) where the user can type a custom server. A social-engineered user types `attacker.example` (or a homograph), completes OIDC against the attacker's IdP, and now `attacker.example` is permanently in `getMeetInstances()`. Worse: on next launch, `initAuth` may pick it as the `firstOrNull()` instance and ship the **legitimate** Meet's tokens to the attacker (chains with H3). Also: if anyone can craft a `visio://auth-callback?code=X&state=Y` deep link **after** the user has clicked the server picker (race), they trigger `exchangePkceCode` against the attacker IdP without an actual browser session.
- **Recommended fix**: validate `meetInstance` against an allow-list of known Meet hosts before adding, and require user confirmation for custom hosts. Also: log the host alongside the token storage so subsequent restores use the correct one.

## Medium

### M1 — PKCE `state` is stored in `EncryptedSharedPreferences` and lives across launches; concurrent flows clobber each other
- **Location**: `OidcAuthManager.kt:50-51, 72-76, 163-181`
- **Attack scenario**: `pendingAuthInstance` is in-memory and reset on process death, but `pkce_verifier` / `pkce_state` persist on disk. If the user starts an OIDC flow, kills the app, then receives a crafted `visio://auth-callback?code=ATTACKER&state=STORED_STATE` from a malicious app, **`handleAuthCallback` correctly returns `null`** at line 113 because `pendingAuthInstance == null`. Good. But: if the user starts a flow, the app is killed by OS pressure mid-flow, then the user opens an attacker-supplied phishing link that re-launches OIDC against an attacker-controlled instance, the **newer** `pkce_state` overwrites the older — the attacker can now feed back the new state and `code` to drive a valid exchange against attacker-IdP, achieving session fixation. Also, two concurrent server-picker taps race-overwrite verifier/state with no locking.
- **Recommended fix**: store `(verifier, state, meetInstance)` as a single atomic tuple keyed by a flow nonce, and require `pendingAuthInstance` to match the saved tuple's instance before exchange. Add a short TTL (e.g. 10 min) and clear stale tuples on launch.

### M2 — No tap-jacking protection on the "Me connecter" button
- **Location**: `HomeScreen.kt:527-554`
- **Attack scenario**: A malicious overlay app with `SYSTEM_ALERT_WINDOW` (granted by many sideloaded "battery saver" apps) draws a transparent decoy over the connect button. The user taps thinking they're clicking elsewhere, OIDC flow launches against an attacker-controlled instance via the visible decoy "server picker" overlay. Compose buttons do not honor `setFilterTouchesWhenObscured` by default.
- **Recommended fix**: on the connect button, wrap with a `Modifier.pointerInteropFilter` that drops events with `MotionEvent.FLAG_WINDOW_IS_OBSCURED`, or set `window.decorView.filterTouchesWhenObscured = true` on the activity.

### M3 — Custom Tab cross-app data sharing
- **Location**: `OidcAuthManager.kt:91-95`
- **Attack scenario**: `CustomTabsIntent.Builder().build()` resolves to the user's default browser. Default Chrome shares cookies / autofill with the user's browsing profile — so the OIDC IdP login may auto-complete with the cached Google account (or whatever) without re-prompting, even when `prompt=login` is set (Chrome can still satisfy the cookie). More importantly, on devices where the default browser is a low-trust app (e.g. shady "fast browser" sideloaded), that browser sees the entire OIDC URL incl. `state`/`code_challenge`, and the post-login `Location: visio://auth-callback?code=…` redirect — meaning **the default browser is an implicit man-in-the-middle for the auth code**.
- **Recommended fix**: explicitly require Chrome (`setPackage("com.android.chrome")` with fallback list of known-trustable browsers — Firefox, Samsung, DuckDuckGo). At minimum, prefer browsers supporting the Custom Tabs service over arbitrary VIEW handlers.

### M4 — `launchMode="singleTask"` + `taskAffinity` default enables StrandHogg-class task hijacking
- **Location**: `AndroidManifest.xml:25-31`
- **Attack scenario**: With default `taskAffinity` (== `applicationId` `io.visio.mobile`) and `singleTask`, a malicious app can declare `taskAffinity="io.visio.mobile"` + `allowTaskReparenting="true"` and inject an activity into Visio's back stack. When the user re-opens Visio, the attacker activity is shown instead — perfect for a phishing "session expired, please re-enter password" screen. Known as StrandHogg 2.0 on Android <12. On Android 12+ the OS partially mitigates, but `singleTask` still exposes back-stack manipulation.
- **Recommended fix**: set `android:taskAffinity=""` on `MainActivity` and consider `launchMode="singleInstance"` for the launcher activity or `android:allowTaskReparenting="false"`.

## Low

### L1 — `Log.i(TAG, "Auth callback: PKCE code received for $meetInstance")` leaks meet hostname to logcat
- **Location**: `MainActivity.kt:82`
- **Attack scenario**: On Android < 7 logcat is per-app, but any debug-bridge-connected device exposes it. Reveals which Meet instance the user authenticates against (privacy signal). Same concern at `VisioManager.kt:1077` ("Participant ${p.sid} (${p.name}): hasVideo=true …") leaks participant names to system logs.
- **Recommended fix**: gate identifying logs behind `BuildConfig.DEBUG`.

### L2 — `pendingDeepLinkDisplayName` from URL is rendered without sanitization
- **Location**: `MainActivity.kt:49, 56, 64`
- **Attack scenario**: A deep link `visio://meet.example.com/abc-defg-hij?visio=<malicious-unicode>` lets an attacker control the display-name prefill. Not a code-exec, but UI-spoofing surface for phishing the user with bidi-override characters in the room title.
- **Recommended fix**: strip control / RTL-override Unicode codepoints before storing in `pendingDeepLinkDisplayName`.

### L3 — `onTokensReceived` is not concurrency-safe
- **Location**: `VisioManager.kt:372-396`
- **Attack scenario**: Two rapid auth callbacks (user double-launches OIDC) both reach `saveTokens` then both call `setTokens` on the Rust client. Outcome depends on which `client.exchangePkceCode` resolved last — could leave stored tokens out-of-sync with Rust client state. Not directly exploitable but a stability/availability bug.
- **Recommended fix**: serialize via a `Mutex` on `authManager` operations, or reject calls when `pendingAuthInstance` is null at the entrypoint of `exchangePkceCode`.

## Info / Non-findings

- **State comparison** at `OidcAuthManager.kt:130` uses Kotlin `!=` (constant-time? no — but state is single-use and cleared, so timing leak is moot).
- **Verifier clearing** at `OidcAuthManager.kt:127-128` happens **before** the mismatch check — correct: prevents replay of a verifier even on failed exchanges.
- **`exported="false"`** on `CallForegroundService` (manifest line 52) is correct.
- **PiP `BroadcastReceiver`** uses `RECEIVER_NOT_EXPORTED` on Tiramisu+ (`MainActivity.kt:160`) — correct, but on pre-Tiramisu it falls back to the default-exported variant. Low risk since the actions only toggle mic / hangup the call.
- **Slug regex** at `MainActivity.kt:33` (`^[a-z]{3}-[a-z]{4}-[a-z]{3}$`) is strict enough to prevent path-traversal via deep link.
- **`getMeetInstances().contains(host)`** check at `MainActivity.kt:53` prevents arbitrary host injection into room URLs — good.
- **Proguard `-keep class io.visio.mobile.** { *; }`** in `proguard-rules.pro:20` keeps `MainActivity` and `OidcAuthManager` un-obfuscated, easing reverse-engineering but no direct vuln.
- **`signingConfigs.release`** falls back to empty strings if env vars unset — `gradle :app:assembleRelease` would still attempt signing with empty creds and fail loudly, not silently produce an unsigned APK. Acceptable.

## Top-3 priorities

1. **C2** — split `network_security_config.xml` into `debug/` only. One-line manifest move, removes a major LAN-MITM token exfil path.
2. **C1** — replace `visio://auth-callback` with `https://` App Links (`autoVerify=true` + `assetlinks.json`).
3. **H2** — set `android:allowBackup="false"` on `<application>`. One attribute.

## Files in scope

- `android/app/src/main/AndroidManifest.xml`
- `android/app/src/main/res/xml/network_security_config.xml`
- `android/app/src/main/kotlin/io/visio/mobile/auth/OidcAuthManager.kt`
- `android/app/src/main/kotlin/io/visio/mobile/MainActivity.kt`
- `android/app/src/main/kotlin/io/visio/mobile/VisioManager.kt`
- `android/app/src/main/kotlin/io/visio/mobile/ui/HomeScreen.kt`
- `android/app/build.gradle.kts`
- `android/app/proguard-rules.pro`
