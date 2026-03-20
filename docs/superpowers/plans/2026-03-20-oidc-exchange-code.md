# OIDC Exchange Code Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace cookie-based OIDC auth with secure one-time exchange codes on all 3 platforms (iOS, Android, Desktop).

**Architecture:** The Meet server (PR suitenumerique/meet#1170) redirects `visio://auth-callback?code={uuid}` after OIDC login. The app extracts the `code` parameter, POSTs it to `/api/v1.0/auth/session-exchange/`, and receives the session ID in response. This eliminates the need for cookie extraction from browsers/webviews. A new `exchange_oidc_code()` function in `visio-core` centralizes the HTTP call, exposed via UniFFI to mobile and directly to desktop.

**Tech Stack:** Rust (visio-core + visio-ffi + visio-desktop), Swift/SwiftUI (iOS), Kotlin/Compose (Android), TypeScript/React (Desktop frontend)

---

## File Structure

| File | Action | Responsibility |
|------|--------|----------------|
| `crates/visio-core/src/session.rs` | Modify | Add `exchange_oidc_code()` — POST code to server, return session ID |
| `crates/visio-ffi/src/lib.rs` | Modify | Expose `exchange_oidc_code()` via UniFFI wrapper |
| `crates/visio-ffi/src/visio.udl` | Modify | Add `exchange_oidc_code` to interface definition |
| `ios/VisioMobile/Auth/OidcAuthManager.swift` | Rewrite | Use `ASWebAuthenticationSession`, extract `code` from callback URL, call `exchange_oidc_code()` |
| `ios/VisioMobile/Views/HomeView.swift` | Modify | Remove WKWebView fallback sheet, use new auth flow |
| `ios/VisioMobile/VisioManager.swift` | Modify | Update `onAuthCookieReceived` flow |
| `android/app/src/main/kotlin/io/visio/mobile/auth/OidcAuthManager.kt` | Modify | Extract `code` param from callback URL instead of cookie |
| `android/app/src/main/kotlin/io/visio/mobile/MainActivity.kt` | Modify | Pass `code` param from deep link to auth handler |
| `crates/visio-desktop/src/lib.rs` | Modify | `launch_oidc()` — open browser with `returnTo`, intercept `visio://auth-callback?code=`, call `exchange_oidc_code()` |

---

### Task 1: Add `exchange_oidc_code()` to Rust core

**Files:**
- Modify: `crates/visio-core/src/session.rs`

- [ ] **Step 1: Write the unit test**

```rust
// In session.rs, add to mod tests:
#[tokio::test]
async fn test_exchange_oidc_code_with_invalid_code() {
    let result = SessionManager::exchange_oidc_code("dev-meet.linagora.com", "invalid-code-too-short").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_exchange_oidc_code_formats_url_correctly() {
    // This will fail with a network error but validates URL construction
    let result = SessionManager::exchange_oidc_code("nonexistent.example.com", "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee").await;
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("Http") || err.contains("error"), "unexpected error: {err}");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p visio-core -- test_exchange_oidc_code`
Expected: FAIL — `exchange_oidc_code` not found

- [ ] **Step 3: Implement `exchange_oidc_code()`**

Add to `SessionManager` impl in `session.rs`:

```rust
/// Exchange a one-time OIDC code for a session ID.
///
/// The Meet server (PR suitenumerique/meet#1170) generates a short-lived UUID
/// code and redirects to `visio://auth-callback?code={uuid}`. This method
/// POSTs the code to the exchange endpoint and returns the session cookie value.
pub async fn exchange_oidc_code(meet_instance: &str, code: &str) -> Result<String, VisioError> {
    let url = format!("https://{}/api/v1.0/auth/session-exchange/", meet_instance);

    let client = reqwest::Client::new();
    let response = client
        .post(&url)
        .json(&serde_json::json!({ "code": code }))
        .send()
        .await
        .map_err(|e| VisioError::Http(e.to_string()))?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(VisioError::Auth(format!(
            "code exchange failed ({status}): {body}"
        )));
    }

    let body: serde_json::Value = response
        .json()
        .await
        .map_err(|e| VisioError::Http(e.to_string()))?;

    // The response key matches SESSION_COOKIE_NAME on the server (default: "meet_sessionid")
    body.get("meet_sessionid")
        .or_else(|| body.get("sessionid"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| VisioError::Auth("no session ID in exchange response".into()))
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p visio-core -- test_exchange_oidc_code`
Expected: PASS (both tests hit network and get expected errors)

- [ ] **Step 5: Commit**

```bash
git add crates/visio-core/src/session.rs
git commit -m "feat(core): add exchange_oidc_code() for secure OIDC code exchange"
```

---

### Task 2: Expose `exchange_oidc_code()` via UniFFI

**Files:**
- Modify: `crates/visio-ffi/src/lib.rs`
- Modify: `crates/visio-ffi/src/visio.udl`

- [ ] **Step 1: Add to UDL interface**

In `visio.udl`, add inside the `VisioClient` interface block (after `authenticate`):

```
[Throws=VisioError]
string exchange_oidc_code(string meet_instance, string code);
```

- [ ] **Step 2: Implement in `lib.rs`**

Add to `impl VisioClient`:

```rust
pub fn exchange_oidc_code(&self, meet_instance: String, code: String) -> Result<String, VisioError> {
    self.rt
        .block_on(visio_core::SessionManager::exchange_oidc_code(&meet_instance, &code))
        .map_err(VisioError::from)
}
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p visio-ffi`
Expected: OK

- [ ] **Step 4: Regenerate UniFFI bindings**

Run: `./scripts/generate-bindings.sh all`
Expected: Updated Kotlin + Swift generated files

- [ ] **Step 5: Commit**

```bash
git add crates/visio-ffi/src/lib.rs crates/visio-ffi/src/visio.udl android/app/src/main/kotlin/generated/ ios/VisioMobile/Generated/
git commit -m "feat(ffi): expose exchange_oidc_code() to mobile platforms"
```

---

### Task 3: iOS — Replace WKWebView with ASWebAuthenticationSession + exchange code

**Files:**
- Rewrite: `ios/VisioMobile/Auth/OidcAuthManager.swift`
- Modify: `ios/VisioMobile/Views/HomeView.swift`
- Modify: `ios/VisioMobile/VisioManager.swift`

- [ ] **Step 1: Rewrite OidcAuthManager.swift**

Replace the entire file content with:

```swift
import AuthenticationServices
import Security
import UIKit

class OidcAuthManager: NSObject, ObservableObject, ASWebAuthenticationPresentationContextProviding {

    @Published var pendingInstance: String?

    private var authSession: ASWebAuthenticationSession?

    /// Launch OIDC flow via ASWebAuthenticationSession (system browser).
    /// The server redirects to visio://auth-callback?code={uuid} after login.
    /// Returns the exchange code via the completion handler.
    func launchOidcFlow(meetInstance: String, completion: @escaping (String?) -> Void) {
        pendingInstance = meetInstance

        let returnTo = "visio://auth-callback"
        let encodedReturnTo = returnTo.addingPercentEncoding(withAllowedCharacters: .urlQueryAllowed) ?? returnTo
        guard let authURL = URL(string: "https://\(meetInstance)/api/v1.0/authenticate/?returnTo=\(encodedReturnTo)") else {
            completion(nil)
            return
        }

        let session = ASWebAuthenticationSession(
            url: authURL,
            callbackURLScheme: "visio"
        ) { [weak self] callbackURL, error in
            guard let self else { return }
            self.authSession = nil

            if let error {
                let nsError = error as NSError
                if nsError.domain == ASWebAuthenticationSessionError.errorDomain
                    && nsError.code == ASWebAuthenticationSessionError.canceledLogin.rawValue {
                    DispatchQueue.main.async {
                        self.pendingInstance = nil
                        completion(nil)
                    }
                    return
                }
                NSLog("[OidcAuthManager] ASWebAuth error: \(error.localizedDescription)")
                DispatchQueue.main.async {
                    self.pendingInstance = nil
                    completion(nil)
                }
                return
            }

            // Extract exchange code from callback URL: visio://auth-callback?code={uuid}
            guard let url = callbackURL,
                  let components = URLComponents(url: url, resolvingAgainstBaseURL: false),
                  let code = components.queryItems?.first(where: { $0.name == "code" })?.value else {
                NSLog("[OidcAuthManager] No code parameter in callback URL")
                DispatchQueue.main.async {
                    self.pendingInstance = nil
                    completion(nil)
                }
                return
            }

            DispatchQueue.main.async {
                self.pendingInstance = nil
                completion(code)
            }
        }

        session.prefersEphemeralWebBrowserSession = false
        session.presentationContextProvider = self
        authSession = session
        session.start()
    }

    // MARK: - ASWebAuthenticationPresentationContextProviding

    func presentationAnchor(for session: ASWebAuthenticationSession) -> ASPresentationAnchor {
        guard let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene,
              let window = scene.windows.first(where: { $0.isKeyWindow }) ?? scene.windows.first else {
            return ASPresentationAnchor()
        }
        return window
    }

    // MARK: - Keychain Storage

    func saveCookie(_ cookie: String) {
        let data = cookie.data(using: .utf8)!
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrAccount as String: "visio_sessionid",
            kSecAttrService as String: "io.visio.mobile",
        ]
        SecItemDelete(query as CFDictionary)
        var addQuery = query
        addQuery[kSecValueData as String] = data
        SecItemAdd(addQuery as CFDictionary, nil)
    }

    func getSavedCookie() -> String? {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrAccount as String: "visio_sessionid",
            kSecAttrService as String: "io.visio.mobile",
            kSecReturnData as String: true,
        ]
        var result: AnyObject?
        let status = SecItemCopyMatching(query as CFDictionary, &result)
        guard status == errSecSuccess, let data = result as? Data else { return nil }
        return String(data: data, encoding: .utf8)
    }

    func clearCookie() {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrAccount as String: "visio_sessionid",
            kSecAttrService as String: "io.visio.mobile",
        ]
        SecItemDelete(query as CFDictionary)
    }
}
```

- [ ] **Step 2: Update HomeView.swift — replace WKWebView sheet with launchOidcFlow**

In `HomeView.swift`, replace the `launchOidc(meetInstance:)` function:

```swift
private func launchOidc(meetInstance: String) {
    manager.authManager.launchOidcFlow(meetInstance: meetInstance) { [weak manager] code in
        guard let code, let manager else { return }
        // Exchange code for session cookie via Rust core
        DispatchQueue.global(qos: .userInitiated).async {
            do {
                let sessionId = try manager.client.exchangeOidcCode(meetInstance: meetInstance, code: code)
                DispatchQueue.main.async {
                    manager.onAuthCookieReceived(sessionId, meetInstance: meetInstance)
                }
            } catch {
                NSLog("[HomeView] OIDC code exchange failed: \(error)")
            }
        }
    }
}
```

Remove the `.sheet(isPresented:)` block that presents `OidcFallbackWebView` (the WKWebView fallback).
Remove the `OidcFallbackWebView` struct if it's defined in HomeView.swift.

Also update `ServerPickerWithOidc.selectInstance()` to use the same pattern.

- [ ] **Step 3: Remove Combine forwarding from VisioManager**

In `VisioManager.swift`, remove the `authCancellable` property and the `objectWillChange` forwarding that was added for the WKWebView sheet (no longer needed since `pendingInstance` is no longer observed by SwiftUI for sheet presentation).

- [ ] **Step 4: Build and verify**

Run: iOS build (Xcode or `scripts/build-ios.sh`)
Expected: Compiles without errors

- [ ] **Step 5: Commit**

```bash
git add ios/VisioMobile/Auth/OidcAuthManager.swift ios/VisioMobile/Views/HomeView.swift ios/VisioMobile/VisioManager.swift
git commit -m "feat(ios): use ASWebAuthenticationSession with exchange code for OIDC"
```

---

### Task 4: Android — Extract exchange code from deep link instead of cookie

**Files:**
- Modify: `android/app/src/main/kotlin/io/visio/mobile/auth/OidcAuthManager.kt`
- Modify: `android/app/src/main/kotlin/io/visio/mobile/MainActivity.kt`
- Modify: `android/app/src/main/kotlin/io/visio/mobile/VisioManager.kt`

- [ ] **Step 1: Update OidcAuthManager — extract code from callback URI**

Replace `handleAuthCallback()` method:

```kotlin
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
```

- [ ] **Step 2: Update MainActivity — pass URI to handler**

In `MainActivity.kt`, update `parseDeepLink()` and `handleAuthCallback()`:

```kotlin
private fun parseDeepLink(intent: Intent?): String? {
    val uri = intent?.data ?: return null
    if (uri.scheme != "visio") return null
    val host = uri.host ?: return null

    if (host == OidcAuthManager.AUTH_CALLBACK_HOST) {
        handleAuthCallback(uri)
        return null
    }

    val slug = uri.path?.trimStart('/') ?: return null
    if (host.isBlank() || slug.isBlank()) return null

    val instances = VisioManager.client.getMeetInstances()
    return if (instances.contains(host)) "https://$host/$slug" else null
}

