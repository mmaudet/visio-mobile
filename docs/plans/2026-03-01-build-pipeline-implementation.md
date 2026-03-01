# Build Pipeline + Video Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make all 3 platforms (Android, iOS, Desktop) build and run with real video rendering.

**Architecture:** UniFFI generates Kotlin + Swift bindings for the control plane. visio-video delivers I420 frames to native surfaces via raw C FFI. Desktop uses Tauri events + canvas rendering. All tied together with build scripts.

**Tech Stack:** Rust (edition 2024), UniFFI 0.29, cargo-ndk 3.5.4, LiveKit 0.7.32, Kotlin/Compose, SwiftUI, Tauri 2.x, Vite + React

---

### Task 1: Add uniffi-bindgen binary to visio-ffi

**Files:**
- Create: `crates/visio-ffi/src/bin/uniffi-bindgen.rs`
- Modify: `crates/visio-ffi/Cargo.toml`

**Step 1: Create the uniffi-bindgen binary**

Create `crates/visio-ffi/src/bin/uniffi-bindgen.rs`:

```rust
fn main() {
    uniffi::uniffi_bindgen_main();
}
```

**Step 2: Add the `cli` feature to uniffi dependency in Cargo.toml**

In `crates/visio-ffi/Cargo.toml`, the `[dependencies]` section needs uniffi with `cli` feature for the binary:

```toml
[dependencies]
visio-core = { path = "../visio-core" }
uniffi = { workspace = true }
tokio = { workspace = true }
thiserror = { workspace = true }

[features]
default = []
cli = ["uniffi/cli"]

[[bin]]
name = "uniffi-bindgen"
required-features = ["cli"]
```

**Step 3: Verify it compiles**

Run: `cd /Users/mmaudet/work/visio-mobile-v2 && cargo build -p visio-ffi --features cli --bin uniffi-bindgen`

Expected: Compiles successfully.

**Step 4: Commit**

```bash
git add crates/visio-ffi/src/bin/uniffi-bindgen.rs crates/visio-ffi/Cargo.toml
git commit -m "feat: add uniffi-bindgen binary for codegen"
```

---

### Task 2: Generate Kotlin bindings

**Files:**
- Create: `android/app/src/main/kotlin/uniffi/visio/visio.kt` (generated, exact path depends on uniffi output)

**Step 1: Run uniffi-bindgen for Kotlin**

```bash
cd /Users/mmaudet/work/visio-mobile-v2
cargo run -p visio-ffi --features cli --bin uniffi-bindgen -- generate \
  crates/visio-ffi/src/visio.udl \
  --language kotlin \
  --out-dir android/app/src/main/kotlin/
```

Expected: Creates files under `android/app/src/main/kotlin/uniffi/visio/`.

**Step 2: Verify generated files exist**

```bash
ls -la android/app/src/main/kotlin/uniffi/visio/
```

Expected: `visio.kt` (or similar) present.

**Step 3: Commit**

```bash
git add android/app/src/main/kotlin/uniffi/
git commit -m "feat: generate UniFFI Kotlin bindings"
```

---

### Task 3: Generate Swift bindings

**Files:**
- Create: `ios/VisioMobile/Generated/visio.swift` (generated)
- Create: `ios/VisioMobile/Generated/visioFFI.h` (generated)
- Create: `ios/VisioMobile/Generated/visioFFI.modulemap` (generated)

**Step 1: Run uniffi-bindgen for Swift**

```bash
cd /Users/mmaudet/work/visio-mobile-v2
cargo run -p visio-ffi --features cli --bin uniffi-bindgen -- generate \
  crates/visio-ffi/src/visio.udl \
  --language swift \
  --out-dir ios/VisioMobile/Generated/
```

Expected: Creates `visio.swift`, `visioFFI.h`, `visioFFI.modulemap`.

**Step 2: Verify generated files exist**

```bash
ls -la ios/VisioMobile/Generated/
```

**Step 3: Commit**

```bash
git add ios/VisioMobile/Generated/
git commit -m "feat: generate UniFFI Swift bindings"
```

---

### Task 4: Android build — cross-compile and wire Gradle

**Files:**
- Modify: `android/app/build.gradle.kts` — add JNA dependency
- Create: `scripts/build-android.sh`

**Step 1: Cross-compile Rust for Android arm64**

```bash
cd /Users/mmaudet/work/visio-mobile-v2
cargo ndk -t arm64-v8a build -p visio-ffi -p visio-video --release
```

Expected: Produces `target/aarch64-linux-android/release/libvisio_ffi.so` and `libvisio_video.so`.

**Step 2: Copy .so to jniLibs**

```bash
mkdir -p android/app/src/main/jniLibs/arm64-v8a
cp target/aarch64-linux-android/release/libvisio_ffi.so android/app/src/main/jniLibs/arm64-v8a/
cp target/aarch64-linux-android/release/libvisio_video.so android/app/src/main/jniLibs/arm64-v8a/
```

**Step 3: Add JNA dependency to build.gradle.kts**

In `android/app/build.gradle.kts`, add to `dependencies`:

```kotlin
implementation("net.java.dev.jna:jna:5.14.0@aar")
```

**Step 4: Create build script**

Create `scripts/build-android.sh`:

```bash
#!/bin/bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

echo "==> Cross-compiling Rust for Android arm64..."
cargo ndk -t arm64-v8a build -p visio-ffi -p visio-video --release

echo "==> Copying .so files to jniLibs..."
mkdir -p android/app/src/main/jniLibs/arm64-v8a
cp target/aarch64-linux-android/release/libvisio_ffi.so android/app/src/main/jniLibs/arm64-v8a/
cp target/aarch64-linux-android/release/libvisio_video.so android/app/src/main/jniLibs/arm64-v8a/

echo "==> Building APK..."
cd android
./gradlew assembleDebug

echo "==> Done! APK at:"
find app/build/outputs/apk -name "*.apk" 2>/dev/null
```

**Step 5: Make script executable and test cross-compile step**

```bash
chmod +x scripts/build-android.sh
```

Note: Full Gradle build may fail until video stubs are wired. The cross-compile step validates the FFI layer builds for Android.

**Step 6: Commit**

```bash
git add android/app/build.gradle.kts scripts/build-android.sh android/app/src/main/jniLibs/
git commit -m "feat: Android build pipeline with cargo-ndk + Gradle"
```

---

### Task 5: iOS build — cross-compile and wire Xcode

**Files:**
- Create: `scripts/build-ios.sh`

**Step 1: Cross-compile Rust for iOS device**

```bash
cd /Users/mmaudet/work/visio-mobile-v2
cargo build --target aarch64-apple-ios -p visio-ffi -p visio-video --release
```

