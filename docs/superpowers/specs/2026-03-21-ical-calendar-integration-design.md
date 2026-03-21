# iCal Calendar Integration — Design Spec

**Issue:** [#73 — feat: iCal calendar integration — upcoming meetings on home screen](https://github.com/mmaudet/visio-mobile/issues/73)
**Date:** 2026-03-21
**Status:** Draft

## Overview

Allow OIDC-authenticated users to configure an iCal calendar URL in their settings. The app periodically fetches the calendar, extracts meetings containing video conference links matching the user's pre-registered servers or `visio://` protocol, and displays them on the home screen in a dedicated "Réunions" tab. Tapping a meeting pre-fills the join screen and routes through the pre-join lobby.

## Feasibility Study

Validated against a production calendar (TwCalendar / SabreDAV 4.7.0):

- **Format**: iCalendar RFC 5545 via tokenized URL — simple HTTP GET, no complex auth
- **Size**: 16 MB, 9,269 events — requires streaming parse (not full load into memory)
- **Detection**: video links reliably found in LOCATION and DESCRIPTION fields
- **Recurring events**: already expanded in iCal export — no RRULE processing needed client-side
- **Filtering**: matching only user's pre-registered servers effectively excludes Teams, Google Meet, WebEx, Zoom

Test results (3-day window, 4 pre-registered servers): 4 joinable meetings correctly identified out of 9,269 events.

## Prerequisites

- User must be authenticated via OIDC

## UX Design

### Home Screen — Tab Navigation

The home screen gains a segment control with two tabs:

- **"Rejoindre"** — the existing join screen (URL/slug input, server picker, display name, history). Unchanged.
- **"Réunions"** — the new meetings list. Shows a badge with the count of upcoming meetings.

When a meeting starts within 15 minutes, a red pulsing dot appears on the "Réunions" tab, visible even when the user is on the "Rejoindre" tab.

The "Réunions" tab is hidden when the user is not authenticated via OIDC.

### Meetings Tab — 4 States

#### State 1: Onboarding (no calendar URL configured)

- Calendar emoji illustration
- Text: "Connectez votre agenda — Retrouvez vos prochaines visioconférences ici en ajoutant l'URL de votre calendrier iCal."
- Button: "Configurer le calendrier" → navigates to Settings > Calendar section

#### State 2: Loading (first sync or refresh in progress)

- Spinner with "Synchronisation du calendrier..."
- Shown briefly, transitions to state 3 or 4

#### State 3: No meetings found

- Sun emoji illustration
- Text: "Aucune réunion à venir — Aucune visioconférence trouvée dans les 7 prochains jours sur vos serveurs."
- Last sync timestamp
- "Rafraîchir" button for manual refresh

#### State 4: Meetings list

Meetings displayed as tappable cards, grouped by day ("Aujourd'hui", "Demain", "Mer 25/03", etc.).

Each card contains:

- **Meeting subject** (from iCal SUMMARY)
- **Time indicator**:
  - < 4 hours: relative ("Dans 45 min", "Dans 2h15")
  - Today but > 4 hours: time only ("16:30")
  - Other day: abbreviated day + time ("Mar 09:00")
- **Server name** (e.g. "meet.linagora.com")
- **"Rejoindre" button**

Visual cues:

- **Imminent (< 15 min)**: card with purple gradient background + red pulsing dot
- **In progress** (DTSTART passed, DTEND not reached): card with "En cours" label instead of relative time, displayed first
- **Normal**: standard card with subtle background

Last sync timestamp shown at the bottom of the list.

### Tap Action

Tapping a meeting card:

1. Switches to the "Rejoindre" tab
2. Pre-fills the room URL and selects the correct server instance
3. User proceeds through the pre-join lobby (issue #55) before connecting

This ensures the user can verify their camera/mic settings before joining.

### Settings — Calendar Section

New section in the Settings screen, placed between "Instances Meet" and "Thème" (since meet instances are used for calendar filtering).

Fields:

- **URL du calendrier (iCal)** — text input for the calendar URL (https:// or webcal://)
- **Fréquence de synchronisation** — dropdown:
  - Toutes les 5 minutes
  - Toutes les 15 minutes (recommended default)
  - Toutes les heures
  - Toutes les 4 heures
  - Manuel uniquement
- **Sync status** — last sync timestamp + meeting count + manual "Synchro" button
- **Notifications** — status line showing "Activées" or "Désactivées — activer dans les réglages système" with link to system settings if denied
- **Supprimer le calendrier** — red text link, clears URL + cache + hides "Réunions" tab

## Notifications

Local notifications (no push server needed) triggered by the CalendarService timer.

### Notification Types

| Trigger | Timing | Content | Action |
|---|---|---|---|
| `MeetingImminent` | 15 min before | "COCO 2025 dans 15 min" | Open app → pre-fill join |
| `MeetingStartingSoon` | 5 min before | "COCO 2025 dans 5 min" | Open app → pre-fill join |
| `MeetingStarted` | At DTSTART | "COCO 2025 commence maintenant" | Open app → pre-fill join |

Each notification is emitted once per meeting (tracked by meeting ID).

### Permission Flow

Permission is requested when the user configures their first iCal URL in Settings (not at app launch).

- **iOS**: `UNUserNotificationCenter.requestAuthorization(options: [.alert, .sound, .badge])` — system dialog
- **Android**: API 33+ requires `POST_NOTIFICATIONS` runtime permission → system dialog. Below API 33, no permission needed.
- **Desktop**: `tauri-plugin-notification` handles permission per OS (macOS asks, Linux/Windows do not)

### If Permission Denied

- Notifications silently disabled
- The "Réunions" tab and in-app urgency indicators (badge, pulsing dot) continue working normally
- Settings > Calendar shows "Notifications: Désactivées — activer dans les réglages système"

## Architecture

### Data Model

```rust
pub struct Meeting {
    pub id: String,           // hash of iCal UID + DTSTART
    pub summary: String,      // VEVENT SUMMARY
    pub start_time: i64,      // UTC timestamp (seconds)
    pub end_time: i64,        // UTC timestamp (seconds)
    pub room_url: String,     // https://meet.linagora.com/abc-defg-hij
    pub deep_link: String,    // visio://meet.linagora.com/abc-defg-hij
    pub server_name: String,  // meet.linagora.com
}

pub enum CalendarRefreshInterval {
    Minutes5,
    Minutes15,
    Hour1,
    Hours4,
    Manual,
}
```

### CalendarService (visio-core, new module `calendar.rs`)

- Owns a `reqwest::Client` for HTTP fetching
- Parses iCal using the `ical` crate in streaming mode (iterator over VEVENTs, not full load — critical for 16 MB+ calendars)
- Filters: keeps only VEVENTs where DTSTART is in the future AND containing an HTTPS link matching a domain from `meet_instances` or a `visio://` link
- Detection scans DESCRIPTION, LOCATION, and URL fields
- Multiple visio links in one event: keeps the first matching a pre-registered server
- Truncated links (e.g. `https://meet.linagora.`) are discarded — must match `https://{server}/{slug}` with valid slug
- Adaptive time window: searches 24h first, extends to 3 days if empty, then 7 days
- Stores results in `Arc<Mutex<Vec<Meeting>>>`
- Local cache: writes meetings to `{data_dir}/calendar_cache.json` (survives app restart)
- Timer: `tokio::time::interval` based on `CalendarRefreshInterval`
- Imminent meeting tracking: emits `MeetingImminent` / `MeetingStartingSoon` / `MeetingStarted` once per meeting ID

When an `https://` link is found matching a known server, the service generates the equivalent `visio://` deep link (e.g. `https://meet.linagora.com/abc-defg-hij` → `visio://meet.linagora.com/abc-defg-hij`).

### Refresh Strategy

- **Periodic**: tokio timer based on user-selected interval
- **Event-driven**: `refresh_calendar_now()` called by UI on:
  - App returns to foreground
  - User switches to "Réunions" tab (if last refresh > 1 minute ago)
- **Manual**: "Rafraîchir" / "Synchro" buttons in the UI

### Events (added to VisioEvent enum)

```
MeetingsUpdated(Vec<Meeting>)     // on each successful refresh
MeetingImminent(Meeting)          // 15 min before start
MeetingStartingSoon(Meeting)      // 5 min before start
MeetingStarted(Meeting)           // at DTSTART
CalendarError(String)             // fetch/parse failure
```

### UniFFI Interface (additions to visio.udl)

New record and enum:

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

enum CalendarRefreshInterval {
    "Minutes5",
    "Minutes15",
    "Hour1",
    "Hours4",
    "Manual",
};
```

New methods on VisioClient:

```
void set_calendar_url(string? url);
string? get_calendar_url();
void set_calendar_refresh_interval(CalendarRefreshInterval interval);
CalendarRefreshInterval get_calendar_refresh_interval();
sequence<Meeting> get_upcoming_meetings();
void refresh_calendar_now();
```

New events added to VisioEvent callback.

### Integration Points

- `CalendarService` is created in `VisioClient`, alongside `RoomManager` (not inside it — independent lifecycle)
- Settings extended with `calendar_url: Option<String>` and `calendar_refresh_interval: CalendarRefreshInterval` in the existing `Settings` struct and `settings.json` persistence
- `get_meet_instances()` provides the domain list for filtering (no duplication)

## Platform UI Implementation

### Android (Jetpack Compose)

- `HomeScreen.kt`: add `TabRow` with "Rejoindre" / "Réunions" segment control
- New `MeetingsTab.kt` composable: observes meetings from `VisioManager`, renders the 4 states
- `SettingsScreen.kt`: add Calendar section
- Notifications: `NotificationManager` + dedicated `NotificationChannel` "Réunions"
- Permission: `ActivityResultContracts.RequestPermission` for `POST_NOTIFICATIONS` (API 33+)

### iOS (SwiftUI)

- `HomeView.swift`: add `Picker` with `.segmented` style for tab switching
- New `MeetingsTabView.swift`: observes meetings from `VisioManager`, renders the 4 states
- `SettingsView.swift`: add Calendar section in the Form
- Notifications: `UNUserNotificationCenter` with `.alert, .sound, .badge`
- Permission: requested via `requestAuthorization` when first calendar URL is set

### Desktop (Tauri + React)

- `App.tsx`: add tab component in home view
- New `MeetingsTab` React component: fetches meetings via Tauri invoke
- Settings panel: add Calendar section
- Notifications: `tauri-plugin-notification`

## Edge Cases

| Case | Behavior |
|---|---|
| Invalid URL / HTTP error | `CalendarError` event → error message in Settings + toast on Réunions tab |
| Network unavailable | Show local cache with "Hors ligne — données du [date]" |
| Empty calendar (all windows) | Adaptive: 24h → 3j → 7j. Still empty → state 3 "Aucune réunion" |
| User not authenticated | "Réunions" tab hidden entirely |
| Meeting in progress | Card shown first with "En cours" label |
| Truncated link | Discarded — must match full `https://{server}/{slug}` pattern |
| Multiple visio links in one event | Keep first matching a pre-registered server |
| Calendar deletion | Clear URL + cache + hide "Réunions" tab |
| Notification permission denied | In-app indicators still work. Settings shows "Désactivées" with system settings link |

## Out of Scope (v1)

- CalDAV protocol support (requires Basic auth + PROPFIND/REPORT — future v2)
- Multiple calendar sources (single iCal URL)
- Calendar write-back (creating events)
- Joining non-Visio meetings (Teams, Google Meet, Zoom, etc.)
