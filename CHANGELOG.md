# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0),
and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Default grid layout with pin-to-view for speakers
  and shared screens on all platforms (#142)
- Room display name via `?room-display-name=` URL param
  or manual input (#113)
- Calendar sync feedback: toast/snackbar on all platforms
  after sync success or error (#121)

### Changed

- Convert desktop settings from modal to full-page view (#80)
- Reduce all functions to cognitive complexity <=15 (#103)
- Reduce cognitive complexity across all platforms (#94)
- Resolve all 224 SonarCloud maintainability issues
  across all platforms (#95, #96)

### Changed

- Move room display name field from join tab to create room dialog (#137)

### Fixed

- Desktop: room creator no longer stuck on
  "waiting for authorization" for public rooms (#140)
- Align settings toggles by splitting adaptive mode label onto two lines (#135)
- Show hours and minutes in planned meeting countdown (#136)
- Red pulsing dot and "En cours" label for
  in-progress meetings instead of negative countdown (#136)
- Preserve Bluetooth audio device selection from lobby to room (#138)
- Keep meetings visible during manual refresh (#123)
- macOS camera/mic permission prompts via infoPlist (#124)
- Timezone-aware iCal parsing with TZID support (#122)
- Widen desktop meetings list to 640 px (#122)
- Live countdown updates every 60 s on all platforms (#122)
- Imminent meeting badge turns red on all platforms (#122)
- Desktop notification on meeting-reminder event (#122)
- Keep planned meetings during transient sync failures (#126)
- Redesign desktop dark theme colors and contrast (#120)
- Assert light theme is default on all platforms (#119)
- Bluetooth device shows real name instead of generic "Bluetooth" (#118)
- Release Bluetooth SCO audio channel on disconnect (#118)
- Deduplicate Bluetooth entries in audio device selectors (#118)
- Apply background blur/image in pre-join lobby camera preview (#111)
- Fix video freeze after surface destruction and orientation distortion (#100)
- Fix 12 SonarCloud reliability issues in desktop frontend (#92)
- Lobby camera/mic/Bluetooth settings now applied when joining room (#98)
- Auto-fallback to default audio device when Bluetooth disconnects (#83)
- Migrate all hardcoded UI strings to i18n system (Android, iOS, Desktop) (#89)
- Fix adaptive mode defaulting to enabled on Desktop (#89)
- Pre-lobby camera preview, audio routing and device selectors (#87)

### Added

- Pre-join lobby screen with live camera preview,
  audio device selection, VU meter, speaker test,
  background filters, and waiting room
  (Desktop, iOS, Android)
- `audio_mode` setting for computer audio / no audio
- Blur-light mode now correctly mapped in FFI layer
- iCal calendar integration — upcoming meetings on home screen (#73)
- E2E test framework with multi-platform orchestration
- Bot interactive mode with stdin/stdout protocol
- Semantic test tags for layout assertions
- 4 test suites, 11 scenarios
- HTML/JSON/terminal reporting with screenshots

### Fixed

- Bot interactive mode now uses --media-file audio
- UIAutomator dump uses temp file for device compat
- Deep link URL escaping no longer corrupts params
- Desktop home tabs fixed at top, no vertical shift
- Desktop tabs styled as segmented control
