# Visio Mobile

Native video conferencing client for [La Suite Meet](https://meet.numerique.gouv.fr), built on the [LiveKit Rust SDK](https://github.com/livekit/rust-sdks).

> **Status: active development (pre-release)**
> Core functionality works end-to-end on all three platforms. Not yet packaged for distribution.

## Platforms

| Platform | UI toolkit | Min version |
|----------|-----------|-------------|
| **Android** | Kotlin + Jetpack Compose | SDK 26 (Android 8) |
| **iOS** | Swift + SwiftUI | iOS 16 |
| **Desktop** | Tauri 2.x + React | macOS 12 / Linux / Windows |

## Architecture

```
┌──────────────┐  ┌─────────────┐  ┌──────────────┐
│   Android    │  │     iOS     │  │   Desktop    │
│  Compose UI  │  │  SwiftUI    │  │ Tauri + React│
└──────┬───────┘  └──────┬──────┘  └──────┬───────┘
       │ UniFFI          │ UniFFI         │ Tauri cmds
       ▼                 ▼                ▼
┌──────────────────────────────────────────────────┐
│                  visio-ffi                       │
│        UniFFI bindings + C FFI (video/audio)     │
├──────────────────────────────────────────────────┤
│                  visio-core                      │
│   RoomManager · AuthService · ChatService        │
│   MeetingControls · ParticipantManager           │
├──────────────────────────────────────────────────┤
│                  visio-video                     │
│   I420 renderer registry · platform renderers    │
├──────────────────────────────────────────────────┤
│            LiveKit Rust SDK (0.7.32)             │
└──────────────────────────────────────────────────┘
```

**4 Rust crates:**

- **`visio-core`** — Room lifecycle, auth (Meet API token fetch), chat (Stream API `lk.chat`), participants, media controls, hand raise (Meet interop), settings, event system
- **`visio-video`** — Video frame rendering: I420 decode, renderer registry, platform-specific renderers
- **`visio-ffi`** — UniFFI `.udl` bindings (control plane) + raw C FFI (video/audio zero-copy)
- **`visio-desktop`** — Tauri 2.x commands + cpal audio + AVFoundation camera capture (macOS)

**Key design decisions:**
- UniFFI for structured control plane (connect, toggle mic, send chat)
- Raw C FFI for video/audio (zero-copy I420 to native surfaces, PCM audio pull)
- No WebView for calls — fully native rendering on each platform
- Guest-first: no auth required, join via Meet URL

## Prerequisites

- **Rust** nightly (edition 2024) — `rustup default nightly`
- **Android**: NDK 27+, SDK 26+, `cargo-ndk`, `rustup target add aarch64-linux-android`
- **iOS**: Xcode 16+, `rustup target add aarch64-apple-ios aarch64-apple-ios-sim`
- **Desktop**: Node.js 18+, Tauri CLI (`cargo install tauri-cli`)

## Building

### Desktop (macOS / Linux / Windows)

```bash
# Dev mode — start Vite and Tauri separately:
cd crates/visio-desktop/frontend && npm install && npm run dev &
cd crates/visio-desktop && cargo tauri dev

# Production build
cd crates/visio-desktop/frontend && npm run build
cd crates/visio-desktop && cargo tauri build
```

The frontend dev server (Vite) must be running on `http://localhost:5173` **before** `cargo tauri dev` — Tauri's `devUrl` points to it. If Vite is not running, the window will be blank. Make sure no other Vite instance (e.g. from a git worktree) is occupying port 5173.

### Android

```bash
# 1. Build Rust libraries for arm64
bash scripts/build-android.sh

# 2. Build APK (i18n JSON files are auto-copied to assets/ by Gradle)
cd android && ./gradlew assembleDebug

# 3. Install on device/emulator
adb install app/build/outputs/apk/debug/app-debug.apk
```

The Gradle `copyI18nAssets` task runs automatically before build, copying `i18n/*.json` into `src/main/assets/i18n/`.

### iOS

```bash
# 1. Build Rust libraries
bash scripts/build-ios.sh sim      # for simulator (aarch64-apple-ios-sim)
bash scripts/build-ios.sh device   # for physical device (aarch64-apple-ios)

# 2. Open and run in Xcode
open ios/VisioMobile.xcodeproj
```

The Xcode "Copy i18n JSON" build phase copies `i18n/*.json` into the app bundle automatically. Select your target device in Xcode and hit Run.

## Internationalization (i18n)

The app supports **6 languages**: English, French, German, Spanish, Italian, and Dutch.

Translations are stored as shared JSON files in the `i18n/` directory at the project root. Each platform loads these files at startup — there is a single source of truth for all strings across Desktop, Android, and iOS.

```
i18n/
  en.json    # English (reference — 92 keys)
  fr.json    # Français
  de.json    # Deutsch
  es.json    # Español
  it.json    # Italiano
  nl.json    # Nederlands
```

**Adding a new language:** Create a new `i18n/<code>.json` file with all 92 keys translated. Then add the language code to `SUPPORTED_LANGS` (Desktop `App.tsx`), `supportedLangs` (Android `Strings.kt`, iOS `Strings.swift`).

**Adding a new key:** Add the key to all 6 JSON files. Use `t("key")` (Desktop), `Strings.t("key", lang)` (Android/iOS) in the UI code.

**Platform integration:**
- **Desktop** — Static JSON imports in `App.tsx`, bundled by Vite at build time
- **Android** — Gradle `copyI18nAssets` task copies JSON to `assets/i18n/` before build, loaded via `Strings.init(context)` in `VisioApplication`
- **iOS** — Xcode "Copy i18n JSON" build phase copies JSON into the app bundle, loaded via `Strings.initialize()` in `VisioMobileApp.init()`

## Running tests

```bash
cargo test -p visio-core
```

## Project structure

```
i18n/               Shared translation JSON files (6 languages)
crates/
  visio-core/       Shared Rust core (room, auth, chat, controls, settings)
  visio-video/      Video rendering (I420, renderer registry)
  visio-ffi/        UniFFI bindings + C FFI (video/audio)
  visio-desktop/    Tauri app (commands, cpal audio, camera)
android/            Kotlin/Compose app
ios/                SwiftUI app
scripts/            Build scripts (Android NDK, iOS fat libs)
docs/plans/         Design docs and implementation plans
```

## What works

**Core:**
- Join a La Suite Meet room via URL (guest mode)
- Real-time room URL validation with debounce (checks Meet API before joining)
- Bidirectional audio (mic + speaker) on all platforms
- Bidirectional video (camera + remote video) on Android and desktop
- iOS: video reception works, camera capture pipeline ready (tested via test pattern, needs physical device)
- Chat (bidirectional with Meet via LiveKit Stream API `lk.chat` topic)
- Participant list with connection quality indicators
- Hand raise with Meet interop (uses `handRaisedAt` attribute, auto-lower after 3s speaking)
- Persistent settings (display name, language, theme, mic/camera on join)
- i18n: 6 languages (EN, FR, DE, ES, IT, NL) with shared JSON files

**Desktop UX (Meet-inspired):**
- Dark/light theme toggle (Meet palette: `#161622` base)
- Remixicon icon set across all controls
- Grouped control bar: mic+chevron, cam+chevron, hand raise, chat, participants, tools, info, hangup
- Device picker popovers (mic/speaker/camera enumeration via WebRTC API)
- Adaptive video grid (1x1 to 3x3) + click-to-focus layout with filmstrip
- Participant tiles with initials avatar (deterministic color), active speaker glow, hand raise badge, connection quality bars
- Chat sidebar (358px, slide-in animation, own messages right-aligned in accent color)
- Participants sidebar with live count
- Info panel with meeting URL copy
- Settings modal (display name, language, theme, join preferences)

**Android UX:**
- Material 3 dark/light theme with Meet color palette
- Remixicon SVG vector drawables
- Room URL validation with real-time status feedback
- Participant list bottom sheet
- Edge-to-edge display support

## What's next

- iOS UX overhaul (Phase 4 — SwiftUI controls, tiles, chat, CallKit, PiP)
- Android UX overhaul (Phase 3 — control bar, tiles, chat, PiP)
- Physical device testing (iOS camera, Android edge cases)
- Push notifications
- ProConnect authentication
- App store packaging (APK/IPA/DMG)

## Configuration

The app connects to any La Suite Meet instance. By default, URLs point to placeholder values (`meet.example.com`). Update the Meet URL at runtime in the app's home screen.

## License

[AGPL-3.0](LICENSE)
