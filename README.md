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
┌─────────────┐  ┌─────────────┐  ┌──────────────┐
│   Android    │  │     iOS     │  │   Desktop    │
│  Compose UI  │  │  SwiftUI    │  │  Tauri + React│
└──────┬───────┘  └──────┬──────┘  └──────┬───────┘
       │ UniFFI          │ UniFFI         │ Tauri cmds
       ▼                 ▼                ▼
┌──────────────────────────────────────────────────┐
│                  visio-ffi                        │
│        UniFFI bindings + C FFI (video/audio)     │
├──────────────────────────────────────────────────┤
│                  visio-core                       │
│   RoomManager · AuthService · ChatService        │
│   MeetingControls · ParticipantManager           │
├──────────────────────────────────────────────────┤
│                  visio-video                      │
│   I420 renderer registry · platform renderers    │
├──────────────────────────────────────────────────┤
│            LiveKit Rust SDK (0.7.32)             │
└──────────────────────────────────────────────────┘
```

**4 Rust crates:**

- **`visio-core`** — Room lifecycle, auth (Meet API token fetch), chat (Stream API `lk.chat`), participants, media controls, settings
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

### Desktop (macOS)

```bash
cd crates/visio-desktop
cargo tauri dev
```

### Android

```bash
# Build Rust libraries
bash scripts/build-android.sh

# Open in Android Studio and run
cd android && ./gradlew assembleDebug
```

### iOS

```bash
# Build Rust libraries (device or sim)
bash scripts/build-ios.sh sim    # or: bash scripts/build-ios.sh device

# Open in Xcode and run
open ios/VisioMobile.xcodeproj
```

## Running tests

```bash
cargo test -p visio-core
```

## Project structure

```
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

- Join a La Suite Meet room via URL (guest mode)
- Bidirectional audio (mic + speaker) on all platforms
- Bidirectional video (camera + remote video) on Android and desktop
- iOS: video reception works, camera capture pipeline ready (tested via test pattern, needs physical device)
- Chat (bidirectional with Meet via LiveKit Stream API)
- Participant list with connection quality indicators
- Hand raise with Meet interop
- Persistent settings (display name, language, mic/camera on join)

## What's next

- Physical device testing (iOS camera, Android edge cases)
- Push notifications
- ProConnect authentication
- App store packaging (APK/IPA/DMG)

## Configuration

The app connects to any La Suite Meet instance. By default, URLs point to placeholder values (`meet.example.com`). Update the Meet URL at runtime in the app's home screen.

## License

[AGPL-3.0](LICENSE)
