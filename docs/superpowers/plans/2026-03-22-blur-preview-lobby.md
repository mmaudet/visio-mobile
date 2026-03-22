# Blur Preview in Pre-Join Lobby — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Apply background blur/image filters in the pre-join lobby camera preview on Android and iOS.

**Architecture:** Replace native camera previews (`LocalCameraPreview` on Android, `LocalCameraPreviewView` on iOS) with `CameraCapture` in `previewMode = true`. Reuse existing FFI functions — `attachSurface("local-camera")` for Android, `visio_push_ios_camera_frame` for iOS. Remove dead code paths.

**Tech Stack:** Rust (visio-ffi), Kotlin/Jetpack Compose (Android), Swift/SwiftUI (iOS)

**Spec:** `docs/superpowers/specs/2026-03-22-blur-preview-lobby-design.md`

---

## Task 1: Android — Add `IS_FRONT_CAMERA` atomic and fix mirror flag

**Files:**
- Modify: `crates/visio-ffi/src/lib.rs:1890-1923` (apply_blur_and_preview)
- Modify: `crates/visio-ffi/src/lib.rs` (add static near line 1760)

- [ ] **Step 1: Add `IS_FRONT_CAMERA` atomic**

In `crates/visio-ffi/src/lib.rs`, near the `LOCAL_PREVIEW_SURFACE` static (line 1760), add:

```rust
/// Whether the active camera is front-facing. Used to decide mirroring
/// for the local preview surface.
#[cfg(target_os = "android")]
static IS_FRONT_CAMERA: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);
```

- [ ] **Step 2: Use `IS_FRONT_CAMERA` in `apply_blur_and_preview`**

In `apply_blur_and_preview()` (line 1918), replace the hardcoded `true`:

```rust
// Before:
true, // mirror for front-camera self-view

// After:
IS_FRONT_CAMERA.load(std::sync::atomic::Ordering::Relaxed),
```

- [ ] **Step 3: Set `IS_FRONT_CAMERA` from JNI when camera opens**

In `nativePushCameraFrame` and `nativeProcessPreviewFrame`, the rotation parameter already encodes camera info. But the simplest approach: add a new small JNI `nativeSetFrontCamera(isFront: Boolean)`. In `crates/visio-ffi/src/lib.rs`, add after `nativeStopCameraCapture` (line 2143):

```rust
/// JNI: NativeVideo.nativeSetFrontCamera(isFront: Boolean)
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "C" fn Java_io_visio_mobile_NativeVideo_nativeSetFrontCamera(
    _env: *mut jni::sys::JNIEnv,
    _class: jni::sys::jobject,
    is_front: jni::sys::jboolean,
) {
    IS_FRONT_CAMERA.store(is_front != 0, std::sync::atomic::Ordering::Relaxed);
}
```

- [ ] **Step 4: Declare JNI in NativeVideo.kt**

In `android/app/src/main/kotlin/io/visio/mobile/NativeVideo.kt`, add:

```kotlin
external fun nativeSetFrontCamera(isFront: Boolean)
```

- [ ] **Step 5: Call from CameraCapture**

In `android/app/src/main/kotlin/io/visio/mobile/CameraCapture.kt`, in `start()` after line 70 (`isFrontCamera = chars.get(...)`), add:

```kotlin
NativeVideo.nativeSetFrontCamera(isFrontCamera)
```

Also in `switchCamera()` after line 201 (`isFrontCamera = chars.get(...)`), add the same call:

```kotlin
NativeVideo.nativeSetFrontCamera(isFrontCamera)
```

- [ ] **Step 6: Build and verify**

Run: `cargo build -p visio-ffi --target aarch64-linux-android` (or `cargo check -p visio-ffi`)
Expected: compiles without errors.

- [ ] **Step 7: Commit**

```bash
git add crates/visio-ffi/src/lib.rs android/app/src/main/kotlin/io/visio/mobile/NativeVideo.kt android/app/src/main/kotlin/io/visio/mobile/CameraCapture.kt
git commit -m "feat(android): add IS_FRONT_CAMERA atomic for correct mirror in preview"
```

---

## Task 2: Android — Add `nativeClearLocalPreviewSurface` JNI

**Files:**
- Modify: `crates/visio-ffi/src/lib.rs` (after detachSurface, ~line 2672)
- Modify: `android/app/src/main/kotlin/io/visio/mobile/NativeVideo.kt`