private fun handleAuthCallback(uri: android.net.Uri) {
    val result = VisioManager.authManager.handleAuthCallback(uri)
    if (result != null) {
        val (code, meetInstance) = result
        Log.i(TAG, "Auth callback: exchange code received for $meetInstance")
        VisioManager.exchangeOidcCode(code, meetInstance)
    } else {
        Log.w(TAG, "Auth callback: failed to extract exchange code")
    }
}
```

- [ ] **Step 3: Add `exchangeOidcCode()` to VisioManager**

In `VisioManager.kt`, add:

```kotlin
fun exchangeOidcCode(code: String, meetInstance: String) {
    scope.launch {
        try {
            val sessionId = client.exchangeOidcCode(meetInstance, code)
            onAuthCookieReceived(sessionId, meetInstance)
        } catch (e: Exception) {
            Log.e("VISIO", "OIDC code exchange failed: ${e.message}")
        }
    }
}
```

- [ ] **Step 4: Run ktlint**

Run: `cd android && ./gradlew ktlintMainSourceSetCheck`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add android/app/src/main/kotlin/io/visio/mobile/auth/OidcAuthManager.kt android/app/src/main/kotlin/io/visio/mobile/MainActivity.kt android/app/src/main/kotlin/io/visio/mobile/VisioManager.kt
git commit -m "feat(android): use exchange code for OIDC instead of cookie extraction"
```

