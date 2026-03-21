# Pre-Join Lobby Screen — Design Spec

## Overview

Add a pre-join screen (lobby / green room) that allows users to preview and configure audio/video settings before entering a meeting room. Modeled after the Microsoft Teams pre-join experience. Applies to all 3 platforms: Desktop (Tauri), iOS (SwiftUI), Android (Jetpack Compose).

Issue: #55

## Navigation Flow

**Current:** `HomeScreen → CallScreen` (immediate connection)

**New:** `HomeScreen → PreJoinScreen → CallScreen`

When the user clicks "Rejoindre" on HomeScreen, the room URL is validated as today, but instead of connecting, the app navigates to PreJoinScreen with the validated URL + display name. The actual LiveKit connection only happens when the user clicks "Rejoindre maintenant" on PreJoinScreen.

"Annuler" returns to HomeScreen. Same flow on all 3 platforms.

## Screen Layout

### Desktop (two-column)

```
┌─────────────────────────────────────────────────────┐
│                    Visio Mobile                      │
│              [Room name / slug display]               │
│         ┌──────────────────────────┐                 │
│         │  Display name (editable) │                 │
│         └──────────────────────────┘                 │
│                                                       │
│  ┌──────────────────────┐  ┌────────────────────────┐│
│  │                      │  │ ● Son de l'ordinateur  ││
│  │   Camera Preview     │  │                        ││
│  │   (live + effects)   │  │  🎤 Device ▾    [on/off]│
│  │                      │  │  ░░░░░░░░░ (VU meter)  ││
│  │                      │  │  🔊 Device ▾           ││
│  ├──────────────────────┤  │  [🔈 Tester]           ││
│  │ 📷 Device ▾  [on/off]│  │                        ││
│  │ 🎬 Filtres d'arrière-│  │ ○ Ne pas utiliser le   ││
│  │    plan              │  │   son                  ││
│  └──────────────────────┘  └────────────────────────┘│
│                                                       │
│                    [Annuler]  [Rejoindre maintenant]  │
└─────────────────────────────────────────────────────┘
```

### Mobile (stacked vertical)

Same content stacked vertically: camera preview on top, audio config below. Camera device selector on mobile is a **front/back toggle button** (not a dropdown) since mobile devices typically have only 2 cameras. Filter picker opens as a bottom sheet instead of a side panel.

## Left Panel — Camera Preview

- **Live camera feed** rendered through the Rust `visio-video` pipeline with real-time effects (blur, background images)
- **Camera device selector**: dropdown on desktop (if multiple cameras), front/back toggle on mobile
- **Camera on/off toggle** next to the device selector
- **"Filtres d'arrière-plan"** link below the preview, opens the filter side panel

When camera is off, the preview area shows the user's avatar/initials on a dark background (same as in-call behavior).

## Right Panel — Audio Configuration

Two radio options:

### "Son de l'ordinateur" (default)
- **Microphone device** dropdown with on/off toggle
- **Real-time VU meter** bar below mic selector — animated bar showing input level (0.0–1.0)
- **Speaker device** dropdown
- **"Tester" button** — plays a short bundled audio file on the selected output device

### "Ne pas utiliser le son"
- Joins the room with mic and speaker disabled

## Display Name

The display name field on PreJoinScreen is pre-filled from `settings.display_name`. Editing it updates the value passed to `connect()` and also persists to settings for next time.

## Background Filter Side Panel

Opened by clicking "Filtres d'arrière-plan" below the camera preview.

**Desktop:** Side panel slides in from the right, overlaying the audio config panel. Close/back button to return.

**Mobile:** Bottom sheet (half-screen).

**Content — thumbnail grid:**
- "Aucun" (no effect) — raw camera preview thumbnail
- "Flou" (blur) — blurred preview thumbnail
- "Flou léger" (blur-light) — lighter blur thumbnail
- Custom background images — 3–5 bundled default images, shown as thumbnails with user silhouette composited

Currently selected filter has a highlighted border (accent color). Selecting a filter applies it in real-time on the camera preview.

**Thumbnail generation:** One captured frame processed with each effect when the panel opens. Static thumbnails, not live — avoids running N blur pipelines simultaneously.

Custom image upload is out of scope for v1.

## Waiting Room State

When the room requires host approval before joining:

1. User clicks "Rejoindre maintenant"
2. Request is sent to the server for admission
3. Screen stays on PreJoin but transitions to a waiting state:
   - "Rejoindre maintenant" button replaced by disabled state with spinner + "En attente d'autorisation..."
   - Camera preview stays live — user can still adjust settings while waiting
   - Audio/video toggles remain functional
   - "Annuler" still available to leave
4. **Admitted** → transition to CallScreen with current settings
5. **Denied** → error message "L'organisateur a refusé votre accès" + return to HomeScreen on user action
6. **Timeout** (host never responds) → after 60 seconds, show "La demande d'accès a expiré" + return to HomeScreen on user action. Uses the existing `LobbyTimeout` event if available from the server, otherwise a client-side timeout.

## Rust Architecture — Preview Mode

New mode in `visio-core` that starts local capture without connecting to LiveKit.

### New API surface

| Function | Description |
|----------|-------------|
| `start_preview()` | Starts camera capture + blur pipeline, renders to local surface. No room connection. |
| `stop_preview()` | Tears down local capture |
| `play_speaker_test()` | Plays bundled audio file on selected output device |

### Preview capture architecture

The in-call camera pipeline depends on `NativeVideoSource` (obtained from publishing a track to a Room). Preview mode cannot use this path since there is no Room connection.

**Solution: standalone preview pipeline.** `start_preview()` creates its own capture session that writes directly to the native render surface, bypassing `NativeVideoSource` / `CAMERA_SOURCE` entirely:

- **Desktop:** Start a platform camera capture (macOS `AVCaptureSession` / Linux PipeWire) → feed frames through `BlurProcessor` → render to the preview canvas. This is a subset of the existing `camera_macos.rs` / `camera_linux.rs` code, minus the `source.capture_frame()` call to LiveKit.
- **iOS:** Start `AVCaptureSession` in Swift → push frames to `visio-video` C FFI for blur processing → render processed frames to `PreviewVideoView` via CVPixelBuffer. The capture session is owned by the platform layer, blur processing happens in Rust.
- **Android:** Start CameraX capture in Kotlin → push frames to `visio-video` JNI for blur processing → render processed frames to `VideoSurfaceView` via SurfaceTexture. Same pattern as iOS.

**Transition to call:** When the user clicks "Join now", `stop_preview()` tears down the standalone capture session. Then `connect()` starts the normal in-call pipeline with a fresh `NativeVideoSource`. There will be a brief (~200ms) camera restart — acceptable since Teams has the same behavior.

### VU meter — platform-native implementation

The VU meter is implemented **entirely in platform code**, not through FFI:

- **Desktop:** Use `cpal` input stream (already available in `audio_engine.rs`) to compute RMS from the capture buffer. Expose via a new Tauri command `get_mic_level() → f32`, polled every 100ms by the UI.
- **iOS:** Use `AVAudioEngine.inputNode.installTap()` to monitor input levels. Compute RMS in the tap callback, expose as a `@Published` property on `PreJoinView`. No FFI crossing needed.
- **Android:** Use `AudioRecord` with a short buffer read loop in a coroutine. Compute RMS from the PCM buffer, expose as a `MutableState<Float>`. No FFI crossing needed.

This avoids the overhead of polling through UniFFI at high frequency.

### Speaker test — platform-native playback

The speaker test plays a bundled short audio file (e.g., a 2-second chime):

- **Desktop:** Use `cpal` output stream to play a bundled WAV file to the selected output device. New Tauri command `play_speaker_test()`.
- **iOS:** Use `AVAudioPlayer` configured with the current `AVAudioSession` output route. Bundled audio file in the app resources.
- **Android:** Use `MediaPlayer` or `SoundPool` with the selected `AudioDeviceInfo` output. Bundled audio file in `res/raw/`.

On mobile, output device routing is handled by the OS audio session (AVAudioSession / AudioManager), so the test sound plays through whatever the user has selected at the OS level.

### Device enumeration on mobile — platform-native

Device enumeration on mobile **stays in platform code** (not in Rust FFI), because it requires OS-specific APIs that `cpal` doesn't support on mobile:

- **iOS:** `AVAudioSession.availableInputs` / output routes via `AVAudioSession.currentRoute`. Exposed as SwiftUI state in `PreJoinView`.
- **Android:** `AudioManager.getDevices(GET_DEVICES_INPUTS)` / `getDevices(GET_DEVICES_OUTPUTS)`. Exposed as Compose state in `PreJoinScreen`.