Expected: Produces `target/aarch64-apple-ios/release/libvisio_ffi.a` and `libvisio_video.a`.

**Step 2: Also build for iOS simulator**

```bash
cargo build --target aarch64-apple-ios-sim -p visio-ffi -p visio-video --release
```

**Step 3: Create build script**

Create `scripts/build-ios.sh`:

```bash
#!/bin/bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

TARGET="${1:-device}"

if [ "$TARGET" = "sim" ]; then
    RUST_TARGET="aarch64-apple-ios-sim"
    echo "==> Building for iOS Simulator..."
else
    RUST_TARGET="aarch64-apple-ios"
    echo "==> Building for iOS Device..."
fi

cargo build --target "$RUST_TARGET" -p visio-ffi -p visio-video --release

echo "==> Libraries at:"
ls -la "target/$RUST_TARGET/release/libvisio_ffi.a"
ls -la "target/$RUST_TARGET/release/libvisio_video.a"

echo ""
echo "To integrate with Xcode:"
echo "  1. Add libvisio_ffi.a and libvisio_video.a to Link Binary With Libraries"
echo "  2. Set Library Search Path to: \$(PROJECT_DIR)/../../target/$RUST_TARGET/release"
echo "  3. Add Other Linker Flags: -lvisio_ffi -lvisio_video"
echo "  4. Add bridging header pointing to ios/VisioMobile/Generated/visioFFI.h"
```

**Step 4: Make executable and test**

```bash
chmod +x scripts/build-ios.sh
```

**Step 5: Commit**

```bash
git add scripts/build-ios.sh
git commit -m "feat: iOS build script with device and simulator targets"
```

---

### Task 6: Implement visio-video shared infrastructure (lib.rs)

**Files:**
- Modify: `crates/visio-video/Cargo.toml` — add dependencies
- Modify: `crates/visio-video/src/lib.rs` — track renderer registry

**Step 1: Update Cargo.toml with needed dependencies**

Replace `crates/visio-video/Cargo.toml` with:

```toml
[package]
name = "visio-video"
version.workspace = true
edition.workspace = true
license.workspace = true

[lib]
crate-type = ["cdylib", "staticlib"]

[dependencies]
livekit = { workspace = true }
tokio = { workspace = true }
tracing = { workspace = true }

[target.'cfg(target_os = "android")'.dependencies]
jni = "0.21"
ndk = { version = "0.9", features = ["media"] }

[target.'cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))'.dependencies]
image = { version = "0.25", default-features = false, features = ["jpeg"] }
base64 = "0.22"
serde = { workspace = true }
serde_json = { workspace = true }
tauri = { version = "2", features = [] }
```

Note: iOS dependencies use raw FFI via `libc` — no extra crate needed. The `objc2` dep can be dropped since we'll use CoreVideo C functions directly.

**Step 2: Rewrite lib.rs with track renderer registry**

Replace `crates/visio-video/src/lib.rs`:

```rust
//! Video frame pipeline with raw C FFI.
//!
//! Delivers I420 frames from LiveKit NativeVideoStream
//! directly to platform-native rendering surfaces.

use std::collections::HashMap;
use std::ffi::{c_char, c_void, CStr};
use std::sync::{Arc, Mutex, OnceLock};

use livekit::prelude::*;
use livekit::webrtc::video_stream::native::NativeVideoStream;
use tokio::sync::watch;

#[cfg(target_os = "android")]
mod android;

#[cfg(target_os = "ios")]
mod ios;

#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
mod desktop;

/// Per-track video renderer.
struct TrackRenderer {
    /// Signals the frame loop to stop.
    cancel_tx: watch::Sender<bool>,
    /// Join handle for the frame loop task.
    _handle: tokio::task::JoinHandle<()>,
}

/// Global renderer registry.
static RENDERERS: OnceLock<Mutex<HashMap<String, TrackRenderer>>> = OnceLock::new();

fn renderers() -> &'static Mutex<HashMap<String, TrackRenderer>> {
    RENDERERS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Global tokio runtime for video frame loops.
static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

fn runtime() -> &'static tokio::runtime::Runtime {
    RT.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .thread_name("visio-video")
            .build()
            .expect("failed to create video runtime")
    })
}

/// Start rendering frames for a remote video track.
///
/// Called from visio-core (or native side) when a video track is subscribed.
/// The `track` is a LiveKit RemoteVideoTrack.
/// The `surface` is a platform-native handle.
///
/// This function is NOT extern "C" — it's called from Rust code
/// that has access to the LiveKit track object.
pub fn start_track_renderer(
    track_sid: String,
    track: RemoteVideoTrack,
    surface: *mut c_void,
) {
    let (cancel_tx, cancel_rx) = watch::channel(false);

    let handle = runtime().spawn(frame_loop(track_sid.clone(), track, surface, cancel_rx));

    let renderer = TrackRenderer {
        cancel_tx,
        _handle: handle,
    };

    renderers().lock().unwrap().insert(track_sid.clone(), renderer);
    tracing::info!("started renderer for track {track_sid}");
}

/// Stop rendering for a track.
pub fn stop_track_renderer(track_sid: &str) {
    if let Some(renderer) = renderers().lock().unwrap().remove(track_sid) {
        let _ = renderer.cancel_tx.send(true);
        tracing::info!("stopped renderer for track {track_sid}");
    }
}

/// Frame loop: reads I420 frames from NativeVideoStream and pushes to surface.
async fn frame_loop(
    track_sid: String,
    track: RemoteVideoTrack,
    surface: *mut c_void,
    mut cancel_rx: watch::Receiver<bool>,
) {
    let rtc_track = track.rtc_track();
    let mut stream = NativeVideoStream::new(rtc_track);
    let mut frame_count: u64 = 0;

    tracing::info!("frame loop started for track {track_sid}");

    loop {
        tokio::select! {
            _ = cancel_rx.changed() => {
                if *cancel_rx.borrow() {
                    tracing::info!("frame loop cancelled for track {track_sid}");
                    break;
                }
            }
            frame = stream.next() => {
                match frame {
                    Some(frame) => {
                        frame_count += 1;

                        #[cfg(target_os = "android")]
                        android::render_frame(&frame, surface);

                        #[cfg(target_os = "ios")]
                        ios::render_frame(&frame, surface);

                        #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
                        {
                            // Desktop: throttle to 10fps
                            if frame_count % 3 == 0 {
                                desktop::render_frame(&frame, surface, &track_sid);
                            }
                        }
                    }
                    None => {
                        tracing::info!("video stream ended for track {track_sid}");
                        break;
                    }
                }
            }
        }
    }
}

// ── C FFI entry points ──────────────────────────────────────────

/// Attach a native rendering surface to a video track.
///
/// # Safety
/// `track_sid` must be a valid null-terminated C string.
/// `surface` must be a valid platform surface handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn visio_video_attach_surface(
    track_sid: *const c_char,
    surface: *mut c_void,
) -> i32 {
    if track_sid.is_null() || surface.is_null() {
        return -1;
    }
    let sid = unsafe { CStr::from_ptr(track_sid) };
    let sid_str = match sid.to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return -1,
    };

    tracing::info!("attach_surface called for track {sid_str} (surface stored for later use)");
    // Note: The actual renderer is started via start_track_renderer()
    // which is called from Rust code that has the LiveKit track object.
    // This C FFI is for native code to provide the surface handle.
    0
}

/// Detach the rendering surface from a video track.
///
/// # Safety
/// `track_sid` must be a valid null-terminated C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn visio_video_detach_surface(
    track_sid: *const c_char,
) -> i32 {
    if track_sid.is_null() {
        return -1;
    }
    let sid = unsafe { CStr::from_ptr(track_sid) };
    let sid_str = match sid.to_str() {
        Ok(s) => s,
        Err(_) => return -1,
    };

    stop_track_renderer(sid_str);
    0
}
```