---

### Task 5: Desktop — Use system browser redirect with exchange code

**Files:**
- Modify: `crates/visio-desktop/src/lib.rs`

- [ ] **Step 1: Rewrite `launch_oidc()` to use deep link with exchange code**

Replace the `launch_oidc` function. Instead of opening a Tauri webview and extracting cookies, open the system default browser with `returnTo=visio://auth-callback`. The app's deep link handler (`onOpenUrl`) will receive the callback URL with the code parameter.

```rust
#[tauri::command]
async fn launch_oidc(
    app: AppHandle,
    state: tauri::State<'_, VisioState>,
    meet_instance: String,
) -> Result<serde_json::Value, String> {
    let return_to = urlencoding::encode("visio://auth-callback");
    let auth_url = format!(
        "https://{}/api/v1.0/authenticate/?returnTo={}",
        meet_instance, return_to
    );

    // Store the pending instance so we know which server to exchange the code with
    *state.pending_oidc_instance.lock().await = Some(meet_instance.clone());

    // Open system browser
    tauri::opener::open_url(&app, auth_url, None::<&str>)
        .map_err(|e| format!("failed to open browser: {e}"))?;

    // The code will arrive via the deep link handler (onOpenUrl)
    // Wait for it on a oneshot channel
    let rx = {
        let (tx, rx) = tokio::sync::oneshot::channel::<String>();
        *state.oidc_code_sender.lock().await = Some(tx);
        rx
    };

    let code = tokio::time::timeout(
        std::time::Duration::from_secs(300), // 5 minute timeout
        rx,
    )
    .await
    .map_err(|_| "OIDC login timed out".to_string())?
    .map_err(|_| "OIDC login cancelled".to_string())?;

    // Exchange code for session ID
    let session_cookie = SessionManager::exchange_oidc_code(&meet_instance, &code)
        .await
        .map_err(|e| e.to_string())?;

    // Fetch user info and store session
    let meet_url = format!("https://{}", meet_instance);
    let user = SessionManager::fetch_user(&meet_url, &session_cookie)
        .await
        .map_err(|e| e.to_string())?;
    let mut session = state.session.lock().await;
    session.set_authenticated(user.clone(), session_cookie, meet_instance.clone());

    Ok(serde_json::json!({
        "display_name": user.display_name(),
        "email": user.email,
        "meet_instance": meet_instance,
    }))
}
```

