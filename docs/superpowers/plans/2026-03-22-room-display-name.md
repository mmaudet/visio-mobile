# Room Display Name — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a friendly display name to rooms, shown in lobby, call, recent rooms, and shared links across all 3 platforms.

**Architecture:** All logic (validation, extraction, storage, migration) lives in `visio-core`. Platforms receive structured `RoomHistoryEntry { url, display_name }` via FFI (mobile) or direct calls (desktop) and only handle display. TDD approach: tests first in Rust, then platform integration.

**Tech Stack:** Rust (visio-core, visio-ffi), UniFFI, Kotlin/Compose (Android), Swift/SwiftUI (iOS), React/TypeScript + Tauri (Desktop)

**Spec:** `docs/superpowers/specs/2026-03-22-room-display-name-design.md`
**Issue:** #113
**Branch:** `feat/room-display-name` (worktree: `.worktrees/room-display-name/`)

---

## File Map

### Create
- `crates/visio-core/src/room_display_name.rs` — validation + URL extraction + stripping logic

### Modify
- `crates/visio-core/src/lib.rs` — add `mod room_display_name` and re-export
- `crates/visio-core/src/settings.rs:43,108,269-284` — `RoomHistoryEntry` struct, migration deserializer, updated `add_room_to_history`/`get_room_history`
- `crates/visio-ffi/src/visio.udl:358-362` — new dictionary + updated signatures
- `crates/visio-ffi/src/lib.rs:923-980,1239-1249` — connect flow + history wrappers
- `android/app/src/main/kotlin/io/visio/mobile/MainActivity.kt:32-52` — deep link display name extraction
- `android/app/src/main/kotlin/io/visio/mobile/ui/HomeScreen.kt:93-104,851-877` — new field + recent rooms display
- `android/app/src/main/kotlin/io/visio/mobile/ui/PreJoinScreen.kt:290-296,600` — display name in header
- `android/app/src/main/kotlin/io/visio/mobile/ui/CallScreen.kt` — header banner
- `android/app/src/main/kotlin/io/visio/mobile/ui/InCallSettingsSheet.kt:87-96,680-802` — share URL with display name
- `ios/VisioMobile/Views/HomeView.swift:7,133-137,235-284` — new field + recent rooms display
- `ios/VisioMobile/Views/PreJoinView.swift:168-171,193-201` — display name in header
- `ios/VisioMobile/Views/CallView.swift:259` — navigation title
- `ios/VisioMobile/Views/InCallSettingsSheet.swift:5-10,214-279` — share URL with display name
- `crates/visio-desktop/src/lib.rs:130-131,441-466` — connect + history commands
- `crates/visio-desktop/frontend/src/App.tsx:817-823,1132-1139,1255-1291,3706-3725,3921-3949` — home, lobby, call, share
- `i18n/en.json`, `i18n/fr.json`, `i18n/de.json`, `i18n/es.json`, `i18n/it.json`, `i18n/nl.json` — new keys

### Test
- `crates/visio-core/src/room_display_name.rs` — inline `#[cfg(test)]` module
- `crates/visio-core/src/settings.rs` — existing test module extended

---

## Task 1: Validation and URL helpers (Rust core)

**Files:**
- Create: `crates/visio-core/src/room_display_name.rs`
- Modify: `crates/visio-core/src/lib.rs`

- [ ] **Step 1: Create module file with test skeleton**

Create `crates/visio-core/src/room_display_name.rs`:

```rust
//! Room display name: validation, URL extraction, and parameter stripping.

/// Validate and sanitize a room display name.
/// Returns `Some(sanitized)` if valid, `None` if empty or invalid.
pub fn validate_room_display_name(_raw: &str) -> Option<String> {
    todo!()
}

/// Extract the `room-display-name` query parameter from a URL.
/// Returns the validated display name, or `None` if absent/invalid.
pub fn extract_room_display_name(_url: &str) -> Option<String> {
    todo!()
}

/// Remove only the `room-display-name` query parameter from a URL.
/// Preserves all other query parameters.
pub fn strip_room_display_name_param(_url: &str) -> String {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── validate_room_display_name ──

    #[test]
    fn validate_simple_name() {
        assert_eq!(
            validate_room_display_name("Weekly Standup"),
            Some("Weekly Standup".to_string())
        );
    }

    #[test]
    fn validate_trims_whitespace() {
        assert_eq!(
            validate_room_display_name("  Comex  "),
            Some("Comex".to_string())
        );
    }

    #[test]
    fn validate_collapses_spaces() {
        assert_eq!(
            validate_room_display_name("Team   Meeting"),
            Some("Team Meeting".to_string())
        );
    }

    #[test]
    fn validate_empty_returns_none() {
        assert_eq!(validate_room_display_name(""), None);
        assert_eq!(validate_room_display_name("   "), None);
    }

    #[test]
    fn validate_too_long_returns_none() {
        let long = "a".repeat(81);
        assert_eq!(validate_room_display_name(&long), None);
    }

    #[test]
    fn validate_max_length_ok() {
        let max = "a".repeat(80);
        assert_eq!(validate_room_display_name(&max), Some(max));
    }

    #[test]
    fn validate_multibyte_chars_count_correctly() {
        // 80 two-byte chars = 160 bytes but 80 chars → should pass
        let name = "é".repeat(80);
        assert_eq!(validate_room_display_name(&name), Some(name));
        // 81 two-byte chars → should fail
        let too_long = "é".repeat(81);
        assert_eq!(validate_room_display_name(&too_long), None);
    }

    #[test]
    fn validate_rejects_angle_brackets() {
        assert_eq!(validate_room_display_name("<script>alert(1)</script>"), None);
    }

    #[test]
    fn validate_rejects_forbidden_chars() {
        for c in ['<', '>', '"', '\'', '`', ';', '&', '\\', '/', '{', '}'] {
            let name = format!("Room{c}Name");
            assert_eq!(validate_room_display_name(&name), None, "should reject '{c}'");
        }
    }

    #[test]
    fn validate_rejects_control_chars() {
        assert_eq!(validate_room_display_name("Room\x00Name"), None);
        assert_eq!(validate_room_display_name("Room\x1FName"), None);
        assert_eq!(validate_room_display_name("Room\x7FName"), None);
    }

    #[test]
    fn validate_rejects_rtl_override() {
        // U+202E = Right-to-Left Override
        assert_eq!(validate_room_display_name("Room\u{202E}Name"), None);
        // U+2066 = Left-to-Right Isolate
        assert_eq!(validate_room_display_name("Room\u{2066}Name"), None);
    }

    #[test]
    fn validate_allows_unicode_letters() {
        assert_eq!(
            validate_room_display_name("Réunion d'équipe"),
            None, // apostrophe is forbidden
        );
        assert_eq!(
            validate_room_display_name("Réunion équipe"),
            Some("Réunion équipe".to_string()),
        );
    }

    #[test]
    fn validate_allows_parens_and_hyphens() {
        assert_eq!(
            validate_room_display_name("Sprint Planning (Week 12)"),
            Some("Sprint Planning (Week 12)".to_string()),
        );
        assert_eq!(
            validate_room_display_name("Team-Meeting"),
            Some("Team-Meeting".to_string()),
        );
    }

    // ── extract_room_display_name ──

    #[test]
    fn extract_present() {
        assert_eq!(
            extract_room_display_name("https://meet.example.com/abc-defg-hij?room-display-name=Weekly%20Standup"),
            Some("Weekly Standup".to_string()),
        );
    }

    #[test]
    fn extract_absent() {
        assert_eq!(
            extract_room_display_name("https://meet.example.com/abc-defg-hij"),
            None,
        );
    }

    #[test]
    fn extract_empty_value() {
        assert_eq!(
            extract_room_display_name("https://meet.example.com/abc-defg-hij?room-display-name="),
            None,
        );
    }

    #[test]
    fn extract_invalid_value() {
        assert_eq!(
            extract_room_display_name("https://meet.example.com/abc?room-display-name=%3Cscript%3E"),
            None,
        );
    }

    #[test]
    fn extract_with_other_params() {
        assert_eq!(
            extract_room_display_name("https://meet.example.com/abc?token=xyz&room-display-name=Comex&lang=fr"),
            Some("Comex".to_string()),
        );
    }

    // ── strip_room_display_name_param ──

    #[test]
    fn strip_removes_param() {
        assert_eq!(
            strip_room_display_name_param("https://meet.example.com/abc?room-display-name=Comex"),
            "https://meet.example.com/abc",
        );
    }

    #[test]
    fn strip_preserves_other_params() {
        assert_eq!(
            strip_room_display_name_param("https://meet.example.com/abc?token=xyz&room-display-name=Comex&lang=fr"),
            "https://meet.example.com/abc?token=xyz&lang=fr",
        );
    }

    #[test]
    fn strip_no_param_unchanged() {
        assert_eq!(
            strip_room_display_name_param("https://meet.example.com/abc-defg-hij"),
            "https://meet.example.com/abc-defg-hij",
        );
    }

    #[test]
    fn strip_only_param_removes_question_mark() {
        assert_eq!(
            strip_room_display_name_param("https://meet.example.com/abc?room-display-name=Test"),
            "https://meet.example.com/abc",
        );
    }
}
```

- [ ] **Step 2: Register module in lib.rs**

In `crates/visio-core/src/lib.rs`, add:
```rust
pub mod room_display_name;
pub use room_display_name::{validate_room_display_name, extract_room_display_name, strip_room_display_name_param};
```

- [ ] **Step 3: Run tests — verify they fail**

Run: `cargo test -p visio-core room_display_name`
Expected: all tests FAIL with `not yet implemented`

- [ ] **Step 4: Implement `validate_room_display_name`**

```rust
pub fn validate_room_display_name(raw: &str) -> Option<String> {
    // Trim and collapse spaces
    let trimmed: String = raw
        .split_whitespace()
        .collect::<Vec<&str>>()
        .join(" ");

    if trimmed.is_empty() || trimmed.chars().count() > 80 {
        return None;
    }

    // Forbidden characters
    const FORBIDDEN: &[char] = &['<', '>', '"', '\'', '`', ';', '&', '\\', '/', '{', '}'];
    if trimmed.chars().any(|c| FORBIDDEN.contains(&c)) {
        return None;
    }

    // Control characters (U+0000–U+001F, U+007F)
    if trimmed.chars().any(|c| c.is_control()) {
        return None;
    }

    // RTL/embedding override characters (U+202A–U+202E, U+2066–U+2069)
    if trimmed.chars().any(|c| matches!(c, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}')) {
        return None;
    }

    Some(trimmed)
}
```

- [ ] **Step 5: Implement `extract_room_display_name`**

```rust
pub fn extract_room_display_name(url: &str) -> Option<String> {
    let query_start = url.find('?')?;
    let query = &url[query_start + 1..];

    for pair in query.split('&') {
        let mut kv = pair.splitn(2, '=');
        let key = kv.next().unwrap_or("");
        let value = kv.next().unwrap_or("");
        if key == "room-display-name" {
            let decoded = urlencoding::decode(value).ok()?;
            return validate_room_display_name(&decoded);
        }
    }
    None
}
```

- [ ] **Step 6: Implement `strip_room_display_name_param`**

```rust
pub fn strip_room_display_name_param(url: &str) -> String {
    let Some(query_start) = url.find('?') else {
        return url.to_string();
    };

    let base = &url[..query_start];
    let query = &url[query_start + 1..];

    let remaining: Vec<&str> = query
        .split('&')
        .filter(|pair| {
            let key = pair.splitn(2, '=').next().unwrap_or("");
            key != "room-display-name"
        })
        .collect();

    if remaining.is_empty() {
        base.to_string()
    } else {
        format!("{}?{}", base, remaining.join("&"))
    }
}
```

- [ ] **Step 7: Run tests — verify they all pass**

Run: `cargo test -p visio-core room_display_name`
Expected: all tests PASS

- [ ] **Step 8: Commit**

```bash
git add crates/visio-core/src/room_display_name.rs crates/visio-core/src/lib.rs
git commit -m "feat(core): add room display name validation and URL helpers (#113)"
```

---

## Task 2: RoomHistoryEntry struct and migration (Rust core)

**Files:**
- Modify: `crates/visio-core/src/settings.rs:43,108,269-284,398-405`

- [ ] **Step 1: Write failing tests for RoomHistoryEntry and migration**

Add to the existing `#[cfg(test)]` module in `settings.rs`:

```rust
#[test]
fn room_history_entry_round_trip() {
    let entry = RoomHistoryEntry {
        url: "https://meet.example.com/abc-defg-hij".to_string(),
        display_name: Some("Comex".to_string()),
    };
    let json = serde_json::to_string(&entry).unwrap();
    let back: RoomHistoryEntry = serde_json::from_str(&json).unwrap();
    assert_eq!(back.url, entry.url);
    assert_eq!(back.display_name, entry.display_name);
}

#[test]
fn room_history_entry_without_name() {
    let entry = RoomHistoryEntry {
        url: "https://meet.example.com/abc-defg-hij".to_string(),
        display_name: None,
    };
    let json = serde_json::to_string(&entry).unwrap();
    let back: RoomHistoryEntry = serde_json::from_str(&json).unwrap();
    assert_eq!(back.display_name, None);
}

#[test]
fn room_history_migrates_old_string_format() {
    let old_json = r#"{"room_history": ["https://meet.example.com/abc", "https://meet.example.com/def"]}"#;
    let settings: Settings = serde_json::from_str(old_json).unwrap();
    assert_eq!(settings.room_history.len(), 2);
    assert_eq!(settings.room_history[0].url, "https://meet.example.com/abc");
    assert_eq!(settings.room_history[0].display_name, None);
}

#[test]
fn room_history_new_format() {
    let new_json = r#"{"room_history": [{"url": "https://meet.example.com/abc", "display_name": "Comex"}]}"#;
    let settings: Settings = serde_json::from_str(new_json).unwrap();
    assert_eq!(settings.room_history[0].display_name, Some("Comex".to_string()));
}

#[test]
fn room_history_mixed_format() {
    let json = r#"{"room_history": ["https://meet.example.com/old", {"url": "https://meet.example.com/new", "display_name": "New Room"}]}"#;
    let settings: Settings = serde_json::from_str(json).unwrap();
    assert_eq!(settings.room_history.len(), 2);
    assert_eq!(settings.room_history[0].display_name, None);
    assert_eq!(settings.room_history[1].display_name, Some("New Room".to_string()));
}

#[test]
fn add_room_updates_display_name() {
    let dir = tempfile::tempdir().unwrap();
    let store = SettingsStore::new(dir.path().to_str().unwrap());
    store.add_room_to_history("https://meet.example.com/abc".to_string(), Some("Old Name".to_string()));
    store.add_room_to_history("https://meet.example.com/abc".to_string(), Some("New Name".to_string()));
    let history = store.get_room_history();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].display_name, Some("New Name".to_string()));
}

#[test]
fn add_room_without_name_preserves_existing() {
    let dir = tempfile::tempdir().unwrap();
    let store = SettingsStore::new(dir.path().to_str().unwrap());
    store.add_room_to_history("https://meet.example.com/abc".to_string(), Some("Comex".to_string()));
    store.add_room_to_history("https://meet.example.com/abc".to_string(), None);
    let history = store.get_room_history();
    assert_eq!(history[0].display_name, Some("Comex".to_string()));
}

#[test]
fn add_room_caps_at_10() {
    let dir = tempfile::tempdir().unwrap();
    let store = SettingsStore::new(dir.path().to_str().unwrap());
    for i in 0..12 {
        store.add_room_to_history(format!("https://meet.example.com/room-{i:03}-aaa"), None);
    }
    assert_eq!(store.get_room_history().len(), 10);
}
```

