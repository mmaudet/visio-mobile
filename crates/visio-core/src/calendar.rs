//! CalendarService — iCal feed fetch, parse, and meeting extraction.
//!
//! Parses iCal/CalDAV feeds to extract upcoming meetings that contain
//! a Visio room URL in their LOCATION, DESCRIPTION, or URL fields.

use std::collections::HashSet;
use std::io::BufReader;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::{NaiveDateTime, TimeZone, Utc};
use regex::Regex;

use crate::events::{EventEmitter, Meeting, VisioEvent};
use crate::settings::{CalendarRefreshInterval, SettingsStore};

// ---------------------------------------------------------------------------
// Timestamp helpers
// ---------------------------------------------------------------------------

/// Parse an iCal timestamp in `YYYYMMDDTHHMMSSZ` (UTC) or `YYYYMMDDTHHMMSS`
/// format into a Unix timestamp (seconds since epoch).
///
/// Returns `None` if the string cannot be parsed.
pub fn parse_ical_timestamp(ts: &str) -> Option<i64> {
    // Strip trailing 'Z' if present — we always treat as UTC.
    let ts = ts.trim_end_matches('Z');
    // Accept both "YYYYMMDDTHHMMSS" and "YYYYMMDDTHHMMSS" variants.
    let fmt = if ts.contains('T') {
        "%Y%m%dT%H%M%S"
    } else {
        "%Y%m%d"
    };
    let ndt = NaiveDateTime::parse_from_str(ts, fmt)
        .or_else(|_| NaiveDateTime::parse_from_str(&format!("{}T000000", ts), "%Y%m%dT%H%M%S"))
        .ok()?;
    Some(Utc.from_utc_datetime(&ndt).timestamp())
}

// ---------------------------------------------------------------------------
// Meeting extraction from a single VEVENT
// ---------------------------------------------------------------------------

/// Generate a deterministic meeting ID from the UID and start_time.
/// Uses a simple format string instead of DefaultHasher (which is not stable across Rust versions).
fn meeting_id(uid: &str, start_time: i64) -> String {
    format!("{}-{}", uid, start_time)
}

