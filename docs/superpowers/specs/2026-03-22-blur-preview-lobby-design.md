# Apply Background Blur/Image in Pre-Join Lobby Preview

**Issue:** [#111](https://github.com/mmaudet/visio-mobile/issues/111)
**Branch:** `feat/blur-preview-111`
**Date:** 2026-03-22

## Problem

The pre-join lobby camera preview does not apply background filters (blur, replacement image) selected by the user. Both Android and iOS use a native camera preview (`LocalCameraPreview` / `LocalCameraPreviewView`) that bypasses the Rust `BlurProcessor` pipeline entirely.

Desktop is unaffected — it already routes preview frames through the same blur pipeline as live calls.

## Root Cause

### Android
- `LocalCameraPreview` in `PreJoinScreen.kt` is a standalone Camera2 → TextureView pipeline that never sends frames to Rust.
- `CameraCapture.previewMode` is never set to `true`.
- `LOCAL_PREVIEW_SURFACE` is not registered for the lobby.

### iOS
- `LocalCameraPreviewView` uses a separate `AVCaptureSession` + `AVCaptureVideoPreviewLayer` with no Rust involvement.
- `CameraCapture.previewMode` is never set to `true`.
- `visio_video_process_preview_frame()` intentionally skips blur processing.

## Solution: Route Lobby Frames Through CameraCapture + BlurProcessor

Replace native camera previews with `CameraCapture` in `previewMode = true`, rendering blurred I420 frames to a platform-native surface.

## Design

### 1. Rust FFI Layer — No New Functions Needed

**Android:** The existing `attachSurface` JNI with `track_sid == "local-camera"` already stores the surface in `LOCAL_PREVIEW_SURFACE` (lib.rs:2579). The lobby `SurfaceView` calls `NativeVideo.attachSurface("local-camera", surface)` to register its surface. The existing `apply_blur_and_preview()` then renders blurred I420 there automatically.

To clear the surface when the lobby is dismissed, call `NativeVideo.detachSurface("local-camera")` (or set `LOCAL_PREVIEW_SURFACE` to `None`). If no `detachSurface("local-camera")` path exists, add a small JNI `nativeClearLocalPreviewSurface()` that sets `LOCAL_PREVIEW_SURFACE` to `None`.

**iOS:** The existing `visio_push_ios_camera_frame` (lib.rs:2348) already applies `BlurProcessor::process_i420()` and delivers via `deliver_i420_to_ios_callback()` with track SID `"local-camera"`. The LiveKit publish step is a no-op when `CAMERA_SOURCE_IOS` is `None` (no connection yet). Therefore, **no new FFI function is needed** — `pushNV12FrameToRust()` can be called from preview mode as-is.

Update `CameraCapture.swift` to always call `pushNV12FrameToRust()` regardless of `previewMode` (remove the `if previewMode` branch).

**Cleanup:** `visio_video_process_preview_frame()` in `crates/visio-video/src/ios.rs` and `pushNV12PreviewFrameToRust()` in `NV12Util.swift` become dead code and will be removed.

### 2. Android Changes

#### Remove `LocalCameraPreview`
Delete the `LocalCameraPreview` class (~150 lines) from `PreJoinScreen.kt`. It is a standalone Camera2 → TextureView pipeline that bypasses the Rust blur.

#### New `BlurredCameraPreview` composable
```kotlin
@Composable
fun BlurredCameraPreview(modifier: Modifier) {
    AndroidView(
        factory = { context ->
            SurfaceView(context).apply {
                holder.addCallback(object : SurfaceHolder.Callback {
                    override fun surfaceCreated(holder: SurfaceHolder) {
                        NativeVideo.attachSurface("local-camera", holder.surface)
                    }
                    override fun surfaceDestroyed(holder: SurfaceHolder) {
                        NativeVideo.nativeClearLocalPreviewSurface()
                    }
                    override fun surfaceChanged(h: SurfaceHolder, f: Int, w: Int, h2: Int) {}
                })
            }
        },
        modifier = modifier,
    )
}
```

Note: `attachSurface` requires a track SID string + Surface. If its current JNI signature does not accept a String + Surface pair cleanly for the `"local-camera"` case, add a thin `nativeClearLocalPreviewSurface()` JNI that sets `LOCAL_PREVIEW_SURFACE` to `None`.

#### `VisioManager.startPreviewCapture()` / `stopPreviewCapture()`

CameraCapture ownership stays centralized in VisioManager (same pattern as call capture):

```kotlin
fun startPreviewCapture() {
    stopPreviewCapture()
    cameraCapture = CameraCapture(appContext).also {
        it.previewMode = true
        it.start()
    }
}

fun stopPreviewCapture() {
    cameraCapture?.stop()
    cameraCapture = null
}
```

#### PreJoinScreen transition (lobby → call)

Use `CameraDevice.StateCallback.onClosed()` instead of a brittle `delay(200)`:

```kotlin
// 1. Stop preview capture (releases Camera2 device)
VisioManager.stopPreviewCapture()
// 2. Clear lobby surface
NativeVideo.nativeClearLocalPreviewSurface()
// 3. Wait for Camera2 device release via onClosed() callback (or fallback delay(300))
// 4. Start normal capture (previewMode = false)
VisioManager.startCameraCapture()
```

If wiring `onClosed()` adds too much complexity, a `delay(300)` fallback is acceptable — `CameraCapture.start()` should handle `openCamera` failures gracefully (log + retry or surface error to UI).

#### Mirror flag for back camera

`apply_blur_and_preview()` (lib.rs:1918) currently hardcodes `mirror: true`. Add a global atomic `IS_FRONT_CAMERA: AtomicBool` in lib.rs, set it from `CameraCapture` when the camera is opened, and read it in `apply_blur_and_preview()` to pass the correct mirror value. This also benefits the in-call self-view when back camera is active.

### 3. iOS Changes

#### Remove `LocalCameraPreviewView`
Delete `LocalCameraPreviewView.swift`. The current implementation uses a separate `AVCaptureSession` + `AVCaptureVideoPreviewLayer` with no Rust involvement.

#### New `BlurredCameraPreviewView` (UIViewRepresentable)

Reuse the existing `VideoDisplayView` + `VideoFrameRouter` pattern instead of building a custom `AVSampleBufferDisplayLayer`:

```swift
struct BlurredCameraPreviewView: UIViewRepresentable {
    let isFront: Bool

    func makeUIView(context: Context) -> VideoDisplayView {
        let view = VideoDisplayView()
        // Register with VideoFrameRouter for "local-camera" frames
        VideoFrameRouter.shared.register(trackSid: "local-camera", view: view)
        // Start preview capture via VisioManager
        VisioManager.shared.startPreviewCapture(isFront: isFront)
        return view
    }

    func dismantleUIView(_ view: VideoDisplayView, context: Context) {
        VisioManager.shared.stopPreviewCapture()
        VideoFrameRouter.shared.unregister(trackSid: "local-camera", view: view)
    }
}
```

Since lobby and call never coexist, `"local-camera"` SID is safe for both — the preview view unregisters before the call self-view registers.

#### VisioManager.swift — centralized CameraCapture ownership

Add `startPreviewCapture(isFront:)` / `stopPreviewCapture()` in VisioManager (mirrors Android pattern):

```swift
func startPreviewCapture(isFront: Bool) {
    stopPreviewCapture()
    let capture = CameraCapture()
    capture.previewMode = true
    capture.start(isFront: isFront)
    self.cameraCapture = capture
}

func stopPreviewCapture() {
    cameraCapture?.stop()
    cameraCapture = nil
}
```

#### CameraCapture.swift — remove preview branch

Remove the `if previewMode` branch in the frame callback. Always call `pushNV12FrameToRust()` — blur is applied by `visio_push_ios_camera_frame`, and LiveKit publish is a no-op when `CAMERA_SOURCE_IOS` is `None`.

#### NV12Util.swift — remove `pushNV12PreviewFrameToRust()`

This function becomes dead code. Remove it.

#### PreJoinView.swift
Replace `LocalCameraPreviewView(isFront:)` with `BlurredCameraPreviewView(isFront:)`.

### 4. Cleanup

- Remove `visio_video_process_preview_frame()` from `crates/visio-video/src/ios.rs`
- Remove `pushNV12PreviewFrameToRust()` from `ios/VisioMobile/NV12Util.swift`
- Remove `deliverLocalPreviewBuffer()` from `ios/VisioMobile/VideoFrameRouter.swift` (dead code)
- Remove `LocalCameraPreview` class from `android/.../PreJoinScreen.kt`
- Remove `LocalCameraPreviewView.swift` from `ios/VisioMobile/Views/`
- Remove dead code in `crates/visio-ffi/src/lib.rs` related to `nativeProcessPreviewFrame` if it becomes unused

### 5. Edge Cases & Degradation

- **ONNX model not loaded:** `BlurProcessor::process_i420()` returns `false` (no-op) if the model is not loaded. Preview falls through to unblurred frames — graceful degradation, no crash.
- **Camera permissions:** Handled by the existing permission flow in PreJoinScreen/PreJoinView before preview starts. No change needed.
- **Camera switching (front ↔ back):** Already handled by `CameraCapture` on both platforms. The `IS_FRONT_CAMERA` atomic ensures correct mirror behavior.
- **Rotation:** Handled by the existing rotation logic in `process_camera_frame_common()` (Android) and `visio_push_ios_camera_frame()` (iOS). No change needed.

## What Does NOT Change

- **Desktop**: already functional, no modifications.
- **BlurProcessor** (process.rs, gaussian.rs, segment.rs): core blur logic unchanged.
- **In-call pipeline**: Android and iOS keep the exact same flow during calls.
- **Background mode selector**: the existing lobby UI for choosing blur/image mode works via `set_background_mode()` — unchanged.
- **Existing tests**: 56 unit tests unaffected.

## File Change Summary

| File | Action |
|------|--------|
| `crates/visio-ffi/src/lib.rs` | + `IS_FRONT_CAMERA` atomic, + `nativeClearLocalPreviewSurface` JNI (Android), mirror fix in `apply_blur_and_preview` |
| `android/.../PreJoinScreen.kt` | Remove `LocalCameraPreview`, add `BlurredCameraPreview` composable |
| `android/.../VisioManager.kt` | + `startPreviewCapture()` / `stopPreviewCapture()` |
| `android/.../CameraCapture.kt` | Set `IS_FRONT_CAMERA` atomic on camera open |
| `ios/.../LocalCameraPreviewView.swift` | **Delete** |
| `ios/.../PreJoinView.swift` | Use `BlurredCameraPreviewView` instead of `LocalCameraPreviewView` |
| `ios/.../VisioManager.swift` | + `startPreviewCapture()` / `stopPreviewCapture()` |
| `ios/.../CameraCapture.swift` | Remove `previewMode` branch — always call `pushNV12FrameToRust()` |
| `ios/.../NV12Util.swift` | Remove `pushNV12PreviewFrameToRust()` |
| `ios/.../VideoFrameRouter.swift` | Remove `deliverLocalPreviewBuffer()` (dead code) |
| `crates/visio-video/src/ios.rs` | Remove `visio_video_process_preview_frame()` |