- [ ] **Step 2: Run tests — verify they fail**

Run: `cargo test -p visio-core room_history`
Expected: FAIL — `RoomHistoryEntry` not defined, `add_room_to_history` wrong signature

- [ ] **Step 3: Implement `RoomHistoryEntry` struct with custom deserializer**

In `settings.rs`, add above the `Settings` struct:

```rust
use crate::room_display_name::strip_room_display_name_param;

#[derive(Debug, Clone, Serialize)]
pub struct RoomHistoryEntry {
    pub url: String,
    pub display_name: Option<String>,
}

impl<'de> serde::Deserialize<'de> for RoomHistoryEntry {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de;
        use serde_json::Value;

        let value = Value::deserialize(deserializer)?;
        match &value {
            Value::Object(map) => {
                let url = map.get("url")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| de::Error::custom("missing 'url' field"))?
                    .to_string();
                let display_name = map.get("display_name")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                Ok(RoomHistoryEntry { url, display_name })
            }
            Value::String(s) => Ok(RoomHistoryEntry {
                url: s.clone(),
                display_name: None,
            }),
            _ => Err(de::Error::custom(format!(
                "expected string or object for RoomHistoryEntry, got {value}"
            ))),
        }
    }
}
```

- [ ] **Step 4: Update `Settings.room_history` field**

Change line 43 from:
```rust
pub room_history: Vec<String>,
```
to:
```rust
pub room_history: Vec<RoomHistoryEntry>,
```

Update the `Default` impl accordingly (line 108).

- [ ] **Step 5: Update `add_room_to_history`**

Replace the existing function (lines 269-276):

```rust
pub fn add_room_to_history(&self, url: String, display_name: Option<String>) {
    let canonical = strip_room_display_name_param(&url);
    let mut s = self.settings.lock().unwrap_or_else(|e| e.into_inner());

    // Find existing entry with same canonical URL
    if let Some(pos) = s.room_history.iter().position(|e| strip_room_display_name_param(&e.url) == canonical) {
        let existing_name = s.room_history[pos].display_name.clone();
        s.room_history.remove(pos);
        s.room_history.insert(0, RoomHistoryEntry {
            url: canonical,
            display_name: display_name.or(existing_name),
        });
    } else {
        s.room_history.insert(0, RoomHistoryEntry {
            url: canonical,
            display_name,
        });
    }

    s.room_history.truncate(10);
    drop(s);
    self.save();
}
```

- [ ] **Step 6: Update `get_room_history`**

Change return type (lines 278-284):
```rust
pub fn get_room_history(&self) -> Vec<RoomHistoryEntry> {
    self.settings
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .room_history
        .clone()
}
```

- [ ] **Step 7: Update `clear_room_history`** (lines 398-405)

No signature change needed — just ensure it still compiles with the new type.

- [ ] **Step 8: Update ALL existing tests and references to old `Vec<String>` room_history**

Search for `room_history` and `add_room_to_history` in the file. The existing tests (around lines 650-693) use the old single-argument `add_room_to_history(url)` and compare history entries to `String`. Update them all:

- Change all `store.add_room_to_history("url".to_string())` to `store.add_room_to_history("url".to_string(), None)`
- Change all `assert_eq!(history[0], "url")` to `assert_eq!(history[0].url, "url")`
- Update any `Vec<String>` type annotations to `Vec<RoomHistoryEntry>`

- [ ] **Step 9: Run tests — verify they pass**

Run: `cargo test -p visio-core`
Expected: all tests PASS (both new and existing)

- [ ] **Step 10: Commit**

```bash
git add crates/visio-core/src/settings.rs
git commit -m "feat(core): add RoomHistoryEntry with backward-compatible migration (#113)"
```

---

## Task 3: Update parse_meet_url and extract_slug for query params

**Files:**
- Modify: `crates/visio-core/src/auth.rs:106-125,144-159`

- [ ] **Step 1: Write test for URL with query params**

Add to existing tests in `auth.rs`:

```rust
#[test]
fn parse_meet_url_strips_display_name_param() {
    let (instance, slug) = AuthService::parse_meet_url(
        "https://meet.example.com/abc-defg-hij?room-display-name=Comex"
    ).unwrap();
    assert_eq!(instance, "meet.example.com");
    assert_eq!(slug, "abc-defg-hij");
}

#[test]
fn extract_slug_with_query_param() {
    let slug = AuthService::extract_slug(
        "https://meet.example.com/abc-defg-hij?room-display-name=Test"
    ).unwrap();
    assert_eq!(slug, "abc-defg-hij");
}
```

- [ ] **Step 2: Run tests — verify they fail**

Run: `cargo test -p visio-core parse_meet_url_strips extract_slug_with`
Expected: FAIL (query param included in slug)

- [ ] **Step 3: Update `parse_meet_url`**

Strip the display name param before parsing:

```rust
pub fn parse_meet_url(url: &str) -> Result<(String, String), VisioError> {
    let url = crate::strip_room_display_name_param(
        url.trim().trim_end_matches('/')
    );
    let url = url
        .replace("https://", "")
        .replace("http://", "");
    // ... rest unchanged
}
```

