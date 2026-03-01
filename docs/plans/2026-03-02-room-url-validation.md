# Room URL Validation — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Validate room URLs in real time as the user types, disabling the Join button until the Meet API confirms the room exists.

**Architecture:** Two-layer validation — instant regex check on the slug format (`[a-z]{3}-[a-z]{4}-[a-z]{3}`), then a debounced (500ms) API call to `/api/v1.0/rooms/{slug}/` to confirm existence. The API response includes the LiveKit token, which we cache and reuse on connect to avoid a redundant second call. Changes span: Rust core (new `validate_room` method + `parse_meet_url` made public), FFI (new UDL enum + method), i18n (3 new keys x 6 langs), and all 3 platform UIs.

**Tech Stack:** Rust (visio-core, visio-ffi), UniFFI 0.29, Tauri 2.x (React/TypeScript), Kotlin/Compose (Android), SwiftUI (iOS)

**Design doc:** `docs/plans/2026-03-02-room-url-validation-design.md`

---

### Task 1: Add `validate_room` to Rust core

**Files:**
- Modify: `crates/visio-core/src/auth.rs`
- Test: `crates/visio-core/src/auth.rs` (inline tests)

**Step 1: Write the failing test**

Add to `crates/visio-core/src/auth.rs` inside the existing `#[cfg(test)] mod tests`:

```rust
#[test]
fn extract_slug_from_full_url() {
    let slug = AuthService::extract_slug("https://meet.linagora.com/dpd-jffv-trg").unwrap();
    assert_eq!(slug, "dpd-jffv-trg");
}

#[test]
fn extract_slug_from_bare_slug() {
    let slug = AuthService::extract_slug("dpd-jffv-trg").unwrap();
    assert_eq!(slug, "dpd-jffv-trg");
}

#[test]
fn extract_slug_invalid_format() {
    assert!(AuthService::extract_slug("hello").is_err());
    assert!(AuthService::extract_slug("").is_err());
    assert!(AuthService::extract_slug("abc-defg-hi").is_err()); // too short last segment
    assert!(AuthService::extract_slug("ABC-DEFG-HIJ").is_err()); // uppercase
}

#[test]
fn extract_slug_from_url_with_trailing_slash() {
    let slug = AuthService::extract_slug("https://meet.example.com/abc-defg-hij/").unwrap();
    assert_eq!(slug, "abc-defg-hij");
}
```

**Step 2: Run tests to verify they fail**

Run: `cd /Users/mmaudet/work/visio-mobile-v2 && cargo test -p visio-core -- extract_slug`
Expected: FAIL — `extract_slug` method does not exist

**Step 3: Implement `extract_slug` and `validate_room`**

Add to `crates/visio-core/src/auth.rs` inside `impl AuthService`:

```rust
/// Slug regex: 3 lowercase + dash + 4 lowercase + dash + 3 lowercase.
const SLUG_PATTERN: &str = r"^[a-z]{3}-[a-z]{4}-[a-z]{3}$";

/// Extract and validate the room slug from user input.
///
/// Accepts:
/// - Full URL: `https://meet.example.com/abc-defg-hij`
/// - Bare slug: `abc-defg-hij`
///
/// Returns the slug if it matches the expected format.
pub fn extract_slug(input: &str) -> Result<String, VisioError> {
    let input = input.trim().trim_end_matches('/');

    // Try to extract slug from URL (take last path segment)
    let candidate = if input.contains('/') {
        input.rsplit('/').next().unwrap_or("")
    } else {
        input
    };

    let re = regex::Regex::new(Self::SLUG_PATTERN).unwrap();
    if re.is_match(candidate) {
        Ok(candidate.to_string())
    } else {
        Err(VisioError::InvalidUrl(format!(
            "invalid room slug format: '{candidate}'"
        )))
    }
}