/// Try to extract a video URL from a text blob.
///
/// Priority:
/// 1. `visio://` deep link.
/// 2. `https://{server}/{path}` for each pre-registered server domain.
///    URLs that have no path component (i.e. just `https://server/`) are
///    considered truncated and are **ignored**.
fn extract_video_link(text: &str, server_domains: &[String]) -> Option<(String, String, String)> {
    // 1. Deep link: visio://meet/<server>/<path>
    let deep_re = Regex::new(r#"visio://[^\s<>"']+"#).ok()?;
    if let Some(m) = deep_re.find(text) {
        let deep = m.as_str().to_string();
        // Parse server and path from visio://<server>/<path>
        let rest = deep.trim_start_matches("visio://");
        // Expect at least one '/' after the host part
        if let Some(slash_pos) = rest.find('/') {
            let server = rest[..slash_pos].to_string();
            let path = &rest[slash_pos + 1..];
            if !path.is_empty() {
                let room_url = format!("https://{}/{}", server, path);
                return Some((deep, room_url, server));
            }
        }
        return None;
    }

    // 2. HTTPS URL on a registered server
    for domain in server_domains {
        // Build a regex that matches https://<domain>/<path> where <path> is non-empty
        let pattern = format!(
            r#"https://{}(/[^\s<>"']*[^/\s<>"'?#])"#,
            regex::escape(domain)
        );
        let Ok(re) = Regex::new(&pattern) else {
            continue;
        };
        if let Some(caps) = re.captures(text) {
            let path = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            if path.len() > 1 {
                let room_url = format!("https://{}{}", domain, path);
                let deep_link = format!("visio://{}{}", domain, path);
                return Some((deep_link, room_url, domain.clone()));
            }
        }
    }

    None
}

/// Parse a single iCal event into a `Meeting`, or return `None` if the event
/// should be excluded (past, too far future, or no matching video URL).
///
/// - `server_domains`: list of known Meet server hostnames.
/// - `now_ts`: current Unix timestamp (seconds).
/// - `cutoff_ts`: upper bound; events starting after this are excluded.
pub fn parse_meeting_from_vevent(
    event: &ical::parser::ical::component::IcalEvent,
    server_domains: &[String],
    now_ts: i64,
    cutoff_ts: i64,
) -> Option<Meeting> {
    let get_prop = |name: &str| -> Option<String> {
        event
            .properties
            .iter()
            .find(|p| p.name == name)
            .and_then(|p| p.value.clone())
    };

    let dtstart_str = get_prop("DTSTART")?;
    let start_time = parse_ical_timestamp(&dtstart_str)?;
    // DURATION (e.g. PT1H30M) is not supported — assume 1h default if DTEND absent.
    // iCal exports from SabreDAV/TwCalendar always expand DTEND, so this is a safe fallback.
    let end_time = get_prop("DTEND")
        .and_then(|s| parse_ical_timestamp(&s))
        .unwrap_or(start_time + 3600);

    // Exclude events that ended before now or start after the cutoff.
    // In-progress meetings (DTSTART < now < DTEND) are kept.
    if end_time < now_ts || start_time > cutoff_ts {
        return None;
    }

    let uid = get_prop("UID").unwrap_or_else(|| dtstart_str.clone());
    let summary = get_prop("SUMMARY").unwrap_or_default();

    // Search LOCATION, DESCRIPTION, URL in priority order
    let search_fields = [
        get_prop("LOCATION"),
        get_prop("DESCRIPTION"),
        get_prop("URL"),
    ];

    let (deep_link, room_url, server_name) = search_fields
        .iter()
        .filter_map(|f| f.as_deref())
        .find_map(|text| extract_video_link(text, server_domains))?;

    let id = meeting_id(&uid, start_time);

    Some(Meeting {
        id,
        summary,
        start_time,
        end_time,
        room_url,
        deep_link,
        server_name,
    })
}

// ---------------------------------------------------------------------------
// CalendarService
// ---------------------------------------------------------------------------

/// Loads and persists the meeting cache to disk.
fn cache_path(data_dir: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(data_dir).join("calendar_cache.json")
}

fn load_cache(data_dir: &str) -> Vec<Meeting> {
    let path = cache_path(data_dir);
    match std::fs::read_to_string(&path) {
        Ok(contents) => serde_json::from_str(&contents).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

fn save_cache(data_dir: &str, meetings: &[Meeting]) {
    let path = cache_path(data_dir);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string(meetings) {
        let _ = std::fs::write(&path, json);
    }
}

/// Service that manages calendar feed fetching, parsing, and notification.
#[derive(Clone)]
pub struct CalendarService {
    settings: Arc<SettingsStore>,
    meetings: Arc<Mutex<Vec<Meeting>>>,
    emitter: EventEmitter,
    /// Tracks meeting IDs for which an imminence notification has already
    /// been fired, so notifications are not repeated across refresh cycles.
    /// Used by the notification logic in the timer task (Task 4).
    #[allow(dead_code)]
    notified_ids: Arc<Mutex<HashSet<String>>>,
    data_dir: String,
    http_client: reqwest::Client,
}

impl CalendarService {
    /// Create a new `CalendarService`, restoring the meeting list from the
    /// on-disk cache if available.
    pub fn new(settings: Arc<SettingsStore>, emitter: EventEmitter, data_dir: String) -> Self {
        let cached = load_cache(&data_dir);
        Self {
            settings,
            meetings: Arc::new(Mutex::new(cached)),
            emitter,
            notified_ids: Arc::new(Mutex::new(HashSet::new())),
            data_dir,
            http_client: reqwest::Client::new(),
        }
    }

    /// Clear all cached meetings from memory and disk.
    pub fn clear_cache(&self) {
        self.meetings
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
        self.notified_ids
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
        let path = std::path::PathBuf::from(&self.data_dir).join("calendar_cache.json");
        let _ = std::fs::remove_file(path);
        self.emitter.emit(VisioEvent::MeetingsUpdated(Vec::new()));
    }

    /// Return cached meetings, filtering out those that have already ended.
    pub fn get_meetings(&self) -> Vec<Meeting> {
        let now_ts = Utc::now().timestamp();
        self.meetings
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .filter(|m| m.end_time >= now_ts)
            .cloned()
            .collect()
    }

    /// Check the current meeting list and emit imminence notification events.
    ///
    /// Should be called periodically (e.g. every 60 s) by a background timer.
    /// Each notification is emitted at most once per meeting ID.
    pub fn check_notifications(&self) {
        let now_ts = Utc::now().timestamp();
        let meetings = {
            self.meetings
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone()
        };
        let mut notified = self.notified_ids.lock().unwrap_or_else(|e| e.into_inner());

        for meeting in &meetings {
            if notified.contains(&meeting.id) {
                continue;
            }
            let delta = meeting.start_time - now_ts;
            let event = if delta <= 0 && delta > -60 {
                Some(VisioEvent::MeetingStarted(meeting.clone()))
            } else if delta <= 300 && delta > 0 {
                Some(VisioEvent::MeetingStartingSoon(meeting.clone()))
            } else if delta <= 900 && delta > 300 {
                Some(VisioEvent::MeetingImminent(meeting.clone()))
            } else {
                None
            };
            if let Some(ev) = event {
                notified.insert(meeting.id.clone());
                drop(notified);
                self.emitter.emit(ev);
                notified = self.notified_ids.lock().unwrap_or_else(|e| e.into_inner());
            }
        }
    }

    /// Convert the configured refresh interval to a `Duration`.
    ///
    /// Returns `Duration::ZERO` for `Manual` (no automatic refresh).
    pub fn refresh_interval(&self) -> Duration {
        match self.settings.get_calendar_refresh_interval() {
            CalendarRefreshInterval::Minutes5 => Duration::from_secs(300),
            CalendarRefreshInterval::Minutes15 => Duration::from_secs(900),
            CalendarRefreshInterval::Hour1 => Duration::from_secs(3_600),
            CalendarRefreshInterval::Hours4 => Duration::from_secs(14_400),
            CalendarRefreshInterval::Manual => Duration::ZERO,
        }
    }

    /// Fetch the iCal feed, parse it, and update the meeting list.
    ///
    /// Uses an adaptive time window: tries 24 h first; if no meetings are
    /// found it widens to 3 days, then 7 days.
    ///
    /// Emits `MeetingsUpdated` on success or `CalendarError` on failure.
    pub async fn refresh(&self) {
        let url = match self.settings.get_calendar_url() {
            Some(u) => u,
            None => {
                tracing::debug!("calendar: no URL configured, skipping refresh");
                return;
            }
        };

        let now_ts = Utc::now().timestamp();
        let server_domains = self.settings.get_meet_instances();

        tracing::info!("calendar: fetching {}", url);

        let body = match self.http_client.get(&url).send().await {
            Ok(resp) => match resp.text().await {
                Ok(t) => t,
                Err(e) => {
                    let msg = format!("calendar: response read error: {e}");
                    tracing::warn!("{}", msg);
                    self.emitter.emit(VisioEvent::CalendarError(msg));
                    return;
                }
            },
            Err(e) => {
                let msg = format!("calendar: fetch error: {e}");
                tracing::warn!("{}", msg);
                self.emitter.emit(VisioEvent::CalendarError(msg));
                return;
            }
        };

        // Adaptive window: 24 h → 3 days → 7 days.
        let windows_secs: &[i64] = &[24 * 3600, 3 * 24 * 3600, 7 * 24 * 3600];
        let mut meetings = Vec::new();
        for &window in windows_secs {
            let cutoff_ts = now_ts + window;
            meetings = self.parse_ical(&body, &server_domains, now_ts, cutoff_ts);
            if !meetings.is_empty() {
                tracing::debug!(
                    "calendar: found {} meeting(s) in {}h window",
                    meetings.len(),
                    window / 3600
                );
                break;
            }
        }

        {
            let mut guard = self.meetings.lock().unwrap_or_else(|e| e.into_inner());
            *guard = meetings.clone();
        }
        save_cache(&self.data_dir, &meetings);
        self.check_notifications();
        self.emitter.emit(VisioEvent::MeetingsUpdated(meetings));
    }

    /// Spawn a background task that refreshes the calendar feed periodically.
    ///
    /// The interval is read from settings on each iteration so changes take
    /// effect without restarting the task.  When the interval is
    /// `Duration::ZERO` (i.e. `Manual` mode) the loop sleeps for 60 s before
    /// re-checking, avoiding a busy-spin.
    pub fn start_periodic_refresh(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            loop {
                let interval = self.refresh_interval();
                if interval == std::time::Duration::ZERO {
                    // "Manual" mode — sleep long and re-check.
                    tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                    continue;
                }
                // Check notifications every 60s within the refresh interval
                let mut elapsed = std::time::Duration::ZERO;
                let check_period = std::time::Duration::from_secs(60);
                while elapsed < interval {
                    let sleep_for = check_period.min(interval - elapsed);
                    tokio::time::sleep(sleep_for).await;
                    elapsed += sleep_for;
                    // Check for imminent meetings using cached list
                    self.check_notifications();
                }
                self.refresh().await;
            }
        })
    }

    /// Parse an iCal string and return filtered meetings.
    pub fn parse_ical(
        &self,
        ical_text: &str,
        server_domains: &[String],
        now_ts: i64,
        cutoff_ts: i64,
    ) -> Vec<Meeting> {
        let reader = BufReader::new(ical_text.as_bytes());
        let parser = ical::IcalParser::new(reader);
        let mut meetings = Vec::new();

        for calendar_result in parser {
            match calendar_result {
                Err(e) => {
                    tracing::warn!("calendar: parse error: {:?}", e);
                }
                Ok(calendar) => {
                    for event in &calendar.events {
                        if let Some(m) =
                            parse_meeting_from_vevent(event, server_domains, now_ts, cutoff_ts)
                        {
                            meetings.push(m);
                        }
                    }
                }
            }
        }

        meetings.sort_by_key(|m| m.start_time);
        meetings
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: build a minimal VEVENT string and parse it into an IcalEvent.
    fn make_event(props: &[(&str, &str)]) -> ical::parser::ical::component::IcalEvent {
        use ical::property::Property;
        let mut event = ical::parser::ical::component::IcalEvent::new();
        for (name, value) in props {
            let mut p = Property::new();
            p.name = name.to_string();
            p.value = Some(value.to_string());
            event.properties.push(p);
        }
        event
    }

    const NOW: i64 = 1_800_000_000; // arbitrary fixed "now"
    const SOON: i64 = NOW + 3600; // 1 hour from now
    const IN_3H: i64 = NOW + 3 * 3600;
    const FAR_FUTURE: i64 = NOW + 8 * 24 * 3600; // beyond 7-day cutoff
    const CUTOFF: i64 = NOW + 7 * 24 * 3600;

    fn ts(unix: i64) -> String {
        let dt = Utc.timestamp_opt(unix, 0).unwrap();
        dt.format("%Y%m%dT%H%M%SZ").to_string()
    }

    fn default_servers() -> Vec<String> {
        vec![
            "meet.linagora.com".to_string(),
            "meet.numerique.gouv.fr".to_string(),
        ]
    }

    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_meeting_with_matching_server() {
        let event = make_event(&[
            ("UID", "uid-001"),
            ("DTSTART", &ts(SOON)),
            ("DTEND", &ts(IN_3H)),
            ("SUMMARY", "Weekly sync"),
            ("LOCATION", "https://meet.linagora.com/my-room"),
        ]);

        let m = parse_meeting_from_vevent(&event, &default_servers(), NOW, CUTOFF).unwrap();
        assert_eq!(m.summary, "Weekly sync");
        assert_eq!(m.room_url, "https://meet.linagora.com/my-room");
        assert_eq!(m.deep_link, "visio://meet.linagora.com/my-room");
        assert_eq!(m.server_name, "meet.linagora.com");
        assert_eq!(m.start_time, SOON);
        assert_eq!(m.end_time, IN_3H);
    }

    #[test]
    fn test_parse_meeting_no_matching_server() {
        let event = make_event(&[
            ("UID", "uid-002"),
            ("DTSTART", &ts(SOON)),
            ("DTEND", &ts(IN_3H)),
            ("SUMMARY", "External call"),
            ("LOCATION", "https://zoom.us/j/123456"),
        ]);

        let result = parse_meeting_from_vevent(&event, &default_servers(), NOW, CUTOFF);
        assert!(result.is_none(), "zoom URL should not match our servers");
    }

    #[test]
    fn test_parse_meeting_past_event_excluded() {
        let past_start = NOW - 7200; // 2 hours ago
        let past_end = NOW - 3600; // 1 hour ago (already ended)

        let event = make_event(&[
            ("UID", "uid-003"),
            ("DTSTART", &ts(past_start)),
            ("DTEND", &ts(past_end)),
            ("SUMMARY", "Past meeting"),
            ("LOCATION", "https://meet.linagora.com/past-room"),
        ]);

        let result = parse_meeting_from_vevent(&event, &default_servers(), NOW, CUTOFF);
        assert!(result.is_none(), "ended events should be excluded");
    }

    #[test]
    fn test_parse_meeting_visio_deep_link() {
        let event = make_event(&[
            ("UID", "uid-004"),
            ("DTSTART", &ts(SOON)),
            ("DTEND", &ts(IN_3H)),
            ("SUMMARY", "Deep link meeting"),
            ("LOCATION", "visio://meet.linagora.com/deep-room"),
        ]);

        let m = parse_meeting_from_vevent(&event, &default_servers(), NOW, CUTOFF).unwrap();
        assert_eq!(m.deep_link, "visio://meet.linagora.com/deep-room");
        assert_eq!(m.room_url, "https://meet.linagora.com/deep-room");
        assert_eq!(m.server_name, "meet.linagora.com");
    }

    #[test]
    fn test_parse_meeting_link_in_description() {
        let event = make_event(&[
            ("UID", "uid-005"),
            ("DTSTART", &ts(SOON)),
            ("DTEND", &ts(IN_3H)),
            ("SUMMARY", "Meeting with description link"),
            ("LOCATION", "Conference room B"),
            (
                "DESCRIPTION",
                "Join at https://meet.numerique.gouv.fr/projet-x\n\nSee you there!",
            ),
        ]);

        let m = parse_meeting_from_vevent(&event, &default_servers(), NOW, CUTOFF).unwrap();
        assert_eq!(m.room_url, "https://meet.numerique.gouv.fr/projet-x");
        assert_eq!(m.deep_link, "visio://meet.numerique.gouv.fr/projet-x");
    }

    #[test]
    fn test_parse_meeting_truncated_link_ignored() {
        // URL has no path beyond the server root — should be ignored
        let event = make_event(&[
            ("UID", "uid-006"),
            ("DTSTART", &ts(SOON)),
            ("DTEND", &ts(IN_3H)),
            ("SUMMARY", "Truncated link meeting"),
            ("LOCATION", "https://meet.linagora.com/"),
        ]);

        let result = parse_meeting_from_vevent(&event, &default_servers(), NOW, CUTOFF);
        assert!(result.is_none(), "truncated links should be ignored");
    }

    #[test]
    fn test_parse_meeting_far_future_excluded() {
        let event = make_event(&[
            ("UID", "uid-007"),
            ("DTSTART", &ts(FAR_FUTURE)),
            ("DTEND", &ts(FAR_FUTURE + 3600)),
            ("SUMMARY", "Far future"),
            ("LOCATION", "https://meet.linagora.com/far-room"),
        ]);

        let result = parse_meeting_from_vevent(&event, &default_servers(), NOW, CUTOFF);
        assert!(result.is_none(), "events beyond cutoff should be excluded");
    }

    #[test]
    fn test_parse_meeting_in_progress_included() {
        // DTSTART is before now, DTEND is after now → in-progress, should be included
        let started = NOW - 1800; // started 30 min ago
        let ends = NOW + 1800; // ends in 30 min

        let event = make_event(&[
            ("UID", "uid-008"),
            ("DTSTART", &ts(started)),
            ("DTEND", &ts(ends)),
            ("SUMMARY", "Ongoing meeting"),
            ("LOCATION", "https://meet.linagora.com/ongoing-room"),
        ]);

        let m = parse_meeting_from_vevent(&event, &default_servers(), NOW, CUTOFF).unwrap();
        assert_eq!(m.summary, "Ongoing meeting");
    }

    #[test]
    fn test_parse_ical_timestamp_utc() {
        let ts = parse_ical_timestamp("20260321T100000Z").unwrap();
        let dt = Utc.timestamp_opt(ts, 0).unwrap();
        assert_eq!(
            dt.format("%Y-%m-%d %H:%M:%S").to_string(),
            "2026-03-21 10:00:00"
        );
    }

    #[test]
    fn test_parse_ical_timestamp_no_z() {
        let ts = parse_ical_timestamp("20260321T100000").unwrap();
        let dt = Utc.timestamp_opt(ts, 0).unwrap();
        assert_eq!(
            dt.format("%Y-%m-%d %H:%M:%S").to_string(),
            "2026-03-21 10:00:00"
        );
    }

    #[test]
    fn test_calendar_service_new_loads_cache() {
        let dir = tempfile::tempdir().unwrap();
        let data_dir = dir.path().to_str().unwrap().to_string();

        // Pre-populate cache with a future meeting so it is not filtered out.
        let future_ts = Utc::now().timestamp() + 3600;
        let meetings = vec![Meeting {
            id: "abc".to_string(),
            summary: "cached".to_string(),
            start_time: future_ts,
            end_time: future_ts + 3600,
            room_url: "https://meet.linagora.com/room".to_string(),
            deep_link: "visio://meet.linagora.com/room".to_string(),
            server_name: "meet.linagora.com".to_string(),
        }];
        save_cache(&data_dir, &meetings);

        let store = Arc::new(SettingsStore::new(&data_dir));
        let emitter = EventEmitter::new();
        let svc = CalendarService::new(store, emitter, data_dir);

        let loaded = svc.get_meetings();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, "abc");
    }

    // -----------------------------------------------------------------------
    // Adaptive window tests
    // -----------------------------------------------------------------------

    /// A meeting starting in 2 hours falls within the 24 h window and is found
    /// on the first attempt.
    #[test]
    fn test_adaptive_window_returns_24h_if_meetings_exist() {
        let now = NOW;
        // Meeting starts in 2 hours — well within 24 h.
        let start = now + 2 * 3600;
        let end = start + 3600;
        let event = make_event(&[
            ("UID", "uid-adaptive-1"),
            ("DTSTART", &ts(start)),
            ("DTEND", &ts(end)),
            ("SUMMARY", "24h meeting"),
            ("LOCATION", "https://meet.linagora.com/adaptive-room"),
        ]);

        let cutoff_24h = now + 24 * 3600;
        let m = parse_meeting_from_vevent(&event, &default_servers(), now, cutoff_24h);
        assert!(m.is_some(), "meeting within 24h should be found");
    }

    /// A meeting starting in 2 days is NOT found within the 24 h window but IS
    /// found within the 3-day window.
    #[test]
    fn test_adaptive_window_empty_24h_finds_in_3d() {
        let now = NOW;
        // Meeting starts in 2 days — beyond 24 h.
        let start = now + 2 * 24 * 3600;
        let end = start + 3600;
        let event = make_event(&[
            ("UID", "uid-adaptive-2"),
            ("DTSTART", &ts(start)),
            ("DTEND", &ts(end)),
            ("SUMMARY", "2-day out meeting"),
            ("LOCATION", "https://meet.linagora.com/future-room"),
        ]);

        let cutoff_24h = now + 24 * 3600;
        let not_found = parse_meeting_from_vevent(&event, &default_servers(), now, cutoff_24h);
        assert!(not_found.is_none(), "should not appear in 24h window");

        let cutoff_3d = now + 3 * 24 * 3600;
        let found = parse_meeting_from_vevent(&event, &default_servers(), now, cutoff_3d);
        assert!(found.is_some(), "should be found in 3-day window");
    }

    // -----------------------------------------------------------------------
    // check_notifications tests
    // -----------------------------------------------------------------------

    struct EventCapture {
        events: Arc<Mutex<Vec<VisioEvent>>>,
    }

    impl crate::events::VisioEventListener for EventCapture {
        fn on_event(&self, event: VisioEvent) {
            self.events.lock().unwrap().push(event);
        }
    }

    fn make_service_with_meetings(
        meetings: Vec<Meeting>,
    ) -> (CalendarService, Arc<Mutex<Vec<VisioEvent>>>) {
        let dir = tempfile::tempdir().unwrap();
        let data_dir = dir.path().to_str().unwrap().to_string();
        // Keep TempDir alive by leaking — acceptable in tests.
        std::mem::forget(dir);

        let store = Arc::new(SettingsStore::new(&data_dir));
        let emitter = EventEmitter::new();
        let captured: Arc<Mutex<Vec<VisioEvent>>> = Arc::new(Mutex::new(Vec::new()));
        emitter.add_listener(Arc::new(EventCapture {
            events: captured.clone(),
        }));
        let svc = CalendarService::new(store, emitter, data_dir);
        {
            let mut guard = svc.meetings.lock().unwrap();
            *guard = meetings;
        }
        (svc, captured)
    }

    /// A meeting starting in 10 minutes (within the 15–5 min imminent window)
    /// should emit `MeetingImminent`.
    #[test]
    fn test_check_notifications_imminent() {
        let now_ts = Utc::now().timestamp();
        let start = now_ts + 10 * 60; // 10 minutes from now
        let meeting = Meeting {
            id: "notif-001".to_string(),
            summary: "Imminent meeting".to_string(),
            start_time: start,
            end_time: start + 3600,
            room_url: "https://meet.linagora.com/notif-room".to_string(),
            deep_link: "visio://meet.linagora.com/notif-room".to_string(),
            server_name: "meet.linagora.com".to_string(),
        };

        let (svc, captured) = make_service_with_meetings(vec![meeting.clone()]);
        svc.check_notifications();

        let events = captured.lock().unwrap();
        assert_eq!(events.len(), 1, "expected exactly one notification event");
        match &events[0] {
            VisioEvent::MeetingImminent(m) => assert_eq!(m.id, "notif-001"),
            other => panic!("expected MeetingImminent, got {:?}", other),
        }
    }

    /// Calling `check_notifications` twice for the same meeting must not emit
    /// duplicate events.
    #[test]
    fn test_check_notifications_no_duplicate() {
        let now_ts = Utc::now().timestamp();
        let start = now_ts + 10 * 60;
        let meeting = Meeting {
            id: "notif-002".to_string(),
            summary: "No dup".to_string(),
            start_time: start,
            end_time: start + 3600,
            room_url: "https://meet.linagora.com/notif-room2".to_string(),
            deep_link: "visio://meet.linagora.com/notif-room2".to_string(),
            server_name: "meet.linagora.com".to_string(),
        };

        let (svc, captured) = make_service_with_meetings(vec![meeting]);
        svc.check_notifications();
        svc.check_notifications(); // second call — must not re-emit

        let events = captured.lock().unwrap();
        assert_eq!(events.len(), 1, "notification should be emitted only once");
    }

    // -----------------------------------------------------------------------
    // get_meetings filtering
    // -----------------------------------------------------------------------

    // -----------------------------------------------------------------------
    // Integration test with realistic multi-event iCal data
    // -----------------------------------------------------------------------

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
        let now_ts = 1774425600_i64; // 2026-03-25T08:00Z
        let cutoff_ts = now_ts + 86400;

        let reader = BufReader::new(ical_text.as_bytes());
        let parser = ical::IcalParser::new(reader);
        let mut meetings = Vec::new();

        for calendar_result in parser {
            if let Ok(calendar) = calendar_result {
                for event in &calendar.events {
                    if let Some(m) = parse_meeting_from_vevent(event, &servers, now_ts, cutoff_ts) {
                        meetings.push(m);
                    }
                }
            }
        }
        meetings.sort_by_key(|m| m.start_time);

        // Should find 2 meetings (COCO 2025 + Point BC).
        // Teams call is excluded (wrong server) and Old meeting is excluded (past).
        assert_eq!(meetings.len(), 2);
        assert_eq!(meetings[0].summary, "COCO 2025");
        assert_eq!(meetings[0].server_name, "meet.linagora.com");
        assert_eq!(meetings[1].summary, "Point BC");
        assert!(meetings[1].room_url.contains("qay-glwz-xxq"));
    }

    /// Past meetings (end_time < now) must be excluded from get_meetings().
    #[test]
    fn test_get_meetings_filters_past() {
        let now_ts = Utc::now().timestamp();
        let past = Meeting {
            id: "past-001".to_string(),
            summary: "Past".to_string(),
            start_time: now_ts - 7200,
            end_time: now_ts - 3600, // ended 1 hour ago
            room_url: "https://meet.linagora.com/past".to_string(),
            deep_link: "visio://meet.linagora.com/past".to_string(),
            server_name: "meet.linagora.com".to_string(),
        };
        let future = Meeting {
            id: "future-001".to_string(),
            summary: "Future".to_string(),
            start_time: now_ts + 3600,
            end_time: now_ts + 7200,
            room_url: "https://meet.linagora.com/future".to_string(),
            deep_link: "visio://meet.linagora.com/future".to_string(),
            server_name: "meet.linagora.com".to_string(),
        };

        let (svc, _) = make_service_with_meetings(vec![past, future]);
        let meetings = svc.get_meetings();

        assert_eq!(meetings.len(), 1, "only future meeting should be returned");
        assert_eq!(meetings[0].id, "future-001");
    }
}