- [ ] **Step 4: Update `extract_slug`**

Strip query params from candidate before regex match:

```rust
let candidate = if candidate.contains('?') {
    candidate.split('?').next().unwrap_or("")
} else {
    candidate
};
```

- [ ] **Step 5: Run all tests**

Run: `cargo test -p visio-core`
Expected: all PASS

- [ ] **Step 6: Commit**

```bash
git add crates/visio-core/src/auth.rs
git commit -m "fix(core): strip room-display-name param from URL before parsing (#113)"
```

---

## Task 4: FFI layer (visio-ffi)

**Files:**
- Modify: `crates/visio-ffi/src/visio.udl:358-362`
- Modify: `crates/visio-ffi/src/lib.rs:923-980,1239-1249`

- [ ] **Step 1: Add `RoomHistoryEntry` dictionary to UDL**

In `visio.udl`, add before the interface block (near line 88):

```udl
dictionary RoomHistoryEntry {
    string url;
    string? display_name;
};
```

- [ ] **Step 2: Update function signatures in UDL**

Replace lines 358-362:

```udl
void add_room_to_history(string url, string? display_name);
sequence<RoomHistoryEntry> get_room_history();
void clear_room_history();
string? extract_room_display_name(string url);
string? validate_room_display_name(string raw);
```

- [ ] **Step 3: Add `RoomHistoryEntry` struct to lib.rs for UniFFI**

In `lib.rs`, add the UniFFI-compatible struct:

```rust
#[derive(uniffi::Record)]
pub struct RoomHistoryEntry {
    pub url: String,
    pub display_name: Option<String>,
}

impl From<visio_core::settings::RoomHistoryEntry> for RoomHistoryEntry {
    fn from(e: visio_core::settings::RoomHistoryEntry) -> Self {
        Self { url: e.url, display_name: e.display_name }
    }
}
```

- [ ] **Step 4: Update `add_room_to_history` in lib.rs**

Replace lines 1239-1241:

```rust
pub fn add_room_to_history(&self, url: String, display_name: Option<String>) {
    self.settings.add_room_to_history(url, display_name);
}
```

- [ ] **Step 5: Update `get_room_history` in lib.rs**

Replace lines 1243-1245:

```rust
pub fn get_room_history(&self) -> Vec<RoomHistoryEntry> {
    self.settings.get_room_history().into_iter().map(Into::into).collect()
}
```

- [ ] **Step 6: Add new FFI functions**

```rust
pub fn extract_room_display_name(&self, url: String) -> Option<String> {
    visio_core::extract_room_display_name(&url)
}

pub fn validate_room_display_name(&self, raw: String) -> Option<String> {
    visio_core::validate_room_display_name(&raw)
}
```

- [ ] **Step 7: Update `connect()` flow — strip URL BEFORE connecting**

In the connect function (lines 923-980), extract display name and strip the param **before** passing the URL to LiveKit. Change the function to:

```rust
pub fn connect(&self, meet_url: String, username: Option<String>) -> Result<(), VisioError> {
    // Extract display name before stripping the param
    let display_name = visio_core::extract_room_display_name(&meet_url);
    let clean_url = visio_core::strip_room_display_name_param(&meet_url);

    // ... (existing cookie retrieval code unchanged) ...

    // Use clean_url (not meet_url) for the actual connection:
    self.room_manager.connect(&clean_url, username.as_deref(), cookie.as_deref())

    // On success (line 961), store with display name:
    self.settings.add_room_to_history(clean_url.clone(), display_name);
```

**Important:** The URL passed to `room_manager.connect()` must be `clean_url` (without `?room-display-name=`), not the original `meet_url`. This matches the spec: "Connect to LiveKit with clean URL."

- [ ] **Step 7b: Fix `BridgeListener::record_room_in_history` (lines 784-792)**

This second call site fires on `ConnectionState::Connected` (including reconnections and lobby acceptance). Since it uses `last_connection_info()` which returns the clean URL (post-connection, no query params), it must pass `None` for display_name. The `or(existing_name)` logic in `add_room_to_history` will preserve any previously stored name:

```rust
impl BridgeListener {
    fn record_room_in_history(&self) {
        let rm = self.room_manager.clone();
        let settings = self.settings.clone();
        tokio::spawn(async move {
            if let Some((url, _)) = rm.last_connection_info().await {
                // Pass None — the display name was already stored by connect().
                // The or(existing_name) logic preserves it.
                settings.add_room_to_history(url, None);
            }
        });
    }
}
```

- [ ] **Step 7c: Update `validate_room()` to strip param before validation**

In `validate_room()` (around line 1293), strip the display name param before passing to `AuthService::validate_room`:

```rust
pub fn validate_room(&self, url: String, username: Option<String>) -> Result<(), VisioError> {
    let clean_url = visio_core::strip_room_display_name_param(&url);
    // Use clean_url for all downstream calls
    // ...
}
```

- [ ] **Step 8: Build to verify compilation**

Run: `cargo build -p visio-ffi`
Expected: compiles without errors

- [ ] **Step 9: Regenerate UniFFI bindings**

Run: `scripts/generate-bindings.sh all`

- [ ] **Step 10: Commit**