/// Validate a room URL by calling the Meet API.
///
/// Returns `Ok(TokenInfo)` if the room exists, `Err` otherwise.
/// The returned token can be cached and reused for `connect_with_token()`.
pub async fn validate_room(
    meet_url: &str,
    username: Option<&str>,
) -> Result<TokenInfo, VisioError> {
    // This calls the same API endpoint as request_token.
    // If the room doesn't exist, the API returns 404 → Auth error.
    Self::request_token(meet_url, username).await
}
```

Also add `regex` to the dependency. Check if it's already in `Cargo.toml`:

```toml
# In crates/visio-core/Cargo.toml, add if missing:
regex = "1"
```

**Step 4: Run tests to verify they pass**

Run: `cd /Users/mmaudet/work/visio-mobile-v2 && cargo test -p visio-core -- extract_slug`
Expected: All 4 tests PASS

**Step 5: Commit**

```bash
git add crates/visio-core/src/auth.rs crates/visio-core/Cargo.toml
git commit -m "feat(core): add extract_slug and validate_room to AuthService"
```

---

### Task 2: Add `RoomValidationResult` enum and FFI method

**Files:**
- Modify: `crates/visio-ffi/src/visio.udl`
- Modify: `crates/visio-ffi/src/lib.rs`

**Step 1: Add enum and method to UDL**

In `crates/visio-ffi/src/visio.udl`, add before the `interface VisioClient` block:

```udl
[Enum]
interface RoomValidationResult {
    Valid(string livekit_url, string token);
    NotFound();
    InvalidFormat(string message);
    NetworkError(string message);
};
```

Inside `interface VisioClient { ... }`, add:

```udl
    RoomValidationResult validate_room(string url, string? username);
