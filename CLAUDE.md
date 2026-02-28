# Visio Mobile — Claude Code Context

## What is this project?
Visio Mobile is an open-source (AGPL-3.0) native video conferencing client
for Android, iOS, and Desktop. Built on the official LiveKit Rust SDK.

## Architecture
- `crates/visio-core/`: Pure Rust business logic. NO platform dependencies.
  Depends on livekit, livekit-api, tokio, serde, thiserror.
- `crates/visio-video/`: Video frame pipeline with raw C FFI.
  Platform-specific: writes I420 frames to native surfaces (SurfaceTexture, CVPixelBuffer).
- `crates/visio-ffi/`: UniFFI bindings wrapping visio-core for Kotlin + Swift.
- `crates/visio-desktop/`: Tauri 2.x desktop app. Calls visio-core directly.
- `android/`: Kotlin + Jetpack Compose app consuming visio-ffi + visio-video.
- `ios/`: Swift + SwiftUI app consuming visio-ffi + visio-video.

## Key Conventions
- Rust edition 2024
- `#[unsafe(no_mangle)]` required (edition 2024)
- `thiserror` for error types, `tracing` for logging
- visio-core public API must be UniFFI-compatible
- visio-video uses raw C FFI only (no UniFFI) for zero-copy video
- Tests run against a local LiveKit server: `livekit-server --dev`
- Android min SDK 26 (Android 8.0)
- iOS deployment target 16.0

## Build Commands
```bash
# Core crate
cargo build -p visio-core
cargo test -p visio-core

# Android native libs
cargo ndk -t arm64-v8a build -p visio-ffi -p visio-video --release

# Android APK
cd android && ./gradlew assembleRelease

# iOS
cargo build -p visio-ffi -p visio-video --target aarch64-apple-ios --release

# Desktop
cargo tauri dev -c crates/visio-desktop/tauri.conf.json

# UniFFI codegen (Phase 4+)
cargo run -p visio-ffi --bin uniffi-bindgen generate \
  crates/visio-ffi/src/visio.udl --language kotlin --out-dir android/app/src/main/kotlin/generated/
cargo run -p visio-ffi --bin uniffi-bindgen generate \
  crates/visio-ffi/src/visio.udl --language swift --out-dir ios/VisioMobile/Generated/
```

## LiveKit SDK Notes
- Pinned to =0.7.32
- Requires rustflags in .cargo/config.toml (-ObjC for macOS/iOS)
- Tokio runtime required
- Video frames: use NativeVideoStream for receiving
- Data channels: use Room::local_participant().publish_data() for chat

## FFI Strategy
- Control plane: UniFFI (visio-ffi wraps visio-core)
- Video plane: raw C FFI (visio-video, zero-copy to native surfaces)
- Desktop: direct Rust calls (no FFI — same process)
