# Room Display Name — Design Spec

**Date:** 2026-03-22
**Issue:** #113
**Replaces:** PR #66 (closed)
**Branch:** `feat/room-display-name`

## Problem

Rooms are identified only by their technical slug (e.g., `abc-defg-hij`). Users have no way to give a room a friendly name. When sharing links or reviewing recent rooms, the slug provides no meaningful context.

## Solution

Add a display name to rooms, sourced from a URL query parameter (`?room-display-name=`) or manual user input. The name is stored locally, shown across all screens, and preserved when sharing links.

## Design Decisions

| Decision | Choice | Rationale |
|---|---|---|
| Name priority | Most recent wins | Simple mental model, no conflict resolution needed |
| Name source | URL param + manual input | Covers both sharing and personal naming |
| Validation location | Rust core only | Single source of truth, testable, shared by all platforms |
| Storage | `RoomHistoryEntry` struct in settings | Natural extension of existing room history |
| Query param name | `?room-display-name=` | Explicit, avoids collision with `?name=` (too generic) |
| Edit from recent list | No | Name updates naturally on next join; keeps UI simple |
| Name clearing | Not supported | Once set, a name persists until replaced by a new one. Accepted limitation for v1. |
| URL encoding | `%20` for spaces | Cross-platform consistency (not `+` which varies by decoder) |

## Data Model

### RoomHistoryEntry (visio-core/src/settings.rs)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomHistoryEntry {
    pub url: String,
    pub display_name: Option<String>,
}
```

### Backward-compatible migration

A **custom `Deserialize` implementation** on `Vec<RoomHistoryEntry>` (not `#[serde(untagged)]` — which swallows error messages and makes debugging difficult):
1. First try to parse each element as `RoomHistoryEntry` struct
2. If that fails, try to parse as a plain `String` and wrap as `RoomHistoryEntry { url: s, display_name: None }`
3. If both fail, skip the entry and log a warning

Formats accepted:
- Old: `["https://meet.example.com/abc"]` → `RoomHistoryEntry { url, display_name: None }`
- New: `[{ "url": "...", "display_name": "Weekly" }]`
- Mixed: both formats in the same array

On next save, all entries are written in the new format. No explicit migration step needed.

## Validation Rules

Function: `validate_room_display_name(raw: &str) -> Option<String>`

| Rule | Detail |
|---|---|
| Length | 1–80 characters (after trim) |
| Forbidden chars | `` < > " ' ` ; & \ / { } `` , control characters (U+0000–U+001F, U+007F), and RTL/embedding override characters (U+202A–U+202E, U+2066–U+2069) |
| Sanitization | Trim leading/trailing whitespace, collapse multiple spaces to one |
| Empty after trim | Returns `None` |
| Invalid from URL | Silently ignored, existing name preserved |

## URL Handling

### Extraction

Function: `extract_room_display_name(url: &str) -> Option<String>`
- Parses query parameter `?room-display-name=`
- URL-decodes the value (percent-encoding)
- Passes through `validate_room_display_name`
- Returns `None` if absent or invalid

### Stripping

Function: `strip_room_display_name_param(url: &str) -> String`
- Removes only the `room-display-name` query parameter from the URL
- Preserves any other query parameters that may exist
- Used before calling LiveKit API and for room deduplication in history
- **Design note:** we strip only our param rather than all query params, to avoid breaking future features that may use other query parameters

## Room History Changes

### add_room_to_history(url, display_name)

1. Strip query params from URL to get canonical URL
2. If canonical URL already in history: update display_name (if new one is `Some`), move to top
3. If new: insert at top with display_name
4. Cap at 10 entries

### get_room_history() -> Vec<RoomHistoryEntry>

Returns all entries with their display names.

## FFI Layer (visio-ffi)

### UDL changes

```udl
dictionary RoomHistoryEntry {
    string url;
    string? display_name;
};
```

### Modified signatures

- `get_room_history() -> sequence<RoomHistoryEntry>`
- `add_room_to_history(string url, string? display_name)` — **breaking change** from previous `add_room_to_history(string url)`. All call sites on Android and iOS must be updated in the same PR.
- `extract_room_display_name(string url) -> string?` (new)
- `validate_room_display_name(string raw) -> string?` (new)

**Note:** The `Settings` dictionary in UDL does not include `room_history` and remains unchanged. Room history is accessed only via the standalone `get_room_history()` function.

### connect() flow

