# iCal Calendar Integration — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add iCal calendar integration so authenticated users see their upcoming video meetings on a dedicated "Réunions" tab on the home screen, with one-tap join and local notifications.

**Architecture:** New `CalendarService` in visio-core handles HTTP fetch, iCal parsing, meeting extraction, and timed notifications. Exposed via UniFFI to mobile, direct Rust to desktop. Each platform adds a tab-based home screen, meetings list, and calendar settings section.

**Tech Stack:** Rust (ical crate, reqwest, tokio, chrono, regex), UniFFI, Jetpack Compose, SwiftUI, React/Tauri

**Spec:** `docs/superpowers/specs/2026-03-21-ical-calendar-integration-design.md`

---

## File Structure

### New Files
- `crates/visio-core/src/calendar.rs` — CalendarService: fetch, parse, filter, cache, timer, notification events
- `android/app/src/main/kotlin/io/visio/mobile/ui/MeetingsTab.kt` — Meetings tab composable (4 states)
- `ios/VisioMobile/Views/MeetingsTabView.swift` — Meetings tab SwiftUI view (4 states)
- `crates/visio-desktop/frontend/src/MeetingsTab.tsx` — Meetings tab React component

### Modified Files
- `crates/visio-core/Cargo.toml` — add `ical` crate dependency
- `crates/visio-core/src/lib.rs:6-39` — add `pub mod calendar` + re-exports
- `crates/visio-core/src/settings.rs:7-48,77-100` — add `calendar_url`, `calendar_refresh_interval` fields + defaults
- `crates/visio-core/src/settings.rs:107-377` — add getter/setter methods for calendar settings
- `crates/visio-core/src/events.rs:5-73` — add calendar event variants to VisioEvent
- `crates/visio-ffi/src/visio.udl:1-349` — add Meeting dict, CalendarRefreshInterval enum, calendar methods, calendar events
- `crates/visio-ffi/src/lib.rs:747-794` — add CalendarService to VisioClient struct + init
- `android/app/src/main/kotlin/io/visio/mobile/ui/HomeScreen.kt` — add tab segment control
- `android/app/src/main/kotlin/io/visio/mobile/ui/SettingsScreen.kt` — add Calendar section
- `android/app/src/main/kotlin/io/visio/mobile/VisioManager.kt` — handle calendar events, meetings state
- `ios/VisioMobile/Views/HomeView.swift` — add tab segment control
- `ios/VisioMobile/Views/SettingsView.swift` — add Calendar section
- `ios/VisioMobile/VisioManager.swift` — handle calendar events, meetings state
- `crates/visio-desktop/frontend/src/App.tsx` — add tab UI + meetings tab + settings fields

---

## Task 1: Settings — calendar fields and persistence

**Files:**
- Modify: `crates/visio-core/src/settings.rs:7-48` (Settings struct)
- Modify: `crates/visio-core/src/settings.rs:77-100` (Default impl)
- Modify: `crates/visio-core/src/settings.rs:107-377` (SettingsStore impl)
- Test: `crates/visio-core/src/settings.rs:379+` (existing test module)

- [ ] **Step 1: Write failing tests for calendar settings**

Add to the `#[cfg(test)] mod tests` block at the end of `settings.rs`:

```rust
#[test]
fn test_calendar_url_default_none() {
    let s = Settings::default();
    assert_eq!(s.calendar_url, None);
}

#[test]
fn test_calendar_refresh_interval_default() {
    let s = Settings::default();
    assert_eq!(s.calendar_refresh_interval, CalendarRefreshInterval::Minutes15);
}

#[test]
fn test_set_calendar_url_persists() {
    let dir = temp_dir();
    let path = dir.path().to_str().unwrap();
    {
        let store = SettingsStore::new(path);
        store.set_calendar_url(Some("https://cal.example.com/feed.ics".to_string()));
    }
    let store = SettingsStore::new(path);
    assert_eq!(
        store.get().calendar_url,
        Some("https://cal.example.com/feed.ics".to_string())
    );
}

#[test]
fn test_clear_calendar_url() {
    let dir = temp_dir();
    let path = dir.path().to_str().unwrap();
    let store = SettingsStore::new(path);
    store.set_calendar_url(Some("https://cal.example.com/feed.ics".to_string()));
    store.set_calendar_url(None);
    assert_eq!(store.get().calendar_url, None);
}

#[test]
fn test_set_calendar_refresh_interval_persists() {
    let dir = temp_dir();
    let path = dir.path().to_str().unwrap();
    {
        let store = SettingsStore::new(path);
        store.set_calendar_refresh_interval(CalendarRefreshInterval::Minutes5);
    }
    let store = SettingsStore::new(path);
    assert_eq!(store.get().calendar_refresh_interval, CalendarRefreshInterval::Minutes5);
}

#[test]
fn test_partial_json_defaults_calendar_fields() {
    let dir = temp_dir();
    let path = dir.path().to_str().unwrap();
    std::fs::write(
        dir.path().join("settings.json"),
        r#"{"display_name":"Eve"}"#,
    )
    .unwrap();
    let store = SettingsStore::new(path);
    let s = store.get();
    assert_eq!(s.calendar_url, None);
    assert_eq!(s.calendar_refresh_interval, CalendarRefreshInterval::Minutes15);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p visio-core -- settings::tests::test_calendar`
Expected: compilation errors — `calendar_url`, `calendar_refresh_interval`, `CalendarRefreshInterval` don't exist

- [ ] **Step 3: Add calendar fields to Settings struct**

In `settings.rs`, add two fields to the `Settings` struct (after line 47, before closing `}`):

```rust
    /// iCal calendar URL for upcoming meetings.
    #[serde(default)]
    pub calendar_url: Option<String>,
    /// Calendar refresh interval.
    #[serde(default)]
    pub calendar_refresh_interval: CalendarRefreshInterval,
```

Add the `CalendarRefreshInterval` enum before the `Settings` struct:

```rust
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum CalendarRefreshInterval {
    Minutes5,
    Minutes15,
    Hour1,
    Hours4,
    Manual,
}

impl Default for CalendarRefreshInterval {
    fn default() -> Self {
        Self::Minutes15
    }
}
```

Add to `Default::default()` for Settings (before closing `}` at line 98):

```rust
            calendar_url: None,
            calendar_refresh_interval: CalendarRefreshInterval::default(),
```

- [ ] **Step 4: Add getter/setter methods to SettingsStore**

Add after `clear_room_history()` method (after line 355):

```rust
    pub fn get_calendar_url(&self) -> Option<String> {
        self.settings
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .calendar_url
            .clone()
    }

    pub fn set_calendar_url(&self, url: Option<String>) {
        self.settings
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .calendar_url = url;
        self.save();
    }

    pub fn get_calendar_refresh_interval(&self) -> CalendarRefreshInterval {
        self.settings
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .calendar_refresh_interval
            .clone()
    }

    pub fn set_calendar_refresh_interval(&self, interval: CalendarRefreshInterval) {
        self.settings
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .calendar_refresh_interval = interval;
        self.save();
    }
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p visio-core -- settings::tests`
Expected: all settings tests pass (existing + 6 new)