**Step 3: Verify it compiles (host target only — desktop path)**

```bash
cd /Users/mmaudet/work/visio-mobile-v2
cargo check -p visio-video
```

Note: This may fail because of the `tauri` dependency on non-Tauri builds. We'll fix the conditional compilation in the desktop module. If it fails, adjust the Cargo.toml to make desktop dependencies optional or feature-gated.

**Step 4: Commit**

```bash
git add crates/visio-video/
git commit -m "feat: implement video track renderer registry"
```

---

### Task 7: Implement desktop video renderer (desktop.rs)

**Files:**
- Modify: `crates/visio-video/src/desktop.rs`

**Step 1: Implement desktop frame renderer**

The desktop renderer converts I420 frames to JPEG base64 and emits them via a callback. Since we can't easily depend on `tauri` from visio-video (it's a separate crate), we'll use a simpler approach: a registered callback function pointer.

Write `crates/visio-video/src/desktop.rs`:

```rust
//! Desktop video renderer — converts I420 frames to JPEG base64.
//!
//! Emits frames via a registered callback so the Tauri app can
//! forward them to the frontend as events.

use std::ffi::c_void;
use std::sync::OnceLock;

use image::{ImageBuffer, Rgb, codecs::jpeg::JpegEncoder};

/// Callback type: (track_sid, base64_jpeg, width, height, user_data)
type FrameCallback = unsafe extern "C" fn(
    track_sid: *const std::ffi::c_char,
    data: *const u8,
    data_len: usize,
    width: u32,
    height: u32,
    user_data: *mut c_void,
);

struct CallbackInfo {
    callback: FrameCallback,
    user_data: *mut c_void,
}

// SAFETY: user_data is managed by the caller (Tauri side) and only
// accessed from the callback which runs on a single thread.
unsafe impl Send for CallbackInfo {}
unsafe impl Sync for CallbackInfo {}

static CALLBACK: OnceLock<CallbackInfo> = OnceLock::new();

/// Register a callback for receiving video frames on desktop.
///
/// # Safety
/// `user_data` must be valid for the lifetime of the application.
/// `callback` must be a valid function pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn visio_video_set_desktop_callback(
    callback: FrameCallback,
    user_data: *mut c_void,
) {
    let _ = CALLBACK.set(CallbackInfo { callback, user_data });
}

/// Render a single I420 frame by converting to JPEG and calling the callback.
pub(crate) fn render_frame(
    frame: &livekit::webrtc::video_frame::VideoFrame<livekit::webrtc::video_frame::native::NativeBuffer>,
    _surface: *mut c_void,
    track_sid: &str,
) {
    let Some(cb) = CALLBACK.get() else {
        return;
    };

    let buffer = frame.buffer();
    let width = buffer.width();
    let height = buffer.height();

    // Convert I420 to RGB
    let i420 = buffer.to_i420();
    let y_data = i420.data_y();
    let u_data = i420.data_u();
    let v_data = i420.data_v();
    let y_stride = i420.stride_y() as usize;
    let u_stride = i420.stride_u() as usize;
    let v_stride = i420.stride_v() as usize;
    let w = width as usize;
    let h = height as usize;

    let mut rgb = vec![0u8; w * h * 3];

    for row in 0..h {
        for col in 0..w {
            let y_idx = row * y_stride + col;
            let u_idx = (row / 2) * u_stride + (col / 2);
            let v_idx = (row / 2) * v_stride + (col / 2);

            let y = y_data[y_idx] as f32;
            let u = u_data[u_idx] as f32 - 128.0;
            let v = v_data[v_idx] as f32 - 128.0;

            let r = (y + 1.402 * v).clamp(0.0, 255.0) as u8;
            let g = (y - 0.344136 * u - 0.714136 * v).clamp(0.0, 255.0) as u8;
            let b = (y + 1.772 * u).clamp(0.0, 255.0) as u8;

            let out_idx = (row * w + col) * 3;
            rgb[out_idx] = r;
            rgb[out_idx + 1] = g;
            rgb[out_idx + 2] = b;
        }
    }

    // Encode as JPEG
    let img: ImageBuffer<Rgb<u8>, Vec<u8>> =
        ImageBuffer::from_raw(width, height, rgb).expect("buffer size mismatch");

    let mut jpeg_buf = Vec::with_capacity(w * h / 4);
    let mut encoder = JpegEncoder::new_with_quality(&mut jpeg_buf, 60);
    if encoder.encode_image(&img).is_err() {
        tracing::warn!("JPEG encode failed for track {track_sid}");
        return;
    }

    // Base64 encode
    let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &jpeg_buf);

    // Call the callback
    let sid_cstr = std::ffi::CString::new(track_sid).unwrap();
    unsafe {
        (cb.callback)(
            sid_cstr.as_ptr(),
            b64.as_ptr(),
            b64.len(),
            width,
            height,
            cb.user_data,
        );
    }
}
```

**Step 2: Verify it compiles**

```bash
cargo check -p visio-video
```

**Step 3: Commit**

```bash
git add crates/visio-video/src/desktop.rs
git commit -m "feat: implement desktop video renderer (I420 → JPEG → callback)"
```

---

### Task 8: Implement Android video renderer (android.rs)

**Files:**
- Modify: `crates/visio-video/src/android.rs`

**Step 1: Implement Android frame renderer using NDK ANativeWindow**

Write `crates/visio-video/src/android.rs`:

```rust
//! Android video renderer — writes I420 frames to ANativeWindow.
//!
//! Native side passes a Surface object. We get ANativeWindow*
//! and write RGBA pixels directly. SurfaceView handles display.

use std::ffi::c_void;

/// Render a single I420 frame to an ANativeWindow surface.
///
/// The `surface` pointer is an `ANativeWindow*` obtained from
/// the Android Surface via `ANativeWindow_fromSurface()`.
pub(crate) fn render_frame(
    frame: &livekit::webrtc::video_frame::VideoFrame<livekit::webrtc::video_frame::native::NativeBuffer>,
    surface: *mut c_void,
) {
    let buffer = frame.buffer();
    let width = buffer.width() as usize;
    let height = buffer.height() as usize;

    // Convert to I420
    let i420 = buffer.to_i420();
    let y_data = i420.data_y();
    let u_data = i420.data_u();
    let v_data = i420.data_v();
    let y_stride = i420.stride_y() as usize;
    let u_stride = i420.stride_u() as usize;
    let v_stride = i420.stride_v() as usize;

    let window = surface as *mut ndk::native_window::NativeWindow;

    // ANativeWindow lock/write/unlock via ndk crate
    // For now, convert I420 → RGBA and write pixel by pixel
    // The ndk crate provides safe wrappers around ANativeWindow

    unsafe {
        let window_ref = &*window;

        // Set buffer geometry
        // ANativeWindow_setBuffersGeometry(window, width, height, WINDOW_FORMAT_RGBA_8888)
        ndk::native_window::NativeWindow::set_buffers_geometry(
            window_ref,
            width as i32,
            height as i32,
            ndk::native_window::WindowFormat::RGBA8888,
        );

        // Lock the buffer
        let mut buffer_out = std::mem::MaybeUninit::uninit();
        let lock_result = ndk_sys::ANativeWindow_lock(
            window as *mut ndk_sys::ANativeWindow,
            buffer_out.as_mut_ptr(),
            std::ptr::null_mut(),
        );

        if lock_result != 0 {
            tracing::warn!("ANativeWindow_lock failed: {lock_result}");
            return;
        }

        let native_buf = buffer_out.assume_init();
        let stride = native_buf.stride as usize;
        let bits = native_buf.bits as *mut u8;

        // Convert I420 → RGBA and write to buffer
        for row in 0..height {
            for col in 0..width {
                let y_idx = row * y_stride + col;
                let u_idx = (row / 2) * u_stride + (col / 2);
                let v_idx = (row / 2) * v_stride + (col / 2);

                let y = y_data[y_idx] as f32;
                let u = u_data[u_idx] as f32 - 128.0;
                let v = v_data[v_idx] as f32 - 128.0;

                let r = (y + 1.402 * v).clamp(0.0, 255.0) as u8;
                let g = (y - 0.344136 * u - 0.714136 * v).clamp(0.0, 255.0) as u8;
                let b = (y + 1.772 * u).clamp(0.0, 255.0) as u8;

                let out_idx = (row * stride + col) * 4;
                *bits.add(out_idx) = r;
                *bits.add(out_idx + 1) = g;
                *bits.add(out_idx + 2) = b;
                *bits.add(out_idx + 3) = 255; // alpha
            }
        }

        ndk_sys::ANativeWindow_unlockAndPost(window as *mut ndk_sys::ANativeWindow);
    }
}
```

Note: The exact ANativeWindow API may need adjustment depending on the `ndk` crate version. The `ndk 0.9` crate provides higher-level wrappers, or we can use `ndk-sys` for raw FFI. Adjust as needed during implementation.

**Step 2: Add ndk-sys dependency if needed**

In `crates/visio-video/Cargo.toml`, under `[target.'cfg(target_os = "android")'.dependencies]`:

```toml
jni = "0.21"
ndk = { version = "0.9", features = ["media"] }
ndk-sys = "0.6"
```

**Step 3: Cross-compile check**

```bash
cargo ndk -t arm64-v8a check -p visio-video
```

**Step 4: Commit**

```bash
git add crates/visio-video/
git commit -m "feat: implement Android video renderer (I420 → ANativeWindow)"
```

---

### Task 9: Implement iOS video renderer (ios.rs)

**Files:**
- Modify: `crates/visio-video/src/ios.rs`

**Step 1: Implement iOS frame renderer using CoreVideo**

Write `crates/visio-video/src/ios.rs`:

```rust
//! iOS video renderer — writes I420 frames to CVPixelBuffer.
//!
//! The surface pointer is a pointer to an Objective-C callback block
//! or a CVPixelBufferRef destination. For simplicity, we use a callback
//! approach: Swift registers a callback that receives (width, height, y, u, v, strides).

use std::ffi::c_void;

/// Callback type for iOS: receives raw YUV plane pointers.
/// Swift side creates CVPixelBuffer from these planes and displays it.
///
/// Signature: (width, height, y_ptr, y_stride, u_ptr, u_stride, v_ptr, v_stride, user_data)
type IosFrameCallback = unsafe extern "C" fn(
    width: u32,
    height: u32,
    y_ptr: *const u8,
    y_stride: u32,
    u_ptr: *const u8,
    u_stride: u32,
    v_ptr: *const u8,
    v_stride: u32,
    track_sid: *const std::ffi::c_char,
    user_data: *mut c_void,
);

struct IosCallbackInfo {
    callback: IosFrameCallback,
    user_data: *mut c_void,
}

unsafe impl Send for IosCallbackInfo {}
unsafe impl Sync for IosCallbackInfo {}

static IOS_CALLBACK: std::sync::OnceLock<IosCallbackInfo> = std::sync::OnceLock::new();

/// Register a frame callback from Swift.
///
/// # Safety
/// `callback` and `user_data` must remain valid for the app's lifetime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn visio_video_set_ios_callback(
    callback: IosFrameCallback,
    user_data: *mut c_void,
) {
    let _ = IOS_CALLBACK.set(IosCallbackInfo { callback, user_data });
}

/// Render a single I420 frame by passing plane pointers to the iOS callback.
/// Swift side handles CVPixelBuffer creation and display.
pub(crate) fn render_frame(
    frame: &livekit::webrtc::video_frame::VideoFrame<livekit::webrtc::video_frame::native::NativeBuffer>,
    _surface: *mut c_void,
) {
    let Some(cb) = IOS_CALLBACK.get() else {
        return;
    };

    let buffer = frame.buffer();
    let width = buffer.width();
    let height = buffer.height();

    let i420 = buffer.to_i420();
    let y_data = i420.data_y();
    let u_data = i420.data_u();
    let v_data = i420.data_v();

    // We need to get the track_sid somehow — for now use empty string
    // In practice, the frame_loop passes track_sid via the render_frame call
    let sid_cstr = std::ffi::CString::new("").unwrap();

    unsafe {
        (cb.callback)(
            width,
            height,
            y_data.as_ptr(),
            i420.stride_y() as u32,
            u_data.as_ptr(),
            i420.stride_u() as u32,
            v_data.as_ptr(),
            i420.stride_v() as u32,
            sid_cstr.as_ptr(),
            cb.user_data,
        );
    }
}
```

