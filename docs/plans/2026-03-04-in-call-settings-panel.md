# In-Call Settings Panel Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add an in-call settings bottom sheet with tabbed sidebar (Micro, Camera, Notifications) accessible via a gear button in the ControlBar.

**Architecture:** New `InCallSettingsSheet.kt` composable with icon sidebar + content panes. Notification preferences stored in Rust settings via FFI. Camera switching via `CameraCapture.switchCamera()`. Audio device listing via Android `AudioManager`.

**Tech Stack:** Kotlin/Jetpack Compose, Material3 ModalBottomSheet, Camera2 API, AudioManager, Rust/UniFFI for settings persistence.

**Design doc:** `docs/plans/2026-03-04-in-call-settings-panel-design.md`

---

### Task 1: Add notification settings to Rust core

**Files:**
- Modify: `crates/visio-core/src/settings.rs`

**Step 1: Add fields to Settings struct (line 7-20)**

Add 3 new fields after `meet_instances`:

```rust
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Settings {
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default = "default_true")]
    pub mic_enabled_on_join: bool,
    #[serde(default)]
    pub camera_enabled_on_join: bool,
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_meet_instances")]
    pub meet_instances: Vec<String>,
    #[serde(default = "default_true")]
    pub notification_participant_join: bool,
    #[serde(default = "default_true")]
    pub notification_hand_raised: bool,
    #[serde(default = "default_true")]
    pub notification_message_received: bool,
}
```

**Step 2: Update Default impl (line 34-44)**

Add the 3 fields to Default:

```rust
impl Default for Settings {
    fn default() -> Self {
        Self {
            display_name: None,
            language: None,
            mic_enabled_on_join: true,
            camera_enabled_on_join: false,
            theme: "light".to_string(),
            meet_instances: default_meet_instances(),
            notification_participant_join: true,
            notification_hand_raised: true,
            notification_message_received: true,
        }
    }
}
```

**Step 3: Add setter methods to SettingsStore (after line 98)**

```rust
pub fn set_notification_participant_join(&self, enabled: bool) {
    self.settings.lock().unwrap().notification_participant_join = enabled;
    self.save();
}

pub fn set_notification_hand_raised(&self, enabled: bool) {
    self.settings.lock().unwrap().notification_hand_raised = enabled;
    self.save();
}

pub fn set_notification_message_received(&self, enabled: bool) {
    self.settings.lock().unwrap().notification_message_received = enabled;
    self.save();
}
```

**Step 4: Run tests**

Run: `cargo test -p visio-core`
Expected: All existing tests pass. New fields use serde defaults so partial JSON test still works.

**Step 5: Commit**

```
feat(core): add notification sound settings
```

---

### Task 2: Expose notification settings via FFI + UDL

**Files:**
- Modify: `crates/visio-ffi/src/visio.udl` (lines 57-64)
- Modify: `crates/visio-ffi/src/lib.rs` (Settings struct ~line 230, VisioClient impl ~line 587)

**Step 1: Update UDL Settings dictionary (visio.udl line 57-64)**

```
dictionary Settings {
    string? display_name;
    string? language;
    boolean mic_enabled_on_join;
    boolean camera_enabled_on_join;
    string theme;
    sequence<string> meet_instances;
    boolean notification_participant_join;
    boolean notification_hand_raised;
    boolean notification_message_received;
};
```

**Step 2: Add setter declarations to VisioClient in UDL (after line 148)**

```
void set_notification_participant_join(boolean enabled);
void set_notification_hand_raised(boolean enabled);
void set_notification_message_received(boolean enabled);
```

**Step 3: Update FFI Settings struct (lib.rs ~line 230)**

Add the 3 fields to the FFI `Settings` struct and its `From<visio_core::Settings>` impl (~line 241):

```rust
pub struct Settings {
    pub display_name: Option<String>,
    pub language: Option<String>,
    pub mic_enabled_on_join: bool,
    pub camera_enabled_on_join: bool,
    pub theme: String,
    pub meet_instances: Vec<String>,
    pub notification_participant_join: bool,
    pub notification_hand_raised: bool,
    pub notification_message_received: bool,
}
```

And in the From impl:
```rust
notification_participant_join: s.notification_participant_join,
notification_hand_raised: s.notification_hand_raised,
notification_message_received: s.notification_message_received,
```

**Step 4: Add setter methods to VisioClient impl (after set_meet_instances)**

```rust
pub fn set_notification_participant_join(&self, enabled: bool) {
    self.settings.set_notification_participant_join(enabled);
}

pub fn set_notification_hand_raised(&self, enabled: bool) {
    self.settings.set_notification_hand_raised(enabled);
}

pub fn set_notification_message_received(&self, enabled: bool) {
    self.settings.set_notification_message_received(enabled);
}
```

**Step 5: Build to verify**

Run: `cargo build -p visio-ffi`
Expected: Compiles without errors.

**Step 6: Commit**

```
feat(ffi): expose notification settings via UniFFI
```

---

### Task 3: Add camera switching to CameraCapture.kt

