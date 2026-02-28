# Visio Mobile v2 — Architecture Rewrite Design

**Date:** 2026-03-01
**Author:** Michel-Marie Maudet + Claude
**Status:** Approved

## Context

Visio Mobile v1.0 shipped on 2026-02-27 using Dioxus 0.7 for cross-platform UI. Three limitations drive a rewrite:

1. **Video rendering performance** — the I420 to JPEG to base64 to img-tag pipeline adds unnecessary latency and CPU overhead
2. **Native API access** — too much JNI/objc2 glue code for sensors, audio routing, permissions
3. **Build system complexity** — the dx build + Kotlin class injection + Gradle patching pipeline is fragile

## Decision: Full Rewrite

New repository. Clean slate. PRD feature scope (La Suite Meet mobile web parity — no context detection, driving mode, or mobility features in v1).

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Platform UI Shells                        │
│                                                             │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────┐  │
│  │   Android    │  │     iOS      │  │     Desktop      │  │
│  │  Kotlin +    │  │  Swift +     │  │  Tauri 2.x +     │  │
│  │  Jetpack     │  │  SwiftUI     │  │  Web frontend    │  │
│  │  Compose     │  │              │  │                  │  │
│  └──────┬───────┘  └──────┬───────┘  └────────┬─────────┘  │
│         │                 │                    │            │
│   ══════╪═════════════════╪══════════════╗     │            │
│         │    UniFFI (control plane)      ║     │            │
│   ══════╪═════════════════╪══════════════╝     │            │
│         │                 │                    │            │
│   ┌─────┴─────────────────┴────────────────────┴─────────┐  │
│   │              visio-core (Rust crate)                  │  │
│   │                                                       │  │
│   │  room.rs  ·  chat.rs  ·  participants.rs             │  │
│   │  controls.rs  ·  auth.rs  ·  settings.rs             │  │
│   │  events.rs  ·  errors.rs                             │  │
│   │                                                       │  │
│   │  Depends on: livekit, livekit-api, tokio, serde      │  │
│   └───────────────────────────────────────────────────────┘  │
│                                                             │
│   ┌───────────────────────────────────────────────────────┐  │
│   │              visio-video (Rust crate)                 │  │
│   │                                                       │  │
│   │  Raw C FFI — #[no_mangle] extern "C"                 │  │
│   │  I420 → native surface (SurfaceTexture / CVPixelBuf) │  │
│   │  Platform-specific: android.rs · ios.rs · desktop.rs │  │
│   └───────────────────────────────────────────────────────┘  │
│                                                             │
│   ┌───────────────────────────────────────────────────────┐  │
│   │              visio-ffi (Rust crate)                   │  │
│   │                                                       │  │
│   │  visio.udl — UniFFI interface definition             │  │
│   │  Generates: Kotlin bindings + Swift bindings         │  │
│   │  Wraps: visio-core public API                        │  │
│   └───────────────────────────────────────────────────────┘  │
│                                                             │
│   ┌───────────────────────────────────────────────────────┐  │
│   │              visio-desktop (Tauri 2.x app)           │  │
│   │                                                       │  │
│   │  Tauri commands → visio-core (direct Rust calls)     │  │
│   │  Web frontend: HTML/CSS/JS                           │  │
│   │  Video: native surface bypass or canvas              │  │
│   └───────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
                      ↕ WebRTC
               LiveKit Server
```

## Key Design Decisions

### 1. FFI Strategy: Hybrid (UniFFI + raw C)

UniFFI for the control plane (room management, chat, participants, settings, events). Raw `#[no_mangle] extern "C"` functions for video frame delivery with platform-native surface handles.

**Why:** UniFFI is excellent for type-safe APIs but adds unnecessary copying for the hot video path. Separating the two lets us get clean APIs for 95% of the surface and zero-copy performance for the critical 5%.

### 2. Video Rendering: Native surface injection

Native UI creates the rendering surface (`SurfaceView`/`SurfaceTexture` on Android, `AVSampleBufferDisplayLayer` on iOS) and passes the handle to Rust. Rust pushes I420 frames directly to the surface. GPU handles YUV to RGB conversion.

**Why:** Eliminates the I420 to JPEG to base64 bottleneck entirely. This is how every production video app works.

### 3. Desktop Framework: Tauri 2.x

Web frontend for polished styling, Tauri backend for Rust integration. Video rendering via canvas or native bypass.

**Why:** egui looks like a developer tool. Web tech makes polished UI trivial. Tauri is mature and cross-platform.

### 4. Crate Structure: 4 crates