Note: The iOS render_frame signature needs `track_sid` passed through. We'll need to update the `frame_loop` in `lib.rs` to pass track_sid to the iOS renderer. This will be adjusted during Task 6 refinement.

**Step 2: Update Cargo.toml iOS dependencies**

In `crates/visio-video/Cargo.toml`, update iOS deps:

```toml
[target.'cfg(target_os = "ios")'.dependencies]
# No extra deps needed — we pass raw pointers to Swift callback
```

Remove the `objc2` dependency since we're using a callback approach.

**Step 3: Cross-compile check**

```bash
cargo build --target aarch64-apple-ios -p visio-video --release 2>&1 | head -20
```

**Step 4: Commit**

```bash
git add crates/visio-video/
git commit -m "feat: implement iOS video renderer (I420 planes → Swift callback)"
```

---

### Task 10: Wire video rendering into visio-core event loop

**Files:**
- Modify: `crates/visio-core/Cargo.toml` — add visio-video dependency
- Modify: `crates/visio-core/src/room.rs` — start/stop renderers on TrackSubscribed/Unsubscribed

**Step 1: Add visio-video as dependency**

This is tricky because visio-core should stay pure. Instead, we'll wire this at the FFI layer.

**Alternative approach:** The native side (Kotlin/Swift/Tauri) listens for TrackSubscribed events, creates a surface, and calls `visio_video_attach_surface()`. The Rust side needs a way to map track SID → RemoteVideoTrack.

Better: Add a track registry to visio-core that stores subscribed tracks, and expose a method for visio-video to retrieve them.

Add to `crates/visio-core/src/room.rs`:

```rust
// Add to RoomManager struct:
subscribed_tracks: Arc<Mutex<HashMap<String, RemoteVideoTrack>>>,

// In TrackSubscribed handler, store the track:
if track_kind == TrackKind::Video {
    if let livekit::track::RemoteTrack::Video(video_track) = &track {
        self.subscribed_tracks.lock().await
            .insert(track.sid().to_string(), video_track.clone());
    }
}

// In TrackUnsubscribed handler, remove it:
self.subscribed_tracks.lock().await.remove(&track.sid().to_string());

// Add public method:
pub async fn get_video_track(&self, track_sid: &str) -> Option<RemoteVideoTrack> {
    self.subscribed_tracks.lock().await.get(track_sid).cloned()
}
```

Then in the FFI layer, `visio_video_attach_surface` can:
1. Look up the track from the registry
2. Call `visio_video::start_track_renderer(track_sid, track, surface)`

**Step 2: Update lib.rs with subscribed_tracks field**

This requires careful modifications to `room.rs` — add `subscribed_tracks` field, update `new()`, and update the event loop handlers.

**Step 3: Verify existing tests still pass**

```bash
cargo test -p visio-core
```

Expected: All 12+ tests pass.

**Step 4: Commit**

```bash
git add crates/visio-core/
git commit -m "feat: track video track subscriptions in RoomManager"
```

---

### Task 11: Wire video attach/detach in visio-ffi

**Files:**
- Modify: `crates/visio-ffi/Cargo.toml` — add visio-video dependency
- Modify: `crates/visio-ffi/src/lib.rs` — expose room to visio-video
- Modify: `crates/visio-ffi/src/visio.udl` — no change needed (video is C FFI)

**Step 1: Add visio-video dependency to visio-ffi**

In `crates/visio-ffi/Cargo.toml`:

```toml
[dependencies]
visio-core = { path = "../visio-core" }
visio-video = { path = "../visio-video" }
uniffi = { workspace = true }
tokio = { workspace = true }
thiserror = { workspace = true }
```

**Step 2: Add attach_video_surface method to VisioClient**

In `crates/visio-ffi/src/lib.rs`, add a C FFI function that bridges the gap:

```rust
/// Attach a native surface for video rendering.
/// Called from native code (Kotlin JNI / Swift C interop).
///
/// # Safety
/// Must be called with valid track_sid and surface pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn visio_attach_video_surface(
    client_ptr: *const VisioClient,
    track_sid: *const std::ffi::c_char,
    surface: *mut std::ffi::c_void,
) -> i32 {
    if client_ptr.is_null() || track_sid.is_null() || surface.is_null() {
        return -1;
    }

    let client = unsafe { &*client_ptr };
    let sid = unsafe { std::ffi::CStr::from_ptr(track_sid) };
    let sid_str = match sid.to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return -1,
    };

    // Look up the track from the room manager
    let track = client.rt.block_on(client.room_manager.get_video_track(&sid_str));
    match track {
        Some(video_track) => {
            visio_video::start_track_renderer(sid_str, video_track, surface);
            0
        }
        None => {
            tracing::warn!("no video track found for SID {sid_str}");
            -2
        }
    }
}

/// Detach video surface for a track.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn visio_detach_video_surface(
    track_sid: *const std::ffi::c_char,
) -> i32 {
    if track_sid.is_null() {
        return -1;
    }
    let sid = unsafe { std::ffi::CStr::from_ptr(track_sid) };
    let sid_str = match sid.to_str() {
        Ok(s) => s,
        Err(_) => return -1,
    };
    visio_video::stop_track_renderer(sid_str);
    0
}
```

**Step 3: Verify it compiles**

```bash
cargo check -p visio-ffi
```

**Step 4: Commit**

```bash
git add crates/visio-ffi/
git commit -m "feat: wire video attach/detach through FFI layer"
```

---

### Task 12: Desktop frontend — Vite + React setup

**Files:**
- Create: `crates/visio-desktop/frontend/package.json`
- Create: `crates/visio-desktop/frontend/vite.config.ts`
- Create: `crates/visio-desktop/frontend/tsconfig.json`
- Create: `crates/visio-desktop/frontend/src/main.tsx`
- Create: `crates/visio-desktop/frontend/src/App.tsx`
- Create: `crates/visio-desktop/frontend/src/App.css`
- Modify: `crates/visio-desktop/frontend/index.html` — replace with Vite entry point
- Modify: `crates/visio-desktop/tauri.conf.json` — point to Vite dev server

**Step 1: Initialize npm project**

```bash
cd /Users/mmaudet/work/visio-mobile-v2/crates/visio-desktop/frontend
npm init -y
npm install react react-dom @tauri-apps/api
npm install -D vite @vitejs/plugin-react typescript @types/react @types/react-dom
```

**Step 2: Create vite.config.ts**

```typescript
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  server: {
    port: 5173,
    strictPort: true,
  },
  build: {
    outDir: "dist",
  },
});
```