The existing `detachSurface("local-camera")` is a no-op by design (to avoid recomposition races during calls). We need a separate function to explicitly clear the surface when the lobby is dismissed.

- [ ] **Step 1: Add JNI function in lib.rs**

After `Java_io_visio_mobile_NativeVideo_detachSurface` (around line 2672), add:

```rust
/// JNI: NativeVideo.nativeClearLocalPreviewSurface()
/// Explicitly clears the LOCAL_PREVIEW_SURFACE. Used when the lobby preview
/// is dismissed (detachSurface("local-camera") is intentionally a no-op
/// during calls to avoid recomposition races).
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "C" fn Java_io_visio_mobile_NativeVideo_nativeClearLocalPreviewSurface(
    _env: *mut jni::sys::JNIEnv,
    _class: jni::sys::jobject,
) {
    visio_log("VISIO JNI: clearing local preview surface (lobby dismissed)");
    LOCAL_PREVIEW_SURFACE.lock().unwrap().take();
}
```

- [ ] **Step 2: Declare in NativeVideo.kt**

Add to `NativeVideo.kt`:

```kotlin
external fun nativeClearLocalPreviewSurface()
```

- [ ] **Step 3: Build and verify**

Run: `cargo check -p visio-ffi`
Expected: compiles without errors.

- [ ] **Step 4: Commit**

```bash
git add crates/visio-ffi/src/lib.rs android/app/src/main/kotlin/io/visio/mobile/NativeVideo.kt
git commit -m "feat(android): add nativeClearLocalPreviewSurface JNI for lobby cleanup"
```

---

## Task 3: Android — Add `startPreviewCapture` / `stopPreviewCapture` in VisioManager

**Files:**
- Modify: `android/app/src/main/kotlin/io/visio/mobile/VisioManager.kt:368-386`

- [ ] **Step 1: Add `startPreviewCapture()` and `stopPreviewCapture()`**

In `VisioManager.kt`, after `stopCameraCapture()` (line 380), add:

```kotlin
/**
 * Start Camera2 capture in preview mode (blur + local render, no LiveKit).
 * Used for the pre-join lobby camera preview.
 */
fun startPreviewCapture() {
    stopCameraCapture() // stop any existing capture
    cameraCapture = CameraCapture(appContext).also {
        it.previewMode = true
        it.start()
    }
}

/**
 * Stop preview capture and clear the local preview surface.
 */
fun stopPreviewCapture() {
    cameraCapture?.stop()
    cameraCapture = null
    NativeVideo.nativeClearLocalPreviewSurface()
}
```

- [ ] **Step 2: Build and verify**

Run: `cd android && ./gradlew compileDebugKotlin`
Expected: compiles without errors.

- [ ] **Step 3: Commit**

```bash
git add android/app/src/main/kotlin/io/visio/mobile/VisioManager.kt
git commit -m "feat(android): add startPreviewCapture/stopPreviewCapture in VisioManager"
```

---

## Task 4: Android — Replace `LocalCameraPreview` with `BlurredCameraPreview`

**Files:**
- Modify: `android/app/src/main/kotlin/io/visio/mobile/ui/PreJoinScreen.kt:134-285` (delete LocalCameraPreview)
- Modify: `android/app/src/main/kotlin/io/visio/mobile/ui/PreJoinScreen.kt:1027-1067` (PreJoinCameraSection)
- Modify: `android/app/src/main/kotlin/io/visio/mobile/ui/PreJoinScreen.kt:558-576` (join transition)

- [ ] **Step 1: Delete `LocalCameraPreview` class**

Remove lines 132-285 of `PreJoinScreen.kt` (the entire `LocalCameraPreview` class including the comment header).

- [ ] **Step 2: Add `BlurredCameraPreview` composable**

In the same location (before `PreJoinScreen` composable), add:

```kotlin
// ── Blurred camera preview (CameraCapture → Rust blur → SurfaceView) ────────

@Composable
private fun BlurredCameraPreview(
    isFrontCamera: Boolean,
    modifier: Modifier = Modifier,
) {
    DisposableEffect(Unit) {
        VisioManager.startPreviewCapture()
        onDispose {
            VisioManager.stopPreviewCapture()
        }
    }

    // Switch camera when user toggles front/back
    LaunchedEffect(isFrontCamera) {
        VisioManager.switchCamera(isFrontCamera)
    }

    AndroidView(
        factory = { context ->
            android.view.SurfaceView(context).apply {
                holder.addCallback(
                    object : android.view.SurfaceHolder.Callback {
                        override fun surfaceCreated(holder: android.view.SurfaceHolder) {
                            NativeVideo.attachSurface("local-camera", holder.surface)
                        }

                        override fun surfaceChanged(
                            holder: android.view.SurfaceHolder,
                            format: Int,
                            width: Int,
                            height: Int,
                        ) {
                            // Re-attach on size change to update ANativeWindow dimensions
                            NativeVideo.attachSurface("local-camera", holder.surface)
                        }

                        override fun surfaceDestroyed(holder: android.view.SurfaceHolder) {
                            // Cleanup handled by DisposableEffect → stopPreviewCapture()
                        }
                    },
                )
            }
        },
        modifier = modifier,
    )
}
```

- [ ] **Step 3: Update `PreJoinCameraSection` to use `BlurredCameraPreview`**

In `PreJoinCameraSection` (around line 1027), replace the parameter:

```kotlin
// Remove this parameter:
cameraPreviewRef: androidx.compose.runtime.MutableState<LocalCameraPreview?>,
```

Replace the `AndroidView` block (lines 1053-1066) with:

```kotlin
if (cameraEnabled && hasCameraPermission) {
    BlurredCameraPreview(
        isFrontCamera = isFrontCamera,
        modifier = Modifier
            .fillMaxSize()
            .clip(RoundedCornerShape(12.dp)),
    )
} else {
```

- [ ] **Step 4: Update join transition**

In the join handler (around lines 558-576), remove the `LocalCameraPreview` stop logic:

```kotlin
// Remove these lines:
withContext(Dispatchers.Main) {
    cameraPreviewRef.value?.stopCamera()
    cameraPreviewRef.value = null
}
// Small delay to let Camera2 fully release the device
kotlinx.coroutines.delay(200)
```

Replace with:

```kotlin
// Stop the preview capture to release Camera2 before starting call capture.
// DisposableEffect handles this when the composable leaves, but we need
// it explicitly here because the composable may still be mounted.
VisioManager.stopPreviewCapture()
kotlinx.coroutines.delay(300)
```

- [ ] **Step 5: Remove `cameraPreviewRef` state**

Search for all references to `cameraPreviewRef` in `PreJoinScreen.kt` and remove them:
- The `remember { mutableStateOf<LocalCameraPreview?>(null) }` declaration
- The `cameraPreviewRef = cameraPreviewRef` parameter in `PreJoinCameraSection` calls

- [ ] **Step 6: Clean up unused imports**

Remove imports that were only used by `LocalCameraPreview`:
- `android.graphics.Matrix`
- `android.graphics.SurfaceTexture`
- `android.hardware.camera2.CameraCaptureSession`
- `android.hardware.camera2.CameraCharacteristics`
- `android.hardware.camera2.CameraDevice`
- `android.hardware.camera2.CameraManager`
- `android.os.Handler`
- `android.os.HandlerThread`
- `android.view.Surface`
- `android.view.TextureView`

Keep any imports still used by other code in the file.

- [ ] **Step 7: Build and verify**

Run: `cd android && ./gradlew compileDebugKotlin`
Expected: compiles without errors.

- [ ] **Step 8: Commit**

```bash
git add android/app/src/main/kotlin/io/visio/mobile/ui/PreJoinScreen.kt
git commit -m "feat(android): replace LocalCameraPreview with BlurredCameraPreview (#111)"
```

---

## Task 5: iOS — Remove preview branch in CameraCapture and NV12Util

**Files:**
- Modify: `ios/VisioMobile/CameraCapture.swift:20,229-233`
- Modify: `ios/VisioMobile/NV12Util.swift:65-74`

- [ ] **Step 1: Remove `previewMode` property from CameraCapture.swift**

In `CameraCapture.swift`, remove line 20:

```swift
// Remove:
var previewMode: Bool = false
```

- [ ] **Step 2: Remove `if previewMode` branch in `captureOutput`**

In `CameraCapture.swift` lines 229-233, replace:

```swift
if previewMode {
    pushNV12PreviewFrameToRust(pixelBuffer, uPlane: &uPlane, vPlane: &vPlane)
} else {
    pushNV12FrameToRust(pixelBuffer, uPlane: &uPlane, vPlane: &vPlane)
}
```

With:

```swift
pushNV12FrameToRust(pixelBuffer, uPlane: &uPlane, vPlane: &vPlane)
```

- [ ] **Step 3: Remove `pushNV12PreviewFrameToRust` from NV12Util.swift**

Delete lines 65-74 of `NV12Util.swift` (the entire `pushNV12PreviewFrameToRust` function).

- [ ] **Step 4: Build and verify**

Run: `xcodebuild -project ios/VisioMobile.xcodeproj -scheme VisioMobile -destination 'generic/platform=iOS' build CODE_SIGNING_ALLOWED=NO 2>&1 | tail -5`
Expected: BUILD SUCCEEDED (or check with `cargo check -p visio-ffi --target aarch64-apple-ios`).

- [ ] **Step 5: Commit**

```bash
git add ios/VisioMobile/CameraCapture.swift ios/VisioMobile/NV12Util.swift
git commit -m "refactor(ios): remove previewMode branch — always use blur pipeline"
```

---

## Task 6: iOS — Replace `LocalCameraPreviewView` with `BlurredCameraPreviewView`

**Files:**
- Rewrite: `ios/VisioMobile/Views/LocalCameraPreviewView.swift` → `BlurredCameraPreviewView`
- Modify: `ios/VisioMobile/Views/PreJoinView.swift:252`
- Modify: `ios/VisioMobile/VisioManager.swift` (add startPreviewCapture/stopPreviewCapture)

- [ ] **Step 1: Add `startPreviewCapture` / `stopPreviewCapture` in VisioManager.swift**

In `ios/VisioMobile/VisioManager.swift`, add after the existing camera-related functions (near line 762 `switchCamera`):

```swift
/// Start camera capture in preview mode for the pre-join lobby.
/// Frames go through the Rust blur pipeline and are delivered via
/// VideoFrameRouter with track SID "local-camera".
func startPreviewCapture(isFront: Bool) {
    cameraCapture?.stop()
    let capture = CameraCapture()
    capture.start()
    if !isFront {
        capture.switchCamera(toFront: false)
    }
    cameraCapture = capture
}

/// Stop the preview camera capture.
func stopPreviewCapture() {
    cameraCapture?.stop()
    cameraCapture = nil
}
```

Note: We no longer need `previewMode` since `CameraCapture` now always calls `pushNV12FrameToRust`. When `CAMERA_SOURCE_IOS` is `None` (no LiveKit connection), the LiveKit publish in `visio_push_ios_camera_frame` is a no-op. Frames still reach the iOS callback via `deliver_i420_to_ios_callback` with SID `"local-camera"`.

- [ ] **Step 2: Rewrite `LocalCameraPreviewView.swift` → `BlurredCameraPreviewView`**

Replace the entire content of `ios/VisioMobile/Views/LocalCameraPreviewView.swift`:

```swift
import SwiftUI

/// Camera preview that routes frames through the Rust blur pipeline.
/// Reuses VideoLayerView + VideoFrameRouter to display processed I420 frames.
struct BlurredCameraPreviewView: UIViewRepresentable {
    let isFront: Bool
    @EnvironmentObject private var manager: VisioManager

    func makeUIView(context: Context) -> VideoDisplayView {
        let view = VideoDisplayView()
        view.setupDisplayLayer(fill: true)
        VideoFrameRouter.shared.register(trackSid: "local-camera", view: view)
        manager.startPreviewCapture(isFront: isFront)
        return view
    }

    func updateUIView(_ uiView: VideoDisplayView, context: Context) {
        // Switch camera direction if user toggles front/back
        manager.cameraCapture?.switchCamera(toFront: isFront)
    }

    static func dismantleUIView(_ uiView: VideoDisplayView, coordinator: ()) {
        VideoFrameRouter.shared.unregister(trackSid: "local-camera", view: uiView)
        // stopPreviewCapture is called by the parent view's onDisappear or join transition
    }
}
```

- [ ] **Step 3: Update PreJoinView.swift**

In `ios/VisioMobile/Views/PreJoinView.swift` line 252, replace:

```swift
LocalCameraPreviewView(isFront: isFrontCamera)
```

With:

```swift
BlurredCameraPreviewView(isFront: isFrontCamera)
```

- [ ] **Step 4: Rename the file**

```bash
git mv ios/VisioMobile/Views/LocalCameraPreviewView.swift ios/VisioMobile/Views/BlurredCameraPreviewView.swift
```

