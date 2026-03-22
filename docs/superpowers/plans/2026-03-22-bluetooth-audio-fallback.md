# Bluetooth Audio Device Fallback — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Auto-fallback to the default audio device when a Bluetooth device disconnects, in both the pre-join lobby and active rooms.

**Architecture:** The macOS backend already emits `audio-devices-changed` Tauri events on device add/remove (via CoreAudio HAL listener). The frontend needs to listen for this event, re-enumerate devices, and if the selected device is gone, switch to the default. For the pre-join lobby, also register the device change callback since it's currently only set during `connect()`.

**Tech Stack:** Rust (cpal, CoreAudio HAL), React (Tauri events), Kotlin (AudioDeviceCallback), Swift (AVAudioSession)

**Issue:** [#83](https://github.com/mmaudet/visio-mobile/issues/83)

---

## File Structure

### Modified Files
- `crates/visio-desktop/src/lib.rs` — register device change callback in `start_mic_preview` (lobby)
- `crates/visio-desktop/frontend/src/App.tsx` — listen to `audio-devices-changed`, re-enumerate, fallback to default
- `android/app/src/main/kotlin/io/visio/mobile/VisioManager.kt` — register AudioDeviceCallback for fallback
- `ios/VisioMobile/VisioManager.swift` — handle routeChangeNotification for fallback

---

## Task 1: Desktop — Register device change callback in pre-join lobby

**Files:**
- Modify: `crates/visio-desktop/src/lib.rs:1517-1524` (start_mic_preview command)

Currently, the `set_device_change_callback` is only called during `connect()` (line 473) and `connect_with_token()` (line 505). The pre-join lobby uses `start_mic_preview()` but never registers the callback, so device changes during lobby are invisible.

- [ ] **Step 1: Add device change callback registration in start_mic_preview**

In `crates/visio-desktop/src/lib.rs`, find `start_mic_preview` (around line 1517) and add callback registration:

```rust
#[tauri::command]
fn start_mic_preview(state: tauri::State<'_, VisioState>) -> Result<(), String> {
    let mut engine = state.audio_engine.lock().unwrap_or_else(|e| e.into_inner());
    let device = state.selected_input_device.lock().unwrap().clone();
    // Register device change callback so frontend is notified during lobby
    engine.set_device_change_callback(Arc::new(|| {
        tracing::info!("audio devices changed (lobby) — re-enumerating");
        if let Some(app) = APP_HANDLE.get() {
            let _ = app.emit("audio-devices-changed", ());
        }
    }));
    engine.start_preview_capture(device.as_deref())
        .map_err(|e| e.to_string())
}
```

- [ ] **Step 2: Verify build**

Run: `cargo build -p visio-desktop`
Expected: compiles

- [ ] **Step 3: Commit**

```bash
git add crates/visio-desktop/src/lib.rs
git commit -m "fix(desktop): register device change callback in pre-join lobby"
```

---

## Task 2: Desktop — Frontend listens to device changes and auto-fallback

**Files:**
- Modify: `crates/visio-desktop/frontend/src/App.tsx` (PreJoinScreen component, around line 3596)

The backend emits `audio-devices-changed` but the PreJoinScreen never listens for it. When a Bluetooth device disconnects, the frontend needs to re-enumerate devices and switch to the default if the selected device is gone.

- [ ] **Step 1: Add useEffect to listen for audio-devices-changed in PreJoinScreen**

In the PreJoinScreen component (after the existing useEffect that loads device lists on mount), add:

```tsx
  // Listen for device changes (Bluetooth disconnect, etc.)
  useEffect(() => {
    let unlisten: (() => void) | null = null
    listen('audio-devices-changed', async () => {
      try {
        const [inputs, outputs] = await Promise.all([
          invoke<AudioDeviceInfo[]>('list_audio_input_devices'),
          invoke<AudioDeviceInfo[]>('list_audio_output_devices'),
        ])
        setInputDevices(inputs)
        setOutputDevices(outputs)

        // If selected input device is no longer available, fallback to default
        if (selectedInput && !inputs.find((d) => d.name === selectedInput)) {
          const defaultInput = inputs.find((d) => d.is_default)
          if (defaultInput) {
            setSelectedInput(defaultInput.name)
            await invoke('select_audio_input', { deviceName: defaultInput.name })
            // Restart mic preview with new device
            if (isMicOn) {
              await invoke('stop_mic_preview')
              await invoke('start_mic_preview')
            }
          }
        }

        // If selected output device is no longer available, fallback to default
        if (selectedOutput && !outputs.find((d) => d.name === selectedOutput)) {
          const defaultOutput = outputs.find((d) => d.is_default)
          if (defaultOutput) {
            setSelectedOutput(defaultOutput.name)
            await invoke('select_audio_output', { deviceName: defaultOutput.name })
          }
        }
      } catch (e) {
        console.warn('Failed to re-enumerate audio devices:', e)
      }
    }).then((fn) => {
      unlisten = fn
    })
    return () => {
      if (unlisten) unlisten()
    }
  }, [selectedInput, selectedOutput, isMicOn])
```

- [ ] **Step 2: Also add the same listener in the call view (App component)**

Find the main App component's call view section. Add a similar listener for `audio-devices-changed` that re-enumerates and falls back during active calls. Look for where `audio-devices-changed` might already be referenced in the in-call settings, and if not, add a top-level listener in App that handles fallback during calls.

- [ ] **Step 3: Verify build**

The frontend auto-rebuilds via Vite. Check the browser console for TypeScript errors.

- [ ] **Step 4: Commit**

```bash
git add crates/visio-desktop/frontend/src/App.tsx
git commit -m "fix(desktop): auto-fallback to default device on Bluetooth disconnect"
```

---

## Task 3: Android — Register AudioDeviceCallback for fallback

**Files:**
- Modify: `android/app/src/main/kotlin/io/visio/mobile/VisioManager.kt`

Android already has `AudioDeviceCallback` in `ContextDetector.kt` for adaptive mode, but `VisioManager` doesn't react to device removal by falling back. Add a callback that detects when the active Bluetooth device disappears and switches to the default.

- [ ] **Step 1: Add AudioDeviceCallback in VisioManager**

In `VisioManager.kt`, add a device callback that triggers on device removal:

```kotlin
private val audioDeviceCallback = object : AudioDeviceCallback() {
    override fun onAudioDevicesRemoved(removedDevices: Array<out AudioDeviceInfo>?) {
        // If a Bluetooth device was removed, restart audio with default device
        val hadBluetooth = removedDevices?.any {
            it.type == AudioDeviceInfo.TYPE_BLUETOOTH_SCO ||
            it.type == AudioDeviceInfo.TYPE_BLUETOOTH_A2DP ||
            it.type == AudioDeviceInfo.TYPE_BLE_HEADSET
        } == true
        if (hadBluetooth) {
            tracing.info("Bluetooth device removed — falling back to default")
            audioCapture?.setPreferredDevice(null) // null = system default
            audioPlayout?.setPreferredDevice(null)
        }
    }
}
```

Register it in `initialize()`:

```kotlin
val audioManager = context.getSystemService(Context.AUDIO_SERVICE) as AudioManager
audioManager.registerAudioDeviceCallback(audioDeviceCallback, null)
```

- [ ] **Step 2: Verify build**

Run: `cd android && ./gradlew compileDebugKotlin`

- [ ] **Step 3: Commit**

```bash
git add android/app/src/main/kotlin/io/visio/mobile/VisioManager.kt
git commit -m "fix(android): auto-fallback to default audio on Bluetooth disconnect"
```

---

## Task 4: iOS — Handle route change for fallback

**Files:**
- Modify: `ios/VisioMobile/VisioManager.swift`

iOS already observes `routeChangeNotification` in `ContextDetector.swift` for adaptive mode. Add handling in `VisioManager` to detect when a Bluetooth route becomes unavailable and reset to the built-in device.

- [ ] **Step 1: Add route change observer in VisioManager**

In `VisioManager.swift`, add in the initializer or `connect()` method:

```swift
NotificationCenter.default.addObserver(
    forName: AVAudioSession.routeChangeNotification,
    object: nil,
    queue: .main
) { [weak self] notification in
    guard let reason = notification.userInfo?[AVAudioSessionRouteChangeReasonKey] as? UInt,
          reason == AVAudioSession.RouteChangeReason.oldDeviceUnavailable.rawValue else {
        return
    }
    // Bluetooth device disconnected — iOS automatically routes to built-in
    // but we should log and update any UI state
    print("Audio route changed: old device unavailable, falling back to default")
}
```

Note: iOS automatically falls back to the built-in speaker when Bluetooth disconnects. The main concern is ensuring the UI reflects this change. If there's a device selector UI, it should update.

- [ ] **Step 2: Commit**

```bash
git add ios/VisioMobile/VisioManager.swift
git commit -m "fix(ios): log audio route change on Bluetooth disconnect"
```

---

## Task 5: Test and verify

- [ ] **Step 1: Desktop test**

1. Connect Bluetooth headset
2. Open pre-join lobby → verify Bluetooth shown as selected device
3. Disconnect Bluetooth → verify dropdown switches to default built-in device
4. Verify VU meter continues working with the new device

- [ ] **Step 2: Build all platforms**

```bash
cargo build -p visio-desktop
cd android && ./gradlew assembleDebug
```
