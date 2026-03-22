# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0),
and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed

- Fix 12 SonarCloud reliability issues in desktop frontend (#92)
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