Update the Xcode project file: the `git mv` handles the filesystem, but the `.pbxproj` references the old filename. Search and replace `LocalCameraPreviewView.swift` → `BlurredCameraPreviewView.swift` in `ios/VisioMobile.xcodeproj/project.pbxproj`.

- [ ] **Step 5: Update join transition in VisioManager**

In the `connect` and `connectWithToken` functions of `VisioManager.swift`, the camera capture is already started after connection. The preview capture needs to be stopped before starting the call capture. In the connect flow (around lines 182-186), before starting the call `CameraCapture`, add:

```swift
// Stop preview capture before starting call capture
// (releases the physical camera device)
await MainActor.run { [weak self] in
    self?.cameraCapture?.stop()
    self?.cameraCapture = nil
}
```

This should go before the `if cam { let capture = CameraCapture() ... }` block.

- [ ] **Step 6: Build and verify**

Run: `xcodebuild -project ios/VisioMobile.xcodeproj -scheme VisioMobile -destination 'generic/platform=iOS' build CODE_SIGNING_ALLOWED=NO 2>&1 | tail -5`
Expected: BUILD SUCCEEDED.

- [ ] **Step 7: Commit**

```bash
git add ios/VisioMobile/Views/BlurredCameraPreviewView.swift ios/VisioMobile/Views/PreJoinView.swift ios/VisioMobile/VisioManager.swift ios/VisioMobile.xcodeproj/project.pbxproj
git commit -m "feat(ios): replace LocalCameraPreviewView with BlurredCameraPreviewView (#111)"
```

---

## Task 7: Cleanup — Remove dead code

**Files:**
- Modify: `crates/visio-video/src/ios.rs:139-228` (remove `visio_video_process_preview_frame`)
- Modify: `ios/VisioMobile/VideoFrameRouter.swift:35-92` (remove `deliverLocalPreviewBuffer`)
- Modify: `crates/visio-ffi/src/lib.rs:2087-2131` (remove `nativeProcessPreviewFrame` JNI)
- Modify: `android/app/src/main/kotlin/io/visio/mobile/NativeVideo.kt:40-66` (remove JNI declaration)

- [ ] **Step 1: Remove `visio_video_process_preview_frame` from ios.rs**

Delete lines 139-228 of `crates/visio-video/src/ios.rs`.

- [ ] **Step 2: Remove `deliverLocalPreviewBuffer` from VideoFrameRouter.swift**

Delete lines 35-92 of `ios/VisioMobile/VideoFrameRouter.swift`.

- [ ] **Step 3: Remove `nativeProcessPreviewFrame` JNI from lib.rs**

Delete lines 2087-2131 of `crates/visio-ffi/src/lib.rs` (the entire `Java_io_visio_mobile_NativeVideo_nativeProcessPreviewFrame` function and its doc comment).

- [ ] **Step 4: Remove JNI declaration from NativeVideo.kt**

Delete lines 40-66 of `android/app/src/main/kotlin/io/visio/mobile/NativeVideo.kt` (the `nativeProcessPreviewFrame` external declaration and its doc comment).

- [ ] **Step 5: Build both platforms**

Run: `cargo check -p visio-ffi && cargo check -p visio-video`
Expected: compiles without errors on both crates.

- [ ] **Step 6: Commit**

```bash
git add crates/visio-video/src/ios.rs ios/VisioMobile/VideoFrameRouter.swift crates/visio-ffi/src/lib.rs android/app/src/main/kotlin/io/visio/mobile/NativeVideo.kt
git commit -m "refactor: remove dead preview code paths (#111)"
```

---

## Task 8: Final verification

- [ ] **Step 1: Run Rust tests**

Run: `cargo test -p visio-core -p visio-ffi`
Expected: all 56 tests pass (48 visio-core + 8 desktop audio).

- [ ] **Step 2: Build Android**

Run: `cd android && ./gradlew assembleDebug`
Expected: BUILD SUCCESSFUL.

- [ ] **Step 3: Build iOS**

Run: `xcodebuild -project ios/VisioMobile.xcodeproj -scheme VisioMobile -destination 'generic/platform=iOS' build CODE_SIGNING_ALLOWED=NO 2>&1 | tail -5`
Expected: BUILD SUCCEEDED.

- [ ] **Step 4: Check for compilation warnings**

Review build output for any warnings related to unused code, unreachable patterns, etc.