**Step 3: Create tsconfig.json**

```json
{
  "compilerOptions": {
    "target": "ES2020",
    "useDefineForClassFields": true,
    "lib": ["ES2020", "DOM", "DOM.Iterable"],
    "module": "ESNext",
    "skipLibCheck": true,
    "moduleResolution": "bundler",
    "allowImportingTsExtensions": true,
    "resolveJsonModule": true,
    "isolatedModules": true,
    "noEmit": true,
    "jsx": "react-jsx",
    "strict": true,
    "noUnusedLocals": true,
    "noUnusedParameters": true,
    "noFallthroughCasesInSwitch": true
  },
  "include": ["src"]
}
```

**Step 4: Replace index.html with Vite entry**

Replace `crates/visio-desktop/frontend/index.html`:

```html
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>Visio</title>
</head>
<body>
    <div id="root"></div>
    <script type="module" src="/src/main.tsx"></script>
</body>
</html>
```

**Step 5: Create src/main.tsx**

```tsx
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import "./App.css";

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>
);
```

**Step 6: Create src/App.tsx with all pages**

```tsx
import { useState, useEffect, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

type Page = "home" | "call" | "chat";

interface Participant {
  sid: string;
  identity: string;
  name: string | null;
  is_muted: boolean;
  has_video: boolean;
  connection_quality: string;
}

interface ChatMsg {
  id: string;
  sender_sid: string;
  sender_name: string;
  text: string;
  timestamp_ms: number;
}

export default function App() {
  const [page, setPage] = useState<Page>("home");
  const [meetUrl, setMeetUrl] = useState("");
  const [username, setUsername] = useState("");
  const [connectionState, setConnectionState] = useState("disconnected");
  const [participants, setParticipants] = useState<Participant[]>([]);
  const [messages, setMessages] = useState<ChatMsg[]>([]);
  const [chatInput, setChatInput] = useState("");
  const [micEnabled, setMicEnabled] = useState(false);
  const [camEnabled, setCamEnabled] = useState(false);
  const [error, setError] = useState("");
  const [joining, setJoining] = useState(false);
  const pollRef = useRef<number | null>(null);

  const poll = useCallback(async () => {
    try {
      const state = await invoke<string>("get_connection_state");
      setConnectionState(state);
      if (state === "connected" || state === "reconnecting") {
        const parts = await invoke<Participant[]>("get_participants");
        setParticipants(parts);
        const msgs = await invoke<ChatMsg[]>("get_messages");
        setMessages(msgs);
      }
      if (state === "disconnected" && page !== "home") {
        setPage("home");
        stopPolling();
      }
    } catch (e) {
      console.error("poll error:", e);
    }
  }, [page]);

  const startPolling = useCallback(() => {
    if (pollRef.current) return;
    pollRef.current = window.setInterval(poll, 1000);
  }, [poll]);

  const stopPolling = useCallback(() => {
    if (pollRef.current) {
      clearInterval(pollRef.current);
      pollRef.current = null;
    }
  }, []);

  useEffect(() => {
    return () => stopPolling();
  }, [stopPolling]);

  const handleJoin = async () => {
    if (!meetUrl.trim()) {
      setError("Please enter a meeting URL");
      return;
    }
    setJoining(true);
    setError("");
    try {
      await invoke("connect", {
        meetUrl: meetUrl.trim(),
        username: username.trim() || null,
      });
      setPage("call");
      startPolling();
    } catch (e) {
      setError(String(e));
    } finally {
      setJoining(false);
    }
  };

  const handleHangup = async () => {
    try {
      await invoke("disconnect");
    } catch (e) {
      console.error(e);
    }
    stopPolling();
    setPage("home");
    setConnectionState("disconnected");
    setMicEnabled(false);
    setCamEnabled(false);
    setMessages([]);
    setParticipants([]);
  };

  const toggleMic = async () => {
    const next = !micEnabled;
    setMicEnabled(next);
    try {
      await invoke("toggle_mic", { enabled: next });
    } catch {
      setMicEnabled(!next);
    }
  };

  const toggleCam = async () => {
    const next = !camEnabled;
    setCamEnabled(next);
    try {
      await invoke("toggle_camera", { enabled: next });
    } catch {
      setCamEnabled(!next);
    }
  };

  const sendMessage = async () => {
    const text = chatInput.trim();
    if (!text) return;
    setChatInput("");
    try {
      await invoke("send_chat", { text });
    } catch (e) {
      console.error(e);
    }
  };

  if (page === "home") {
    return (
      <div className="app">
        <header>
          <h1>Visio</h1>
          <span className={`badge ${connectionState}`}>{connectionState}</span>
        </header>
        <main className="home">
          <div className="join-form">
            <h2>Join a Room</h2>
            <p>Enter a meeting URL and your display name</p>
            <label>Meeting URL</label>
            <input
              value={meetUrl}
              onChange={(e) => setMeetUrl(e.target.value)}
              placeholder="meet.example.com/my-room"
              onKeyDown={(e) => e.key === "Enter" && handleJoin()}
            />
            <label>Display Name (optional)</label>
            <input
              value={username}
              onChange={(e) => setUsername(e.target.value)}
              placeholder="Your name"
              onKeyDown={(e) => e.key === "Enter" && handleJoin()}
            />
            <button
              className="btn-primary"
              onClick={handleJoin}
              disabled={joining}
            >
              {joining ? "Connecting..." : "Join"}
            </button>
            {error && <p className="error">{error}</p>}
          </div>
        </main>
      </div>
    );
  }

  if (page === "chat") {
    return (
      <div className="app">
        <header>
          <button className="back-btn" onClick={() => setPage("call")}>
            Back
          </button>
          <h1>Chat</h1>
          <span />
        </header>
        <main className="chat-page">
          <div className="message-list">
            {messages.length === 0 ? (
              <p className="empty">No messages yet</p>
            ) : (
              messages.map((m) => (
                <div key={m.id} className="message">
                  <div className="msg-header">
                    <span className="sender">{m.sender_name || "Unknown"}</span>
                    <span className="time">
                      {new Date(m.timestamp_ms).toLocaleTimeString([], {
                        hour: "2-digit",
                        minute: "2-digit",
                      })}
                    </span>
                  </div>
                  <p className="msg-text">{m.text}</p>
                </div>
              ))
            )}
          </div>
          <div className="chat-input">
            <input
              value={chatInput}
              onChange={(e) => setChatInput(e.target.value)}
              placeholder="Type a message..."
              onKeyDown={(e) => e.key === "Enter" && sendMessage()}
            />
            <button onClick={sendMessage} disabled={!chatInput.trim()}>
              Send
            </button>
          </div>
        </main>
      </div>
    );
  }

  // Call page
  return (
    <div className="app">
      <header>
        <h1>Visio</h1>
        <span className={`badge ${connectionState}`}>{connectionState}</span>
      </header>
      <main className="call-page">
        <div className="participants">
          <h3>Participants ({participants.length})</h3>
          {participants.length === 0 ? (
            <p className="empty">No other participants yet</p>
          ) : (
            participants.map((p) => (
              <div key={p.sid} className="participant">
                <div className="avatar">
                  {(p.name || p.identity || "?").substring(0, 2).toUpperCase()}
                </div>
                <div className="info">
                  <span className="name">{p.name || p.identity}</span>
                  <span className="quality">{p.connection_quality}</span>
                </div>
                <div className="indicators">
                  {p.is_muted && <span className="muted">Muted</span>}
                  {p.has_video && <span className="video">Video</span>}
                </div>
              </div>
            ))
          )}
        </div>
      </main>
      <div className="controls">
        <button
          className={`ctrl ${micEnabled ? "active" : ""}`}
          onClick={toggleMic}
        >
          Mic
        </button>
        <button
          className={`ctrl ${camEnabled ? "active" : ""}`}
          onClick={toggleCam}
        >
          Cam
        </button>
        <button className="ctrl" onClick={() => setPage("chat")}>
          Chat
        </button>
        <button className="ctrl hangup" onClick={handleHangup}>
          End
        </button>
      </div>
    </div>
  );
}
```