- [ ] **Step 2: Add `pending_oidc_instance` and `oidc_code_sender` to VisioState**

In the `VisioState` struct definition, add:

```rust
pending_oidc_instance: tokio::sync::Mutex<Option<String>>,
oidc_code_sender: tokio::sync::Mutex<Option<tokio::sync::oneshot::Sender<String>>>,
```

Initialize both to `Mutex::new(None)` where VisioState is constructed.

- [ ] **Step 3: Handle deep link callback in the `onOpenUrl` handler**

In the deep link handler (the `on_url_open` or `tauri::Builder.on_url_scheme_request` setup), add handling for `visio://auth-callback?code={uuid}`:

```rust
// In the deep link handler:
if url.host() == Some("auth-callback") {
    if let Some(code) = url.query_pairs().find(|(k, _)| k == "code").map(|(_, v)| v.to_string()) {
        let sender = state.oidc_code_sender.lock().await.take();
        if let Some(tx) = sender {
            let _ = tx.send(code);
        }
        return; // Don't process as a room deep link
    }
}
```

- [ ] **Step 4: Verify compilation**

Run: `cargo check -p visio-desktop`
Expected: OK

- [ ] **Step 5: Commit**

```bash
git add crates/visio-desktop/src/lib.rs
git commit -m "feat(desktop): use system browser + exchange code for OIDC login"
```

