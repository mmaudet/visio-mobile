<p align="center">
  <img src="docs/screenshots/visio-mobile-banner.png" alt="Visio Mobile" maxWidth="100%">
</p>

<p align="center">
  <a href="https://github.com/mmaudet/visio-mobile/stargazers/">
    <img src="https://img.shields.io/github/stars/mmaudet/visio-mobile" alt="">
  </a>
  <a href='http://makeapullrequest.com'><img alt='PRs Welcome' src='https://img.shields.io/badge/PRs-welcome-brightgreen.svg?style=shields'/></a>
  <img alt="GitHub commit activity" src="https://img.shields.io/github/commit-activity/m/mmaudet/visio-mobile"/>
  <img alt="GitHub closed issues" src="https://img.shields.io/github/issues-closed/mmaudet/visio-mobile"/>
  <a href="https://github.com/mmaudet/visio-mobile/blob/main/LICENSE">
    <img alt="License" src="https://img.shields.io/github/license/mmaudet/visio-mobile"/>
  </a>
</p>

<p align="center">
  <a href="https://livekit.io/">LiveKit</a> - <a href="https://github.com/mmaudet/visio-mobile/blob/main/CHANGELOG.md">Changelog</a> - <a href="https://github.com/mmaudet/visio-mobile/issues/new?labels=bug">Bug reports</a> - <a href="https://github.com/mmaudet/visio-mobile/issues">Roadmap</a>
</p>

## Visio Mobile: Native Video Conferencing