```

**Step 2: Implement in Rust FFI**

In `crates/visio-ffi/src/lib.rs`, add the enum:

```rust
pub enum RoomValidationResult {
    Valid { livekit_url: String, token: String },
    NotFound,
    InvalidFormat { message: String },
    NetworkError { message: String },
}
```

Inside `impl VisioClient`, add:

```rust
pub fn validate_room(&self, url: String, username: Option<String>) -> RoomValidationResult {
    // First check format
    if let Err(e) = visio_core::AuthService::extract_slug(&url) {
        return RoomValidationResult::InvalidFormat { message: e.to_string() };
    }

    match self.rt.block_on(
        visio_core::AuthService::validate_room(&url, username.as_deref())
    ) {
        Ok(token_info) => RoomValidationResult::Valid {
            livekit_url: token_info.livekit_url,
            token: token_info.token,
        },
        Err(visio_core::VisioError::Auth(msg)) if msg.contains("404") => {
            RoomValidationResult::NotFound
        }
        Err(e) => RoomValidationResult::NetworkError { message: e.to_string() },
    }
}
```

**Step 3: Verify it compiles**

Run: `cd /Users/mmaudet/work/visio-mobile-v2 && cargo build -p visio-ffi`
Expected: Compiles without errors

**Step 4: Commit**

```bash
git add crates/visio-ffi/src/visio.udl crates/visio-ffi/src/lib.rs
git commit -m "feat(ffi): add validate_room method and RoomValidationResult enum"
```

---

### Task 3: Add Tauri `validate_room` command (Desktop)

**Files:**
- Modify: `crates/visio-desktop/src/lib.rs`

**Step 1: Add the Tauri command**

In `crates/visio-desktop/src/lib.rs`, add after the `connect` command:

```rust
#[tauri::command]
async fn validate_room(
    state: tauri::State<'_, VisioState>,
    url: String,
    username: Option<String>,
) -> Result<serde_json::Value, String> {
    let room = state.room.lock().await;

    // Format check first
    if let Err(e) = visio_core::AuthService::extract_slug(&url) {
        return Ok(serde_json::json!({ "status": "invalid_format", "message": e.to_string() }));
    }

    match visio_core::AuthService::validate_room(&url, username.as_deref()).await {
        Ok(token_info) => Ok(serde_json::json!({
            "status": "valid",
            "livekit_url": token_info.livekit_url,
            "token": token_info.token,
        })),
        Err(visio_core::VisioError::Auth(msg)) if msg.contains("404") => {
            Ok(serde_json::json!({ "status": "not_found" }))
        }
        Err(e) => Ok(serde_json::json!({ "status": "error", "message": e.to_string() })),
    }
}
```

Register it in the Tauri builder's `invoke_handler`. Find the existing `.invoke_handler(tauri::generate_handler![...])` line and add `validate_room` to the list.

**Step 2: Verify it compiles**

Run: `cd /Users/mmaudet/work/visio-mobile-v2 && cargo build -p visio-desktop`
Expected: Compiles

**Step 3: Commit**

```bash
git add crates/visio-desktop/src/lib.rs
git commit -m "feat(desktop): add validate_room Tauri command"
```

---

### Task 4: Add i18n keys for validation messages

**Files:**
- Modify: `i18n/en.json`
- Modify: `i18n/fr.json`
- Modify: `i18n/de.json`
- Modify: `i18n/es.json`
- Modify: `i18n/it.json`
- Modify: `i18n/nl.json`

**Step 1: Add 3 new keys to all 6 language files**

Add these keys to each file:

**en.json:**
```json
"home.room.checking": "Checking room...",
"home.room.valid": "Room found",
"home.room.notFound": "Room not found"
```

**fr.json:**
```json
"home.room.checking": "Vérification de la salle...",
"home.room.valid": "Salle trouvée",
"home.room.notFound": "Salle introuvable"
```

**de.json:**
```json
"home.room.checking": "Raum wird überprüft...",
"home.room.valid": "Raum gefunden",
"home.room.notFound": "Raum nicht gefunden"
```

**es.json:**
```json
"home.room.checking": "Verificando la sala...",
"home.room.valid": "Sala encontrada",
"home.room.notFound": "Sala no encontrada"
```

**it.json:**
```json
"home.room.checking": "Verifica della stanza...",
"home.room.valid": "Stanza trovata",
"home.room.notFound": "Stanza non trovata"
```

**nl.json:**
```json
"home.room.checking": "Kamer controleren...",
"home.room.valid": "Kamer gevonden",
"home.room.notFound": "Kamer niet gevonden"
```

**Step 2: Copy updated i18n to Android assets**

Run: `cp /Users/mmaudet/work/visio-mobile-v2/i18n/*.json /Users/mmaudet/work/visio-mobile-v2/android/app/src/main/assets/i18n/`

**Step 3: Commit**

```bash
git add i18n/ android/app/src/main/assets/i18n/
git commit -m "feat(i18n): add room validation messages in 6 languages"
```

---

### Task 5: Desktop UI — add real-time validation to HomeView

**Files:**
- Modify: `crates/visio-desktop/frontend/src/App.tsx`

**Step 1: Add validation state and debounce to HomeView**

In `crates/visio-desktop/frontend/src/App.tsx`, modify the `HomeView` function.

Add a slug regex constant at module level (near the existing `SUPPORTED_LANGS`):

```typescript
const SLUG_REGEX = /^[a-z]{3}-[a-z]{4}-[a-z]{3}$/;

function extractSlug(input: string): string | null {
  const trimmed = input.trim().replace(/\/$/, "");
  // Try last path segment for URLs
  const candidate = trimmed.includes("/")
    ? trimmed.split("/").pop() || ""
    : trimmed;
  return SLUG_REGEX.test(candidate) ? candidate : null;
}
```

In the `HomeView` function body, add state and effect:

```typescript
const [roomStatus, setRoomStatus] = useState<
  "idle" | "checking" | "valid" | "not_found" | "error"
>("idle");
const [cachedToken, setCachedToken] = useState<{
  livekit_url: string;
  token: string;
} | null>(null);

// Debounced room validation
useEffect(() => {
  const slug = extractSlug(meetUrl);
  if (!slug) {
    setRoomStatus("idle");
    setCachedToken(null);
    return;
  }

  setRoomStatus("checking");
  setCachedToken(null);
  const controller = new AbortController();

  const timer = setTimeout(async () => {
    try {
      const result = await invoke<{
        status: string;
        livekit_url?: string;
        token?: string;
      }>("validate_room", { url: meetUrl.trim(), username: displayName.trim() || null });

      if (controller.signal.aborted) return;

      if (result.status === "valid") {
        setRoomStatus("valid");
        setCachedToken({
          livekit_url: result.livekit_url!,
          token: result.token!,
        });
      } else if (result.status === "not_found") {
        setRoomStatus("not_found");
      } else {
        setRoomStatus("error");
      }
    } catch {
      if (!controller.signal.aborted) setRoomStatus("error");
    }
  }, 500);

  return () => {
    clearTimeout(timer);
    controller.abort();
  };
}, [meetUrl]);
```

**Step 2: Update the Join button to use validation state**

Replace the existing `<button>` join line:

```tsx
<button
  className="btn btn-primary"
  disabled={joining || roomStatus !== "valid"}
  onClick={handleJoin}
>
  {joining ? t("home.connecting") : t("home.join")}
</button>
```

**Step 3: Add status indicator below the URL input**

After the `meetUrl` input's closing `</div>`, add:

```tsx
{roomStatus === "checking" && (
  <div className="room-status checking">{t("home.room.checking")}</div>
)}
{roomStatus === "valid" && (
  <div className="room-status valid">{t("home.room.valid")}</div>
)}
{roomStatus === "not_found" && (
  <div className="room-status not-found">{t("home.room.notFound")}</div>
)}
```

**Step 4: Add CSS for status indicators**

In `App.css`, add:

```css
.room-status {
  font-size: 0.85rem;
  margin-top: 4px;
  padding: 2px 0;
}
.room-status.checking { color: var(--greyscale-400); }
.room-status.valid { color: #18753c; }
.room-status.not-found { color: #e1000f; }
```

**Step 5: Verify it builds**

Run: `cd /Users/mmaudet/work/visio-mobile-v2/crates/visio-desktop/frontend && npm run build`
Expected: Builds without errors

**Step 6: Commit**

```bash
git add crates/visio-desktop/frontend/src/App.tsx crates/visio-desktop/frontend/src/App.css
git commit -m "feat(desktop): real-time room URL validation with debounce"
```

---

### Task 6: Android UI — add real-time validation to HomeScreen

**Files:**
- Modify: `android/app/src/main/kotlin/io/visio/mobile/ui/HomeScreen.kt`

**Step 1: Add validation state and debounced effect**

Add imports at the top:

```kotlin
import androidx.compose.material3.CircularProgressIndicator
import kotlinx.coroutines.delay
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import uniffi.visio.RoomValidationResult
```

Inside `HomeScreen`, add after the existing state variables:

```kotlin
var roomStatus by remember { mutableStateOf<String>("idle") } // idle, checking, valid, not_found, error

// Slug regex: 3 lowercase, dash, 4 lowercase, dash, 3 lowercase
val slugRegex = remember { Regex("^[a-z]{3}-[a-z]{4}-[a-z]{3}$") }

fun extractSlug(input: String): String? {
    val trimmed = input.trim().trimEnd('/')
    val candidate = if ('/' in trimmed) trimmed.substringAfterLast('/') else trimmed
    return if (slugRegex.matches(candidate)) candidate else null
}

// Debounced room validation
LaunchedEffect(roomUrl) {
    val slug = extractSlug(roomUrl)
    if (slug == null) {
        roomStatus = "idle"
        return@LaunchedEffect
    }
    roomStatus = "checking"
    delay(500)
    try {
        val result = withContext(Dispatchers.IO) {
            VisioManager.client.validateRoom(roomUrl.trim(), username.trim().ifEmpty { null })
        }
        roomStatus = when (result) {
            is RoomValidationResult.Valid -> "valid"
            is RoomValidationResult.NotFound -> "not_found"
            is RoomValidationResult.InvalidFormat -> "idle"
            is RoomValidationResult.NetworkError -> "error"
        }
    } catch (_: Exception) {
        roomStatus = "error"
    }
}
```

**Step 2: Add status text below the URL TextField**

After the first `TextField` (roomUrl), before `Spacer(modifier = Modifier.height(16.dp))`, add:

```kotlin
when (roomStatus) {
    "checking" -> Text(
        Strings.t("home.room.checking", lang),
        style = MaterialTheme.typography.bodySmall,
        color = VisioColors.Greyscale400,
        modifier = Modifier.padding(top = 4.dp)
    )
    "valid" -> Text(
        Strings.t("home.room.valid", lang),
        style = MaterialTheme.typography.bodySmall,
        color = Color(0xFF18753C),
        modifier = Modifier.padding(top = 4.dp)
    )
    "not_found" -> Text(
        Strings.t("home.room.notFound", lang),
        style = MaterialTheme.typography.bodySmall,
        color = Color(0xFFE1000F),
        modifier = Modifier.padding(top = 4.dp)
    )
}
```

**Step 3: Update the Join button enabled condition**

Change:
```kotlin
enabled = roomUrl.isNotBlank(),
```
To:
```kotlin
enabled = roomStatus == "valid",
```

**Step 4: Verify it compiles**

Build the Android project to verify compilation.

**Step 5: Commit**

```bash
git add android/app/src/main/kotlin/io/visio/mobile/ui/HomeScreen.kt
git commit -m "feat(android): real-time room URL validation with debounce"
```

---

### Task 7: iOS UI — add real-time validation to HomeView

**Files:**
- Modify: `ios/VisioMobile/Views/HomeView.swift`

**Step 1: Add validation state and slug extraction**

At the top of `HomeView`, add state:

```swift
@State private var roomStatus: String = "idle" // idle, checking, valid, not_found, error

private static let slugRegex = /^[a-z]{3}-[a-z]{4}-[a-z]{3}$/

private func extractSlug(_ input: String) -> String? {
    let trimmed = input.trimmingCharacters(in: .whitespacesAndNewlines)
        .trimmingCharacters(in: CharacterSet(charactersIn: "/"))
    let candidate = trimmed.contains("/")
        ? String(trimmed.split(separator: "/").last ?? "")
        : trimmed
    return candidate.wholeMatch(of: Self.slugRegex) != nil ? candidate : nil
}
```

**Step 2: Add debounced validation with `.task(id:)`**

After the input VStack, add:

```swift
.task(id: roomURL) {
    guard let slug = extractSlug(roomURL) else {
        roomStatus = "idle"
        return
    }
    roomStatus = "checking"
    try? await Task.sleep(for: .milliseconds(500))
    guard !Task.isCancelled else { return }

    let result = manager.client.validateRoom(
        url: roomURL.trimmingCharacters(in: .whitespacesAndNewlines),
        username: displayName.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
            ? nil
            : displayName.trimmingCharacters(in: .whitespacesAndNewlines)
    )
    guard !Task.isCancelled else { return }

    switch result {
    case .valid: roomStatus = "valid"
    case .notFound: roomStatus = "not_found"
    case .invalidFormat: roomStatus = "idle"
    case .networkError: roomStatus = "error"
    }
}
```

**Step 3: Add status label below URL field**

After the `TextField` for roomURL, add:

```swift
if roomStatus == "checking" {
    Text(Strings.t("home.room.checking", lang: lang))
        .font(.caption)
        .foregroundStyle(.secondary)
} else if roomStatus == "valid" {
    Text(Strings.t("home.room.valid", lang: lang))
        .font(.caption)
        .foregroundStyle(.green)
} else if roomStatus == "not_found" {
    Text(Strings.t("home.room.notFound", lang: lang))
        .font(.caption)
        .foregroundStyle(.red)
}
```

**Step 4: Update the Join button disabled condition**

Change:
```swift
.disabled(roomURL.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
```
To:
```swift
.disabled(roomStatus != "valid")
```

**Step 5: Commit**

```bash
git add ios/VisioMobile/Views/HomeView.swift
git commit -m "feat(ios): real-time room URL validation with debounce"
```

---

### Task 8: Manual test on all 3 platforms

**Step 1: Desktop test**

Run desktop app. In the URL field, type progressively:
- `abc` → no indicator, button disabled
- `abc-defg-hij` → "Checking room..." then "Room not found", button disabled
- Paste a real URL like `https://meet.linagora.com/dpd-jffv-trg` → "Checking room..." then "Room found", button enabled
- Click Join → should connect

**Step 2: Android test**

Build APK, install on device/emulator. Same test flow as above.

**Step 3: iOS test**

Build and run in simulator. Same test flow as above.

**Step 4: Commit any fixes**

If any adjustments are needed from testing, fix and commit.