```bash
git add crates/visio-ffi/
git commit -m "feat(ffi): expose RoomHistoryEntry and display name helpers via UniFFI (#113)"
```

---

## Task 5: i18n — add translation keys

**Files:**
- Modify: `i18n/en.json`, `i18n/fr.json`, `i18n/de.json`, `i18n/es.json`, `i18n/it.json`, `i18n/nl.json`

- [ ] **Step 1: Add keys to all 6 language files**

Add near existing `home.*` keys:

| Key | EN | FR | DE | ES | IT | NL |
|---|---|---|---|---|---|---|
| `home.roomDisplayName` | Room name (optional) | Nom de la room (optionnel) | Raumname (optional) | Nombre de sala (opcional) | Nome stanza (opzionale) | Kamernaam (optioneel) |
| `home.roomDisplayNamePlaceholder` | e.g. "Weekly standup" | ex. "Réunion hebdo" | z.B. "Wöchentliches Meeting" | ej. "Reunión semanal" | es. "Riunione settimanale" | bijv. "Wekelijkse vergadering" |

- [ ] **Step 2: Verify JSON is valid**

Run: `python3 -c "import json; [json.load(open(f'i18n/{l}.json')) for l in ['en','fr','de','es','it','nl']]"`
Expected: no errors

- [ ] **Step 3: Commit**

```bash
git add i18n/
git commit -m "feat(i18n): add room display name translation keys (#113)"
```

---

## Task 6: Desktop — Tauri commands and frontend

**Files:**
- Modify: `crates/visio-desktop/src/lib.rs:130-131,441-466`
- Modify: `crates/visio-desktop/frontend/src/App.tsx:817-823,1132-1139,1255-1291,3706-3725,3921-3949`

- [ ] **Step 1: Update `handle_connection_state_changed` in lib.rs**

Around line 130, change:
```rust
settings.add_room_to_history(url);
```
to:
```rust
let display_name = visio_core::extract_room_display_name(&url);
let clean_url = visio_core::strip_room_display_name_param(&url);
settings.add_room_to_history(clean_url, display_name);
```

- [ ] **Step 2: Update `get_room_history` Tauri command**

Update the return type to serialize `RoomHistoryEntry` structs instead of strings. Create a Tauri-serializable struct if needed:

```rust
#[derive(serde::Serialize)]
struct RoomHistoryEntryJs {
    url: String,
    display_name: Option<String>,
}

#[tauri::command]
async fn get_room_history(state: tauri::State<'_, AppState>) -> Result<Vec<RoomHistoryEntryJs>, String> {
    Ok(state.settings.get_room_history().into_iter().map(|e| RoomHistoryEntryJs {
        url: e.url,
        display_name: e.display_name,
    }).collect())
}
```

- [ ] **Step 3: Add `validate_room_display_name` Tauri command**

```rust
#[tauri::command]
fn validate_room_display_name(raw: String) -> Option<String> {
    visio_core::validate_room_display_name(&raw)
}
```

Register in Tauri builder's `invoke_handler`.

- [ ] **Step 4: Build backend**

Run: `cargo build -p visio-desktop`
Expected: compiles

- [ ] **Step 5: Update TypeScript types in App.tsx**

Add interface and update state:

```typescript
interface RoomHistoryEntry {
  url: string
  display_name: string | null
}

// Change line 819:
const [roomHistory, setRoomHistory] = useState<RoomHistoryEntry[]>([])
const [roomDisplayName, setRoomDisplayName] = useState('')
```

- [ ] **Step 6: Update room history loading**

Around line 823, change `invoke<string[]>` to `invoke<RoomHistoryEntry[]>`.

- [ ] **Step 7: Add display name input field on Home screen**

After the meetUrl input (around line 1139), add:

```tsx
<label htmlFor="roomDisplayName">{t('home.roomDisplayName')}</label>
<input
  id="roomDisplayName"
  type="text"
  placeholder={t('home.roomDisplayNamePlaceholder')}
  value={roomDisplayName}
  onChange={(e) => setRoomDisplayName(e.target.value)}
/>
```

- [ ] **Step 8: Update `handleJoin` to pass display name**

In the join flow, if user entered a display name, append it to the URL before connecting:

```typescript
const urlWithName = roomDisplayName.trim()
  ? `${trimmed}?room-display-name=${encodeURIComponent(roomDisplayName.trim())}`
  : trimmed
```

- [ ] **Step 9: Update recent rooms rendering**

Around line 1258, change from rendering just URL to rendering display name + slug:

```tsx
{roomHistory.map((entry, i) => {
  const slug = entry.url.includes('/') ? entry.url.split('/').pop() : entry.url
  const host = entry.url.replace(/^https?:\/\//, '').split('/')[0]
  return (
    <div key={i} className="room-history-item">
      <span className="room-name">{entry.display_name || slug}</span>
      {entry.display_name && <span className="room-slug">{slug} · {host}</span>}
      {!entry.display_name && <span className="room-host">{host}</span>}
    </div>
  )
})}
```

- [ ] **Step 10: Update PreJoin (lobby) header**

Around line 3725, update:

```typescript
const slug = roomUrl.includes('/') ? roomUrl.split('/').pop() : roomUrl
// Add: receive roomDisplayName prop and show it
```

Display the display name as the main title, slug as subtitle.