---

### Task 6: Remove dead WKWebView code from iOS

**Files:**
- Delete: WKWebView fallback code in `HomeView.swift` (`OidcFallbackWebView` struct)
- Modify: `ios/VisioMobile/Auth/OidcAuthManager.swift` — remove cookie extraction methods

- [ ] **Step 1: Remove `OidcFallbackWebView` from HomeView.swift**

Delete the `OidcFallbackWebView` struct and the `.sheet(isPresented:)` modifier that presented it.

- [ ] **Step 2: Remove dead code from OidcAuthManager**

Remove `extractSessionCookie()`, `onWebViewCookie()`, `onComplete` callback, `cookieNames` — all the cookie-based code that's no longer used.

- [ ] **Step 3: Remove OidcWebViewDialog from Android**

The file `android/app/src/main/kotlin/io/visio/mobile/ui/OidcWebViewDialog.kt` is the WebView fallback. Remove it if no longer referenced.

- [ ] **Step 4: Build all platforms**

Run: `cargo check -p visio-core && cargo check -p visio-ffi && cargo check -p visio-desktop`
Run: `cd android && ./gradlew ktlintMainSourceSetCheck`

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "refactor: remove dead WKWebView/cookie-extraction code from OIDC flow"
```

---

### Task 7: Integration test on dev-meet.linagora.com

- [ ] **Step 1: Build all platforms**

```bash
cargo tauri build -c crates/visio-desktop/tauri.conf.json
cd android && ./gradlew assembleDebug
# iOS: build via Xcode
```

- [ ] **Step 2: Test Desktop**
- Open desktop app → Settings → ensure `dev-meet.linagora.com` is in instances
- Click "Se connecter" → select dev-meet → system browser opens
- Complete OIDC login → browser redirects to `visio://auth-callback?code=...`
- App receives callback → exchanges code → user is authenticated

- [ ] **Step 3: Test Android**
- Install debug APK on device
- Add `dev-meet.linagora.com` in Settings
- Tap connect → Chrome Custom Tab opens
- Complete OIDC → redirects to `visio://auth-callback?code=...`
- App receives deep link → exchanges code → authenticated

- [ ] **Step 4: Test iOS**
- Build and run on device
- Add `dev-meet.linagora.com` in Settings
- Tap connect → Safari sheet opens (ASWebAuthenticationSession)
- Complete OIDC → callback with code → exchanges → authenticated

- [ ] **Step 5: Create PR and push**

```bash
git push -u origin feat/oidc-exchange-code
gh pr create --title "feat: secure OIDC login with one-time exchange codes" --body "..."
```