**Files:**
- Modify: `android/app/src/main/kotlin/io/visio/mobile/CameraCapture.kt`
- Modify: `android/app/src/main/kotlin/io/visio/mobile/VisioManager.kt`

**Step 1: Add switchCamera method to CameraCapture.kt (after stop() method, ~line 147)**

```kotlin
/**
 * Switch to a different camera by ID. Stops current capture and restarts with new camera.
 */
@SuppressLint("MissingPermission")
fun switchCamera(useFront: Boolean) {
    if (!running) return

    val cameraManager = context.getSystemService(Context.CAMERA_SERVICE) as CameraManager
    val newId = if (useFront) findFrontCamera(cameraManager) else findBackCamera(cameraManager)
    if (newId == null) {
        Log.e(TAG, "Requested camera not found (front=$useFront)")
        return
    }

    // Stop current session
    captureSession?.close()
    captureSession = null
    cameraDevice?.close()
    cameraDevice = null
    imageReader?.close()
    imageReader = null

    // Update orientation info
    val chars = cameraManager.getCameraCharacteristics(newId)
    sensorOrientation = chars.get(CameraCharacteristics.SENSOR_ORIENTATION) ?: 0
    isFrontCamera = chars.get(CameraCharacteristics.LENS_FACING) == CameraCharacteristics.LENS_FACING_FRONT
    Log.i(TAG, "Switching to camera $newId: sensorOrientation=$sensorOrientation, front=$isFrontCamera")

    // Recreate ImageReader
    imageReader = ImageReader.newInstance(WIDTH, HEIGHT, ImageFormat.YUV_420_888, MAX_IMAGES).apply {
        setOnImageAvailableListener({ reader ->
            val image = reader.acquireLatestImage() ?: return@setOnImageAvailableListener
            try {
                val yPlane = image.planes[0]
                val uPlane = image.planes[1]
                val vPlane = image.planes[2]
                val displayDegrees = (displayManager
                    .getDisplay(Display.DEFAULT_DISPLAY)?.rotation ?: 0) * 90
                val rotation = if (isFrontCamera) {
                    (sensorOrientation + displayDegrees) % 360
                } else {
                    (sensorOrientation - displayDegrees + 360) % 360
                }
                NativeVideo.nativePushCameraFrame(
                    yPlane.buffer, uPlane.buffer, vPlane.buffer,
                    yPlane.rowStride, uPlane.rowStride, vPlane.rowStride,
                    uPlane.pixelStride, vPlane.pixelStride,
                    image.width, image.height, rotation
                )
            } finally {
                image.close()
            }
        }, handler)
    }

    // Open new camera
    cameraManager.openCamera(newId, object : CameraDevice.StateCallback() {
        override fun onOpened(camera: CameraDevice) {
            Log.i(TAG, "Switched camera opened: ${camera.id}")
            cameraDevice = camera
            createCaptureSession(camera)
        }
        override fun onDisconnected(camera: CameraDevice) {
            camera.close()
            cameraDevice = null
        }
        override fun onError(camera: CameraDevice, error: Int) {
            Log.e(TAG, "Camera switch error: $error")
            camera.close()
            cameraDevice = null
        }
    }, handler)
}

/** Returns true if currently using front camera. */
fun isFront(): Boolean = isFrontCamera
```

**Step 2: Add switchCamera to VisioManager.kt (after stopCameraCapture)**

```kotlin
fun switchCamera(useFront: Boolean) {
    cameraCapture?.switchCamera(useFront)
}

fun isFrontCamera(): Boolean = cameraCapture?.isFront() ?: true
```

**Step 3: Build**

Run: `cd android && ./gradlew assembleDebug`

**Step 4: Commit**

```
feat(android): add camera switching support
```

---

### Task 4: Add i18n keys

**Files:**
- Modify: `android/app/src/main/assets/i18n/fr.json`
- Modify: `android/app/src/main/assets/i18n/en.json`

**Step 1: Add keys to fr.json (before closing brace)**

```json
"settings.incall": "Paramètres",
"settings.incall.micro": "Micro",
"settings.incall.camera": "Caméra",
"settings.incall.notifications": "Notifications sonores",
"settings.incall.audioInput": "Entrée audio",
"settings.incall.audioOutput": "Sortie audio",
"settings.incall.cameraSelect": "Sélectionner la caméra",
"settings.incall.cameraFront": "Caméra frontale",
"settings.incall.cameraBack": "Caméra arrière",
"settings.incall.notifParticipant": "Un nouveau participant",
"settings.incall.notifHandRaised": "Une main levée",
"settings.incall.notifMessage": "Un message reçu"
```

**Step 2: Add keys to en.json**

```json
"settings.incall": "Settings",
"settings.incall.micro": "Microphone",
"settings.incall.camera": "Camera",
"settings.incall.notifications": "Sound notifications",
"settings.incall.audioInput": "Audio input",
"settings.incall.audioOutput": "Audio output",
"settings.incall.cameraSelect": "Select camera",
"settings.incall.cameraFront": "Front camera",
"settings.incall.cameraBack": "Back camera",
"settings.incall.notifParticipant": "New participant",
"settings.incall.notifHandRaised": "Hand raised",
"settings.incall.notifMessage": "Message received"
```