- [ ] **Step 6: Commit**

```bash
git add crates/visio-core/src/settings.rs
git commit -m "feat(core): add calendar_url and calendar_refresh_interval to settings"
```

---

## Task 2: Calendar events in VisioEvent

**Files:**
- Modify: `crates/visio-core/src/events.rs:5-73` (VisioEvent enum)

- [ ] **Step 1: Add calendar event variants to VisioEvent**

In `events.rs`, add after `MuteRequested` (line 72, before closing `}`):

```rust
    /// Calendar meetings list was refreshed.
    MeetingsUpdated(Vec<Meeting>),
    /// A meeting starts in less than 15 minutes.
    MeetingImminent(Meeting),
    /// A meeting starts in less than 5 minutes.
    MeetingStartingSoon(Meeting),
    /// A meeting is starting now.
    MeetingStarted(Meeting),
    /// Calendar fetch or parse error.
    CalendarError(String),
```

Add the `Meeting` struct definition before `VisioEvent` (after line 2):

```rust
/// A parsed calendar meeting with a video conference link.
#[derive(Debug, Clone, PartialEq)]
pub struct Meeting {
    pub id: String,
    pub summary: String,
    pub start_time: i64,
    pub end_time: i64,
    pub room_url: String,
    pub deep_link: String,
    pub server_name: String,
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo build -p visio-core`
Expected: compiles successfully

- [ ] **Step 3: Commit**

```bash
git add crates/visio-core/src/events.rs
git commit -m "feat(core): add Meeting struct and calendar events to VisioEvent"
```

---

## Task 3: CalendarService — iCal fetch, parse, and filter

**Files:**
- Create: `crates/visio-core/src/calendar.rs`
- Modify: `crates/visio-core/Cargo.toml:7-21` (add `ical` dependency)
- Modify: `crates/visio-core/src/lib.rs:6-39` (add module + re-exports)

- [ ] **Step 1: Add `ical` crate dependency**

In `crates/visio-core/Cargo.toml`, add after `regex = "1"` (line 21):

```toml
ical = "0.11"
```

- [ ] **Step 2: Write failing test for iCal parsing**

Create `crates/visio-core/src/calendar.rs`:

```rust
use std::sync::{Arc, Mutex};

use crate::events::{EventEmitter, Meeting};
use crate::settings::SettingsStore;

/// Parses an iCal VEVENT block and extracts a Meeting if it contains
/// a video link matching one of the given server domains.
fn parse_meeting_from_vevent(
    event: &ical::parser::ical::component::IcalEvent,
    servers: &[String],
    now_ts: i64,
    cutoff_ts: i64,
) -> Option<Meeting> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_ical(summary: &str, dtstart: &str, dtend: &str, location: &str) -> String {
        format!(
            "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\n\
             UID:test-uid-123\r\n\
             SUMMARY:{summary}\r\n\
             DTSTART:{dtstart}\r\n\
             DTEND:{dtend}\r\n\
             LOCATION:{location}\r\n\
             END:VEVENT\r\nEND:VCALENDAR"
        )
    }

    fn parse_ical_events(ical_text: &str) -> Vec<ical::parser::ical::component::IcalEvent> {
        use ical::parser::ical::IcalParser;
        use std::io::BufReader;
        let reader = BufReader::new(ical_text.as_bytes());
        let parser = IcalParser::new(reader);
        parser
            .flat_map(|cal| cal.ok().into_iter().flat_map(|c| c.events))
            .collect()
    }

    #[test]
    fn test_parse_meeting_with_matching_server() {
        let servers = vec!["meet.linagora.com".to_string()];
        let ical = sample_ical(
            "COCO 2025",
            "20260325T093000Z",
            "20260325T103000Z",
            "https://meet.linagora.com/tui-ytsh-uta",
        );
        let events = parse_ical_events(&ical);
        let now_ts = 1742900000; // before the event
        let cutoff_ts = 1742990000; // after the event

        let meeting = parse_meeting_from_vevent(&events[0], &servers, now_ts, cutoff_ts);
        assert!(meeting.is_some());
        let m = meeting.unwrap();
        assert_eq!(m.summary, "COCO 2025");
        assert_eq!(m.room_url, "https://meet.linagora.com/tui-ytsh-uta");
        assert_eq!(m.deep_link, "visio://meet.linagora.com/tui-ytsh-uta");
        assert_eq!(m.server_name, "meet.linagora.com");
    }

    #[test]
    fn test_parse_meeting_no_matching_server() {
        let servers = vec!["meet.linagora.com".to_string()];
        let ical = sample_ical(
            "Teams Meeting",
            "20260325T093000Z",
            "20260325T103000Z",
            "https://teams.microsoft.com/l/meetup-join/abc",
        );
        let events = parse_ical_events(&ical);
        let now_ts = 1742900000;
        let cutoff_ts = 1742990000;

        let meeting = parse_meeting_from_vevent(&events[0], &servers, now_ts, cutoff_ts);
        assert!(meeting.is_none());
    }

    #[test]
    fn test_parse_meeting_past_event_excluded() {
        let servers = vec!["meet.linagora.com".to_string()];
        let ical = sample_ical(
            "Past meeting",
            "20260101T090000Z",
            "20260101T100000Z",
            "https://meet.linagora.com/abc-defg-hij",
        );
        let events = parse_ical_events(&ical);
        let now_ts = 1742900000; // well after the event
        let cutoff_ts = 1742990000;

        let meeting = parse_meeting_from_vevent(&events[0], &servers, now_ts, cutoff_ts);
        assert!(meeting.is_none());
    }

    #[test]
    fn test_parse_meeting_visio_deep_link() {
        let servers = vec!["meet.linagora.com".to_string()];
        let ical = sample_ical(
            "Deep link test",
            "20260325T140000Z",
            "20260325T150000Z",
            "visio://meet.linagora.com/xyz-mnop-qrs",
        );
        let events = parse_ical_events(&ical);
        let now_ts = 1742900000;
        let cutoff_ts = 1742990000;

        let meeting = parse_meeting_from_vevent(&events[0], &servers, now_ts, cutoff_ts);
        assert!(meeting.is_some());
        let m = meeting.unwrap();
        assert_eq!(m.deep_link, "visio://meet.linagora.com/xyz-mnop-qrs");
    }

    #[test]
    fn test_parse_meeting_link_in_description() {
        let ical = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\n\
            UID:desc-test\r\n\
            SUMMARY:Description link\r\n\
            DTSTART:20260325T140000Z\r\n\
            DTEND:20260325T150000Z\r\n\
            DESCRIPTION:Join at https://meet.linagora.com/abc-defg-hij\r\n\
            END:VEVENT\r\nEND:VCALENDAR";
        let events = parse_ical_events(ical);
        let servers = vec!["meet.linagora.com".to_string()];
        let now_ts = 1742900000;
        let cutoff_ts = 1742990000;

        let meeting = parse_meeting_from_vevent(&events[0], &servers, now_ts, cutoff_ts);
        assert!(meeting.is_some());
        assert_eq!(
            meeting.unwrap().room_url,
            "https://meet.linagora.com/abc-defg-hij"
        );
    }

    #[test]
    fn test_parse_meeting_truncated_link_ignored() {
        let servers = vec!["meet.linagora.com".to_string()];
        let ical = sample_ical(
            "Truncated",
            "20260325T140000Z",
            "20260325T150000Z",
            "https://meet.linagora.",
        );
        let events = parse_ical_events(&ical);
        let now_ts = 1742900000;
        let cutoff_ts = 1742990000;

        let meeting = parse_meeting_from_vevent(&events[0], &servers, now_ts, cutoff_ts);
        assert!(meeting.is_none());
    }
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p visio-core -- calendar::tests`
Expected: FAIL — `parse_meeting_from_vevent` has `todo!()`

