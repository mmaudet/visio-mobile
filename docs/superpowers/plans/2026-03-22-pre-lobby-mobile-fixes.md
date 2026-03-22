# Pre-Lobby Mobile Fixes — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix 4 bugs on mobile pre-lobby screen: camera preview not showing, wrong audio label, missing audio route selector, mic VU meter not working.

**Architecture:** Permission requests added before camera/mic activation on both platforms. i18n key `prejoin.computerAudio` renamed to device-neutral wording. Audio route picker added (iOS: `AVRoutePickerView`, Android: `AudioManager.getDevices()` picker).

**Tech Stack:** Kotlin/Jetpack Compose (Android), Swift/SwiftUI (iOS), i18n JSON files

**Issue:** [#87](https://github.com/mmaudet/visio-mobile/issues/87)

---

## File Structure

| Action | File | Responsibility |
|--------|------|----------------|
| Modify | `i18n/en.json` | Rename `prejoin.computerAudio` → "Device audio", add `prejoin.audioRoute` |
| Modify | `i18n/fr.json` | Same |
| Modify | `i18n/de.json` | Same |
| Modify | `i18n/es.json` | Same |
| Modify | `i18n/it.json` | Same |
| Modify | `i18n/nl.json` | Same |
| Modify | `android/app/src/main/kotlin/io/visio/mobile/ui/PreJoinScreen.kt` | Add permission launchers, audio route picker |
| Modify | `ios/VisioMobile/Views/PreJoinView.swift` | Add permission requests, audio route picker |
| Create | `ios/VisioMobile/Views/AudioRoutePickerButton.swift` | UIViewRepresentable wrapping `AVRoutePickerView` |

---

### Task 1: Update i18n labels

**Files:**
- Modify: `i18n/en.json:201`
- Modify: `i18n/fr.json:201`
- Modify: `i18n/de.json:201`
- Modify: `i18n/es.json:201`
- Modify: `i18n/it.json:201`
- Modify: `i18n/nl.json:201`

- [ ] **Step 1: Update all 6 i18n files**

Change `prejoin.computerAudio` value and add `prejoin.audioRoute` key in each file:

| File | `prejoin.computerAudio` new value | `prejoin.audioRoute` value |
|------|-----------------------------------|---------------------------|
| en.json | `"Device audio"` | `"Audio route"` |
| fr.json | `"Audio de l'appareil"` | `"Sortie audio"` |
| de.json | `"Geräteaudio"` | `"Audioausgang"` |
| es.json | `"Audio del dispositivo"` | `"Salida de audio"` |
| it.json | `"Audio del dispositivo"` | `"Uscita audio"` |
| nl.json | `"Apparaataudio"` | `"Audio-uitgang"` |

Add `prejoin.audioRoute` after `prejoin.computerAudio` (line 202) in each file.

- [ ] **Step 2: Commit**

```bash
git add i18n/
git commit -m "fix(i18n): rename 'computer audio' to 'device audio' for mobile"
```

---

### Task 2: Android — Add camera & mic permission requests in PreJoinScreen

**Files:**
- Modify: `android/app/src/main/kotlin/io/visio/mobile/ui/PreJoinScreen.kt`

The existing `CallScreen.kt` already uses `rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission())` for camera and mic — follow the same pattern.

- [ ] **Step 1: Add imports**

Add these imports at the top of `PreJoinScreen.kt` (after existing imports around line 91):

```kotlin
import android.util.Log
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
```

- [ ] **Step 2: Add permission launchers inside PreJoinScreen composable**

Insert after `val coroutineScope = rememberCoroutineScope()` (line 230), before the display name state:

```kotlin
    // Camera permission launcher
    val cameraPermissionLauncher =
        rememberLauncherForActivityResult(
            ActivityResultContracts.RequestPermission(),
        ) { granted ->
            if (granted) {
                cameraEnabled = true
            }
        }

    // Mic permission launcher
    val micPermissionLauncher =
        rememberLauncherForActivityResult(
            ActivityResultContracts.RequestPermission(),
        ) { granted ->
            if (granted) {
                micEnabled = true
            }
        }
```

- [ ] **Step 3: Gate camera toggle on permission**

Replace the camera toggle clickable (line 477 `.clickable { cameraEnabled = !cameraEnabled }`) with:

```kotlin
.clickable {
    if (!cameraEnabled) {
        val hasPermission = ContextCompat.checkSelfPermission(
            context, Manifest.permission.CAMERA
        ) == PackageManager.PERMISSION_GRANTED
        if (hasPermission) {
            cameraEnabled = true
        } else {
            cameraPermissionLauncher.launch(Manifest.permission.CAMERA)
        }
    } else {
        cameraEnabled = false
    }
}
```

- [ ] **Step 4: Gate mic toggle on permission**

Replace the mic `Switch` `onCheckedChange` (line 538 `onCheckedChange = { micEnabled = it }`) with:

```kotlin
onCheckedChange = { wantEnabled ->
    if (wantEnabled) {
        val hasPermission = ContextCompat.checkSelfPermission(
            context, Manifest.permission.RECORD_AUDIO
        ) == PackageManager.PERMISSION_GRANTED
        if (hasPermission) {
            micEnabled = true
        } else {
            micPermissionLauncher.launch(Manifest.permission.RECORD_AUDIO)
        }
    } else {
        micEnabled = false
    }
},
```

- [ ] **Step 5: Commit**

```bash
git add android/app/src/main/kotlin/io/visio/mobile/ui/PreJoinScreen.kt
git commit -m "fix(android): request camera and mic permissions from pre-lobby"
```

---

### Task 3: Android — Add audio route picker

**Files:**
- Modify: `android/app/src/main/kotlin/io/visio/mobile/ui/PreJoinScreen.kt`

Add an audio route selector button that shows available audio outputs (speaker, earpiece, Bluetooth) using `AudioManager.getDevices()`.

- [ ] **Step 1: Add AudioManager imports**

```kotlin
import android.media.AudioDeviceInfo
import android.media.AudioManager
```

- [ ] **Step 2: Add audio route state and device list**

Insert after the `vuLevel` state (line 256):

```kotlin
    // ── Audio route state ──────────────────────────────────────────────────
    var showAudioRouteMenu by remember { mutableStateOf(false) }
    val audioManager = remember { context.getSystemService(Context.AUDIO_SERVICE) as AudioManager }
    val audioOutputDevices = remember(showAudioRouteMenu) {
        audioManager.getDevices(AudioManager.GET_DEVICES_OUTPUTS)
            .filter { device ->
                device.type in listOf(
                    AudioDeviceInfo.TYPE_BUILTIN_SPEAKER,
                    AudioDeviceInfo.TYPE_BUILTIN_EARPIECE,
                    AudioDeviceInfo.TYPE_BLUETOOTH_SCO,
                    AudioDeviceInfo.TYPE_BLUETOOTH_A2DP,
                    AudioDeviceInfo.TYPE_BLE_HEADSET,
                    AudioDeviceInfo.TYPE_WIRED_HEADSET,
                    AudioDeviceInfo.TYPE_WIRED_HEADPHONES,
                    AudioDeviceInfo.TYPE_USB_HEADSET,
                )
            }
    }
    var selectedAudioRoute by remember { mutableStateOf<String?>(null) }
```

- [ ] **Step 3: Add audio route button in the audio config section**

Insert after the speaker test / VU meter block (inside the `if (audioMode == "computer_audio")` block, after the VU meter around line 570), before the closing `}` of the computer_audio block:

```kotlin
                    // Audio route selector
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(start = 40.dp),
                        horizontalArrangement = Arrangement.SpaceBetween,
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Text(
                            text = Strings.t("prejoin.audioRoute", lang),
                            style = MaterialTheme.typography.bodyMedium,
                            color = if (isDark) VisioColors.White else VisioColors.LightOnBackground,
                        )
                        Box {
                            OutlinedButton(
                                onClick = { showAudioRouteMenu = true },
                                shape = RoundedCornerShape(8.dp),
                            ) {
                                Icon(
                                    painter = painterResource(R.drawable.ri_volume_up_line),
                                    contentDescription = null,
                                    modifier = Modifier.size(16.dp),
                                )
                                Spacer(modifier = Modifier.size(4.dp))
                                Text(
                                    text = selectedAudioRoute ?: audioOutputDevices.firstOrNull()
                                        ?.productName?.toString() ?: "Speaker",
                                    style = MaterialTheme.typography.bodySmall,
                                )
                            }
                            androidx.compose.material3.DropdownMenu(
                                expanded = showAudioRouteMenu,
                                onDismissRequest = { showAudioRouteMenu = false },
                            ) {
                                audioOutputDevices.forEach { device ->
                                    val label = device.productName?.toString()
                                        ?: audioDeviceTypeLabel(device.type)
                                    androidx.compose.material3.DropdownMenuItem(
                                        text = { Text(label) },
                                        onClick = {
                                            selectedAudioRoute = label
                                            showAudioRouteMenu = false
                                            // Note: actual route switching happens at join time
                                            // via AudioManager.MODE_IN_COMMUNICATION
                                        },
                                    )
                                }
                            }
                        }
                    }
```

- [ ] **Step 4: Add helper function for device type labels**

Add at the bottom of the file, before the closing:

```kotlin
private fun audioDeviceTypeLabel(type: Int): String = when (type) {
    AudioDeviceInfo.TYPE_BUILTIN_SPEAKER -> "Speaker"
    AudioDeviceInfo.TYPE_BUILTIN_EARPIECE -> "Earpiece"
    AudioDeviceInfo.TYPE_BLUETOOTH_SCO,
    AudioDeviceInfo.TYPE_BLUETOOTH_A2DP -> "Bluetooth"
    AudioDeviceInfo.TYPE_BLE_HEADSET -> "BLE Headset"
    AudioDeviceInfo.TYPE_WIRED_HEADSET,
    AudioDeviceInfo.TYPE_WIRED_HEADPHONES -> "Wired"
    AudioDeviceInfo.TYPE_USB_HEADSET -> "USB"
    else -> "Audio"
}
```

- [ ] **Step 5: Verify `ri_volume_up_line` drawable exists**

```bash
ls android/app/src/main/res/drawable/ri_volume_up_line.*
```

If missing, use `Icons.Filled.VolumeUp` from Material Icons instead (import `androidx.compose.material.icons.filled.VolumeUp`).

- [ ] **Step 6: Commit**

```bash
git add android/app/src/main/kotlin/io/visio/mobile/ui/PreJoinScreen.kt
git commit -m "feat(android): add audio route picker in pre-lobby"
```

---

### Task 4: iOS — Add camera & mic permission requests in PreJoinView

**Files:**
- Modify: `ios/VisioMobile/Views/PreJoinView.swift`

The existing `VisioManager.swift` already has `ensureMediaPermissions(mic:camera:)` and `AVCaptureDevice.requestAccess`. Follow the same pattern but inline since we need to update SwiftUI state.

- [ ] **Step 1: Request camera permission when toggling camera on**

Replace the camera toggle (line 274) with a permission-aware version. Change:

```swift
Toggle(isOn: $isCameraOn) {
```

to:

```swift
Toggle(isOn: Binding(
    get: { isCameraOn },
    set: { newValue in
        if newValue {
            let status = AVCaptureDevice.authorizationStatus(for: .video)
            if status == .authorized {
                isCameraOn = true
            } else if status == .notDetermined {
                AVCaptureDevice.requestAccess(for: .video) { granted in
                    DispatchQueue.main.async {
                        isCameraOn = granted
                    }
                }
            }
            // If .denied or .restricted, don't enable
        } else {
            isCameraOn = false
        }
    }
)) {
```

- [ ] **Step 2: Request mic permission when toggling mic on**

Replace the mic toggle (line 325) with a permission-aware version. Change:

```swift
Toggle("", isOn: $isMicOn)
```

to:

```swift
Toggle("", isOn: Binding(
    get: { isMicOn },
    set: { newValue in
        if newValue {
            let status = AVCaptureDevice.authorizationStatus(for: .audio)
            if status == .authorized {
                isMicOn = true
            } else if status == .notDetermined {
                AVCaptureDevice.requestAccess(for: .audio) { granted in
                    DispatchQueue.main.async {
                        isMicOn = granted
                    }
                }
            }
        } else {
            isMicOn = false
        }
    }
))
```

- [ ] **Step 3: Configure audio session for MicLevelMonitor**

In `MicLevelMonitor.start()` (line 11), add audio session configuration before creating the engine so the VU meter actually works:

```swift
func start() {
    guard engine == nil else { return }

    // Configure audio session for recording
    let session = AVAudioSession.sharedInstance()
    try? session.setCategory(.playAndRecord, mode: .measurement, options: [.defaultToSpeaker, .allowBluetooth])
    try? session.setActive(true)

    let engine = AVAudioEngine()
    // ... rest unchanged
```

- [ ] **Step 4: Commit**

```bash
git add ios/VisioMobile/Views/PreJoinView.swift
git commit -m "fix(ios): request camera/mic permissions and configure audio session in pre-lobby"
```

---

### Task 5: iOS — Add audio route picker

**Files:**
- Create: `ios/VisioMobile/Views/AudioRoutePickerButton.swift`
- Modify: `ios/VisioMobile/Views/PreJoinView.swift`

iOS provides `AVRoutePickerView` which shows the native system audio route picker (AirPlay, Bluetooth, wired). Wrap it in a `UIViewRepresentable`.

- [ ] **Step 1: Create AudioRoutePickerButton.swift**

```swift
import SwiftUI
import AVKit

struct AudioRoutePickerButton: UIViewRepresentable {
    let tintColor: UIColor

    func makeUIView(context: Context) -> AVRoutePickerView {
        let picker = AVRoutePickerView()
        picker.tintColor = tintColor
        picker.activeTintColor = tintColor
        // Use compact style if available
        if #available(iOS 16.0, *) {
            picker.prioritizesVideoDevices = false
        }
        return picker
    }

    func updateUIView(_ uiView: AVRoutePickerView, context: Context) {
        uiView.tintColor = tintColor
    }
}
```

- [ ] **Step 2: Add audio route picker in PreJoinView**

In `audioConfigSection`, after the speaker test button (line 347), before the closing `}` of the `if audioMode == .computer` block:

```swift
// Audio route picker
HStack {
    Image(systemName: "speaker.wave.2")
        .foregroundStyle(VisioColors.primary500)
        .frame(width: 20)
    Text(Strings.t("prejoin.audioRoute", lang: lang))
        .font(.subheadline)
    Spacer()
    AudioRoutePickerButton(tintColor: UIColor(VisioColors.primary500))
        .frame(width: 36, height: 36)
}
.padding(.horizontal, 12)
```

- [ ] **Step 3: Commit**

```bash
git add ios/VisioMobile/Views/AudioRoutePickerButton.swift ios/VisioMobile/Views/PreJoinView.swift
git commit -m "feat(ios): add audio route picker in pre-lobby"
```

---

### Task 6: Build verification

- [ ] **Step 1: Build Android**

```bash
cd android && ./gradlew assembleDebug 2>&1 | tail -5
```

Expected: `BUILD SUCCESSFUL`

- [ ] **Step 2: Build iOS** (if Xcode available)

```bash
xcodebuild -project ios/VisioMobile.xcodeproj -scheme VisioMobile -sdk iphoneos build 2>&1 | tail -10
```

Or if workspace:
```bash
xcodebuild -workspace ios/VisioMobile.xcworkspace -scheme VisioMobile -sdk iphoneos build 2>&1 | tail -10
```

Expected: `BUILD SUCCEEDED`

- [ ] **Step 3: Build Rust core** (verify no regressions)

```bash
cargo test -p visio-core 2>&1 | tail -5
```

Expected: all tests pass