Native video conferencing client for [La Suite Meet](https://github.com/suitenumerique/meet), powered by the [LiveKit Rust SDK](https://github.com/livekit/rust-sdks). Available on Android, iOS, and Desktop.

> **Status: Beta** — Core functionality works end-to-end on all three platforms. Currently in closed beta on Android (Firebase) and iOS (TestFlight).
>
> **Want to join the beta?** Contact [mmaudet@linagora.com](mailto:mmaudet@linagora.com).

## Screenshots

<p align="center">
  <img src="docs/screenshots/ios_home.png" alt="iOS — Home screen" width="300" />
  &nbsp;&nbsp;&nbsp;&nbsp;
  <img src="docs/screenshots/android_call.png" alt="Android — Video call with 2 participants" width="300" />
</p>
<p align="center">
  <em>Left: iOS — Home screen &nbsp;|&nbsp; Right: Android — Active video call</em>
</p>

### Features

- OIDC / ProConnect authentication with persistent sessions
- Room creation with 3 access levels (public, trusted, restricted)
- Waiting room / lobby management (admit/deny participants)
- Bidirectional audio and video on all platforms
- Real-time chat via LiveKit Stream API (`lk.chat`)
- On-device background blur and replacement (ONNX Runtime, no data leaves device)
- Animated emoji reactions via data channels
- Adaptive context modes (Office, Pedestrian, Car) with auto audio routing
- Bluetooth audio auto-routing with smart fallback on disconnect
- Hand raise with Meet interop
- Participant list with connection quality indicators
- Deep links: `visio://host/slug` on all platforms
- 6 languages (EN, FR, DE, ES, IT, NL)
- Picture-in-Picture (Android + iOS)
- CallKit integration (iOS)
- Dark/light theme

## Table of Contents

- [Platforms](#platforms)
- [Get started](#get-started)
- [Architecture](#architecture)
- [Building](#building)
- [Authentication (OIDC)](#authentication-oidc--proconnect)
- [Internationalization](#internationalization-i18n)
- [Deep Links](#deep-links)
- [Running tests](#running-tests)
- [Contributing](#contributing)
- [Philosophy](#philosophy)
- [Open source](#open-source)

## Platforms

| Platform | UI toolkit | Min version |
|----------|-----------|-------------|
| **Android** | Kotlin + Jetpack Compose | SDK 26 (Android 8) |
| **iOS** | Swift + SwiftUI | iOS 16 |
| **Desktop** | Tauri 2.x + React | macOS 12 / Linux / Windows |

## Get started

Connect to any [La Suite Meet](https://github.com/suitenumerique/meet) instance. The default instance `meet.numerique.gouv.fr` is pre-configured. Add your own servers in Settings.

## Architecture

```
+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+
|   Android    |      iOS     |   Desktop    |
|  Compose UI  |   SwiftUI    | Tauri + React|
+------+-------+------+-------+------+-------+
       | UniFFI        | UniFFI       | Tauri cmds
       v               v              v
+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+
|                  visio-ffi                       |
|        UniFFI bindings + C FFI (video/audio)     |
+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+
|                  visio-core                      |
|   RoomManager . AuthService . ChatService        |
|   MeetingControls . ParticipantManager           |
|   HandRaiseManager . SettingsStore               |
|   LobbyService . AccessService . SessionManager  |
|   AdaptiveEngine (Office/Pedestrian/Car modes)   |
+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+
|                  visio-video                     |
|   I420 renderer registry . platform renderers    |
+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+
|            LiveKit Rust SDK (0.7.32)             |
+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+
```

**4 Rust crates:**

- **`visio-core`** — Room lifecycle, auth (Meet API + OIDC session), chat, participants, media controls, hand raise, active speaker tracking, persistent settings, event system, lobby/waiting room, room access management, reactions, adaptive context engine
- **`visio-video`** — Video frame rendering: I420 decode, renderer registry, platform-specific renderers
- **`visio-ffi`** — UniFFI `.udl` bindings (control plane) + raw C FFI (video/audio zero-copy) + on-device background blur/replacement (ONNX Runtime selfie segmentation)
- **`visio-desktop`** — Tauri 2.x commands + cpal audio + AVFoundation camera capture (macOS)

**Key design decisions:**
- UniFFI for structured control plane (connect, toggle mic, send chat)
- Raw C FFI for video/audio (zero-copy I420 to native surfaces, PCM audio pull)
- No WebView for calls — fully native rendering on each platform
- Guest-first: no auth required, join via Meet URL
- Edge-first AI: background blur/replacement runs on-device via ONNX Runtime

## Building

### Prerequisites

- **Rust** nightly (edition 2024) — `rustup default nightly`
- Platform-specific requirements listed per section below

### Desktop (macOS / Linux / Windows)

**Prerequisites:** Node.js 18+, Tauri CLI (`cargo install tauri-cli@^2`)

**Linux only:** Install system dependencies:
```bash
# Debian/Ubuntu
sudo apt-get install libgtk-3-dev libwebkit2gtk-4.1-dev librsvg2-dev \
  libasound2-dev libpipewire-0.3-dev libclang-dev libgbm-dev

# Fedora
sudo dnf install gtk3-devel webkit2gtk4.1-devel librsvg2-devel alsa-lib-devel
```

```bash
# Install frontend dependencies (first time only)
cd crates/visio-desktop/frontend && npm install

# Dev mode
cd crates/visio-desktop && cargo tauri dev

# Production build
cd crates/visio-desktop && cargo tauri build
```

### Android

**Prerequisites:** NDK 27+, SDK 26+, `cargo-ndk` (`cargo install cargo-ndk`), `rustup target add aarch64-linux-android`

```bash
# 1. Build Rust libraries for arm64
bash scripts/build-android.sh

# 2. Build APK
cd android && ./gradlew assembleDebug

# 3. Install on device/emulator
adb install app/build/outputs/apk/debug/app-debug.apk
```

### iOS

**Prerequisites:** Xcode 16+, `rustup target add aarch64-apple-ios aarch64-apple-ios-sim`

```bash
# 1. Build Rust libraries
bash scripts/build-ios.sh sim      # for simulator
bash scripts/build-ios.sh device   # for physical device

# 2. Open and run in Xcode
open ios/VisioMobile.xcodeproj
```

## Authentication (OIDC / ProConnect)

Visio Mobile supports authentication via **OpenID Connect** (OIDC), compatible with ProConnect and any OIDC provider configured on the Meet server. Authentication is optional — anonymous users can still join public rooms via URL.

<p align="center">
  <img src="docs/screenshots/01_Screenshot_Home_with_IODC.png" alt="Home screen with OIDC login button" width="220" />
  &nbsp;
  <img src="docs/screenshots/02_Screenshot_OIDC_Connect.png" alt="Authenticated user with avatar and logout" width="220" />
  &nbsp;
  <img src="docs/screenshots/03_Screenshot_New_Trusted_Room.png" alt="Room creation dialog with access levels" width="220" />
</p>

### Room access levels

| Level | Description | Authentication required |
|-------|-------------|------------------------|
| **Public** | Anyone with the link can join immediately | No |
| **Trusted** | Authenticated users join directly; anonymous users enter waiting room | Host: Yes |
| **Restricted** | Invitation only — only explicitly added members can join | Yes (all) |

## Background blur and replacement

On-device background processing powered by **ONNX Runtime selfie segmentation**. All processing runs locally — no frames are sent to any server.

- **Background blur** — Gaussian blur applied to the background
- **Background replacement** — Replace with one of 8 built-in images

## Adaptive context modes

The app automatically adapts the interface based on physical context:

| Mode | Trigger | UI | Audio |
|------|---------|-----|-------|
| **Office** | Default | Full video grid | Phone speaker/mic |
| **Pedestrian** | Walking detected | Single active speaker | Bluetooth headset |
| **Car** | Car Bluetooth detected | Audio-only, large buttons | Car Bluetooth |

## Internationalization (i18n)

6 languages supported: English, French, German, Spanish, Italian, Dutch.

Translations are stored as shared JSON files in `i18n/`. All platforms load from the same source of truth.

## Deep Links

The app registers the `visio://` URL scheme on all platforms.

**Format:** `visio://host/slug` — e.g., `visio://meet.numerique.gouv.fr/abc-defg-hij`

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
```

## Contributing

We love contributions! See [CONTRIBUTING.md](CONTRIBUTING.md) for details.

- Open a PR (see [building locally](#building))
- Submit a [feature request](https://github.com/mmaudet/visio-mobile/issues/new?labels=enhancement) or [bug report](https://github.com/mmaudet/visio-mobile/issues/new?labels=bug)
- Read about our [CI/CD pipeline](docs/ci-pipeline.md) (linters, tests, security scans)

## Philosophy

We're building the best open-source **native** video conferencing client. Native performance, edge-first AI, and zero compromise on privacy.

Most of the heavy engineering is handled by the incredible [LiveKit](https://livekit.io/) team, allowing us to focus on delivering a great cross-platform experience. We favor quick, iterative releases and simplicity over complexity.

Our users come first. We're committed to making Visio Mobile as accessible and performant as proprietary solutions.

## Open source

This project is available under the [AGPL-3.0 license](LICENSE).

All features we develop will always remain open-source, and we are committed to contributing back to the LiveKit community whenever feasible.

## Contributors

<a href="https://github.com/mmaudet/visio-mobile/graphs/contributors">
  <img src="https://contrib.rocks/image?repo=mmaudet/visio-mobile" />
</a>

## Credits

Built with the awesome [LiveKit Rust SDK](https://livekit.io/). Visio Mobile is a native companion to [La Suite Meet](https://github.com/suitenumerique/meet) by [DINUM](https://www.numerique.gouv.fr/).

We're also thankful to the teams behind [UniFFI](https://github.com/mozilla/uniffi-rs), [Tauri](https://tauri.app/), [ONNX Runtime](https://onnxruntime.ai/), and [Jetpack Compose](https://developer.android.com/compose).

## License

Code in this repository is published under the [AGPL-3.0 license](LICENSE).