- [ ] **Step 4: Implement `parse_meeting_from_vevent`**

Replace the `todo!()` in `parse_meeting_from_vevent`:

```rust
fn parse_meeting_from_vevent(
    event: &ical::parser::ical::component::IcalEvent,
    servers: &[String],
    now_ts: i64,
    cutoff_ts: i64,
) -> Option<Meeting> {
    use regex::Regex;
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let get_prop = |name: &str| -> Option<String> {
        event
            .properties
            .iter()
            .find(|p| p.name == name)
            .and_then(|p| p.value.clone())
    };

    // Parse DTSTART
    let dtstart_str = get_prop("DTSTART")?;
    let start_time = parse_ical_timestamp(&dtstart_str)?;

    // Parse DTEND early so we can check in-progress meetings
    let dtend_str = get_prop("DTEND").unwrap_or_default();
    let end_time = parse_ical_timestamp(&dtend_str).unwrap_or(start_time + 3600);

    // Include in-progress meetings (DTSTART passed but DTEND not yet)
    // Exclude meetings fully in the past or beyond the cutoff
    if end_time < now_ts || start_time > cutoff_ts {
        return None;
    }

    let summary = get_prop("SUMMARY").unwrap_or_else(|| "(sans objet)".to_string());
    let uid = get_prop("UID").unwrap_or_default();

    // Search for video links in LOCATION, DESCRIPTION, URL
    let search_text = [
        get_prop("LOCATION").unwrap_or_default(),
        get_prop("DESCRIPTION").unwrap_or_default(),
        get_prop("URL").unwrap_or_default(),
    ]
    .join(" ");

    // Try visio:// deep links first
    let visio_re = Regex::new(r"visio://([^/\s]+)(/[^\s]*)").unwrap();
    if let Some(caps) = visio_re.captures(&search_text) {
        let host = caps.get(1).unwrap().as_str();
        let path = caps.get(2).unwrap().as_str();
        let deep_link = format!("visio://{host}{path}");
        let room_url = format!("https://{host}{path}");

        let mut hasher = DefaultHasher::new();
        format!("{uid}-{start_time}").hash(&mut hasher);
        let id = format!("{:x}", hasher.finish());

        return Some(Meeting {
            id,
            summary,
            start_time,
            end_time,
            room_url,
            deep_link,
            server_name: host.to_string(),
        });
    }

    // Try https:// links matching pre-registered servers
    for server in servers {
        let pattern = format!(r"https?://{}/([^\s<>\"]+)", regex::escape(server));
        let re = Regex::new(&pattern).unwrap();
        if let Some(caps) = re.captures(&search_text) {
            let path = caps.get(1).unwrap().as_str().trim_end_matches('/');
            if path.is_empty() {
                continue;
            }
            let room_url = format!("https://{server}/{path}");
            let deep_link = format!("visio://{server}/{path}");

            let mut hasher = DefaultHasher::new();
            format!("{uid}-{start_time}").hash(&mut hasher);
            let id = format!("{:x}", hasher.finish());

            return Some(Meeting {
                id,
                summary,
                start_time,
                end_time,
                room_url,
                deep_link,
                server_name: server.clone(),
            });
        }
    }

    None
}

/// Parse iCal timestamp (YYYYMMDDTHHMMSSZ or YYYYMMDDTHHMMSS) to Unix seconds.
fn parse_ical_timestamp(s: &str) -> Option<i64> {
    let s = s.trim().replace('Z', "");
    let dt = chrono::NaiveDateTime::parse_from_str(&s, "%Y%m%dT%H%M%S").ok()?;
    Some(dt.and_utc().timestamp())
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p visio-core -- calendar::tests`
Expected: all 6 tests pass

- [ ] **Step 6: Register the module in lib.rs**

In `crates/visio-core/src/lib.rs`, add after line 12 (`pub mod chat;`):

```rust
pub mod calendar;
```

Add to re-exports after line 28 (`pub use chat::ChatService;`):

```rust
pub use calendar::CalendarService;
pub use settings::CalendarRefreshInterval;
```

- [ ] **Step 7: Verify full crate compiles**

