# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0),
and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.9.0] - 2026-04-07

### Added

- Core: smart video subscription management with anti-jitter
  delays for bandwidth optimization (#198)
- Core: screen share tracks always subscribed at HIGH
  quality (#202)
- FFI: subscription stats for monitoring (#198)
- Core: client-side AES-256-GCM chat message encryption with
  HKDF key derivation from room token (#197)
- Core: FeatureService with Unleash proxy client and compiled-in
  defaults for runtime feature flags (#192)
- Core: runtime OIDC auth toggle via feature flags (#192)
- FFI: is_feature_enabled() and set_feature_flags_url() (#192)
- CI: translation key verification workflow with check-i18n.sh
  script (#194)
- Core: `prepare_connection()` pre-check validates server
  reachability before connecting (#193)
- Core: phased initialization with progress callbacks (#195)
- Core: token cache for faster reconnection (#193)
- FFI: `newWithListener` constructor with init progress (#195)
- Core: LayoutEngine with voice-activity participant sorting
  and 10s anti-flicker window (#201)
- Core: paginated layout with set_page_size/set_current_page
  and precache for adjacent pages (#201)
- Core: speaker mode with dominant speaker, 3s anti-flicker,
  pin override, and thumbnail strip (#201)
- FFI: LayoutEngine API exposed via UniFFI (10 new methods,
  4 new events) (#201)
- Android: paginated video grid with HorizontalPager and
  adaptive page size (#199)
- Android: speaker mode with dominant speaker tile and
  thumbnail strip (#200)
- iOS: paginated video grid with TabView swipeable pages (#199)
- iOS: speaker mode with main tile and thumbnail strip (#200)
- Desktop: voice-activity sorted grid and speaker mode with
  Tauri commands (#199, #200)
- i18n: layout mode translation keys for all 6 locales

### Fixed

- Security: derive chat encryption key from room name instead
  of LiveKit URL to ensure per-room key isolation (#197)
- Security: use VC1: prefix for encrypted messages to prevent
  false positive detection on plain text (#197)
- Android: restart AudioTrack/AudioRecord on Bluetooth device
  change for reliable routing mid-call (#203)
- Android: debounce Bluetooth routing with SCO confirmation
  to prevent premature audio switching (#203)
- iOS: handle A2DP to HFP Bluetooth transition gap (#203)
- Core: re-sync participants after reconnection (#193)
- Core: reduce reconnection timeout from 60s to 20s (#193)

- Security: remove sensitive data (tokens, cookies) from API logs
- Security: add XSS sanitization to chat messages
- Security: reject dangerous URL schemes (javascript:, data:, file:)
- Security: update rustls-webpki to 0.103.10 (RUSTSEC-2026-0049)
- Desktop: prevent meet instances list from being overwritten when
  adding a new instance (#169)
- Desktop: hide optional room name field on home page when
  authenticated via OIDC (#171)
- Desktop: fix race condition causing "waiting for authorization"
  on public rooms (#180)
- Desktop: remove room name field from join form (#180)
- Desktop: fix camera not showing on join from lobby (#180)
- iOS: restore iOS 16 compatibility by migrating all onChange(of:) calls
  to the iOS 16-compatible perform: closure form, adding missing
  requestNotificationPermissionIfNeeded() and Equatable conformance
  on TestConnectParams (#185)
- iOS: return to home screen immediately after hanging up a call,
  instead of landing on the pre-join setup page
- iOS: opening a visio:// deep link now navigates directly to the
  pre-join setup page instead of just filling in the URL field

### Added

- Friendly URLs: alias names (e.g. "COMEX") can be used as shortcuts
  to visio rooms on all platforms (#156)
- Simplified URL shown after room creation when a display name is
  provided (e.g. `visio://server/COMEX`) (#156)
- Alias resolution in deep links and URL validation on Android, iOS,
  and Desktop (#156)
- Button to clear recent visios history in settings (#152)
- AGENTS.md with build, test, lint, and code style guide for all
  platforms to assist agentic coding tools (#183)

### Changed

- CI: iOS build now uses Xcode 26.3 (previously defaulted to 16.4)
- iOS: replace segmented picker on home screen with native bottom tab bar
- iOS: simplify pre-join screen — camera controls overlaid on preview,
  audio settings collapsed by default
- Extract `useDeviceEnumeration` hook to unify device
  enumeration across lobby and in-call picker (#163)
- Rename `RoomHistoryEntry` to `VisioHistoryEntry` across core, FFI,
  and all platforms for consistency (#156)

### Fixed

- Lobby mic/camera overrides now correctly applied when
  joining a visio on Desktop (#172)
- Desktop: record visio in recent history when
  connecting via `connect_with_token` path (#173)
- Regenerate UniFFI bindings and fix build errors for
  friendly URLs feature (#156)
- PiP only activates during an active call; no longer triggers
  when backgrounding from home or other non-call screens (#154)
- Stop spurious microphone permission request when
  opening desktop settings (#161)
- Clarify display name labels on home and pre-join
  screens with "Your" prefix in all 6 locales (#153)
- Rename "meeting"/"room" to "visio" in all 6 i18n files
  and remove obsolete home subtitle (#150)
- Only show calendar sync toast when meetings actually
  change, add pull-to-refresh on meetings tab (#151)
- Move clear-history button from footer into an inline
  settings row on desktop (#168)

## [0.7.0] - 2026-03-23

### Added

- Default grid layout with pin-to-view for speakers
  and shared screens on all platforms (#142)
- Room display name via `?visio=` URL param
  or manual input (#113, #145)
- Calendar sync feedback: toast/snackbar on all platforms
  after sync success or error (#121)
- Bandwidth degradation banner on all platforms when
  connection quality drops (#144)
- "Video paused" placeholder in participant tiles when
  video is disabled due to poor bandwidth (#144)
- Pre-join lobby screen with live camera preview,
  audio device selection, VU meter, speaker test,
  background filters, and waiting room
  (Desktop, iOS, Android)
- `audio_mode` setting for computer audio / no audio
- iCal calendar integration — upcoming meetings
  on home screen (#73)
- E2E test framework with multi-platform orchestration,
  bot interactive mode, semantic test tags,
  4 test suites / 11 scenarios,
  HTML/JSON/terminal reporting with screenshots

### Changed

- Convert desktop settings from modal to full-page view (#80)
- Reduce all functions to cognitive complexity <=15 (#103)
- Reduce cognitive complexity across all platforms (#94)
- Resolve all 224 SonarCloud maintainability issues
  across all platforms (#95, #96)
- Move room display name field from join tab
  to create room dialog (#137)

### Fixed

- Always send slug (not display name) to Meet server API,
  rename URL param to `?visio=`, add display name to
  iOS/Desktop create room dialog (#145)
- Desktop: room creator no longer stuck on
  "waiting for authorization" for public rooms (#140)
- Align settings toggles by splitting adaptive mode label
  onto two lines (#135)
- Show hours and minutes in planned meeting countdown (#136)
- Red pulsing dot and "En cours" label for
  in-progress meetings instead of negative countdown (#136)
- Preserve Bluetooth audio device selection from lobby
  to room (#138)
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
- Bluetooth device shows real name instead of
  generic "Bluetooth" (#118)
- Release Bluetooth SCO audio channel on disconnect (#118)
- Deduplicate Bluetooth entries in audio device
  selectors (#118)
- Apply background blur/image in pre-join lobby
  camera preview (#111)
- Fix video freeze after surface destruction
  and orientation distortion (#100)
- Fix 12 SonarCloud reliability issues in desktop
  frontend (#92)
- Lobby camera/mic/Bluetooth settings now applied
  when joining room (#98)
- Auto-fallback to default audio device when Bluetooth
  disconnects (#83)
- Migrate all hardcoded UI strings to i18n system
  (Android, iOS, Desktop) (#89)
- Fix adaptive mode defaulting to enabled on Desktop (#89)
- Pre-lobby camera preview, audio routing and device
  selectors (#87)
- Bot interactive mode now uses --media-file audio
- UIAutomator dump uses temp file for device compat
- Deep link URL escaping no longer corrupts params
- Desktop home tabs fixed at top, no vertical shift
- Desktop tabs styled as segmented control

## [0.6.0] - 2026-03-20

### Added

- Secure OIDC login with one-time exchange codes
  on all platforms — replaces cookie extraction
- Expose `exchange_oidc_code()` to mobile via UniFFI
- PipeWire camera portal with V4L2 fallback on Linux
- Flatpak packaging for desktop (GNOME SDK sandbox)
- Linux camera capture and WebKitGTK fixes
- Cover/fill scaling for camera video feeds
  on iOS and Android

### Changed

- iOS strict concurrency checking enabled (Xcode 26)
- Migrate all Swift files to strict concurrency

### Fixed

- Force clean OIDC session with `prompt=login`
  on all platforms
- Desktop uses system browser for OIDC instead
  of Tauri webview
- Desktop 720p camera preset to match NativeVideoSource
- Desktop audio device enumeration and deduplication
- Populate room history on successful connection (#53)
- iOS transient error banner flash on room join
- iOS infinite recursion in effectiveAdaptiveMode
- iOS blurred local preview instead of raw camera feed
- Self-echoed reactions filtered by participant SID
- Cross-platform video fixes (VP8 codec, green flash,
  camera reconnect)
- Linux PipeWire 0.9 API compatibility
- Meet instances sync between settings and OIDC
  server selector
- iOS scrollable home page with collapsing header
- iOS Bluetooth crash when sample rate below 48 kHz
- iOS live theme switching in settings sheet

## [0.5.0] - 2026-03-16

### Added

- Active speaker auto-focus on iOS and Android
- Real microphone capture via AVAudioEngine on iOS
- Room history on iOS and Desktop
- Direct join from room history on all platforms
- OIDC login via system browser on iOS and Android
- Screen share display with focus/thumbnail layout
  on all platforms
- Screen capture via xcap on desktop
- Bandwidth controller with hysteresis logic
- Adaptive context modes: Office, Pedestrian, Car
  with auto-detection and manual override
- Desktop audio engines with AEC
  (macOS VoiceProcessingIO, Windows WASAPI,
  Linux PulseAudio)
- Desktop screen share picker with thumbnails
- Desktop native audio/video device enumeration
- Unified app icons across iOS, Android, and Desktop
- VP9 codec for camera tracks
- Audio device hot-swap detection on macOS
- Fastlane metadata for App Store and Google Play
- E2E bot with real audio/video from media files
- visio-bot E2E test participant binary

### Changed

- Differentiate disconnect reasons — no reconnect
  for kick/duplicate
- Extend connect timeout to 60 s and retries to 5
- Share HTTPS links instead of deep links
  for browser compatibility

### Fixed

- Android Bluetooth auto-switch, fullscreen screen share,
  green frame flash, OIDC WebView auth
- iOS mic toggle crash — AudioCapture.start()
  moved off main thread
- Desktop white screen share and audio output
  device selection
- Desktop auto-enable mic/camera on join
- Desktop Windows audio (WASAPI types,
  polling rewrite, crate pinning)
- Desktop Linux libgbm and Windows audio build errors
- Windows CRT mismatch with libwebrtc (load-dynamic)
- iOS audio lifecycle, video race, URL parsing, rotation
- Desktop screen share fullscreen and capture logging
- Android audio zombie, phone call detection,
  BT preservation
- Android pending surface for screen share tracks
- Desktop memory leak and Android screen share stability
- Per-track audio mixing instead of concatenation
- iOS camera/mic permissions requested before use

## [0.4.0] - 2026-03-08

### Added

- Restricted rooms with user invite and members
  management on all platforms
- Waiting room / lobby flow with host approval UI
  on all platforms
- Room creation dialog with auto-join
  on all platforms
- Room info tab with share links (HTTPS and visio://)
- Animated reactions via LiveKit data channels
  on all platforms
- Background blur with ONNX-based selfie segmentation
- Background mode picker in settings
  (Android, iOS, Desktop)
- OIDC authentication with server picker
  on all platforms
- Desktop OIDC auth flow with system browser
- VoiceOver accessibility labels on iOS call controls
- Adaptive mode enable/disable toggle in settings
- Desktop audio output selection and routing

### Changed

- Redesigned link display with share buttons
  on all platforms

### Fixed

- Persistent lobby banner, iOS OIDC WKWebView,
  iOS participant dedup
- Android reload meet instances on room validation
- Background image orientation on rotated sensors
- Launcher icons aligned with homepage logo
- Camera not stopping on chat navigation (Android)
- ONNX segmentation model loaded at startup
- Room slug resolution tries all configured servers
- Desktop audio resampling with linear interpolation
- Android synchronized audio/camera capture lifecycle
- Android cancelled CoroutineScope on disconnect
- Android crash on room join