No new UniFFI bindings needed for device enumeration. The platform UI directly queries OS APIs and presents device lists.

### Background blur on mobile (new)

All platforms use **ONNX Runtime** (`ort` crate) consistently for the segmentation model:

- **iOS:** The `ort` crate compiles for iOS via `aarch64-apple-ios` target. ONNX Runtime provides a CoreNeuralNetwork execution provider on iOS for hardware acceleration, but the default CPU provider also works. The blur processing runs in the `visio-video` C FFI layer, same as desktop.
- **Android:** ONNX Runtime Android (via `ort` crate compiled for `aarch64-linux-android`). Uses NNAPI execution provider for hardware acceleration where available, falls back to CPU. Same blur processing in `visio-video` JNI layer.
- Same `set_background_mode()` API across all platforms.

**Prerequisite fix:** The `set_background_mode()` handler in `visio-ffi/src/lib.rs` currently does not map `"blur-light"` (it falls through to `Off`). This must be fixed to handle `"blur-light" → BackgroundMode::BlurLight` before or during this feature, matching the desktop implementation in `visio-desktop/src/lib.rs`.

## Settings Persistence

The following are persisted to `SettingsStore` when the user changes them on the PreJoin screen:

| Field | Type | Notes |
|-------|------|-------|
| `audio_input_device` | `Option<String>` | Existing in core, **needs FFI/UDL exposure** |
| `audio_output_device` | `Option<String>` | Existing in core, **needs FFI/UDL exposure** |
| `camera_device` | `Option<String>` | Existing in core, **needs FFI/UDL exposure** |
| `mic_enabled_on_join` | `bool` | Existing, already in FFI |
| `camera_enabled_on_join` | `bool` | Existing, already in FFI |
| `background_mode` | `String` | **New** — `off`, `blur`, `blur-light`, `image:N`. Add to core + FFI/UDL |
| `audio_mode` | `String` | **New** — `computer` or `none`. Add to core + FFI/UDL |

The FFI `Settings` struct and UDL `Settings` dictionary must be updated to include all fields needed by the PreJoin screen (`audio_input_device`, `audio_output_device`, `camera_device`, `background_mode`, `audio_mode`).

Next time the user opens PreJoin, all settings are pre-populated. Preview starts with saved camera + effect, mic in saved state, audio mode pre-selected.

## Platform Implementation

### Desktop (Tauri / React)

- New `PreJoinScreen.tsx` component between HomeScreen and CallView in `App.tsx`
- Camera preview rendered to `<canvas>` via standalone Rust capture pipeline
- Device lists via existing Tauri commands (`list_audio_input_devices`, etc.)
- VU meter: poll `get_mic_level` every 100ms via Tauri command, render as animated CSS bar
- Speaker test: new Tauri command `play_speaker_test`
- Filter panel: CSS slide-in panel from the right

### iOS (SwiftUI)

- New `PreJoinView.swift` in NavigationStack between HomeView and CallView
- Camera preview via new `PreviewVideoView` (same `VideoLayerView` renderer, fed by standalone `AVCaptureSession`)
- Device selectors: platform-native via `AVAudioSession` APIs
- VU meter: `AVAudioEngine` input tap, no FFI needed
- Speaker test: `AVAudioPlayer` with bundled audio file
- Background blur: ONNX Runtime via `visio-video` C FFI, applied to captured frames
- Filter picker: `.sheet` bottom sheet with thumbnail grid

### Android (Jetpack Compose)

- New `PreJoinScreen.kt` composable in nav graph between HomeScreen and CallScreen
- Camera preview via existing `VideoSurfaceView`, fed by standalone CameraX capture
- Device selectors: platform-native via `AudioManager` APIs
- VU meter: `AudioRecord` buffer in coroutine, no FFI needed
- Speaker test: `MediaPlayer` with bundled audio file from `res/raw/`
- Background blur: ONNX Runtime via `visio-video` JNI, applied to captured frames
- Filter picker: `ModalBottomSheet` with thumbnail grid

## Out of Scope (v1)

- Custom background image upload
- "Phone audio" option (no telephony)
- Virtual camera / screen share on pre-join
- Multiple simultaneous blur pipeline renders (thumbnails are static)
- Seamless camera handoff from preview to call (brief restart is acceptable)
