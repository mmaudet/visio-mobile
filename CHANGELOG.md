# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0),
and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
