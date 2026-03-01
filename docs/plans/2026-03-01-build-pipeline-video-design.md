# Build Pipeline + Video Implementation Design

**Date:** 2026-03-01
**Status:** Approved
**Builds on:** `2026-03-01-v2-rewrite-design.md`

## Scope

Five workstreams to take the v2 codebase from "compiles" to "builds and runs on all platforms":

1. UniFFI codegen — generate Kotlin + Swift bindings
2. Android build — cross-compile + Gradle integration
3. iOS build — cross-compile + Xcode integration
4. Desktop — Vite/React frontend for Tauri
5. Video pipeline — real I420 frame rendering on all 3 platforms

## 1. UniFFI Codegen

Add a `uniffi-bindgen` binary to `visio-ffi`:

```
crates/visio-ffi/src/bin/uniffi-bindgen.rs
  → uniffi::uniffi_bindgen_main()
```

Generate bindings:
- Kotlin → `android/app/src/main/kotlin/io/visio/mobile/generated/`
- Swift → `ios/VisioMobile/Generated/`

Generated files are committed to git so native builds don't require Rust toolchain.

## 2. Android Build

### Cross-compilation

```bash
cargo ndk -t arm64-v8a build -p visio-ffi -p visio-video --release
```

Outputs:
- `target/aarch64-linux-android/release/libvisio_ffi.so`
- `target/aarch64-linux-android/release/libvisio_video.so`

Copy to `android/app/src/main/jniLibs/arm64-v8a/`.

### Gradle integration

- Add `net.java.dev.jna:jna:5.14.0@aar` dependency (UniFFI runtime)
- Source sets include `generated/` directory
- `VisioManager.kt` already calls `System.loadLibrary("visio_ffi")`

### Build script

`scripts/build-android.sh`:
1. `cargo ndk` cross-compile
2. Copy `.so` to jniLibs
3. `./gradlew assembleDebug`

## 3. iOS Build

### Cross-compilation

```bash
cargo build --target aarch64-apple-ios -p visio-ffi -p visio-video --release
```

Outputs:
- `target/aarch64-apple-ios/release/libvisio_ffi.a`
- `target/aarch64-apple-ios/release/libvisio_video.a`

### Xcode integration

- Add `.a` files to Xcode project link phase
- Add generated Swift files to compile sources
- Generate `visioFFI.h` bridging header via uniffi-bindgen
- Library search path: `../../target/aarch64-apple-ios/release/`
- Linker flags: `-lvisio_ffi -lvisio_video` + framework deps (Security, SystemConfiguration)

### Build script

`scripts/build-ios.sh`:
1. `cargo build` cross-compile
2. Copy artifacts to known location
3. `xcodebuild` archive

## 4. Desktop Frontend (Vite + React)

### Setup

```
crates/visio-desktop/frontend/
├── package.json
├── vite.config.ts
├── index.html
├── src/
│   ├── main.tsx
│   ├── App.tsx
│   ├── pages/
│   │   ├── HomePage.tsx      # Join form
│   │   ├── CallPage.tsx      # Video grid + controls
│   │   └── ChatPage.tsx      # Message list + input
│   ├── components/
│   │   ├── VideoTile.tsx     # Canvas-based video renderer
│   │   └── ControlBar.tsx    # Mic/camera/chat toggle buttons
│   └── hooks/
│       └── useTauri.ts       # Tauri invoke wrappers
```

### Tauri integration

- `tauri.conf.json` → `devUrl: "http://localhost:5173"`, `frontendDist: "../frontend/dist"`
- Video frames received via Tauri events: `listen("video-frame", ...)`
- Control commands via `invoke("connect", ...)` etc.

## 5. Video Pipeline (visio-video)

### Architecture

Track-centric subscription model. When `visio-core` emits `TrackSubscribed` for a video track, the native UI:
1. Creates a rendering surface
2. Calls `visio_video_attach_surface(track_sid, surface_handle)`
3. Rust spawns a `NativeVideoStream` listener for that track
4. Frame loop pushes I420 data to the native surface

### C FFI API

```c
// Lifecycle
int32_t visio_video_init(void* room_ptr);
int32_t visio_video_cleanup();

// Per-track surface management
int32_t visio_video_attach_surface(const char* track_sid, void* surface);
int32_t visio_video_detach_surface(const char* track_sid);
```

### Android (android.rs)

- Receive `jobject` Surface from Kotlin via JNI
- Get `ANativeWindow*` via `ANativeWindow_fromSurface()`
- Frame loop: `NativeVideoStream::new(track)` → for each I420 frame:
  - `ANativeWindow_lock()` → get buffer
  - Convert I420 → RGBA (libyuv or manual) → write to buffer
  - `ANativeWindow_unlockAndPost()`
- On detach: drop stream, release ANativeWindow

Kotlin side: `SurfaceView` in `CallScreen.kt`, pass `Surface` to JNI on `surfaceCreated`.

### iOS (ios.rs)

- Receive `CVPixelBufferPoolRef` or layer pointer from Swift
- Frame loop: for each I420 frame:
  - `CVPixelBufferCreate` with `kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange`
  - Copy Y plane + interleave UV → NV12 format
  - Wrap in `CMSampleBuffer`
  - Enqueue on `AVSampleBufferDisplayLayer`
- On detach: drop stream

Swift side: `AVSampleBufferDisplayLayer` in `CallView.swift`, pass layer ref to C FFI.

### Desktop (desktop.rs)

- No native surface — emit frames as Tauri events
- Frame loop: for each I420 frame:
  - Convert I420 → RGB
  - Encode as JPEG (quality 60, 10fps cap)
  - Base64 encode
  - Emit `video-frame` event with `{track_sid, width, height, data}`
- React `VideoTile` component: `<canvas>` + `drawImage()` on each event

This reuses the v1 approach for desktop. Acceptable latency for desktop use.

### Shared infrastructure (lib.rs)

- `TrackRenderer` struct: holds `NativeVideoStream` + join handle + cancel token
- `HashMap<String, TrackRenderer>` — active renderers by track SID
- `attach_surface` creates renderer, `detach_surface` cancels and removes
- Thread-safe via `Mutex` or `RwLock`

## Build Matrix

| Platform | Rust target | Output | Build tool |
|----------|------------|--------|------------|
| Android arm64 | `aarch64-linux-android` | `.so` (cdylib) | cargo-ndk |
| iOS device | `aarch64-apple-ios` | `.a` (staticlib) | cargo build |
| iOS simulator | `aarch64-apple-ios-sim` | `.a` (staticlib) | cargo build |
| Desktop macOS | `aarch64-apple-darwin` | binary | cargo tauri build |

## Dependencies Added

### visio-video Cargo.toml

- `livekit` — NativeVideoStream
- `tokio` — async frame loop
- `tracing` — logging
- `image` — JPEG encoding (desktop only)
- `base64` — encoding (desktop only)
- Android: `jni`, `ndk` (ANativeWindow)
- iOS: `core-foundation`, `core-video` (via raw FFI or objc2 bindings)

### Android build.gradle.kts

- `net.java.dev.jna:jna:5.14.0@aar` (UniFFI runtime)

### Desktop frontend package.json

- `react`, `react-dom`, `@tauri-apps/api`, `vite`, `@vitejs/plugin-react`

## Success Criteria

- `scripts/build-android.sh` produces a working APK
- `scripts/build-ios.sh` produces a working IPA
- `cargo tauri dev` runs desktop app with React UI
- Video frames render on all 3 platforms
- 12 existing `visio-core` tests still pass