Run: `cargo build -p visio-core`
Expected: compiles (CalendarService struct doesn't exist yet — the pub use will fail. Add a placeholder struct.)

Add at the top of `calendar.rs`, after imports:

```rust
/// Service that periodically fetches an iCal calendar and extracts meetings.
#[derive(Clone)]
pub struct CalendarService {
    settings: Arc<SettingsStore>,
    meetings: Arc<Mutex<Vec<Meeting>>>,
    emitter: EventEmitter,
    notified_ids: Arc<Mutex<std::collections::HashSet<String>>>,
    data_dir: String,
    http_client: reqwest::Client,
}
```

Run: `cargo build -p visio-core`
Expected: compiles successfully

- [ ] **Step 8: Commit**

```bash
git add crates/visio-core/Cargo.toml crates/visio-core/src/calendar.rs crates/visio-core/src/lib.rs
git commit -m "feat(core): add CalendarService with iCal parsing and meeting extraction"
```

---

## Task 4: CalendarService — fetch, adaptive window, cache, and timer

**Files:**
- Modify: `crates/visio-core/src/calendar.rs`

- [ ] **Step 1: Write failing tests for adaptive time window**

Add to the `tests` module in `calendar.rs`. These test `parse_meetings()` directly with different data to verify the adaptive window logic:

```rust
#[test]
fn test_adaptive_window_returns_24h_if_meetings_exist() {
    // Event within 24h → parse_meetings with 24h window finds it
    let now_ts = 1742900000i64;
    let ical = sample_ical(
        "Soon meeting",
        "20260325T120000Z", // within 24h of now_ts
        "20260325T130000Z",
        "https://meet.linagora.com/abc-defg-hij",
    );
    let servers = vec!["meet.linagora.com".to_string()];
    let reader = std::io::BufReader::new(ical.as_bytes());
    let parser = IcalParser::new(reader);
    let mut meetings = Vec::new();
    for cal in parser.flatten() {
        for ev in &cal.events {
            if let Some(m) = parse_meeting_from_vevent(ev, &servers, now_ts, now_ts + 86400) {
                meetings.push(m);
            }
        }
    }
    assert_eq!(meetings.len(), 1);
}

#[test]
fn test_adaptive_window_empty_24h_finds_in_3d() {
    // Event in 2 days → not found in 24h, found in 3d
    let now_ts = 1742900000i64;
    let ical = sample_ical(
        "Later meeting",
        "20260327T120000Z", // ~2 days from now_ts
        "20260327T130000Z",
        "https://meet.linagora.com/abc-defg-hij",
    );
    let servers = vec!["meet.linagora.com".to_string()];
    let parse = |cutoff: i64| {
        let reader = std::io::BufReader::new(ical.as_bytes());
        let parser = IcalParser::new(reader);
        let mut meetings = Vec::new();
        for cal in parser.flatten() {
            for ev in &cal.events {
                if let Some(m) = parse_meeting_from_vevent(ev, &servers, now_ts, cutoff) {
                    meetings.push(m);
                }
            }
        }
        meetings
    };
    assert!(parse(now_ts + 86400).is_empty()); // 24h: nothing
    assert_eq!(parse(now_ts + 86400 * 3).len(), 1); // 3d: found
}
```

- [ ] **Step 2: Implement CalendarService methods**

Add to `calendar.rs`:

```rust
use std::collections::HashSet;
use std::io::BufReader;

use chrono::Utc;
use ical::parser::ical::IcalParser;
use regex::Regex;
use tracing::{info, warn};

impl CalendarService {
    pub fn new(settings: Arc<SettingsStore>, emitter: EventEmitter, data_dir: String) -> Self {
        // Load cached meetings from disk
        let cached = Self::load_cache(&data_dir);
        Self {
            settings,
            meetings: Arc::new(Mutex::new(cached)),
            emitter,
            notified_ids: Arc::new(Mutex::new(HashSet::new())),
            data_dir,
            http_client: reqwest::Client::new(),
        }
    }

    /// Fetch and parse the calendar, returning meetings matching known servers.
    pub async fn refresh(&self) -> Result<Vec<Meeting>, String> {
        let url = self
            .settings
            .get_calendar_url()
            .ok_or_else(|| "No calendar URL configured".to_string())?;
        let servers = self.settings.get_meet_instances();
        let now_ts = Utc::now().timestamp();

        // Fetch iCal data (reuse client for connection pooling)
        let response = self.http_client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Calendar fetch failed: {e}"))?;
        let body = response
            .text()
            .await
            .map_err(|e| format!("Calendar read failed: {e}"))?;

        // Parse with adaptive window: try 24h, 3d, 7d
        let windows = [86400i64, 86400 * 3, 86400 * 7];
        let mut meetings = Vec::new();

        for &window in &windows {
            let cutoff_ts = now_ts + window;
            meetings = self.parse_meetings(&body, &servers, now_ts, cutoff_ts);
            if !meetings.is_empty() {
                break;
            }
        }

        meetings.sort_by_key(|m| m.start_time);

        // Update cached meetings
        *self.meetings.lock().unwrap_or_else(|e| e.into_inner()) = meetings.clone();

        // Cache to disk
        self.save_cache(&meetings);

        // Emit update event
        self.emitter.emit(crate::events::VisioEvent::MeetingsUpdated(meetings.clone()));

        // Check for imminent meetings
        self.check_notifications(&meetings, now_ts);

        info!("Calendar refreshed: {} meetings found", meetings.len());
        Ok(meetings)
    }

    fn parse_meetings(
        &self,
        ical_text: &str,
        servers: &[String],
        now_ts: i64,
        cutoff_ts: i64,
    ) -> Vec<Meeting> {
        let reader = BufReader::new(ical_text.as_bytes());
        let parser = IcalParser::new(reader);
        let mut meetings = Vec::new();

        for calendar in parser.flatten() {
            for event in &calendar.events {
                if let Some(meeting) =
                    parse_meeting_from_vevent(event, servers, now_ts, cutoff_ts)
                {
                    meetings.push(meeting);
                }
            }
        }

        meetings
    }

    fn check_notifications(&self, meetings: &[Meeting], now_ts: i64) {
        let mut notified = self.notified_ids.lock().unwrap_or_else(|e| e.into_inner());
        for meeting in meetings {
            let delta = meeting.start_time - now_ts;
            let key_imminent = format!("{}-imminent", meeting.id);
            let key_soon = format!("{}-soon", meeting.id);
            let key_started = format!("{}-started", meeting.id);

            if delta <= 900 && delta > 300 && !notified.contains(&key_imminent) {
                notified.insert(key_imminent);
                self.emitter
                    .emit(crate::events::VisioEvent::MeetingImminent(meeting.clone()));
            }
            if delta <= 300 && delta > 0 && !notified.contains(&key_soon) {
                notified.insert(key_soon);
                self.emitter
                    .emit(crate::events::VisioEvent::MeetingStartingSoon(meeting.clone()));
            }
            if delta <= 0 && delta > -60 && !notified.contains(&key_started) {
                notified.insert(key_started);
                self.emitter
                    .emit(crate::events::VisioEvent::MeetingStarted(meeting.clone()));
            }
        }
    }

    pub fn get_meetings(&self) -> Vec<Meeting> {
        let now_ts = Utc::now().timestamp();
        self.meetings
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .filter(|m| m.end_time > now_ts) // filter out past meetings
            .cloned()
            .collect()
    }

    fn save_cache(&self, meetings: &[Meeting]) {
        let path = std::path::PathBuf::from(&self.data_dir).join("calendar_cache.json");
        if let Ok(json) = serde_json::to_string(meetings) {
            let _ = std::fs::write(path, json);
        }
    }

    pub fn load_cache(data_dir: &str) -> Vec<Meeting> {
        let path = std::path::PathBuf::from(data_dir).join("calendar_cache.json");
        match std::fs::read_to_string(path) {
            Ok(contents) => serde_json::from_str(&contents).unwrap_or_default(),
            Err(_) => Vec::new(),
        }
    }

    /// Parse the refresh interval setting into a Duration.
    pub fn refresh_interval(&self) -> std::time::Duration {
        use crate::settings::CalendarRefreshInterval;
        match self.settings.get_calendar_refresh_interval() {
            CalendarRefreshInterval::Minutes5 => std::time::Duration::from_secs(300),
            CalendarRefreshInterval::Minutes15 => std::time::Duration::from_secs(900),
            CalendarRefreshInterval::Hour1 => std::time::Duration::from_secs(3600),
            CalendarRefreshInterval::Hours4 => std::time::Duration::from_secs(14400),
            CalendarRefreshInterval::Manual => std::time::Duration::ZERO,
        }
    }
}
```

- [ ] **Step 3: Add Serialize/Deserialize to Meeting for cache**

Add `Serialize, Deserialize` derives to `Meeting` in `events.rs` (the `save_cache`/`load_cache` methods are already implemented in Task 3):

```rust
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Meeting { ... }
```

- [ ] **Step 4: Run all tests**

Run: `cargo test -p visio-core`
Expected: all tests pass

- [ ] **Step 5: Commit**

```bash
git add crates/visio-core/src/calendar.rs crates/visio-core/src/events.rs
git commit -m "feat(core): implement CalendarService with fetch, adaptive window, cache, and notifications"
```

---

## Task 5: UniFFI interface — Meeting, CalendarRefreshInterval, calendar methods

**Files:**
- Modify: `crates/visio-ffi/src/visio.udl:1-349`
- Modify: `crates/visio-ffi/src/lib.rs:747-794`

- [ ] **Step 1: Add Meeting dictionary and CalendarRefreshInterval to UDL**

In `visio.udl`, add after `CreateRoomResult` dictionary (after line 183):

```
dictionary Meeting {
    string id;
    string summary;
    i64 start_time;
    i64 end_time;
    string room_url;
    string deep_link;
    string server_name;
};
```

- [ ] **Step 2: Add CalendarRefreshInterval enum and calendar fields to Settings**

In `visio.udl`, add the enum after `NetworkType` (after line 49):

```
enum CalendarRefreshInterval {
    "Minutes5",
    "Minutes15",
    "Hour1",
    "Hours4",
    "Manual",
};
```

Add to the `Settings` dictionary (after line 96, before `};`):

```
    string? calendar_url;
    CalendarRefreshInterval calendar_refresh_interval;
```

- [ ] **Step 3: Add calendar events to VisioEvent**

In `visio.udl`, add before `};` closing VisioEvent (after line 125):

```
    MeetingsUpdated(sequence<Meeting> meetings);
    MeetingImminent(Meeting meeting);
    MeetingStartingSoon(Meeting meeting);
    MeetingStarted(Meeting meeting);
    CalendarError(string message);
```

- [ ] **Step 4: Add calendar methods to VisioClient interface**

In `visio.udl`, add to the VisioClient interface (after `get_background_mode` at line 341):

```
    void set_calendar_url(string? url);
    string? get_calendar_url();
    void set_calendar_refresh_interval(CalendarRefreshInterval interval);
    CalendarRefreshInterval get_calendar_refresh_interval();
    sequence<Meeting> get_upcoming_meetings();
    void refresh_calendar_now();
```

- [ ] **Step 5: Add CalendarService to VisioClient struct in FFI lib.rs**

In `crates/visio-ffi/src/lib.rs`, add field to `VisioClient` struct (after line 752):

```rust
    calendar: visio_core::CalendarService,
```

In `VisioClient::new()`, initialize after `session_manager` (before line 785):

```rust
        let calendar = Arc::new(visio_core::CalendarService::new(
            settings.clone(),
            room_manager.emitter(),
            data_dir.clone(),
        ));
```

Add `calendar` to the `Self { ... }` block. Update the struct field type to `Arc<visio_core::CalendarService>`.

- [ ] **Step 6: Implement FFI wrapper methods**

Add to the `impl VisioClient` block:

```rust
    pub fn set_calendar_url(&self, url: Option<String>) {
        self.settings.set_calendar_url(url);
    }

    pub fn get_calendar_url(&self) -> Option<String> {
        self.settings.get_calendar_url()
    }

    pub fn set_calendar_refresh_interval(&self, interval: visio_core::settings::CalendarRefreshInterval) {
        self.settings.set_calendar_refresh_interval(interval);
    }

    pub fn get_calendar_refresh_interval(&self) -> visio_core::settings::CalendarRefreshInterval {
        self.settings.get_calendar_refresh_interval()
    }

    pub fn get_upcoming_meetings(&self) -> Vec<Meeting> {
        self.calendar.get_meetings()
    }

    pub fn refresh_calendar_now(&self) {
        let calendar = self.calendar.clone();
        self.rt.spawn(async move {
            if let Err(e) = calendar.refresh().await {
                tracing::warn!("Calendar refresh failed: {e}");
            }
        });
    }
```

Note: `CalendarService` needs to implement `Clone`. Add `#[derive(Clone)]` or manual Clone on CalendarService (all fields are Arc-wrapped).

- [ ] **Step 7: Verify FFI crate compiles**

Run: `cargo build -p visio-ffi`
Expected: compiles. Fix any type mismatches between UDL and Rust.

- [ ] **Step 8: Regenerate bindings**

Run: `scripts/generate-bindings.sh all`

- [ ] **Step 9: Commit**

```bash
git add crates/visio-ffi/src/visio.udl crates/visio-ffi/src/lib.rs
git commit -m "feat(ffi): expose calendar API via UniFFI — Meeting type, calendar methods, events"
```

---

## Task 6: Android — Settings Calendar section

**Files:**
- Modify: `android/app/src/main/kotlin/io/visio/mobile/ui/SettingsScreen.kt`

- [ ] **Step 1: Add calendar state variables**

In `SettingsScreen` composable, add state variables alongside existing ones:

```kotlin
var calendarUrl by remember { mutableStateOf("") }
var calendarRefreshInterval by remember { mutableStateOf(CalendarRefreshInterval.MINUTES15) }
```

Load in `LaunchedEffect`:

```kotlin
calendarUrl = VisioManager.client.getCalendarUrl() ?: ""
calendarRefreshInterval = VisioManager.client.getCalendarRefreshInterval()
```

- [ ] **Step 2: Add Calendar section UI**

Add after the "Meet Instances" section and before "Theme" section:

```kotlin
// Calendar section
Text(
    Strings.t("settings.calendar", lang),
    style = MaterialTheme.typography.titleMedium,
    modifier = Modifier.padding(top = 16.dp, bottom = 8.dp)
)

OutlinedTextField(
    value = calendarUrl,
    onValueChange = {
        calendarUrl = it
        VisioManager.client.setCalendarUrl(it.ifBlank { null })
    },
    label = { Text(Strings.t("settings.calendar_url", lang)) },
    placeholder = { Text("https://calendar.example.com/feed.ics") },
    singleLine = true,
    modifier = Modifier.fillMaxWidth()
)

// Refresh interval dropdown
val intervalOptions = listOf(
    CalendarRefreshInterval.MINUTES5 to "5 min",
    CalendarRefreshInterval.MINUTES15 to "15 min",
    CalendarRefreshInterval.HOUR1 to "1 heure",
    CalendarRefreshInterval.HOURS4 to "4 heures",
    CalendarRefreshInterval.MANUAL to "Manuel",
)
var expanded by remember { mutableStateOf(false) }
ExposedDropdownMenuBox(expanded = expanded, onExpandedChange = { expanded = it }) {
    OutlinedTextField(
        value = intervalOptions.find { it.first == calendarRefreshInterval }?.second ?: "15 min",
        onValueChange = {},
        readOnly = true,
        label = { Text(Strings.t("settings.refresh_interval", lang)) },
        trailingIcon = { ExposedDropdownMenuDefaults.TrailingIcon(expanded) },
        modifier = Modifier.menuAnchor().fillMaxWidth()
    )
    ExposedDropdownMenu(expanded = expanded, onDismissRequest = { expanded = false }) {
        intervalOptions.forEach { (enumValue, label) ->
            DropdownMenuItem(
                text = { Text(label) },
                onClick = {
                    calendarRefreshInterval = enumValue
                    VisioManager.client.setCalendarRefreshInterval(enumValue)
                    expanded = false
                }
            )
        }
    }
}

// Remove calendar button
if (calendarUrl.isNotBlank()) {
    TextButton(
        onClick = {
            calendarUrl = ""
            VisioManager.client.setCalendarUrl(null)
        },
        modifier = Modifier.padding(top = 8.dp)
    ) {
        Text(
            Strings.t("settings.remove_calendar", lang),
            color = MaterialTheme.colorScheme.error
        )
    }
}
```

- [ ] **Step 3: Verify Android builds**

Run: `cd android && ./gradlew compileDebugKotlin`
Expected: compiles (may need binding regeneration first)

- [ ] **Step 4: Commit**

```bash
git add android/app/src/main/kotlin/io/visio/mobile/ui/SettingsScreen.kt
git commit -m "feat(android): add Calendar section to settings screen"
```

---

## Task 7: Android — Meetings tab on home screen

**Files:**
- Create: `android/app/src/main/kotlin/io/visio/mobile/ui/MeetingsTab.kt`
- Modify: `android/app/src/main/kotlin/io/visio/mobile/ui/HomeScreen.kt`
- Modify: `android/app/src/main/kotlin/io/visio/mobile/VisioManager.kt`

- [ ] **Step 1: Add meetings state to VisioManager**

In `VisioManager.kt`, add state:

```kotlin
private val _upcomingMeetings = MutableStateFlow<List<Meeting>>(emptyList())
val upcomingMeetings: StateFlow<List<Meeting>> = _upcomingMeetings
```

Handle calendar events in the `onEvent` handler:

```kotlin
is VisioEvent.MeetingsUpdated -> {
    _upcomingMeetings.value = event.meetings
}
is VisioEvent.MeetingImminent -> { /* trigger notification */ }
is VisioEvent.MeetingStartingSoon -> { /* trigger notification */ }
is VisioEvent.MeetingStarted -> { /* trigger notification */ }
is VisioEvent.CalendarError -> {
    tracing.warn("Calendar error: ${event.message}")
}
```

- [ ] **Step 2: Create MeetingsTab composable**

Create `android/app/src/main/kotlin/io/visio/mobile/ui/MeetingsTab.kt`:

```kotlin
@Composable
fun MeetingsTab(
    meetings: List<Meeting>,
    hasCalendarUrl: Boolean,
    isLoading: Boolean,
    onSettings: () -> Unit,
    onJoinMeeting: (roomUrl: String, serverName: String) -> Unit,
) {
    when {
        !hasCalendarUrl -> OnboardingState(onSettings)
        isLoading && meetings.isEmpty() -> LoadingState()
        meetings.isEmpty() -> EmptyState(onRefresh = { VisioManager.client.refreshCalendarNow() })
        else -> MeetingsList(meetings, onJoinMeeting)
    }
}

@Composable
private fun OnboardingState(onSettings: () -> Unit) {
    Column(
        modifier = Modifier.fillMaxSize().padding(32.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center
    ) {
        Text("📅", fontSize = 48.sp)
        Spacer(Modifier.height(16.dp))
        Text("Connectez votre agenda", style = MaterialTheme.typography.titleLarge)
        Spacer(Modifier.height(8.dp))
        Text(
            "Retrouvez vos prochaines visioconférences ici en ajoutant l'URL de votre calendrier iCal.",
            textAlign = TextAlign.Center, color = MaterialTheme.colorScheme.onSurfaceVariant
        )
        Spacer(Modifier.height(24.dp))
        Button(onClick = onSettings) { Text("Configurer le calendrier") }
    }
}

@Composable
private fun MeetingsList(
    meetings: List<Meeting>,
    onJoinMeeting: (String, String) -> Unit,
) {
    val now = System.currentTimeMillis() / 1000
    // Group meetings by day
    val grouped = meetings.groupBy { dayLabel(it.startTime, now) }

    LazyColumn(modifier = Modifier.fillMaxSize().padding(horizontal = 16.dp)) {
        grouped.forEach { (dayLabel, dayMeetings) ->
            item {
                Text(
                    dayLabel, style = MaterialTheme.typography.labelSmall,
                    modifier = Modifier.padding(top = 12.dp, bottom = 6.dp)
                )
            }
            items(dayMeetings) { meeting ->
                MeetingCard(meeting, now, onJoinMeeting)
            }
        }
    }
}

@Composable
private fun MeetingCard(
    meeting: Meeting,
    nowSecs: Long,
    onJoin: (String, String) -> Unit,
) {
    val delta = meeting.startTime - nowSecs
    val isImminent = delta in 0..900
    val isInProgress = delta < 0 && meeting.endTime > nowSecs

    val timeText = when {
        isInProgress -> "En cours"
        delta < 3600 * 4 -> "Dans ${delta / 60} min"
        dayLabel(meeting.startTime, nowSecs) == "Aujourd'hui" ->
            formatTime(meeting.startTime) // "14:30"
        else -> formatDayTime(meeting.startTime) // "Mar 09:00"
    }

    Card(
        modifier = Modifier.fillMaxWidth().padding(vertical = 4.dp)
            .clickable { onJoin(meeting.roomUrl, meeting.serverName) },
        colors = if (isImminent || isInProgress)
            CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.primaryContainer)
        else CardDefaults.cardColors()
    ) {
        Row(
            modifier = Modifier.padding(14.dp),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically
        ) {
            Column(modifier = Modifier.weight(1f)) {
                Text(meeting.summary, style = MaterialTheme.typography.titleSmall)
                Text(timeText, style = MaterialTheme.typography.bodySmall)
                Text(meeting.serverName, style = MaterialTheme.typography.labelSmall)
            }
            Button(onClick = { onJoin(meeting.roomUrl, meeting.serverName) }) {
                Text("Rejoindre")
            }
        }
    }
}
```

Implement `dayLabel()`, `formatTime()`, `formatDayTime()` helper functions using `java.time.Instant` and `java.time.LocalDate` for grouping/formatting.

- [ ] **Step 3: Add tab segment control to HomeScreen**

Wrap existing HomeScreen content in a tab structure:

```kotlin
var selectedTab by remember { mutableIntStateOf(0) }
val meetings by VisioManager.upcomingMeetings.collectAsState()
val hasCalendar = VisioManager.client.getCalendarUrl() != null
val isAuthenticated = /* check session state */

// Segment control — visible when user is OIDC-authenticated
// (even without calendar URL, to show onboarding state)
if (isAuthenticated) {
    TabRow(selectedTabIndex = selectedTab) {
        Tab(selected = selectedTab == 0, onClick = { selectedTab = 0 }) {
            Text("Rejoindre")
        }
        Tab(selected = selectedTab == 1, onClick = { selectedTab = 1 }) {
            BadgedBox(badge = {
                if (meetings.isNotEmpty()) Badge { Text("${meetings.size}") }
            }) {
                Text("Réunions")
            }
        }
    }
}

// Tab content
when (selectedTab) {
    0 -> { /* existing join content */ }
    1 -> MeetingsTab(
        meetings = meetings,
        hasCalendarUrl = hasCalendar,
        onSettings = onSettings,
        onJoinMeeting = { url, _ -> onJoin(url, username) }
    )
}
```

- [ ] **Step 4: Trigger refresh on tab switch and foreground**

Add `LaunchedEffect` for refresh:

```kotlin
LaunchedEffect(selectedTab) {
    if (selectedTab == 1) {
        VisioManager.client.refreshCalendarNow()
    }
}
```

- [ ] **Step 5: Verify Android builds**

Run: `cd android && ./gradlew compileDebugKotlin`
Expected: compiles

- [ ] **Step 6: Commit**

```bash
git add android/app/src/main/kotlin/io/visio/mobile/ui/MeetingsTab.kt \
        android/app/src/main/kotlin/io/visio/mobile/ui/HomeScreen.kt \
        android/app/src/main/kotlin/io/visio/mobile/VisioManager.kt
git commit -m "feat(android): add Meetings tab to home screen with calendar integration"
```

---

## Task 8: Android — local notifications

**Files:**
- Modify: `android/app/src/main/kotlin/io/visio/mobile/VisioManager.kt`
- Modify: `android/app/src/main/AndroidManifest.xml` (add notification permission)

- [ ] **Step 1: Add POST_NOTIFICATIONS permission to manifest**

```xml
<uses-permission android:name="android.permission.POST_NOTIFICATIONS" />
```

- [ ] **Step 2: Create notification channel on app start**

In `VisioManager` or `MainActivity`, create channel:

```kotlin
if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
    val channel = NotificationChannel(
        "meetings",
        "Réunions",
        NotificationManager.IMPORTANCE_HIGH
    ).apply { description = "Rappels de visioconférences" }
    getSystemService(NotificationManager::class.java).createNotificationChannel(channel)
}
```

- [ ] **Step 3: Request permission when calendar is first configured**

In `SettingsScreen`, when user sets a calendar URL for the first time:

```kotlin
if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
    requestPermissionLauncher.launch(Manifest.permission.POST_NOTIFICATIONS)
}
```

- [ ] **Step 4: Send notifications on MeetingImminent/StartingSoon/Started events**

```kotlin
is VisioEvent.MeetingImminent -> {
    sendMeetingNotification(event.meeting, "Dans 15 min")
}
is VisioEvent.MeetingStartingSoon -> {
    sendMeetingNotification(event.meeting, "Dans 5 min")
}
is VisioEvent.MeetingStarted -> {
    sendMeetingNotification(event.meeting, "Commence maintenant")
}
```

- [ ] **Step 5: Commit**

```bash
git add android/
git commit -m "feat(android): add local notifications for upcoming meetings"
```

---

## Task 9: iOS — Settings Calendar section + Meetings tab + notifications

**Files:**
- Modify: `ios/VisioMobile/Views/SettingsView.swift`
- Create: `ios/VisioMobile/Views/MeetingsTabView.swift`
- Modify: `ios/VisioMobile/Views/HomeView.swift`
- Modify: `ios/VisioMobile/VisioManager.swift`

- [ ] **Step 1: Add calendar state to VisioManager.swift**

```swift
@Published var upcomingMeetings: [Meeting] = []
@Published var calendarUrl: String? = nil
```

Handle events:

```swift
case .meetingsUpdated(let meetings):
    self.upcomingMeetings = meetings
case .meetingImminent(let meeting):
    sendLocalNotification(meeting: meeting, subtitle: "Dans 15 min")
case .meetingStartingSoon(let meeting):
    sendLocalNotification(meeting: meeting, subtitle: "Dans 5 min")
case .meetingStarted(let meeting):
    sendLocalNotification(meeting: meeting, subtitle: "Commence maintenant")
case .calendarError(let message):
    print("Calendar error: \(message)")
```

- [ ] **Step 2: Add Calendar section to SettingsView.swift**

Add after Meet Instances section:

```swift
Section("Calendrier") {
    TextField("URL du calendrier (iCal)", text: $calendarUrl)
        .textContentType(.URL)
        .autocapitalization(.none)
        .onChange(of: calendarUrl) { _, newValue in
            VisioManager.shared.client.setCalendarUrl(url: newValue.isEmpty ? nil : newValue)
        }

    Picker("Fréquence de synchro", selection: $refreshInterval) {
        Text("5 min").tag("5m")
        Text("15 min").tag("15m")
        Text("1 heure").tag("1h")
        Text("4 heures").tag("4h")
        Text("Manuel").tag("manual")
    }

    if !calendarUrl.isEmpty {
        Button("Supprimer le calendrier", role: .destructive) {
            calendarUrl = ""
            VisioManager.shared.client.setCalendarUrl(url: nil)
        }
    }
}
```

- [ ] **Step 3: Create MeetingsTabView.swift**

Create the view with 4 states mirroring Android. Each meeting card navigates to join with pre-filled URL.

- [ ] **Step 4: Add tab segment to HomeView.swift**

```swift
@State private var selectedTab = 0

Picker("", selection: $selectedTab) {
    Text("Rejoindre").tag(0)
    Text("Réunions").tag(1)
}
.pickerStyle(.segmented)

if selectedTab == 0 {
    // existing join content
} else {
    MeetingsTabView(onJoin: { url in
        roomURL = url
        selectedTab = 0
    })
}
```

- [ ] **Step 5: Request notification permission**

When calendar URL is first set:

```swift
UNUserNotificationCenter.current().requestAuthorization(options: [.alert, .sound, .badge]) { granted, _ in
    // stored for later reference
}
```

- [ ] **Step 6: Verify iOS builds**

Run: `scripts/build-ios.sh sim`
Expected: compiles

- [ ] **Step 7: Commit**

```bash
git add ios/
git commit -m "feat(ios): add Calendar settings, Meetings tab, and local notifications"
```

---

## Task 10: Desktop — Settings + Meetings tab

**Files:**
- Modify: `crates/visio-desktop/frontend/src/App.tsx`

- [ ] **Step 1: Add calendar settings to Settings interface**

```typescript
interface Settings {
    // ... existing fields
    calendar_url: string | null;
    calendar_refresh_interval: string;
}
```

- [ ] **Step 2: Add Meeting type**

```typescript
interface Meeting {
    id: string;
    summary: string;
    start_time: number;
    end_time: number;
    room_url: string;
    deep_link: string;
    server_name: string;
}
```

- [ ] **Step 3: Add Tauri command handlers in Rust backend**

In `crates/visio-desktop/src-tauri/src/main.rs` (or the commands module), register new Tauri commands:

```rust
#[tauri::command]
fn set_calendar_url(state: tauri::State<AppState>, url: Option<String>) {
    state.client.set_calendar_url(url);
}

#[tauri::command]
fn get_calendar_url(state: tauri::State<AppState>) -> Option<String> {
    state.client.get_calendar_url()
}

#[tauri::command]
fn get_upcoming_meetings(state: tauri::State<AppState>) -> Vec<Meeting> {
    state.client.get_upcoming_meetings()
}

#[tauri::command]
fn refresh_calendar_now(state: tauri::State<AppState>) {
    state.client.refresh_calendar_now();
}
```

Register them in the Tauri builder `.invoke_handler(tauri::generate_handler![..., set_calendar_url, get_calendar_url, get_upcoming_meetings, refresh_calendar_now])`.

- [ ] **Step 4: Add TypeScript invoke wrappers in frontend**

```typescript
const setCalendarUrl = (url: string | null) => invoke("set_calendar_url", { url });
const getUpcomingMeetings = () => invoke<Meeting[]>("get_upcoming_meetings");
const refreshCalendar = () => invoke("refresh_calendar_now");
```

- [ ] **Step 5: Add tab UI in home view**

Add a tab selector (Rejoindre / Réunions) and the MeetingsTab component showing the 4 states.

- [ ] **Step 6: Add calendar fields to settings panel**

Input for calendar URL + select dropdown for refresh interval (enum values: Minutes5, Minutes15, Hour1, Hours4, Manual).

- [ ] **Step 7: Add desktop notifications**

Use `tauri-plugin-notification` for MeetingImminent/StartingSoon/Started events.

- [ ] **Step 8: Verify desktop builds**

Run: `cd crates/visio-desktop && cargo tauri build`
Expected: compiles

- [ ] **Step 9: Commit**

```bash
git add crates/visio-desktop/
git commit -m "feat(desktop): add Calendar settings, Meetings tab, and notifications"
```

---

## Task 11: CalendarService timer — background periodic refresh

**Files:**
- Modify: `crates/visio-core/src/calendar.rs`
- Modify: `crates/visio-ffi/src/lib.rs`

- [ ] **Step 1: Add `start_periodic_refresh` method to CalendarService**

```rust
impl CalendarService {
    pub fn start_periodic_refresh(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            loop {
                let interval = self.refresh_interval();
                if interval == std::time::Duration::ZERO {
                    // "manual" mode — just sleep long and re-check
                    tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                    continue;
                }
                tokio::time::sleep(interval).await;
                if let Err(e) = self.refresh().await {
                    warn!("Periodic calendar refresh failed: {e}");
                    self.emitter.emit(crate::events::VisioEvent::CalendarError(e));
                }
            }
        })
    }
}
```

- [ ] **Step 2: Start timer in VisioClient::new()**

In FFI `lib.rs`, after creating CalendarService:

```rust
let calendar = Arc::new(visio_core::CalendarService::new(
    settings.clone(),
    room_manager.emitter(),
    data_dir.clone(),
));

// Start periodic refresh if calendar is configured
if settings.get_calendar_url().is_some() {
    let cal = calendar.clone();
    rt.spawn(async move {
        // Initial refresh
        let _ = cal.refresh().await;
        // Then start periodic
        cal.start_periodic_refresh().await;
    });
}
```

- [ ] **Step 3: Restart timer when calendar URL changes**

In `set_calendar_url`, trigger a refresh if URL is set:

```rust
pub fn set_calendar_url(&self, url: Option<String>) {
    self.settings.set_calendar_url(url.clone());
    if url.is_some() {
        self.refresh_calendar_now();
    }
}
```

- [ ] **Step 4: Run full test suite**

Run: `cargo test -p visio-core`
Expected: all tests pass

- [ ] **Step 5: Commit**

```bash
git add crates/visio-core/src/calendar.rs crates/visio-ffi/src/lib.rs
git commit -m "feat(core): add periodic background refresh timer for CalendarService"
```

---

## Task 12: Integration test with real iCal data

**Files:**
- Modify: `crates/visio-core/src/calendar.rs` (test module)

- [ ] **Step 1: Add integration test with real iCal sample**

Add to the test module:

```rust
#[test]
fn test_parse_real_ical_sample() {
    let ical_text = "\
BEGIN:VCALENDAR\r\n\
VERSION:2.0\r\n\
CALSCALE:GREGORIAN\r\n\
PRODID:-//SabreDAV//SabreDAV 4.7.0//EN\r\n\
BEGIN:VEVENT\r\n\
UID:event1@example\r\n\
SUMMARY:COCO 2025\r\n\
DTSTART:20260325T093000Z\r\n\
DTEND:20260325T103000Z\r\n\
LOCATION:https://meet.linagora.com/tui-ytsh-uta\r\n\
END:VEVENT\r\n\
BEGIN:VEVENT\r\n\
UID:event2@example\r\n\
SUMMARY:Teams call\r\n\
DTSTART:20260325T110000Z\r\n\
DTEND:20260325T120000Z\r\n\
LOCATION:https://teams.microsoft.com/l/meetup-join/abc\r\n\
END:VEVENT\r\n\
BEGIN:VEVENT\r\n\
UID:event3@example\r\n\
SUMMARY:Point BC\r\n\
DTSTART:20260325T153000Z\r\n\
DTEND:20260325T163000Z\r\n\
DESCRIPTION:Rejoindre sur https://meet.linagora.com/qay-glwz-xxq\r\n\
END:VEVENT\r\n\
BEGIN:VEVENT\r\n\
UID:event4@example\r\n\
SUMMARY:Old meeting\r\n\
DTSTART:20250101T090000Z\r\n\
DTEND:20250101T100000Z\r\n\
LOCATION:https://meet.linagora.com/old-room\r\n\
END:VEVENT\r\n\
END:VCALENDAR";

    let servers = vec!["meet.linagora.com".to_string()];
    let now_ts = 1742900000; // 2025-03-25T08:00:00Z approx
    let cutoff_ts = now_ts + 86400;

    let reader = std::io::BufReader::new(ical_text.as_bytes());
    let parser = IcalParser::new(reader);
    let mut meetings = Vec::new();
    for calendar in parser.flatten() {
        for event in &calendar.events {
            if let Some(m) = parse_meeting_from_vevent(event, &servers, now_ts, cutoff_ts) {
                meetings.push(m);
            }
        }
    }

    // Should find 2 meetings (COCO + Point BC), exclude Teams + old
    assert_eq!(meetings.len(), 2);
    assert_eq!(meetings[0].summary, "COCO 2025");
    assert_eq!(meetings[0].server_name, "meet.linagora.com");
    assert_eq!(meetings[1].summary, "Point BC");
    assert!(meetings[1].room_url.contains("qay-glwz-xxq"));
}
```

- [ ] **Step 2: Run integration test**

Run: `cargo test -p visio-core -- calendar::tests::test_parse_real_ical_sample`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/visio-core/src/calendar.rs
git commit -m "test(core): add integration test with realistic multi-event iCal data"
```

---

## Task 13: Final build verification

- [ ] **Step 1: Run full Rust test suite**

Run: `cargo test -p visio-core`
Expected: all tests pass (existing 56 + new calendar tests)

- [ ] **Step 2: Build all platforms**

```bash
cargo build -p visio-core
cargo build -p visio-ffi
cd android && ./gradlew assembleDebug
scripts/build-ios.sh sim
cd crates/visio-desktop && cargo tauri build
```

- [ ] **Step 3: Regenerate UniFFI bindings**

Run: `scripts/generate-bindings.sh all`

- [ ] **Step 4: Commit any remaining fixes**

```bash
git add -A
git commit -m "chore: fix build issues from calendar integration"
```