**Step 3: Commit**

```
feat(android): add i18n keys for in-call settings panel
```

---

### Task 5: Create InCallSettingsSheet.kt composable

**Files:**
- Create: `android/app/src/main/kotlin/io/visio/mobile/ui/InCallSettingsSheet.kt`

**Step 1: Create the composable**

This is the main UI component. It contains:
- A `ModalBottomSheet` with a `Row` layout
- Left column: 3 `IconButton` items (mic, camera, bell) with highlight on selected
- Right column: content for the selected tab

The Micro tab lists input/output audio devices using `AudioManager.getDevices()` with radio buttons.
The Camera tab shows Front/Back camera radio buttons.
The Notifications tab shows 3 Switch toggles persisted via `VisioManager.client.setNotification*()`.

Key imports: `AudioManager`, `AudioDeviceInfo`, `ModalBottomSheet`, `rememberModalBottomSheetState`, Material3 components, `VisioManager`, `VisioColors`, `Strings`.

The sheet takes parameters:
```kotlin
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun InCallSettingsSheet(
    initialTab: Int = 0,  // 0=Micro, 1=Camera, 2=Notifications
    onDismiss: () -> Unit,
    onSelectAudioOutput: (AudioDeviceInfo) -> Unit,
    onSwitchCamera: (Boolean) -> Unit,  // true=front
    isFrontCamera: Boolean
)
```

Use the existing `AudioDeviceSheet` (CallScreen.kt lines 836-894) as pattern reference for the bottom sheet styling (containerColor = `VisioColors.PrimaryDark75`).

For audio INPUT devices, filter `AudioManager.getDevices(GET_DEVICES_INPUTS)` to types: `TYPE_BUILTIN_MIC`, `TYPE_BLUETOOTH_SCO`, `TYPE_USB_HEADSET`, `TYPE_WIRED_HEADSET`.

For audio OUTPUT devices, reuse the same filter as existing `AudioDeviceSheet`.

**Step 2: Build**

Run: `cd android && ./gradlew assembleDebug`

**Step 3: Commit**

```
feat(android): add InCallSettingsSheet composable
```

---

### Task 6: Wire up InCallSettingsSheet in CallScreen.kt

**Files:**
- Modify: `android/app/src/main/kotlin/io/visio/mobile/ui/CallScreen.kt`

**Step 1: Add state variables (after showAudioSheet ~line 110)**

```kotlin
var showInCallSettings by remember { mutableStateOf(false) }
var inCallSettingsTab by remember { mutableStateOf(0) }
```

**Step 2: Replace AudioDeviceSheet with InCallSettingsSheet**

Replace the `if (showAudioSheet)` block (~lines 240-251) with:

```kotlin
if (showInCallSettings) {
    InCallSettingsSheet(
        initialTab = inCallSettingsTab,
        onDismiss = { showInCallSettings = false },
        onSelectAudioOutput = { device ->
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
                val audioManager = context.getSystemService(Context.AUDIO_SERVICE) as AudioManager
                audioManager.setCommunicationDevice(device)
            }
        },
        onSwitchCamera = { useFront ->
            VisioManager.switchCamera(useFront)
        },
        isFrontCamera = VisioManager.isFrontCamera()
    )
}
```

**Step 3: Update onAudioPicker to open Micro tab**

Change `onAudioPicker = { showAudioSheet = true }` (~line 384) to:

```kotlin
onAudioPicker = {
    inCallSettingsTab = 0
    showInCallSettings = true
},
```

**Step 4: Add gear button to ControlBar**

Add a `onSettings: () -> Unit` parameter to the `ControlBar` composable. Add a gear IconButton between Chat and Hangup in the ControlBar Row:

```kotlin
// Settings gear
IconButton(
    onClick = onSettings,
    modifier = Modifier
        .size(44.dp)
        .background(VisioColors.PrimaryDark100, RoundedCornerShape(8.dp))
) {
    Icon(
        painter = painterResource(R.drawable.ri_settings_3_line),
        contentDescription = Strings.t("settings.incall", lang),
        tint = VisioColors.White,
        modifier = Modifier.size(20.dp)
    )
}
```

And wire it up in the ControlBar call:

```kotlin
onSettings = {
    inCallSettingsTab = 0
    showInCallSettings = true
},
```

**Step 5: Remove old AudioDeviceSheet composable** (lines 826-895 approximately)

It is now replaced by the Micro tab of InCallSettingsSheet.

**Step 6: Build and test**

Run: `cd android && ./gradlew assembleDebug`

**Step 7: Commit**

```
feat(android): wire in-call settings panel with gear button in ControlBar
```

---

### Task 7: Final integration test and push

**Step 1: Full build**

Run: `cd android && ./gradlew assembleDebug`

**Step 2: Git push**

```bash
git push origin main
```

**Step 3: Trigger CI build**

```bash
gh workflow run build-android.yml --ref main
```