- [ ] **Step 11: Update call header**

Show display name when available in the call header area.

- [ ] **Step 12: Update share/copy URL**

When building the URL for sharing, append `?room-display-name=` if a display name exists (percent-encoded with `%20` for spaces).

- [ ] **Step 13: Handle deep links**

In the Desktop deep link handler (look for `visio://` protocol handling in `lib.rs` or `App.tsx`), extract the `room-display-name` query parameter using `URL` or `URLSearchParams`:

```typescript
const url = new URL(deepLinkUrl.replace('visio://', 'https://'))
const displayName = url.searchParams.get('room-display-name')
url.searchParams.delete('room-display-name')
const cleanUrl = url.toString()
// Pass cleanUrl + displayName to the join flow
```

- [ ] **Step 14: Build and test frontend**

Run: `cd crates/visio-desktop && cargo tauri dev`
Expected: app launches, new field visible, recent rooms show names

- [ ] **Step 15: Commit**

```bash
git add crates/visio-desktop/
git commit -m "feat(desktop): display room name in home, lobby, call and share (#113)"
```

---

## Task 7: Android — all screens

**Files:**
- Modify: `android/app/src/main/kotlin/io/visio/mobile/MainActivity.kt:32-52`
- Modify: `android/app/src/main/kotlin/io/visio/mobile/ui/HomeScreen.kt`
- Modify: `android/app/src/main/kotlin/io/visio/mobile/ui/PreJoinScreen.kt`
- Modify: `android/app/src/main/kotlin/io/visio/mobile/ui/CallScreen.kt`
- Modify: `android/app/src/main/kotlin/io/visio/mobile/ui/InCallSettingsSheet.kt`

- [ ] **Step 1: Update `parseDeepLink()` in MainActivity**

Extract display name before reconstructing URL (lines 32-52):

```kotlin
private fun parseDeepLink(intent: Intent?): Pair<String, String?>? {
    val uri = intent?.data ?: return null
    if (uri.scheme != "visio") return null
    val host = uri.host ?: return null

    if (host == OidcAuthManager.AUTH_CALLBACK_HOST) {
        handleAuthCallback(uri)
        return null
    }

    val slug = uri.path?.trimStart('/') ?: return null
    if (host.isBlank() || slug.isBlank()) return null

    val displayName = uri.getQueryParameter("room-display-name")

    val instances = VisioManager.client.getMeetInstances()
    return if (instances.contains(host)) {
        Pair("https://$host/$slug", displayName)
    } else {
        null
    }
}
```

Update all callers to handle the `Pair<String, String?>` return type.

- [ ] **Step 2: Update HomeScreen — add display name field**

Add state and TextField below the URL input:

```kotlin
var roomDisplayName by remember { mutableStateOf("") }

// After the URL TextField:
OutlinedTextField(
    value = roomDisplayName,
    onValueChange = { roomDisplayName = it },
    label = { Text(Strings.t("home.roomDisplayName", lang)) },
    placeholder = { Text(Strings.t("home.roomDisplayNamePlaceholder", lang)) },
    singleLine = true,
    modifier = Modifier.fillMaxWidth(),
)
```

- [ ] **Step 3: Update HomeScreen — recent rooms list**

Change room history from `List<String>` to use `RoomHistoryEntry` from FFI:

```kotlin
val history = VisioManager.client.getRoomHistory() // now returns List<RoomHistoryEntry>

// Rendering:
history.forEach { entry ->
    val slug = if ('/' in entry.url) entry.url.substringAfterLast('/') else entry.url
    val host = try { java.net.URI(entry.url).host ?: "" } catch (_: Exception) { "" }

    // Primary: display_name or slug
    Text(
        text = entry.displayName ?: slug,
        fontWeight = FontWeight.Bold,
    )
    // Secondary: slug + host (only if display name exists)
    if (entry.displayName != null) {
        Text(
            text = "$slug · $host",
            style = MaterialTheme.typography.bodySmall,
        )
    }
}
```

- [ ] **Step 4: Update HomeScreen — join flow**

Pass display name when navigating to lobby/call. If user typed a name, include it.

- [ ] **Step 5: Update PreJoinScreen — accept and show display name**

Add parameter to function signature:

```kotlin
fun PreJoinScreen(
    roomUrl: String,
    roomDisplayName: String? = null,  // NEW
    initialUsername: String,
    onJoin: (finalUsername: String) -> Unit,
    onCancel: () -> Unit,
)
```

In header, show display name or slug:

```kotlin
Text(
    text = roomDisplayName ?: roomSlug,
    style = MaterialTheme.typography.headlineSmall,
    fontWeight = FontWeight.SemiBold,
)
if (roomDisplayName != null) {
    Text(
        text = roomSlug,
        style = MaterialTheme.typography.bodySmall,
        color = if (isDark) VisioColors.Greyscale400 else VisioColors.LightTextSecondary,
    )
}
```

- [ ] **Step 6: Update CallScreen — header banner**

Add `roomDisplayName: String?` parameter. Show as header when present.

- [ ] **Step 7: Update InCallSettingsSheet — share with display name**

Add `roomName: String?` parameter. When building share/deep link URLs:

```kotlin
val shareUrl = if (!roomName.isNullOrBlank()) {
    val encoded = java.net.URLEncoder.encode(roomName, "UTF-8").replace("+", "%20")
    "$roomUrl?room-display-name=$encoded"
} else {
    roomUrl
}
```

- [ ] **Step 8: Add accessibility**

- `contentDescription` on the new display name TextField
- `semantics` block on recent room entries combining display name, slug, and host

- [ ] **Step 9: Build Android**

Run: `cd android && ./gradlew assembleDebug`
Expected: compiles without errors

- [ ] **Step 10: Commit**

```bash
git add android/
git commit -m "feat(android): display room name in home, lobby, call and share (#113)"
```

---

## Task 8: iOS — all screens

**Files:**
- Modify: `ios/VisioMobile/Views/HomeView.swift:7,133-137,235-284`
- Modify: `ios/VisioMobile/Views/PreJoinView.swift:168-171,193-201`
- Modify: `ios/VisioMobile/Views/CallView.swift:259`
- Modify: `ios/VisioMobile/Views/InCallSettingsSheet.swift:5-10,214-279`

- [ ] **Step 1: Update HomeView — add display name field**

Add state and TextField:

```swift
@State private var roomDisplayName: String = ""

// After URL TextField:
TextField(Strings.t("home.roomDisplayName", lang: lang), text: $roomDisplayName)
    .textFieldStyle(.roundedBorder)
    .accessibilityLabel(Strings.t("home.roomDisplayName", lang: lang))
```

- [ ] **Step 2: Update HomeView — recent rooms list**

Change from `[String]` to structured entries. Update rendering:

```swift
let entry = roomHistory[i] // now RoomHistoryEntry
let slug = entry.url.contains("/") ? String(entry.url.split(separator: "/").last ?? "") : entry.url
let host = URL(string: entry.url)?.host ?? ""

Text(entry.displayName ?? slug)
    .fontWeight(.semibold)
if entry.displayName != nil {
    Text("\(slug) · \(host)")
        .font(.caption)
        .foregroundStyle(.secondary)
}
```

- [ ] **Step 3: Update HomeView — join flow**

Pass display name to PreJoinView and CallView navigation.

- [ ] **Step 4: Update PreJoinView — accept and show display name**

Add property:

```swift
let roomDisplayName: String?
```

Update header (lines 193-201):

```swift
Text(roomDisplayName ?? slug)
    .font(.title2)
    .fontWeight(.semibold)
    .foregroundStyle(VisioColors.onBackground(dark: isDark))
if roomDisplayName != nil {
    Text(slug)
        .font(.subheadline)
        .foregroundStyle(.secondary)
}
```

- [ ] **Step 5: Update CallView — navigation title**

Change line 259:

```swift
.navigationTitle(roomDisplayName ?? Strings.t("call.title", lang: lang))
```

- [ ] **Step 6: Update InCallSettingsSheet — share with display name**

Add property `let roomDisplayName: String?`. Update deep link construction:

```swift
let shareUrl: String
if let name = roomDisplayName, !name.isEmpty {
    // .urlQueryAllowed includes spaces as allowed, so use a custom set
    var allowed = CharacterSet.urlQueryAllowed
    allowed.remove(charactersIn: " +&=")
    let encoded = name.addingPercentEncoding(withAllowedCharacters: allowed) ?? name
    shareUrl = "\(roomURL)?room-display-name=\(encoded)"
} else {
    shareUrl = roomURL
}
```

- [ ] **Step 7: Handle deep links**

In the app's deep link handler (look for `visio://` scheme handling, typically in the `App` struct or `SceneDelegate`), extract `room-display-name` from the URL components:

```swift
if let components = URLComponents(url: deepLinkURL, resolvingAgainstBaseURL: false),
   let displayName = components.queryItems?.first(where: { $0.name == "room-display-name" })?.value {
    // Pass displayName alongside the clean URL through navigation
}
```

Reconstruct the clean URL without the `room-display-name` param for connection, but pass the display name separately to the lobby/call views.

- [ ] **Step 8: Add accessibility**

- `.accessibilityLabel` on new TextField
- `.accessibilityElement(children: .combine)` on recent room entries

- [ ] **Step 9: Build iOS**

Run: `scripts/build-ios.sh sim`
Expected: compiles without errors

- [ ] **Step 10: Commit**

```bash
git add ios/
git commit -m "feat(ios): display room name in home, lobby, call and share (#113)"
```

---

## Task 9: Final integration test and cleanup

- [ ] **Step 1: Run full Rust test suite**

Run: `cargo test -p visio-core -p visio-desktop`
Expected: all pass

- [ ] **Step 2: Build all platforms**

```bash
cargo build -p visio-core
cargo build -p visio-ffi
cargo build -p visio-desktop
cd android && ./gradlew assembleDebug
```

- [ ] **Step 3: Verify CHANGELOG**

Add entry under `[Unreleased]`:
```markdown
### Added

- Room display name: friendly names for rooms via `?room-display-name=` URL parameter or manual input (#113)
```

- [ ] **Step 4: Final commit**

```bash
git add CHANGELOG.md
git commit -m "chore: add room display name to CHANGELOG (#113)"
```

- [ ] **Step 5: Push branch and create PR**

```bash
git push -u origin feat/room-display-name
```

Create PR targeting `main` referencing issue #113.