**Step 7: Create src/App.css**

Reuse the dark theme from the existing index.html (CSS variables + styles). Keep it minimal — port the key styles from the current inline CSS.

```css
:root {
  --bg-primary: #1a1a2e;
  --bg-secondary: #16213e;
  --bg-card: #0f3460;
  --bg-input: #1a1a3e;
  --accent: #e94560;
  --accent-hover: #ff6b81;
  --text-primary: #eee;
  --text-secondary: #aab;
  --text-muted: #778;
  --border: #2a2a4e;
  --success: #2ecc71;
  --warning: #f39c12;
  --danger: #e74c3c;
  --radius: 8px;
}

* { margin: 0; padding: 0; box-sizing: border-box; }

body {
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
  background: var(--bg-primary);
  color: var(--text-primary);
  min-height: 100vh;
}

.app {
  display: flex;
  flex-direction: column;
  height: 100vh;
}

header {
  background: var(--bg-secondary);
  padding: 12px 20px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  border-bottom: 1px solid var(--border);
}

header h1 { font-size: 1.1rem; font-weight: 600; }

.badge {
  font-size: 0.75rem;
  padding: 4px 10px;
  border-radius: 12px;
  background: var(--bg-card);
  color: var(--text-muted);
}
.badge.connected { background: rgba(46,204,113,.15); color: var(--success); }
.badge.connecting, .badge.reconnecting { background: rgba(243,156,18,.15); color: var(--warning); }

main { flex: 1; display: flex; flex-direction: column; overflow: hidden; }

/* Home */
.home {
  justify-content: center;
  align-items: center;
  padding: 40px 20px;
}
.join-form { width: 100%; max-width: 360px; }
.join-form h2 { font-size: 1.3rem; margin-bottom: 6px; text-align: center; }
.join-form p { color: var(--text-secondary); font-size: 0.85rem; text-align: center; margin-bottom: 24px; }
.join-form label { display: block; font-size: 0.8rem; color: var(--text-secondary); margin: 12px 0 6px; }
.join-form input {
  width: 100%; padding: 10px 14px; border: 1px solid var(--border);
  border-radius: var(--radius); background: var(--bg-input); color: var(--text-primary);
  font-size: 0.9rem; outline: none;
}
.join-form input:focus { border-color: var(--accent); }
.join-form input::placeholder { color: var(--text-muted); }

.btn-primary {
  display: block; width: 100%; padding: 10px 20px; margin-top: 20px;
  border: none; border-radius: var(--radius); background: var(--accent);
  color: #fff; font-size: 0.9rem; font-weight: 500; cursor: pointer;
}
.btn-primary:hover:not(:disabled) { background: var(--accent-hover); }
.btn-primary:disabled { opacity: 0.5; cursor: not-allowed; }
.error { color: var(--danger); font-size: 0.8rem; margin-top: 12px; text-align: center; }

/* Call page */
.call-page { flex: 1; overflow-y: auto; padding: 16px; }
.call-page h3 { font-size: 0.75rem; color: var(--text-muted); text-transform: uppercase; letter-spacing: 1px; margin-bottom: 10px; }
.participant {
  display: flex; align-items: center; gap: 10px; padding: 10px 12px;
  background: var(--bg-secondary); border-radius: var(--radius); margin-bottom: 8px;
}
.avatar {
  width: 36px; height: 36px; border-radius: 50%; background: var(--bg-card);
  display: flex; align-items: center; justify-content: center;
  font-size: 0.85rem; font-weight: 600; color: var(--accent); flex-shrink: 0;
}
.info { flex: 1; min-width: 0; }
.name { font-size: 0.9rem; display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.quality { font-size: 0.75rem; color: var(--text-muted); }
.indicators { display: flex; gap: 6px; font-size: 0.75rem; }
.muted { color: var(--danger); }
.video { color: var(--success); }
.empty { text-align: center; color: var(--text-muted); font-size: 0.85rem; padding: 30px 0; }

/* Controls */
.controls {
  display: flex; gap: 12px; padding: 16px; background: var(--bg-secondary);
  border-top: 1px solid var(--border); justify-content: center;
}
.ctrl {
  width: 48px; height: 48px; border-radius: 50%; border: none; cursor: pointer;
  font-size: 0.75rem; display: flex; align-items: center; justify-content: center;
  background: var(--bg-card); color: var(--text-muted); transition: background 0.2s;
}
.ctrl:hover { transform: scale(1.05); }
.ctrl.active { background: var(--success); color: #fff; }
.ctrl.hangup { background: var(--danger); color: #fff; }
.ctrl.hangup:hover { background: #c0392b; }

/* Chat page */
.chat-page { flex: 1; display: flex; flex-direction: column; }
.back-btn { background: none; border: none; color: var(--text-secondary); cursor: pointer; font-size: 0.85rem; }
.back-btn:hover { color: var(--text-primary); }
.message-list { flex: 1; overflow-y: auto; padding: 16px; display: flex; flex-direction: column; gap: 10px; }
.message { max-width: 85%; }
.msg-header { display: flex; justify-content: space-between; margin-bottom: 3px; }
.sender { font-size: 0.7rem; color: var(--accent); }
.time { font-size: 0.65rem; color: var(--text-muted); }
.msg-text { padding: 8px 12px; border-radius: 12px; background: var(--bg-secondary); font-size: 0.85rem; line-height: 1.4; word-break: break-word; }
.chat-input {
  display: flex; gap: 8px; padding: 12px 16px; background: var(--bg-secondary);
  border-top: 1px solid var(--border);
}
.chat-input input {
  flex: 1; padding: 8px 12px; border: 1px solid var(--border); border-radius: 20px;
  background: var(--bg-input); color: var(--text-primary); font-size: 0.85rem; outline: none;
}
.chat-input input:focus { border-color: var(--accent); }
.chat-input input::placeholder { color: var(--text-muted); }
.chat-input button {
  padding: 8px 16px; background: var(--accent); color: #fff; border: none;
  border-radius: 20px; cursor: pointer; font-size: 0.85rem;
}
.chat-input button:hover { background: var(--accent-hover); }
.chat-input button:disabled { opacity: 0.5; cursor: not-allowed; }
```

