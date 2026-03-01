# Room URL Validation — Design

## Problem

Users can type any text into the room URL field and attempt to join. If the room doesn't exist, the connection fails with a confusing error. We want to validate the room URL in real time as the user types, and only enable the "Join" button when the room is confirmed to exist.

## Investigation Results

The Meet API at `/api/v1.0/rooms/{slug}/` returns:
- **200** + full JSON (including LiveKit token) for valid rooms
- **404** + `{"detail":"No Room matches the given query."}` for invalid rooms

Room slugs follow the pattern `xxx-xxxx-xxx` (3 lowercase letters, dash, 4 lowercase letters, dash, 3 lowercase letters).

Users may enter either a full URL (`https://meet.linagora.com/dpd-jffv-trg`) or just the slug (`dpd-jffv-trg`).

## Approach: Format Validation + API Check with Debounce

### Validation Pipeline

1. **Format validation** (instant, on every keystroke):
   - Extract slug from input (strip scheme + host if present, or use raw input)
   - Validate against regex: `^[a-z]{3}-[a-z]{4}-[a-z]{3}$`
   - If format invalid → button disabled, no API call

2. **API validation** (debounced 500ms after last keystroke):
   - Only triggered when format is valid
   - Calls `/api/v1.0/rooms/{slug}/` to verify room exists
   - Cancels any in-flight request when input changes

3. **Token caching**:
   - The API response includes the LiveKit token
   - Cache the `TokenInfo` from the validation response
   - Reuse it in `connect()` to avoid a redundant second API call
   - Invalidate cache if the URL changes

### UI States

| State | TextField indicator | Join button |
|---|---|---|
| Empty / format invalid | None | Disabled |
| Format valid, checking... | Spinner | Disabled |
| Room exists (API 200) | Green checkmark | Enabled |
| Room not found (API 404) | Error message below field | Disabled |
| Network error | Warning icon, retry hint | Disabled |

### Architecture

**Rust core** (`crates/visio-core/src/auth.rs`):
- Add `AuthService::validate_room(meet_url: &str, username: Option<&str>) -> Result<TokenInfo, VisioError>`
- Identical to `request_token()` but semantically named for validation
- In practice, reuse the same code path — validation IS requesting a token

**FFI** (`crates/visio-ffi/src/lib.rs`):
- Add `validate_room(url: String, username: Option<String>) -> RoomValidationResult`
- `RoomValidationResult` enum: `Valid { livekit_url: String, token: String }` | `NotFound` | `InvalidFormat` | `NetworkError { message: String }`
- Add to UDL

**Desktop** (`App.tsx`):
- `useEffect` with debounce on `meetUrl` changes
- `AbortController` to cancel in-flight fetches
- State machine: `idle | checking | valid | not_found | error`

**Android** (`HomeScreen.kt`):
- `LaunchedEffect(roomUrl)` with `delay(500)` for debounce
- Coroutine cancellation handles in-flight requests
- State: `ValidationState` sealed class

**iOS** (`HomeView.swift`):
- `.task(id: roomURL)` with `try await Task.sleep(for: .milliseconds(500))`
- Task cancellation handles debounce naturally
- State: `ValidationState` enum

### Format Extraction

```
Input: "https://meet.linagora.com/dpd-jffv-trg"
       → strip scheme → "meet.linagora.com/dpd-jffv-trg"
       → split on "/" → slug = "dpd-jffv-trg"
       → regex match → valid format

Input: "dpd-jffv-trg"
       → no "/" → treat as slug directly
       → regex match → valid format

Input: "hello"
       → regex fail → invalid format
```

For API calls when only a slug is provided, we need to know the Meet instance. This is currently hardcoded as `meet.example.com` placeholder. The same `parse_meet_url` logic in `AuthService` handles both cases.