1. Extract display name from URL via `extract_room_display_name()`
2. Strip query params from URL
3. Connect to LiveKit with clean URL
4. On success: `add_room_to_history(clean_url, display_name)`

### validate_room() flow

Strip query params before server validation (unchanged behavior).

## Android

### HomeScreen

- **New field:** Optional `TextField` for room display name, below the Meeting URL field
  - Placeholder: `t("home.roomDisplayNamePlaceholder")`
  - Label: `t("home.roomDisplayName")`
  - Validation via `VisioManager.client.validateRoomDisplayName()`
  - Value passed to `connect()` alongside the URL
- **Recent rooms list:** If `display_name` present → bold primary text, slug+host as secondary. If absent → slug as primary (current behavior).

### PreJoinScreen (Lobby)

- Receives `roomDisplayName: String?` parameter
- Header: display name when present, slug as fallback
- Slug shown as subtitle for context

### CallScreen

- Header banner shows display name when present, slug otherwise

### Accessibility (Android)

- Room display name `TextField`: `contentDescription` set to label
- Recent rooms: two-line entries use `semantics { contentDescription = "$displayName, $slug on $host" }`

### InCallSettingsSheet

- Shared/copied URL includes `?room-display-name=` (URL-encoded) when name exists
- Deep link: `visio://host/slug?room-display-name=Encoded%20Name` (spaces as `%20`, not `+`, for cross-platform consistency)

### MainActivity (deep links)

- **Important:** current `parseDeepLink()` reconstructs the URL as `"https://$host/$slug"`, discarding all query parameters. Must be modified to extract `room-display-name` from the URI **before** reconstruction and propagate it separately through navigation.
- The clean URL (without display name param) is used for connection; the display name is passed as a separate navigation argument.

## iOS

### HomeView

- **New field:** Optional `TextField` for room display name (same as Android)
- **Recent rooms list:** Same display logic as Android

### PreJoinView (Lobby)

- `roomDisplayName: String?` parameter
- Header: display name or slug fallback, slug as subtitle

### CallView

- `.navigationTitle` set to display name when available, else `Strings.t("call.title")`

### InCallSettingsSheet

- URL and deep link include `?room-display-name=` when name exists

### Accessibility (iOS)

- Room display name `TextField`: `accessibilityLabel` set
- Recent rooms: two-line entries use `.accessibilityElement(children: .combine)`

### Deep links

- Extract query param from `visio://` URL scheme, propagate via SwiftUI navigation

## Desktop

### App.tsx — Home

- **New field:** Optional input for room display name below URL field
- **Recent rooms:** Same display logic as mobile

### App.tsx — Lobby (PreJoin)

- Display name shown in header when present, slug as fallback
- Slug as subtitle for context

### App.tsx — Call

- Header shows display name when present

### App.tsx — Share

- Copied URL includes `?room-display-name=` encoded

### lib.rs — Tauri commands

- `connect`: extracts display name, strips query params, stores in history (direct visio-core call)
- `get_room_history`: returns `Vec<RoomHistoryEntry>` serialized as JSON
- `validate_room_display_name(raw: String) -> Option<String>`: new command

### Deep links

- Extract query param, propagate to connection flow

## i18n

New keys in all 6 language files (en, fr, de, es, it, nl):

| Key | EN | FR |
|---|---|---|
| `home.roomDisplayName` | Room name (optional) | Nom de la room (optionnel) |
| `home.roomDisplayNamePlaceholder` | e.g. "Weekly standup" | ex. "Réunion hebdo" |

DE, ES, IT, NL translations TBD — to be provided during implementation.

**Note:** No new key needed for lobby/call fallback. When no display name is set, the existing slug extraction logic is used as-is.

## Testing

### Rust unit tests (visio-core)

- `RoomHistoryEntry` serde: new format round-trip
- Backward migration: old `Vec<String>` → `Vec<RoomHistoryEntry>`
- `validate_room_display_name`: valid names, empty, too long, forbidden chars, XSS attempts, control chars
- `extract_room_display_name`: with param, without, invalid, encoded special chars
- `strip_room_display_name_param`: with and without params
- `add_room_to_history`: new entry, update existing (same URL different name), deduplication, cap at 10

### Integration tests (per platform)

- Join via URL with `?room-display-name=` → name shown in lobby, call, recent rooms
- Join via manual name input → name stored and shown
- Join same room with new name → name updated
- Join same room without name → previous name preserved
- Share/copy URL → `?room-display-name=` present
- Deep link with name → name propagated through flow
- Invalid name in URL → silently ignored