**Step 8: Update tauri.conf.json for Vite**

Update `crates/visio-desktop/tauri.conf.json`:

```json
{
  "$schema": "https://raw.githubusercontent.com/tauri-apps/tauri/dev/crates/tauri-config-schema/schema.json",
  "productName": "Visio Mobile",
  "version": "0.1.0",
  "identifier": "io.visio.desktop",
  "build": {
    "devUrl": "http://localhost:5173",
    "frontendDist": "./frontend/dist"
  },
  "app": {
    "withGlobalTauri": true,
    "windows": [
      {
        "title": "Visio",
        "width": 420,
        "height": 720,
        "minWidth": 320,
        "minHeight": 480
      }
    ]
  },
  "bundle": {
    "active": true,
    "targets": "all",
    "icon": ["icons/icon.png"]
  }
}
```

**Step 9: Test Vite dev server starts**

```bash
cd /Users/mmaudet/work/visio-mobile-v2/crates/visio-desktop/frontend
npm run dev
```

**Step 10: Add .gitignore for node_modules**

Create `crates/visio-desktop/frontend/.gitignore`:

```
node_modules/
dist/
```

**Step 11: Commit**

```bash
git add crates/visio-desktop/frontend/ crates/visio-desktop/tauri.conf.json
git commit -m "feat: replace skeleton HTML with Vite + React frontend"
```

---

### Task 13: Update Android CallScreen with SurfaceView for video

**Files:**
- Modify: `android/app/src/main/kotlin/io/visio/mobile/ui/CallScreen.kt`

**Step 1: Add a SurfaceView for each participant with video**

This is the native integration point. When `TrackSubscribed` fires for a video track, the Kotlin side creates a `SurfaceView`, gets the `Surface`, and passes it to `visio_video_attach_surface()` via JNI.

For now, add a placeholder `AndroidView` with `SurfaceView` that calls the native method. The full JNI plumbing will need a helper class.

Create `android/app/src/main/kotlin/io/visio/mobile/VideoSurfaceView.kt`:

```kotlin
package io.visio.mobile

import android.content.Context
import android.view.SurfaceHolder
import android.view.SurfaceView

class VideoSurfaceView(
    context: Context,
    private val trackSid: String
) : SurfaceView(context), SurfaceHolder.Callback {

    init {
        holder.addCallback(this)
    }

    override fun surfaceCreated(holder: SurfaceHolder) {
        // Pass surface to Rust via JNI
        // NativeVideo.attachSurface(trackSid, holder.surface)
    }

    override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) {}

    override fun surfaceDestroyed(holder: SurfaceHolder) {
        // NativeVideo.detachSurface(trackSid)
    }
}
```

Note: Full JNI binding for `visio_video_attach_surface` will require a JNI wrapper in Rust. This is scaffolding for now.

**Step 2: Commit**

```bash
git add android/app/src/main/kotlin/io/visio/mobile/VideoSurfaceView.kt
git commit -m "feat: add VideoSurfaceView for Android video rendering"
```

---

### Task 14: Update iOS CallView with video layer

**Files:**
- Create: `ios/VisioMobile/Views/VideoLayerView.swift`
- Modify: `ios/VisioMobile/Views/CallView.swift`

**Step 1: Create VideoLayerView UIViewRepresentable**

```swift
import SwiftUI
import AVFoundation

struct VideoLayerView: UIViewRepresentable {
    let trackSid: String

    func makeUIView(context: Context) -> VideoDisplayView {
        let view = VideoDisplayView()
        view.trackSid = trackSid
        return view
    }

    func updateUIView(_ uiView: VideoDisplayView, context: Context) {}
}

class VideoDisplayView: UIView {
    var trackSid: String = ""
    private var displayLayer: AVSampleBufferDisplayLayer?

    override func layoutSubviews() {
        super.layoutSubviews()
        displayLayer?.frame = bounds
    }

    func setupDisplayLayer() {
        let layer = AVSampleBufferDisplayLayer()
        layer.videoGravity = .resizeAspect
        layer.frame = bounds
        self.layer.addSublayer(layer)
        displayLayer = layer
    }
}
```

Note: The actual frame delivery from Rust → Swift will use the `visio_video_set_ios_callback` C function. Full wiring deferred to runtime testing.

**Step 2: Commit**

```bash
git add ios/VisioMobile/Views/VideoLayerView.swift
git commit -m "feat: add VideoLayerView for iOS video rendering"
```

---

### Task 15: Verify full build pipeline

**Step 1: Run existing tests**

```bash
cd /Users/mmaudet/work/visio-mobile-v2
cargo test -p visio-core
```

Expected: All tests pass (12+).

**Step 2: Check all crates compile (host)**

```bash
cargo check --workspace
```

**Step 3: Cross-compile for Android**

```bash
cargo ndk -t arm64-v8a build -p visio-ffi -p visio-video --release
```

**Step 4: Cross-compile for iOS**

```bash
cargo build --target aarch64-apple-ios -p visio-ffi -p visio-video --release
```

**Step 5: Verify desktop frontend builds**

```bash
cd crates/visio-desktop/frontend && npm run build
```

**Step 6: Final commit**

```bash
git add -A
git commit -m "chore: verify full build pipeline compiles for all platforms"
```

---

## Execution Notes

- Tasks 1-5 are straightforward build plumbing (low risk)
- Tasks 6-11 are the video pipeline core (medium risk — LiveKit API surface may surprise)
- Task 12 is the React frontend (low risk)
- Tasks 13-14 are native video integration (high risk — JNI/CoreVideo plumbing needs runtime testing)
- Task 15 is verification

The plan is ordered so each task can be committed independently. If a task fails, the previous commits are still valid.

**Key risk:** The `ndk` crate ANativeWindow API and the iOS CoreVideo integration are the trickiest parts. The plan uses simplified approaches (raw pixel writes for Android, callback-based for iOS) that will work but may need optimization later.