| Crate | Purpose | FFI |
|-------|---------|-----|
| `visio-core` | Pure business logic, no platform deps | Wrapped by UniFFI |
| `visio-video` | Video pipeline, platform-specific rendering | Raw C FFI |
| `visio-ffi` | UniFFI bindings for visio-core | Generates Kotlin + Swift |
| `visio-desktop` | Tauri desktop app | Direct Rust calls |

**Why:** Separates the two FFI mechanisms cleanly. `visio-core` stays pure and testable. `visio-video` isolates platform-specific complexity.

## Data Flow

### Control plane (UniFFI)

```
Native UI  ──→  UniFFI binding  ──→  visio-core function
                                          │
                                     emit VisioEvent
                                          │
Native UI  ←──  callback interface  ←─────┘
```

### Video plane (raw FFI)

```
LiveKit NativeVideoStream
  │ I420 frame
  └──→ visio-video on_frame_received()
         │
         ├─ Android: write I420 → SurfaceTexture (JNI) → GPU renders
         ├─ iOS: write I420 → CVPixelBuffer → AVSampleBufferDisplayLayer
         └─ Desktop: write to canvas/wgpu texture
```

## Event Model

```rust
enum VisioEvent {
    ConnectionStateChanged(ConnectionState),
    ParticipantJoined(ParticipantInfo),
    ParticipantLeft(String),
    TrackSubscribed(TrackInfo),
    TrackUnsubscribed(String),
    ActiveSpeakerChanged(Vec<String>),
    MicStateChanged(bool),
    CameraStateChanged(bool),
    HandRaiseChanged(String, bool),
    ChatMessageReceived(ChatMessage),
    NetworkQualityChanged(NetworkQuality),
}
```

Rust owns the source of truth. Events push changes to native UI. Native side never polls.

## API Surface

### visio-core

- `RoomManager` — connect, disconnect, connection state
- `AuthService` — Meet API token request (`?username=` param)
- `MeetingControls` — mic/camera toggle, camera switch, hand raise
- `ChatService` — send/receive ephemeral messages via data channel
- `ParticipantManager` — read-only participant list, active speakers
- `SettingsStore` — display name, language
- `VisioEventListener` — callback trait for all events

### visio-video (raw C FFI)

- `visio_video_attach_surface(track_sid, surface)` — Android
- `visio_video_detach_surface(track_sid)` — Android
- `visio_video_attach_layer(track_sid, layer)` — iOS
- `visio_video_detach_layer(track_sid)` — iOS

## Build System

| Platform | Command | Output |
|----------|---------|--------|
| Core tests | `cargo test -p visio-core` | Unit tests |
| Android .so | `cargo ndk -t arm64-v8a build -p visio-ffi -p visio-video --release` | `.so` libs |
| Android APK | `cd android && ./gradlew assembleRelease` | Standard Gradle |
| iOS | `cargo build --target aarch64-apple-ios --release` + `xcodebuild` | Standard Xcode |
| Desktop | `cd crates/visio-desktop && cargo tauri build` | Native app |
| UniFFI codegen | `cargo run -p visio-ffi --bin uniffi-bindgen generate ...` | Kotlin + Swift |

No dx, no Kotlin injection, no Gradle patching. Standard native build tools.

## Phases

| Phase | Goal | Key output |
|-------|------|------------|
| 0 | Bootstrap | Repo, workspace, build verification, project skeletons |
| 1 | Core: Room | RoomManager, AuthService, ParticipantManager, events |
| 2 | Core: Media | MeetingControls, local track publishing |
| 3 | Core: Chat | ChatService, data channel messaging |
| 4 | FFI Layer | UniFFI .udl + codegen, visio-video raw FFI, video pipeline |
| 5 | Android | Kotlin + Compose app, all screens from PRD |
| 6 | iOS | SwiftUI app, CallKit, PiP |
| 7 | Desktop | Tauri app, keyboard shortcuts |

Phases 1-3: TDD. Phases 5-7: parallel (independent UI shells). Each phase = feature branch, merged after review.

## Non-Goals (v1)

- Context detection / mobility modes (v1.0 feature, deferred)
- ProConnect OIDC authentication
- E2E encryption
- Recording / transcription
- Background blur / virtual backgrounds
- Calendar integration
- Bluetooth/connectivity context management

## Success Metrics

| Metric | Target |
|--------|--------|
| Android APK size | < 25 MB |
| Cold start to home screen | < 2 seconds |
| Time to join room | < 3 seconds |
| Video latency (same network) | < 200ms |
| Battery usage (1h call) | < 15% |
