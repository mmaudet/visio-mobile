# Pre-Join Lobby Screen — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a Teams-style pre-join lobby with live camera preview (with blur effects), audio device selection, VU meter, speaker test, and waiting room across Desktop, iOS, and Android.

**Architecture:** Standalone preview pipeline (no LiveKit connection) for camera + blur. VU meter and speaker test are platform-native (no FFI overhead). Device enumeration is platform-native on mobile, existing cpal-based on desktop. Settings struct extended with `background_mode` and `audio_mode`.

**Tech Stack:** Rust (visio-core, visio-ffi, visio-video), TypeScript/React (desktop), Swift/SwiftUI (iOS), Kotlin/Compose (Android), ONNX Runtime (blur), cpal (desktop audio)

**Spec:** `docs/superpowers/specs/2026-03-20-pre-join-lobby-design.md`

---

## Phase 1: Settings & FFI Foundation

### Task 1: Extend Settings struct with new fields

**Files:**
- Modify: `crates/visio-core/src/settings.rs`

- [ ] **Step 1: Add `audio_mode` field to Settings**

In `crates/visio-core/src/settings.rs`, add after the `background_mode` field (line 27):

```rust
    // Already exists: pub background_mode: String,        // line 27
    #[serde(default = "default_audio_mode")]
    pub audio_mode: String,                                // NEW
```

Add the default function alongside existing defaults:

```rust
fn default_audio_mode() -> String {
    "computer".to_string()
}
```

- [ ] **Step 2: Verify Settings compiles**

Run: `cargo build -p visio-core`
Expected: Success

- [ ] **Step 3: Commit**

```bash
git add crates/visio-core/src/settings.rs
git commit -m "feat(core): add audio_mode to Settings struct"
```

### Task 2: Expose missing Settings fields in FFI + UDL

**Files:**
- Modify: `crates/visio-ffi/src/lib.rs:320-331` (FFI Settings struct)
- Modify: `crates/visio-ffi/src/visio.udl:80-91` (UDL Settings dictionary)

- [ ] **Step 1: Add missing fields to FFI Settings struct**

In `crates/visio-ffi/src/lib.rs`, the FFI `Settings` struct (lines 320-331) is missing several fields that exist in core. Add them:

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
    pub adaptive_mode_enabled: bool,
    // NEW fields for pre-join lobby:
    pub background_mode: String,
    pub audio_mode: String,
    pub audio_input_device: Option<String>,
    pub audio_output_device: Option<String>,
    pub camera_device: Option<String>,
    pub video_resolution: String,
}
```

Update the `From<core_settings::Settings>` impl to map all new fields.

- [ ] **Step 2: Add matching fields to UDL Settings dictionary**

In `crates/visio-ffi/src/visio.udl` (lines 80-91), add:

```idl
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
    boolean adaptive_mode_enabled;
    // NEW:
    string background_mode;
    string audio_mode;
    string? audio_input_device;
    string? audio_output_device;
    string? camera_device;
    string video_resolution;
};
```

- [ ] **Step 3: Add setter methods for new settings**

Add to the `VisioClient` impl in `crates/visio-ffi/src/lib.rs`:

Follow the pattern used by `set_display_name` (line ~1036 in lib.rs), which uses `self.settings.set_display_name(name)`:

```rust
pub fn set_audio_mode(&self, mode: String) {
    self.settings.set_audio_mode(mode);
}

pub fn set_audio_input_device(&self, name: Option<String>) {
    self.settings.set_audio_input_device(name);
}

pub fn set_audio_output_device(&self, name: Option<String>) {
    self.settings.set_audio_output_device(name);
}

pub fn set_camera_device(&self, name: Option<String>) {
    self.settings.set_camera_device(name);
}
```

Add corresponding setters to `SettingsStore` in `crates/visio-core/src/settings.rs` if they don't already exist.

Add the UDL method declarations:

```idl
void set_audio_mode(string mode);
void set_audio_input_device(string? name);
void set_audio_output_device(string? name);
void set_camera_device(string? name);
```

- [ ] **Step 4: Build and verify**

Run: `cargo build -p visio-ffi`
Expected: Success

- [ ] **Step 5: Commit**

```bash
git add crates/visio-ffi/src/lib.rs crates/visio-ffi/src/visio.udl crates/visio-core/src/settings.rs
git commit -m "feat(ffi): expose background_mode, audio_mode, and device settings in FFI/UDL"
```

### Task 3: Fix blur-light FFI mapping

**Files:**
- Modify: `crates/visio-ffi/src/lib.rs` (set_background_mode handler)

- [ ] **Step 1: Find and fix the set_background_mode handler**

In `crates/visio-ffi/src/lib.rs`, find the `set_background_mode` method. Currently `"blur-light"` falls through to `BackgroundMode::Off`. Fix:

```rust
pub fn set_background_mode(&self, mode: String) {
    let bg_mode = if mode == "blur" {
        BackgroundMode::Blur
    } else if mode == "blur-light" {
        BackgroundMode::BlurLight  // WAS MISSING — fell through to Off
    } else if mode.starts_with("image:") {
        let id = mode[6..].parse::<u8>().unwrap_or(0);
        BackgroundMode::Image(id)
    } else {
        BackgroundMode::Off
    };
    blur::BlurProcessor::set_mode(bg_mode);
}
```

- [ ] **Step 2: Build and verify**

Run: `cargo build -p visio-ffi`
Expected: Success

- [ ] **Step 3: Commit**

```bash
git add crates/visio-ffi/src/lib.rs
git commit -m "fix(ffi): handle blur-light mode in set_background_mode"
```

---

## Phase 2: Desktop PreJoin Screen

### Task 4: Add "lobby" view state and navigation

**Files:**
- Modify: `crates/visio-desktop/frontend/src/App.tsx`

- [ ] **Step 1: Extend View type and add lobby state**

In `App.tsx`, line 47:

```typescript
// WAS: type View = "home" | "call";
type View = "home" | "lobby" | "call";
```

Add lobby state variables near line 2360:

```typescript
const [lobbyRoomUrl, setLobbyRoomUrl] = useState("");
const [lobbyUsername, setLobbyUsername] = useState<string | null>(null);
```

- [ ] **Step 2: Change join handler to navigate to lobby instead of call**

In the `handleJoin()` function (around line 2874), instead of `setView("call")`:

```typescript
// WAS: setView("call");
setLobbyRoomUrl(url);
setLobbyUsername(uname);
setView("lobby");
```

Do NOT call `invoke("connect", ...)` yet — that moves to the lobby's "Join now" button.

- [ ] **Step 3: Add lobby view rendering in the main switch**

In the view rendering section (lines 3050-3161), add between home and call:

```tsx
{view === "lobby" && (
  <PreJoinScreen
    roomUrl={lobbyRoomUrl}
    username={lobbyUsername}
    lang={lang}
    isDark={isDark}
    onJoin={async (finalUsername) => {
      try {
        await invoke("connect", { meetUrl: lobbyRoomUrl, username: finalUsername });
        setView("call");
      } catch (e) {
        // Error handled by connection state listener
      }
    }}
    onCancel={() => setView("home")}
  />
)}
```

- [ ] **Step 4: Verify it compiles (PreJoinScreen will be a stub)**

Create a minimal stub for `PreJoinScreen` inline or as a placeholder:

```tsx
function PreJoinScreen({ roomUrl, username, lang, isDark, onJoin, onCancel }: {
  roomUrl: string;
  username: string | null;
  lang: string;
  isDark: boolean;
  onJoin: (username: string | null) => void;
  onCancel: () => void;
}) {
  return (
    <div className="prejoin-container">
      <h2>{roomUrl}</h2>
      <button onClick={() => onJoin(username)}>Join</button>
      <button onClick={onCancel}>Cancel</button>
    </div>
  );
}
```

- [ ] **Step 5: Commit**

```bash
git add crates/visio-desktop/frontend/src/App.tsx
git commit -m "feat(desktop): add lobby view state and navigation skeleton"
```

### Task 5: Desktop standalone camera preview

**Files:**
- Modify: `crates/visio-desktop/src/camera_macos.rs`
- Modify: `crates/visio-desktop/src/lib.rs` (new Tauri commands)

- [ ] **Step 1: Add preview-only capture mode to camera_macos.rs**

In `camera_macos.rs`, add a new function that starts capture but only renders to the local preview (no `NativeVideoSource`, no `source.capture_frame()`):

```rust
/// Start camera capture in preview mode — renders to desktop frame callback
/// but does NOT feed into any LiveKit NativeVideoSource.
pub fn start_preview() -> Result<Self, String> {
    Self::start_internal(None, None)
}

pub fn start_preview_with_unique_id(unique_id: &str) -> Result<Self, String> {
    Self::start_internal(None, Some(unique_id))
}
```

Refactor the existing `start()` and `start_with_unique_id()` to use a shared `start_internal(source: Option<NativeVideoSource>, unique_id: Option<&str>)`.

In `process_camera_frame()`, after blur processing, check if `source` is Some before calling `source.capture_frame()`. The local preview rendering (`visio_video::render_local_i420()` → desktop frame callback) always runs regardless.

- [ ] **Step 2: Add Tauri commands for preview mode**

In `crates/visio-desktop/src/lib.rs`, add:

```rust
#[tauri::command]
async fn start_camera_preview(state: State<'_, AppState>) -> Result<(), String> {
    let mut cam = state.camera_capture.lock().map_err(|e| e.to_string())?;
    if cam.is_some() {
        return Ok(()); // Already running
    }

    // Check saved camera device preference
    let settings = state.settings.get();

    let capture = if let Some(ref device_id) = settings.camera_device {
        MacCameraCapture::start_preview_with_unique_id(device_id)
    } else {
        MacCameraCapture::start_preview()
    };

    *cam = Some(capture.map_err(|e| e.to_string())?);
    Ok(())
}

#[tauri::command]
async fn stop_camera_preview(state: State<'_, AppState>) -> Result<(), String> {
    let mut cam = state.camera_capture.lock().map_err(|e| e.to_string())?;
    if let Some(capture) = cam.take() {
        capture.stop();
    }
    Ok(())
}
```

Register both commands in the Tauri builder.

- [ ] **Step 3: Build and verify**

Run: `cargo build -p visio-desktop`
Expected: Success

- [ ] **Step 4: Commit**

```bash
git add crates/visio-desktop/src/camera_macos.rs crates/visio-desktop/src/lib.rs
git commit -m "feat(desktop): add standalone camera preview mode (no LiveKit connection)"
```

### Task 6: Desktop VU meter

**Files:**
- Modify: `crates/visio-desktop/src/audio_engine.rs`
- Modify: `crates/visio-desktop/src/lib.rs` (new Tauri command)

- [ ] **Step 1: Add mic level tracking to audio engine**

In `audio_engine.rs`, add an atomic for mic level:

```rust
use std::sync::atomic::{AtomicU32, Ordering};

static MIC_LEVEL: AtomicU32 = AtomicU32::new(0);

/// Returns the current mic RMS level as 0.0–1.0
pub fn get_mic_level() -> f32 {
    f32::from_bits(MIC_LEVEL.load(Ordering::Relaxed))
}
```

In the capture drain thread (`start_drain_thread`, around line 61-108), after reading PCM frames, compute RMS:

```rust
// Inside the frame processing loop, after getting the audio data:
let samples: &[i16] = /* the PCM buffer */;
if !samples.is_empty() {
    let sum_sq: f64 = samples.iter().map(|&s| {
        let f = s as f64 / i16::MAX as f64;
        f * f
    }).sum();
    let rms = (sum_sq / samples.len() as f64).sqrt() as f32;
    // Clamp to 0.0–1.0
    let level = rms.min(1.0);
    MIC_LEVEL.store(level.to_bits(), Ordering::Relaxed);
}
```

- [ ] **Step 2: Add preview-mode mic capture (no LiveKit source)**

Add a method to start mic capture without a `NativeAudioSource`:

```rust
/// Start mic capture for VU meter only — no LiveKit source.
pub fn start_preview_capture(&mut self, device_name: Option<&str>, noise_reduction: bool) -> Result<(), String> {
    // Open cpal input stream on the selected device
    // Process frames through noise reduction if enabled
    // Update MIC_LEVEL atomic but don't feed to any source
    // ...
}

pub fn stop_preview_capture(&mut self) {
    // Stop the preview input stream
}
```

- [ ] **Step 3: Add Tauri commands**

In `lib.rs`:

```rust
#[tauri::command]
fn get_mic_level() -> f32 {
    audio_engine::get_mic_level()
}

#[tauri::command]
async fn start_mic_preview(state: State<'_, AppState>) -> Result<(), String> {
    let mut engine = state.audio_engine.lock().map_err(|e| e.to_string())?;
    let settings = /* get settings */;
    engine.start_preview_capture(
        settings.audio_input_device.as_deref(),
        settings.noise_reduction_enabled,
    )
}

#[tauri::command]
async fn stop_mic_preview(state: State<'_, AppState>) -> Result<(), String> {
    let mut engine = state.audio_engine.lock().map_err(|e| e.to_string())?;
    engine.stop_preview_capture();
    Ok(())
}
```

Register commands in Tauri builder.

- [ ] **Step 4: Build and verify**

Run: `cargo build -p visio-desktop`
Expected: Success

- [ ] **Step 5: Commit**

```bash
git add crates/visio-desktop/src/audio_engine.rs crates/visio-desktop/src/lib.rs
git commit -m "feat(desktop): add mic level monitoring for VU meter preview"
```

### Task 7: Desktop speaker test

**Files:**
- Create: `crates/visio-desktop/frontend/public/speaker-test.mp3` (bundled audio file)
- Modify: `crates/visio-desktop/src/lib.rs` (new Tauri command)
- Modify: `crates/visio-desktop/src/audio_engine.rs`

- [ ] **Step 1: Add a bundled speaker test audio file**

Generate or source a short (~2 second) chime/beep sound as MP3. Place it at:
`crates/visio-desktop/frontend/public/speaker-test.mp3`

The file should be small (< 50KB).

- [ ] **Step 2: Add speaker test playback to audio engine**

In `audio_engine.rs`, add:

```rust
/// Play a short audio file on the current output device for speaker testing.
pub fn play_speaker_test(audio_bytes: &[u8]) -> Result<(), String> {
    // Decode MP3 to PCM samples
    // Open cpal output stream on current default/selected output device
    // Play samples, then close stream
    // Use rodio or minimp3 for decoding
}
```

- [ ] **Step 3: Add Tauri command**

```rust
#[tauri::command]
async fn play_speaker_test(app: tauri::AppHandle) -> Result<(), String> {
    let resource_path = app.path().resolve("speaker-test.mp3", tauri::path::BaseDirectory::Resource)
        .map_err(|e| e.to_string())?;
    let audio_bytes = std::fs::read(&resource_path).map_err(|e| e.to_string())?;
    audio_engine::play_speaker_test(&audio_bytes)
}
```

- [ ] **Step 4: Build and verify**

Run: `cargo build -p visio-desktop`
Expected: Success

- [ ] **Step 5: Commit**

```bash
git add crates/visio-desktop/frontend/public/speaker-test.mp3 crates/visio-desktop/src/audio_engine.rs crates/visio-desktop/src/lib.rs
git commit -m "feat(desktop): add speaker test playback command"
```

### Task 8: Desktop PreJoinScreen UI — Camera preview panel

**Files:**
- Modify: `crates/visio-desktop/frontend/src/App.tsx` (replace stub)
- Modify: `crates/visio-desktop/frontend/src/App.css` (add styles)

- [ ] **Step 1: Build the left panel — camera preview with controls**

Replace the `PreJoinScreen` stub with a full component. The camera preview reuses the same base64 JPEG frame approach as CallView — listen for `"video-frame"` events with a special `"local-preview"` track SID:

```tsx
function PreJoinScreen({ roomUrl, username, lang, isDark, onJoin, onCancel }: PreJoinProps) {
  const [displayName, setDisplayName] = useState(username || "");
  const [isCameraOn, setIsCameraOn] = useState(true);
  const [isMicOn, setIsMicOn] = useState(true);
  const [audioMode, setAudioMode] = useState<"computer" | "none">("computer");
  const [previewFrame, setPreviewFrame] = useState<string | null>(null);
  const [videoDevices, setVideoDevices] = useState<VideoDeviceInfo[]>([]);
  const [selectedCamera, setSelectedCamera] = useState<string>("");

  // Start/stop camera preview
  useEffect(() => {
    if (isCameraOn) {
      invoke("start_camera_preview");
    } else {
      invoke("stop_camera_preview");
      setPreviewFrame(null);
    }
    return () => { invoke("stop_camera_preview"); };
  }, [isCameraOn]);

  // Listen for preview frames
  useEffect(() => {
    const unlisten = listen<VideoFrame>("video-frame", (event) => {
      if (event.payload.track_sid === "local-preview") {
        setPreviewFrame(event.payload.data);
      }
    });
    return () => { unlisten.then(fn => fn()); };
  }, []);

  // Load device lists
  useEffect(() => {
    invoke<VideoDeviceInfo[]>("list_video_input_devices").then(setVideoDevices);
  }, []);

  const slug = roomUrl.includes("/") ? roomUrl.split("/").pop() : roomUrl;

  return (
    <div className="prejoin-container" data-theme={isDark ? "dark" : "light"}>
      <h2 className="prejoin-room-name">{slug}</h2>
      <input
        className="prejoin-display-name"
        value={displayName}
        onChange={(e) => setDisplayName(e.target.value)}
        placeholder={t("prejoin.displayName", lang)}
      />

      <div className="prejoin-panels">
        {/* Left: Camera */}
        <div className="prejoin-camera-panel">
          <div className="prejoin-preview">
            {previewFrame ? (
              <img src={`data:image/jpeg;base64,${previewFrame}`} alt="" />
            ) : (
              <div className="prejoin-avatar">{(displayName || "?")[0].toUpperCase()}</div>
            )}
          </div>
          <div className="prejoin-camera-controls">
            <select value={selectedCamera} onChange={async (e) => {
              setSelectedCamera(e.target.value);
              await invoke("select_video_input", { uniqueId: e.target.value });
            }}>
              {videoDevices.map(d => (
                <option key={d.unique_id} value={d.unique_id}>{d.name}</option>
              ))}
            </select>
            <label className="prejoin-toggle">
              <input type="checkbox" checked={isCameraOn} onChange={(e) => setIsCameraOn(e.target.checked)} />
              <span>{t("prejoin.camera", lang)}</span>
            </label>
          </div>
          <button className="prejoin-filters-btn" onClick={() => {/* open filter panel */}}>
            {t("prejoin.backgroundFilters", lang)}
          </button>
        </div>

        {/* Right: Audio — see Task 9 */}
        <PreJoinAudioPanel
          audioMode={audioMode}
          setAudioMode={setAudioMode}
          isMicOn={isMicOn}
          setIsMicOn={setIsMicOn}
          lang={lang}
          isDark={isDark}
        />
      </div>

      <div className="prejoin-actions">
        <button className="btn-secondary" onClick={onCancel}>{t("prejoin.cancel", lang)}</button>
        <button className="btn-primary" onClick={() => onJoin(displayName.trim() || null)}>
          {t("prejoin.joinNow", lang)}
        </button>
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Add CSS for the prejoin layout**

In `App.css`, add styles matching the Teams-style two-column layout:

```css
.prejoin-container {
  display: flex;
  flex-direction: column;
  align-items: center;
  padding: 32px;
  height: 100vh;
  background: var(--bg);
}

.prejoin-room-name {
  font-size: 1.2rem;
  margin-bottom: 8px;
}

.prejoin-display-name {
  width: 300px;
  padding: 8px 12px;
  border: 1px solid var(--border);
  border-radius: var(--radius);
  background: var(--bg);
  color: var(--text);
  font-size: 0.95rem;
  margin-bottom: 24px;
  text-align: center;
}

.prejoin-panels {
  display: flex;
  gap: 24px;
  width: 100%;
  max-width: 720px;
}

.prejoin-camera-panel {
  flex: 1;
  display: flex;
  flex-direction: column;
}

.prejoin-preview {
  aspect-ratio: 4/3;
  background: #000;
  border-radius: var(--radius);
  overflow: hidden;
  display: flex;
  align-items: center;
  justify-content: center;
}

.prejoin-preview img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.prejoin-avatar {
  width: 64px;
  height: 64px;
  border-radius: 50%;
  background: var(--accent);
  color: #fff;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 1.5rem;
  font-weight: 600;
}

.prejoin-camera-controls {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 8px;
}

.prejoin-camera-controls select {
  flex: 1;
  padding: 6px 8px;
  border: 1px solid var(--border);
  border-radius: var(--radius);
  background: var(--bg);
  color: var(--text);
  font-size: 0.85rem;
}

.prejoin-filters-btn {
  margin-top: 8px;
  background: none;
  border: none;
  color: var(--accent);
  cursor: pointer;
  font-size: 0.85rem;
  text-align: left;
  padding: 4px 0;
}

.prejoin-actions {
  display: flex;
  gap: 12px;
  margin-top: 24px;
}
```

- [ ] **Step 3: Verify desktop builds and preview renders**

Run: `cd crates/visio-desktop && cargo tauri dev`
Expected: Clicking "Rejoindre" shows the PreJoin screen with camera preview

- [ ] **Step 4: Commit**

```bash
git add crates/visio-desktop/frontend/src/App.tsx crates/visio-desktop/frontend/src/App.css
git commit -m "feat(desktop): PreJoinScreen with live camera preview"
```

### Task 9: Desktop PreJoinScreen UI — Audio panel + VU meter

**Files:**
- Modify: `crates/visio-desktop/frontend/src/App.tsx`
- Modify: `crates/visio-desktop/frontend/src/App.css`

- [ ] **Step 1: Build the audio panel component**

```tsx
function PreJoinAudioPanel({ audioMode, setAudioMode, isMicOn, setIsMicOn, lang, isDark }: AudioPanelProps) {
  const [inputDevices, setInputDevices] = useState<AudioDeviceInfo[]>([]);
  const [outputDevices, setOutputDevices] = useState<AudioDeviceInfo[]>([]);
  const [selectedInput, setSelectedInput] = useState("");
  const [selectedOutput, setSelectedOutput] = useState("");
  const [micLevel, setMicLevel] = useState(0);

  // Load device lists
  useEffect(() => {
    invoke<AudioDeviceInfo[]>("list_audio_input_devices").then(devices => {
      setInputDevices(devices);
      const def = devices.find(d => d.is_default);
      if (def) setSelectedInput(def.name);
    });
    invoke<AudioDeviceInfo[]>("list_audio_output_devices").then(devices => {
      setOutputDevices(devices);
      const def = devices.find(d => d.is_default);
      if (def) setSelectedOutput(def.name);
    });
  }, []);

  // Start/stop mic preview for VU meter
  useEffect(() => {
    if (audioMode === "computer" && isMicOn) {
      invoke("start_mic_preview");
      const interval = setInterval(async () => {
        const level = await invoke<number>("get_mic_level");
        setMicLevel(level);
      }, 100);
      return () => { clearInterval(interval); invoke("stop_mic_preview"); };
    } else {
      invoke("stop_mic_preview");
      setMicLevel(0);
    }
  }, [audioMode, isMicOn]);

  return (
    <div className="prejoin-audio-panel">
      {/* Computer audio option */}
      <label className={`prejoin-audio-option ${audioMode === "computer" ? "active" : ""}`}>
        <input type="radio" name="audioMode" checked={audioMode === "computer"}
          onChange={() => setAudioMode("computer")} />
        <span>{t("prejoin.computerAudio", lang)}</span>
      </label>

      {audioMode === "computer" && (
        <div className="prejoin-audio-devices">
          {/* Mic selector */}
          <div className="prejoin-device-row">
            <span className="prejoin-device-icon">🎤</span>
            <select value={selectedInput} onChange={async (e) => {
              setSelectedInput(e.target.value);
              await invoke("select_audio_input", { deviceName: e.target.value });
            }}>
              {inputDevices.map(d => (
                <option key={d.name} value={d.name}>{d.name}</option>
              ))}
            </select>
            <label className="prejoin-toggle">
              <input type="checkbox" checked={isMicOn} onChange={(e) => setIsMicOn(e.target.checked)} />
            </label>
          </div>

          {/* VU meter */}
          {isMicOn && (
            <div className="prejoin-vu-meter">
              <div className="prejoin-vu-bar" style={{ width: `${Math.min(micLevel * 100, 100)}%` }} />
            </div>
          )}

          {/* Speaker selector */}
          <div className="prejoin-device-row">
            <span className="prejoin-device-icon">🔊</span>
            <select value={selectedOutput} onChange={async (e) => {
              setSelectedOutput(e.target.value);
              await invoke("select_audio_output", { deviceName: e.target.value });
            }}>
              {outputDevices.map(d => (
                <option key={d.name} value={d.name}>{d.name}</option>
              ))}
            </select>
          </div>

          {/* Speaker test */}
          <button className="prejoin-speaker-test" onClick={() => invoke("play_speaker_test")}>
            🔈 {t("prejoin.testSpeaker", lang)}
          </button>
        </div>
      )}

      {/* No audio option */}
      <label className={`prejoin-audio-option ${audioMode === "none" ? "active" : ""}`}>
        <input type="radio" name="audioMode" checked={audioMode === "none"}
          onChange={() => setAudioMode("none")} />
        <span>{t("prejoin.noAudio", lang)}</span>
      </label>
    </div>
  );
}
```

- [ ] **Step 2: Add CSS for audio panel and VU meter**

```css
.prejoin-audio-panel {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.prejoin-audio-option {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 12px;
  border: 1px solid var(--border);
  border-radius: var(--radius);
  cursor: pointer;
  font-size: 0.9rem;
  color: var(--text);
}

.prejoin-audio-option.active {
  border-color: var(--accent);
  background: rgba(0, 0, 145, 0.05);
}

.prejoin-audio-devices {
  padding: 0 12px 12px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.prejoin-device-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.prejoin-device-row select {
  flex: 1;
  padding: 6px 8px;
  border: 1px solid var(--border);
  border-radius: var(--radius);
  background: var(--bg);
  color: var(--text);
  font-size: 0.85rem;
}

.prejoin-vu-meter {
  height: 4px;
  background: var(--bg-tertiary);
  border-radius: 2px;
  overflow: hidden;
  margin: 0 24px;
}

.prejoin-vu-bar {
  height: 100%;
  background: #2ecc71;
  border-radius: 2px;
  transition: width 0.1s ease;
}

.prejoin-speaker-test {
  background: none;
  border: 1px solid var(--border);
  border-radius: var(--radius);
  padding: 6px 12px;
  color: var(--text);
  cursor: pointer;
  font-size: 0.85rem;
  align-self: flex-start;
  margin-left: 24px;
}

.prejoin-speaker-test:hover {
  background: var(--bg-secondary);
}
```

- [ ] **Step 3: Verify VU meter animates with mic input**

Run: `cd crates/visio-desktop && cargo tauri dev`
Expected: VU meter bar animates when speaking into mic

- [ ] **Step 4: Commit**

```bash
git add crates/visio-desktop/frontend/src/App.tsx crates/visio-desktop/frontend/src/App.css
git commit -m "feat(desktop): PreJoin audio panel with device selectors and VU meter"
```

### Task 10: Desktop background filter side panel

**Files:**
- Modify: `crates/visio-desktop/frontend/src/App.tsx`
- Modify: `crates/visio-desktop/frontend/src/App.css`

- [ ] **Step 1: Add filter panel state and component**

Add state to PreJoinScreen:

```tsx
const [showFilters, setShowFilters] = useState(false);
const [backgroundMode, setBackgroundMode] = useState("off");
```

Build the filter panel:

```tsx
function FilterPanel({ backgroundMode, setBackgroundMode, onClose, lang, isDark }: FilterPanelProps) {
  const options = [
    { mode: "off", label: t("prejoin.bgOff", lang), icon: "🚫" },
    { mode: "blur", label: t("prejoin.bgBlur", lang), icon: "🌫️" },
    { mode: "blur-light", label: t("prejoin.bgBlurLight", lang), icon: "🌁" },
  ];

  // Load background image thumbnails (1-8)
  const [thumbnails, setThumbnails] = useState<string[]>([]);
  useEffect(() => {
    // Load thumbnail images from public/backgrounds/thumbnails/
    const paths = Array.from({ length: 8 }, (_, i) => `backgrounds/thumbnails/${i + 1}.jpg`);
    setThumbnails(paths);
  }, []);

  const selectMode = async (mode: string) => {
    setBackgroundMode(mode);
    if (mode.startsWith("image:")) {
      const id = parseInt(mode.split(":")[1]);
      await invoke("load_background_image", { id, jpegPath: `backgrounds/${id}.jpg` });
    }
    await invoke("set_background_mode", { mode });
  };

  return (
    <div className={`prejoin-filter-panel ${showFilters ? "open" : ""}`}>
      <div className="prejoin-filter-header">
        <h3>{t("prejoin.backgroundFilters", lang)}</h3>
        <button onClick={onClose}>✕</button>
      </div>
      <div className="prejoin-filter-options">
        {options.map(opt => (
          <button
            key={opt.mode}
            className={`prejoin-filter-option ${backgroundMode === opt.mode ? "selected" : ""}`}
            onClick={() => selectMode(opt.mode)}
          >
            <span>{opt.icon}</span>
            <span>{opt.label}</span>
          </button>
        ))}
      </div>
      <div className="prejoin-filter-grid">
        {thumbnails.map((thumb, i) => (
          <button
            key={i}
            className={`prejoin-filter-thumb ${backgroundMode === `image:${i + 1}` ? "selected" : ""}`}
            onClick={() => selectMode(`image:${i + 1}`)}
          >
            <img src={thumb} alt={`Background ${i + 1}`} />
          </button>
        ))}
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Add CSS for slide-in filter panel**

```css
.prejoin-filter-panel {
  position: fixed;
  top: 0;
  right: -320px;
  width: 320px;
  height: 100vh;
  background: var(--bg);
  border-left: 1px solid var(--border);
  transition: right 0.3s ease;
  z-index: 100;
  padding: 16px;
  overflow-y: auto;
}

.prejoin-filter-panel.open {
  right: 0;
}

.prejoin-filter-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 16px;
}

.prejoin-filter-options {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin-bottom: 16px;
}

.prejoin-filter-option {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 12px;
  border: 2px solid transparent;
  border-radius: var(--radius);
  background: var(--bg-secondary);
  color: var(--text);
  cursor: pointer;
  font-size: 0.9rem;
}

.prejoin-filter-option.selected {
  border-color: var(--accent);
}

.prejoin-filter-grid {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: 8px;
}

.prejoin-filter-thumb {
  border: 2px solid transparent;
  border-radius: var(--radius);
  overflow: hidden;
  cursor: pointer;
  padding: 0;
  background: none;
}

.prejoin-filter-thumb.selected {
  border-color: var(--accent);
}

.prejoin-filter-thumb img {
  width: 100%;
  aspect-ratio: 16/9;
  object-fit: cover;
}
```

- [ ] **Step 3: Wire the filter button to open the panel**

Update the "Filtres d'arrière-plan" button in PreJoinScreen:

```tsx
<button className="prejoin-filters-btn" onClick={() => setShowFilters(true)}>
  🎬 {t("prejoin.backgroundFilters", lang)}
</button>

{showFilters && (
  <FilterPanel
    backgroundMode={backgroundMode}
    setBackgroundMode={setBackgroundMode}
    onClose={() => setShowFilters(false)}
    lang={lang}
    isDark={isDark}
  />
)}
```

- [ ] **Step 4: Verify filter panel opens, effects apply in real-time on preview**

Run: `cd crates/visio-desktop && cargo tauri dev`
Expected: Filter panel slides in, selecting blur shows effect on camera preview

- [ ] **Step 5: Commit**

```bash
git add crates/visio-desktop/frontend/src/App.tsx crates/visio-desktop/frontend/src/App.css
git commit -m "feat(desktop): background filter side panel with live preview"
```

### Task 11: Desktop waiting room state

**Files:**
- Modify: `crates/visio-desktop/frontend/src/App.tsx`

- [ ] **Step 1: Add waiting state to PreJoinScreen**

```tsx
const [waitingState, setWaitingState] = useState<"idle" | "waiting" | "denied" | "timeout">("idle");
const waitingTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

const handleJoinNow = async () => {
  const finalUsername = displayName.trim() || null;

  // Save settings before joining
  await invoke("set_display_name", { name: finalUsername });
  await invoke("set_background_mode", { mode: backgroundMode });

  // If "no audio" mode, disable mic before connecting
  if (audioMode === "none") {
    await invoke("set_mic_enabled_on_join", { enabled: false });
  } else {
    await invoke("set_mic_enabled_on_join", { enabled: isMicOn });
  }

  setWaitingState("waiting");

  // Start 60s timeout
  waitingTimerRef.current = setTimeout(() => {
    setWaitingState("timeout");
  }, 60000);

  try {
    await invoke("connect", { meetUrl: roomUrl, username: finalUsername });
    // Connection succeeded (or enters lobby — check connection state)
    if (waitingTimerRef.current) clearTimeout(waitingTimerRef.current);
    onJoin(finalUsername);
  } catch (e) {
    if (waitingTimerRef.current) clearTimeout(waitingTimerRef.current);
    setWaitingState("idle");
  }
};

// Listen for lobby events
useEffect(() => {
  const unlistenDenied = listen("lobby-denied", () => {
    if (waitingTimerRef.current) clearTimeout(waitingTimerRef.current);
    setWaitingState("denied");
  });
  const unlistenAdmitted = listen("lobby-admitted", () => {
    if (waitingTimerRef.current) clearTimeout(waitingTimerRef.current);
    onJoin(displayName.trim() || null);
  });
  return () => {
    unlistenDenied.then(fn => fn());
    unlistenAdmitted.then(fn => fn());
  };
}, []);
```

- [ ] **Step 2: Update the "Join now" button to reflect waiting state**

```tsx
<div className="prejoin-actions">
  <button className="btn-secondary" onClick={onCancel} disabled={waitingState === "waiting"}>
    {t("prejoin.cancel", lang)}
  </button>

  {waitingState === "idle" && (
    <button className="btn-primary" onClick={handleJoinNow}>
      {t("prejoin.joinNow", lang)}
    </button>
  )}

  {waitingState === "waiting" && (
    <button className="btn-primary" disabled>
      <span className="spinner" /> {t("prejoin.waitingForApproval", lang)}
    </button>
  )}

  {waitingState === "denied" && (
    <div className="prejoin-denied">
      {t("prejoin.accessDenied", lang)}
      <button className="btn-secondary" onClick={onCancel}>
        {t("prejoin.backToHome", lang)}
      </button>
    </div>
  )}

  {waitingState === "timeout" && (
    <div className="prejoin-timeout">
      {t("prejoin.requestTimeout", lang)}
      <button className="btn-secondary" onClick={onCancel}>
        {t("prejoin.backToHome", lang)}
      </button>
    </div>
  )}
</div>
```

- [ ] **Step 3: Commit**

```bash
git add crates/visio-desktop/frontend/src/App.tsx
git commit -m "feat(desktop): waiting room state with timeout on PreJoin screen"
```

### Task 12: Desktop i18n strings

**Files:**
- Modify: `i18n/en.json`
- Modify: `i18n/fr.json`

- [ ] **Step 1: Add prejoin strings to all language files**

In `i18n/fr.json`:

```json
"prejoin.displayName": "Nom d'affichage",
"prejoin.camera": "Caméra",
"prejoin.backgroundFilters": "Filtres d'arrière-plan",
"prejoin.bgOff": "Aucun",
"prejoin.bgBlur": "Flou",
"prejoin.bgBlurLight": "Flou léger",
"prejoin.microphone": "Micro",
"prejoin.computerAudio": "Son de l'ordinateur",
"prejoin.noAudio": "Ne pas utiliser le son",
"prejoin.testSpeaker": "Tester",
"prejoin.cancel": "Annuler",
"prejoin.joinNow": "Rejoindre maintenant",
"prejoin.waitingForApproval": "En attente d'autorisation...",
"prejoin.accessDenied": "L'organisateur a refusé votre accès",
"prejoin.requestTimeout": "La demande d'accès a expiré",
"prejoin.backToHome": "Retour"
```

Add equivalent English translations to `i18n/en.json` and other language files.

- [ ] **Step 2: Commit**

```bash
git add i18n/
git commit -m "feat(i18n): add pre-join lobby screen translations"
```

---

## Phase 3: iOS PreJoin Screen

### Task 13: iOS PreJoinView — Navigation + Layout

**Files:**
- Create: `ios/VisioMobile/Views/PreJoinView.swift`
- Modify: `ios/VisioMobile/Views/HomeView.swift`

- [ ] **Step 1: Update HomeView navigation to go through PreJoinView**

In `HomeView.swift`, change the `.navigationDestination` (line 276-281):

```swift
// WAS:
// .navigationDestination(isPresented: $navigateToCall) {
//     CallView(roomURL: resolvedRoomURL, displayName: ...)
// }

// NEW:
.navigationDestination(isPresented: $navigateToCall) {
    PreJoinView(
        roomURL: resolvedRoomURL,
        initialDisplayName: displayName.trimmingCharacters(in: .whitespacesAndNewlines)
    )
}
```

- [ ] **Step 2: Create PreJoinView with stacked vertical layout**

```swift
import SwiftUI
import AVFoundation

struct PreJoinView: View {
    let roomURL: String
    let initialDisplayName: String

    @EnvironmentObject private var manager: VisioManager
    @Environment(\.dismiss) private var dismiss

    @State private var displayName: String = ""
    @State private var isCameraOn = true
    @State private var isMicOn = true
    @State private var audioMode: AudioMode = .computer
    @State private var navigateToCall = false
    @State private var waitingState: WaitingState = .idle
    @State private var isFrontCamera = true

    // Computed properties — must not be `let` since these are instance @Published properties
    private var isDark: Bool { manager.currentTheme == "dark" }
    private var lang: String { manager.currentLang }

    enum AudioMode { case computer, none }
    enum WaitingState { case idle, waiting, denied, timeout }

    var body: some View {
        let slug = roomURL.contains("/") ? String(roomURL.split(separator: "/").last ?? "") : roomURL

        ScrollView {
            VStack(spacing: 20) {
                // Room name
                Text(slug)
                    .font(.title2)
                    .fontWeight(.semibold)
                    .foregroundStyle(VisioColors.onBackground(dark: isDark))

                // Display name
                TextField(Strings.t("prejoin.displayName", lang: lang), text: $displayName)
                    .textFieldStyle(.roundedBorder)
                    .frame(maxWidth: 300)

                // Camera preview
                cameraPreviewSection

                // Audio config
                audioConfigSection

                // Actions
                actionsSection
            }
            .padding(24)
        }
        .background(VisioColors.background(dark: isDark).ignoresSafeArea())
        .navigationBarBackButtonHidden(true)
        .onAppear {
            displayName = initialDisplayName
            let settings = manager.client.getSettings()
            isCameraOn = settings.cameraEnabledOnJoin
            isMicOn = settings.micEnabledOnJoin
        }
        .navigationDestination(isPresented: $navigateToCall) {
            CallView(roomURL: roomURL, displayName: displayName)
        }
    }
}
```

- [ ] **Step 3: Add camera preview section**

```swift
private var cameraPreviewSection: some View {
    VStack(spacing: 8) {
        // Camera preview area
        ZStack {
            if isCameraOn {
                LocalCameraPreviewView(isFront: isFrontCamera)
                    .aspectRatio(4.0/3.0, contentMode: .fit)
                    .clipShape(RoundedRectangle(cornerRadius: 12))
            } else {
                RoundedRectangle(cornerRadius: 12)
                    .fill(Color.black)
                    .aspectRatio(4.0/3.0, contentMode: .fit)
                    .overlay(
                        Text(String((displayName.first ?? "?").uppercased()))
                            .font(.system(size: 40, weight: .semibold))
                            .foregroundStyle(.white)
                            .frame(width: 72, height: 72)
                            .background(VisioColors.primary500)
                            .clipShape(Circle())
                    )
            }
        }

        // Camera controls: front/back toggle + on/off
        HStack {
            Button {
                isFrontCamera.toggle()
            } label: {
                Image(systemName: "arrow.triangle.2.circlepath.camera")
                    .foregroundStyle(VisioColors.primary500)
            }

            Toggle(isOn: $isCameraOn) {
                Text(Strings.t("prejoin.camera", lang: lang))
                    .font(.subheadline)
            }
            .toggleStyle(.switch)
            .tint(VisioColors.primary500)
        }
        .padding(.horizontal, 12)

        // Background filters button
        Button {
            // Open filter sheet — see Task 16
        } label: {
            Label(Strings.t("prejoin.backgroundFilters", lang: lang), systemImage: "camera.filters")
                .font(.subheadline)
                .foregroundStyle(VisioColors.primary500)
        }
    }
}
```

- [ ] **Step 4: Commit**

```bash
git add ios/VisioMobile/Views/PreJoinView.swift ios/VisioMobile/Views/HomeView.swift
git commit -m "feat(ios): PreJoinView layout with camera preview section"
```

### Task 14: iOS LocalCameraPreviewView

**Files:**
- Create: `ios/VisioMobile/Views/LocalCameraPreviewView.swift`

- [ ] **Step 1: Create a SwiftUI wrapper for AVCaptureVideoPreviewLayer**

```swift
import SwiftUI
import AVFoundation

struct LocalCameraPreviewView: UIViewRepresentable {
    let isFront: Bool

    func makeCoordinator() -> Coordinator {
        Coordinator()
    }

    func makeUIView(context: Context) -> PreviewUIView {
        let view = PreviewUIView()
        context.coordinator.view = view
        context.coordinator.startSession(front: isFront)
        return view
    }

    func updateUIView(_ uiView: PreviewUIView, context: Context) {
        context.coordinator.switchCamera(front: isFront)
    }

    static func dismantleUIView(_ uiView: PreviewUIView, coordinator: Coordinator) {
        coordinator.stopSession()
    }

    class Coordinator {
        weak var view: PreviewUIView?
        private let session = AVCaptureSession()
        private var currentInput: AVCaptureDeviceInput?
        private var currentPosition: AVCaptureDevice.Position = .front

        func startSession(front: Bool) {
            session.sessionPreset = .medium
            let position: AVCaptureDevice.Position = front ? .front : .back
            guard let device = AVCaptureDevice.default(.builtInWideAngleCamera, for: .video, position: position),
                  let input = try? AVCaptureDeviceInput(device: device) else { return }

            session.beginConfiguration()
            session.addInput(input)
            session.commitConfiguration()

            currentInput = input
            currentPosition = position

            DispatchQueue.main.async {
                self.view?.previewLayer.session = self.session
            }

            DispatchQueue.global(qos: .userInitiated).async {
                self.session.startRunning()
            }
        }

        func switchCamera(front: Bool) {
            let newPosition: AVCaptureDevice.Position = front ? .front : .back
            guard newPosition != currentPosition else { return }
            guard let device = AVCaptureDevice.default(.builtInWideAngleCamera, for: .video, position: newPosition),
                  let input = try? AVCaptureDeviceInput(device: device) else { return }

            session.beginConfiguration()
            if let old = currentInput { session.removeInput(old) }
            session.addInput(input)
            session.commitConfiguration()

            currentInput = input
            currentPosition = newPosition
        }

        func stopSession() {
            session.stopRunning()
        }
    }
}

class PreviewUIView: UIView {
    let previewLayer = AVCaptureVideoPreviewLayer()

    override init(frame: CGRect) {
        super.init(frame: frame)
        previewLayer.videoGravity = .resizeAspectFill
        layer.addSublayer(previewLayer)
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override func layoutSubviews() {
        super.layoutSubviews()
        previewLayer.frame = bounds
    }
}
```

Note: This is a simple native preview using `AVCaptureVideoPreviewLayer` for v1. To show blur effects in real-time on the preview, this will need to be replaced with a frame-processing approach (capture frames → Rust blur → render back). This can be a follow-up task since the blur pipeline on iOS needs the mobile blur infrastructure (Task 19).

- [ ] **Step 2: Commit**

```bash
git add ios/VisioMobile/Views/LocalCameraPreviewView.swift
git commit -m "feat(ios): LocalCameraPreviewView using AVCaptureVideoPreviewLayer"
```

### Task 15: iOS Audio config + VU meter

**Files:**
- Modify: `ios/VisioMobile/Views/PreJoinView.swift`

- [ ] **Step 1: Add audio config section with device selectors**

```swift
private var audioConfigSection: some View {
    VStack(spacing: 8) {
        // Computer audio option
        Button {
            audioMode = .computer
        } label: {
            HStack {
                Image(systemName: audioMode == .computer ? "largecircle.fill.circle" : "circle")
                    .foregroundStyle(VisioColors.primary500)
                Text(Strings.t("prejoin.computerAudio", lang: lang))
                    .foregroundStyle(VisioColors.onBackground(dark: isDark))
                Spacer()
            }
            .padding(12)
            .background(
                RoundedRectangle(cornerRadius: 8)
                    .stroke(audioMode == .computer ? VisioColors.primary500 : VisioColors.border(dark: isDark), lineWidth: 1)
            )
        }

        if audioMode == .computer {
            // Mic toggle
            HStack {
                Image(systemName: "mic.fill")
                    .foregroundStyle(VisioColors.primary500)
                    .frame(width: 20)
                Text(Strings.t("prejoin.microphone", lang: lang))
                    .font(.subheadline)
                Spacer()
                Toggle("", isOn: $isMicOn)
                    .toggleStyle(.switch)
                    .tint(VisioColors.primary500)
                    .labelsHidden()
            }
            .padding(.horizontal, 12)

            // VU meter
            if isMicOn {
                MicLevelView()
                    .frame(height: 4)
                    .padding(.horizontal, 36)
            }

            // Speaker test
            Button {
                playSpeakerTest()
            } label: {
                Label(Strings.t("prejoin.testSpeaker", lang: lang), systemImage: "speaker.wave.2")
                    .font(.subheadline)
                    .foregroundStyle(VisioColors.primary500)
            }
            .padding(.horizontal, 12)
        }

        // No audio option
        Button {
            audioMode = .none
        } label: {
            HStack {
                Image(systemName: audioMode == .none ? "largecircle.fill.circle" : "circle")
                    .foregroundStyle(VisioColors.primary500)
                Text(Strings.t("prejoin.noAudio", lang: lang))
                    .foregroundStyle(VisioColors.onBackground(dark: isDark))
                Spacer()
            }
            .padding(12)
            .background(
                RoundedRectangle(cornerRadius: 8)
                    .stroke(audioMode == .none ? VisioColors.primary500 : VisioColors.border(dark: isDark), lineWidth: 1)
            )
        }
    }
}
```

- [ ] **Step 2: Create MicLevelView using AVAudioEngine**

```swift
/// Retains the AVAudioEngine and exposes mic level as a published property.
class MicLevelMonitor: ObservableObject {
    @Published var level: Float = 0
    private var engine: AVAudioEngine?

    func start() {
        let engine = AVAudioEngine()
        let inputNode = engine.inputNode
        let format = inputNode.outputFormat(forBus: 0)

        inputNode.installTap(onBus: 0, bufferSize: 1024, format: format) { [weak self] buffer, _ in
            guard let data = buffer.floatChannelData?[0] else { return }
            let frameCount = Int(buffer.frameLength)
            var sumSq: Float = 0
            for i in 0..<frameCount {
                let sample = data[i]
                sumSq += sample * sample
            }
            let rms = sqrt(sumSq / Float(frameCount))
            DispatchQueue.main.async {
                self?.level = min(rms * 3.0, 1.0)
            }
        }

        try? engine.start()
        self.engine = engine  // Retain the engine
    }

    func stop() {
        engine?.inputNode.removeTap(onBus: 0)
        engine?.stop()
        engine = nil
        level = 0
    }

    deinit { stop() }
}

struct MicLevelView: View {
    @StateObject private var monitor = MicLevelMonitor()

    var body: some View {
        GeometryReader { geometry in
            ZStack(alignment: .leading) {
                RoundedRectangle(cornerRadius: 2)
                    .fill(Color.gray.opacity(0.3))
                RoundedRectangle(cornerRadius: 2)
                    .fill(Color.green)
                    .frame(width: geometry.size.width * CGFloat(min(monitor.level, 1.0)))
                    .animation(.linear(duration: 0.1), value: monitor.level)
            }
        }
        .onAppear { monitor.start() }
        .onDisappear { monitor.stop() }
    }
}
```

- [ ] **Step 3: Add speaker test using AVAudioPlayer**

Add a `@State` property to retain the player:

```swift
@State private var speakerTestPlayer: AVAudioPlayer?

private func playSpeakerTest() {
    guard let url = Bundle.main.url(forResource: "speaker-test", withExtension: "mp3") else { return }
    speakerTestPlayer = try? AVAudioPlayer(contentsOf: url)
    speakerTestPlayer?.play()
}
```

Bundle the same `speaker-test.mp3` file in the iOS app resources.

- [ ] **Step 4: Commit**

```bash
git add ios/VisioMobile/Views/PreJoinView.swift
git commit -m "feat(ios): PreJoin audio config with VU meter and speaker test"
```

### Task 16: iOS background filter bottom sheet

**Files:**
- Modify: `ios/VisioMobile/Views/PreJoinView.swift`

- [ ] **Step 1: Add filter sheet state and presentation**

Add to PreJoinView:

```swift
@State private var showFilterSheet = false
@State private var backgroundMode = "off"
```

Update the background filters button:

```swift
Button {
    showFilterSheet = true
} label: {
    Label(Strings.t("prejoin.backgroundFilters", lang: lang), systemImage: "camera.filters")
        .font(.subheadline)
        .foregroundStyle(VisioColors.primary500)
}
.sheet(isPresented: $showFilterSheet) {
    BackgroundFilterSheet(backgroundMode: $backgroundMode, lang: lang, isDark: isDark)
        .presentationDetents([.medium])
        .environmentObject(manager)
}
```

- [ ] **Step 2: Create BackgroundFilterSheet**

Reuse the existing pattern from `InCallSettingsSheet.swift` (lines 124-157) which already has blur option buttons and image grid:

```swift
struct BackgroundFilterSheet: View {
    @Binding var backgroundMode: String
    let lang: String
    let isDark: Bool
    @EnvironmentObject private var manager: VisioManager

    var body: some View {
        NavigationStack {
            List {
                // Off
                Button { setMode("off") } label: {
                    HStack {
                        Image(systemName: "circle.slash").foregroundStyle(VisioColors.primary500)
                        Text(Strings.t("prejoin.bgOff", lang: lang))
                        Spacer()
                        if backgroundMode == "off" {
                            Image(systemName: "checkmark").foregroundStyle(VisioColors.primary500)
                        }
                    }
                }

                // Blur
                Button { setMode("blur") } label: {
                    HStack {
                        Image(systemName: "aqi.medium").foregroundStyle(VisioColors.primary500)
                        Text(Strings.t("prejoin.bgBlur", lang: lang))
                        Spacer()
                        if backgroundMode == "blur" {
                            Image(systemName: "checkmark").foregroundStyle(VisioColors.primary500)
                        }
                    }
                }

                // Blur light
                Button { setMode("blur-light") } label: {
                    HStack {
                        Image(systemName: "aqi.low").foregroundStyle(VisioColors.primary500)
                        Text(Strings.t("prejoin.bgBlurLight", lang: lang))
                        Spacer()
                        if backgroundMode == "blur-light" {
                            Image(systemName: "checkmark").foregroundStyle(VisioColors.primary500)
                        }
                    }
                }

                // Background images grid
                LazyVGrid(columns: Array(repeating: GridItem(.flexible(), spacing: 8), count: 4), spacing: 8) {
                    ForEach(1...8, id: \.self) { id in
                        if let path = Bundle.main.path(forResource: "\(id)", ofType: "jpg", inDirectory: "backgrounds/thumbnails"),
                           let img = UIImage(contentsOfFile: path) {
                            Image(uiImage: img)
                                .resizable()
                                .aspectRatio(16.0/9.0, contentMode: .fill)
                                .frame(height: 50)
                                .clipShape(RoundedRectangle(cornerRadius: 6))
                                .overlay(
                                    RoundedRectangle(cornerRadius: 6)
                                        .stroke(backgroundMode == "image:\(id)" ? VisioColors.primary500 : Color.clear, lineWidth: 2)
                                )
                                .onTapGesture { setMode("image:\(id)") }
                        }
                    }
                }
            }
            .navigationTitle(Strings.t("prejoin.backgroundFilters", lang: lang))
            .navigationBarTitleDisplayMode(.inline)
        }
    }

    private func setMode(_ mode: String) {
        backgroundMode = mode
        let client = manager.client
        Task.detached {
            if mode.hasPrefix("image:") {
                let id = UInt8(mode.dropFirst(6)) ?? 0
                if let path = Bundle.main.path(forResource: "\(id)", ofType: "jpg", inDirectory: "backgrounds") {
                    try? client.loadBackgroundImage(id: id, jpegPath: path)
                }
            }
            client.setBackgroundMode(mode: mode)
        }
    }
}
```

- [ ] **Step 3: Commit**

```bash
git add ios/VisioMobile/Views/PreJoinView.swift
git commit -m "feat(ios): background filter bottom sheet on PreJoin"
```

### Task 17: iOS waiting room + join action

**Files:**
- Modify: `ios/VisioMobile/Views/PreJoinView.swift`

- [ ] **Step 1: Add actions section with waiting room state**

```swift
private var actionsSection: some View {
    HStack(spacing: 12) {
        Button(Strings.t("prejoin.cancel", lang: lang)) {
            dismiss()
        }
        .buttonStyle(.bordered)
        .disabled(waitingState == .waiting)

        switch waitingState {
        case .idle:
            Button {
                joinRoom()
            } label: {
                Text(Strings.t("prejoin.joinNow", lang: lang))
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.borderedProminent)
            .tint(VisioColors.primary500)

        case .waiting:
            Button {} label: {
                HStack {
                    ProgressView().scaleEffect(0.7)
                    Text(Strings.t("prejoin.waitingForApproval", lang: lang))
                }
                .frame(maxWidth: .infinity)
            }
            .buttonStyle(.borderedProminent)
            .tint(VisioColors.primary500)
            .disabled(true)

        case .denied:
            VStack {
                Text(Strings.t("prejoin.accessDenied", lang: lang))
                    .foregroundStyle(VisioColors.error500)
                Button(Strings.t("prejoin.backToHome", lang: lang)) { dismiss() }
                    .buttonStyle(.bordered)
            }

        case .timeout:
            VStack {
                Text(Strings.t("prejoin.requestTimeout", lang: lang))
                    .foregroundStyle(VisioColors.error500)
                Button(Strings.t("prejoin.backToHome", lang: lang)) { dismiss() }
                    .buttonStyle(.bordered)
            }
        }
    }
}

private func joinRoom() {
    waitingState = .waiting

    // Save settings + display name
    let name = displayName.trimmingCharacters(in: .whitespacesAndNewlines)
    manager.client.setDisplayName(name: name.isEmpty ? nil : name)
    manager.client.setCameraEnabledOnJoin(enabled: isCameraOn)
    // If "no audio", disable mic before connecting
    if audioMode == .none {
        manager.client.setMicEnabledOnJoin(enabled: false)
    } else {
        manager.client.setMicEnabledOnJoin(enabled: isMicOn)
    }

    let name = displayName.trimmingCharacters(in: .whitespacesAndNewlines)
    manager.connect(url: roomURL, username: name.isEmpty ? nil : name)

    // Start 60s timeout
    DispatchQueue.main.asyncAfter(deadline: .now() + 60) { [self] in
        if waitingState == .waiting {
            waitingState = .timeout
        }
    }

    // Navigate to call on success (observe connectionState)
    // This will need to observe manager.connectionState changes
    // and set navigateToCall = true when connected
}
```

- [ ] **Step 2: Observe connection state to transition to call**

Add a `.onChange` observer:

```swift
.onChange(of: manager.connectionState) { newState in
    if case .connected = newState {
        waitingState = .idle
        navigateToCall = true
    }
}
```

- [ ] **Step 3: Commit**

```bash
git add ios/VisioMobile/Views/PreJoinView.swift
git commit -m "feat(ios): PreJoin waiting room state and join action"
```

---

## Phase 4: Android PreJoin Screen

### Task 18: Android PreJoinScreen — Navigation + Layout

**Files:**
- Create: `android/app/src/main/kotlin/io/visio/mobile/ui/PreJoinScreen.kt`
- Modify: `android/app/src/main/kotlin/io/visio/mobile/navigation/AppNavigation.kt`

- [ ] **Step 1: Add lobby route to AppNavigation.kt**

In `AppNavigation.kt`, change the home `onJoin` to navigate to lobby:

```kotlin
composable("home") {
    HomeScreen(
        onJoin = { roomUrl, username ->
            val encoded = URLEncoder.encode(roomUrl, "UTF-8")
            val encodedName = URLEncoder.encode(username.ifBlank { "" }, "UTF-8")
            // WAS: navController.navigate("call/$encoded?username=$encodedName")
            navController.navigate("lobby/$encoded?username=$encodedName")
        },
        // ...
    )
}

// NEW: Lobby route
composable(
    route = "lobby/{roomUrl}?username={username}",
    arguments = listOf(
        navArgument("roomUrl") { type = NavType.StringType },
        navArgument("username") { type = NavType.StringType; defaultValue = "" },
    ),
) { backStackEntry ->
    val roomUrl = URLDecoder.decode(backStackEntry.arguments?.getString("roomUrl") ?: "", "UTF-8")
    val username = URLDecoder.decode(backStackEntry.arguments?.getString("username") ?: "", "UTF-8")
    PreJoinScreen(
        roomUrl = roomUrl,
        initialUsername = username,
        onJoin = { finalUsername ->
            val enc = URLEncoder.encode(roomUrl, "UTF-8")
            val encName = URLEncoder.encode(finalUsername.ifBlank { "" }, "UTF-8")
            navController.navigate("call/$enc?username=$encName") {
                popUpTo("home")
            }
        },
        onCancel = { navController.popBackStack() },
    )
}
```

- [ ] **Step 2: Create PreJoinScreen composable skeleton**

```kotlin
@Composable
fun PreJoinScreen(
    roomUrl: String,
    initialUsername: String,
    onJoin: (String) -> Unit,
    onCancel: () -> Unit,
) {
    val isDark = VisioManager.currentTheme == "dark"
    val lang = VisioManager.currentLang
    var displayName by remember { mutableStateOf(initialUsername) }
    var isCameraOn by remember { mutableStateOf(true) }
    var isMicOn by remember { mutableStateOf(true) }
    var audioMode by remember { mutableStateOf("computer") }
    var waitingState by remember { mutableStateOf("idle") }

    val slug = if ('/' in roomUrl) roomUrl.substringAfterLast('/') else roomUrl

    Column(
        modifier = Modifier
            .fillMaxSize()
            .background(MaterialTheme.colorScheme.background)
            .padding(24.dp)
            .verticalScroll(rememberScrollState()),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        // Room name
        Text(slug, style = MaterialTheme.typography.titleLarge, color = MaterialTheme.colorScheme.onBackground)
        Spacer(modifier = Modifier.height(8.dp))

        // Display name
        OutlinedTextField(
            value = displayName,
            onValueChange = { displayName = it },
            label = { Text(Strings.t("prejoin.displayName", lang)) },
            modifier = Modifier.width(300.dp),
            singleLine = true,
        )
        Spacer(modifier = Modifier.height(20.dp))

        // Camera preview section — see Task 18 step 3
        CameraPreviewSection(isCameraOn, { isCameraOn = it }, isDark, lang)

        Spacer(modifier = Modifier.height(16.dp))

        // Audio config section — see Task 20
        AudioConfigSection(audioMode, { audioMode = it }, isMicOn, { isMicOn = it }, isDark, lang)

        Spacer(modifier = Modifier.height(20.dp))

        // Actions
        Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            OutlinedButton(onClick = onCancel) {
                Text(Strings.t("prejoin.cancel", lang))
            }
            Button(
                onClick = { onJoin(displayName.trim()) },
                colors = ButtonDefaults.buttonColors(containerColor = VisioColors.Primary500),
            ) {
                Text(Strings.t("prejoin.joinNow", lang))
            }
        }
    }
}
```

- [ ] **Step 3: Add camera preview section with front/back toggle**

```kotlin
@Composable
fun CameraPreviewSection(
    isCameraOn: Boolean,
    onToggle: (Boolean) -> Unit,
    isDark: Boolean,
    lang: String,
) {
    var isFrontCamera by remember { mutableStateOf(true) }
    val context = LocalContext.current

    Column {
        // Camera preview area
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .aspectRatio(4f / 3f)
                .clip(RoundedCornerShape(12.dp))
                .background(Color.Black),
            contentAlignment = Alignment.Center,
        ) {
            if (isCameraOn) {
                AndroidView(
                    factory = { ctx -> LocalCameraPreview(ctx, isFrontCamera) },
                    update = { it.switchCamera(isFrontCamera) },
                    modifier = Modifier.fillMaxSize(),
                )
            } else {
                // Avatar placeholder
                Box(
                    modifier = Modifier
                        .size(72.dp)
                        .background(VisioColors.Primary500, CircleShape),
                    contentAlignment = Alignment.Center,
                ) {
                    Text("?", color = Color.White, fontSize = 28.sp, fontWeight = FontWeight.SemiBold)
                }
            }
        }

        Spacer(modifier = Modifier.height(8.dp))

        // Camera controls
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            IconButton(onClick = { isFrontCamera = !isFrontCamera }) {
                Icon(Icons.Default.Cameraswitch, contentDescription = "Switch camera", tint = VisioColors.Primary500)
            }
            Text(Strings.t("prejoin.camera", lang), style = MaterialTheme.typography.bodyMedium)
            Spacer(modifier = Modifier.weight(1f))
            Switch(checked = isCameraOn, onCheckedChange = onToggle, colors = SwitchDefaults.colors(checkedThumbColor = VisioColors.Primary500))
        }
    }
}
```

- [ ] **Step 4: Create LocalCameraPreview Android View**

Create a simple camera preview using Camera2 API + SurfaceView (similar pattern to existing `CameraCapture.kt` but rendering to a local SurfaceView instead of pushing via JNI):

```kotlin
class LocalCameraPreview(context: Context, private var useFront: Boolean) : SurfaceView(context), SurfaceHolder.Callback {
    private var session: CameraCaptureSession? = null
    private var camera: CameraDevice? = null
    private val handlerThread = HandlerThread("PreviewCamera").also { it.start() }
    private val handler = Handler(handlerThread.looper)

    init {
        holder.addCallback(this)
    }

    override fun surfaceCreated(holder: SurfaceHolder) {
        openCamera()
    }

    override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) {}

    override fun surfaceDestroyed(holder: SurfaceHolder) {
        stopCamera()
    }

    fun switchCamera(front: Boolean) {
        if (front == useFront) return
        useFront = front
        stopCamera()
        openCamera()
    }

    @SuppressLint("MissingPermission")
    private fun openCamera() {
        val cm = context.getSystemService(Context.CAMERA_SERVICE) as CameraManager
        val cameraId = cm.cameraIdList.firstOrNull { id ->
            val facing = cm.getCameraCharacteristics(id).get(CameraCharacteristics.LENS_FACING)
            if (useFront) facing == CameraCharacteristics.LENS_FACING_FRONT
            else facing == CameraCharacteristics.LENS_FACING_BACK
        } ?: return

        cm.openCamera(cameraId, object : CameraDevice.StateCallback() {
            override fun onOpened(dev: CameraDevice) {
                camera = dev
                val surface = holder.surface
                dev.createCaptureSession(listOf(surface), object : CameraCaptureSession.StateCallback() {
                    override fun onConfigured(sess: CameraCaptureSession) {
                        session = sess
                        val req = dev.createCaptureRequest(CameraDevice.TEMPLATE_PREVIEW).apply {
                            addTarget(surface)
                        }.build()
                        sess.setRepeatingRequest(req, null, handler)
                    }
                    override fun onConfigureFailed(sess: CameraCaptureSession) {}
                }, handler)
            }
            override fun onDisconnected(dev: CameraDevice) { dev.close() }
            override fun onError(dev: CameraDevice, error: Int) { dev.close() }
        }, handler)
    }

    private fun stopCamera() {
        session?.close(); session = null
        camera?.close(); camera = null
    }
}
```

Note: Like the iOS preview, this uses a native preview surface for v1. Real-time blur effects require routing frames through the Rust pipeline, which will be wired up once the mobile blur infrastructure is ready.

- [ ] **Step 5: Commit**

```bash
git add android/app/src/main/kotlin/io/visio/mobile/ui/PreJoinScreen.kt android/app/src/main/kotlin/io/visio/mobile/navigation/AppNavigation.kt
git commit -m "feat(android): PreJoinScreen with camera preview and navigation"
```

### Task 19: Android Audio config + VU meter

**Files:**
- Modify: `android/app/src/main/kotlin/io/visio/mobile/ui/PreJoinScreen.kt`

- [ ] **Step 1: Add AudioConfigSection composable**

```kotlin
@Composable
fun AudioConfigSection(
    audioMode: String,
    onAudioModeChange: (String) -> Unit,
    isMicOn: Boolean,
    onMicToggle: (Boolean) -> Unit,
    isDark: Boolean,
    lang: String,
) {
    val context = LocalContext.current
    val audioManager = remember { context.getSystemService(Context.AUDIO_SERVICE) as AudioManager }
    val inputDevices = remember { audioManager.getDevices(AudioManager.GET_DEVICES_INPUTS).toList() }
    val outputDevices = remember { audioManager.getDevices(AudioManager.GET_DEVICES_OUTPUTS).toList() }
    var micLevel by remember { mutableFloatStateOf(0f) }

    // VU meter: monitor mic level when mic is on
    LaunchedEffect(audioMode, isMicOn) {
        if (audioMode == "computer" && isMicOn) {
            withContext(Dispatchers.IO) {
                val bufferSize = AudioRecord.getMinBufferSize(44100, AudioFormat.CHANNEL_IN_MONO, AudioFormat.ENCODING_PCM_16BIT)
                if (ActivityCompat.checkSelfPermission(context, Manifest.permission.RECORD_AUDIO) != PackageManager.PERMISSION_GRANTED) return@withContext
                val recorder = AudioRecord(MediaRecorder.AudioSource.MIC, 44100, AudioFormat.CHANNEL_IN_MONO, AudioFormat.ENCODING_PCM_16BIT, bufferSize)
                recorder.startRecording()
                val buffer = ShortArray(bufferSize / 2)
                try {
                    while (isActive) {
                        val read = recorder.read(buffer, 0, buffer.size)
                        if (read > 0) {
                            var sumSq = 0.0
                            for (i in 0 until read) {
                                val s = buffer[i].toDouble() / Short.MAX_VALUE
                                sumSq += s * s
                            }
                            val rms = sqrt(sumSq / read).toFloat()
                            micLevel = (rms * 3f).coerceAtMost(1f)
                        }
                        delay(100)
                    }
                } finally {
                    recorder.stop()
                    recorder.release()
                }
            }
        } else {
            micLevel = 0f
        }
    }

    Column(modifier = Modifier.fillMaxWidth()) {
        // Computer audio option
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .clickable { onAudioModeChange("computer") }
                .border(1.dp, if (audioMode == "computer") VisioColors.Primary500 else MaterialTheme.colorScheme.outline, RoundedCornerShape(8.dp))
                .padding(12.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            RadioButton(selected = audioMode == "computer", onClick = { onAudioModeChange("computer") })
            Text(Strings.t("prejoin.computerAudio", lang), modifier = Modifier.padding(start = 8.dp))
        }

        if (audioMode == "computer") {
            Spacer(modifier = Modifier.height(8.dp))

            // Mic toggle
            Row(
                modifier = Modifier.fillMaxWidth().padding(horizontal = 12.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Icon(Icons.Default.Mic, contentDescription = null, tint = VisioColors.Primary500, modifier = Modifier.size(20.dp))
                Text(Strings.t("prejoin.microphone", lang), modifier = Modifier.weight(1f).padding(start = 8.dp), style = MaterialTheme.typography.bodySmall)
                Switch(checked = isMicOn, onCheckedChange = onMicToggle, colors = SwitchDefaults.colors(checkedThumbColor = VisioColors.Primary500))
            }

            // VU meter
            if (isMicOn) {
                LinearProgressIndicator(
                    progress = { micLevel },
                    modifier = Modifier.fillMaxWidth().padding(horizontal = 36.dp).height(4.dp).clip(RoundedCornerShape(2.dp)),
                    color = Color(0xFF2ecc71),
                    trackColor = MaterialTheme.colorScheme.surfaceVariant,
                )
            }

            Spacer(modifier = Modifier.height(8.dp))

            // Speaker test
            OutlinedButton(
                onClick = {
                    val mp = MediaPlayer.create(context, R.raw.speaker_test)
                    mp?.setOnCompletionListener { it.release() }
                    mp?.start()
                },
                modifier = Modifier.padding(start = 36.dp),
            ) {
                Icon(Icons.Default.VolumeUp, contentDescription = null, modifier = Modifier.size(16.dp))
                Spacer(modifier = Modifier.width(4.dp))
                Text(Strings.t("prejoin.testSpeaker", lang), style = MaterialTheme.typography.bodySmall)
            }
        }

        Spacer(modifier = Modifier.height(8.dp))

        // No audio option
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .clickable { onAudioModeChange("none") }
                .border(1.dp, if (audioMode == "none") VisioColors.Primary500 else MaterialTheme.colorScheme.outline, RoundedCornerShape(8.dp))
                .padding(12.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            RadioButton(selected = audioMode == "none", onClick = { onAudioModeChange("none") })
            Text(Strings.t("prejoin.noAudio", lang), modifier = Modifier.padding(start = 8.dp))
        }
    }
}
```

- [ ] **Step 2: Add speaker test audio file**

Place `speaker_test.mp3` in `android/app/src/main/res/raw/speaker_test.mp3`

- [ ] **Step 3: Commit**

```bash
git add android/app/src/main/kotlin/io/visio/mobile/ui/PreJoinScreen.kt android/app/src/main/res/raw/speaker_test.mp3
git commit -m "feat(android): PreJoin audio config with VU meter and speaker test"
```

### Task 20: Android background filter bottom sheet

**Files:**
- Modify: `android/app/src/main/kotlin/io/visio/mobile/ui/PreJoinScreen.kt`

- [ ] **Step 1: Add filter bottom sheet**

Follow the same pattern used in `InCallSettingsSheet.kt` for background mode selection. Add a `ModalBottomSheet` with blur options and image grid:

```kotlin
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun BackgroundFilterSheet(
    backgroundMode: String,
    onModeChange: (String) -> Unit,
    onDismiss: () -> Unit,
    isDark: Boolean,
    lang: String,
) {
    val context = LocalContext.current
    val sheetState = rememberModalBottomSheetState()

    ModalBottomSheet(onDismissRequest = onDismiss, sheetState = sheetState) {
        Column(modifier = Modifier.padding(16.dp)) {
            Text(Strings.t("prejoin.backgroundFilters", lang), style = MaterialTheme.typography.titleMedium)
            Spacer(modifier = Modifier.height(16.dp))

            // Off option
            FilterOption("off", Strings.t("prejoin.bgOff", lang), backgroundMode) { onModeChange("off") }
            // Blur option
            FilterOption("blur", Strings.t("prejoin.bgBlur", lang), backgroundMode) { onModeChange("blur") }
            // Blur light option
            FilterOption("blur-light", Strings.t("prejoin.bgBlurLight", lang), backgroundMode) { onModeChange("blur-light") }

            Spacer(modifier = Modifier.height(12.dp))

            // Image grid
            LazyVerticalGrid(
                columns = GridCells.Fixed(4),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                items(8) { index ->
                    val id = index + 1
                    val isSelected = backgroundMode == "image:$id"
                    // Load thumbnail from assets
                    val bitmap = remember {
                        try {
                            context.assets.open("backgrounds/thumbnails/$id.jpg").use {
                                BitmapFactory.decodeStream(it)
                            }
                        } catch (_: Exception) { null }
                    }
                    bitmap?.let {
                        Image(
                            bitmap = it.asImageBitmap(),
                            contentDescription = "Background $id",
                            contentScale = ContentScale.Crop,
                            modifier = Modifier
                                .aspectRatio(16f / 9f)
                                .clip(RoundedCornerShape(6.dp))
                                .border(
                                    2.dp,
                                    if (isSelected) VisioColors.Primary500 else Color.Transparent,
                                    RoundedCornerShape(6.dp),
                                )
                                .clickable {
                                    // Load full image and set mode
                                    val file = java.io.File(context.cacheDir, "bg_$id.jpg")
                                    if (!file.exists()) {
                                        context.assets.open("backgrounds/$id.jpg").use { input ->
                                            file.outputStream().use { output -> input.copyTo(output) }
                                        }
                                    }
                                    try { VisioManager.client.loadBackgroundImage(id.toUByte(), file.absolutePath) } catch (_: Exception) {}
                                    onModeChange("image:$id")
                                },
                        )
                    }
                }
            }
        }
    }
}

@Composable
private fun FilterOption(mode: String, label: String, currentMode: String, onClick: () -> Unit) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onClick)
            .padding(vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(label, modifier = Modifier.weight(1f))
        if (currentMode == mode) {
            Icon(Icons.Default.Check, contentDescription = null, tint = VisioColors.Primary500)
        }
    }
}
```

- [ ] **Step 2: Wire filter sheet into PreJoinScreen**

Add state and trigger in `CameraPreviewSection`:

```kotlin
var showFilterSheet by remember { mutableStateOf(false) }

// Add below camera controls:
TextButton(onClick = { showFilterSheet = true }) {
    Icon(Icons.Default.FilterVintage, contentDescription = null, tint = VisioColors.Primary500, modifier = Modifier.size(16.dp))
    Spacer(modifier = Modifier.width(4.dp))
    Text(Strings.t("prejoin.backgroundFilters", lang), color = VisioColors.Primary500, style = MaterialTheme.typography.bodySmall)
}

if (showFilterSheet) {
    BackgroundFilterSheet(
        backgroundMode = backgroundMode,
        onModeChange = { mode ->
            backgroundMode = mode
            VisioManager.client.setBackgroundMode(mode)
        },
        onDismiss = { showFilterSheet = false },
        isDark = isDark,
        lang = lang,
    )
}
```

- [ ] **Step 3: Commit**

```bash
git add android/app/src/main/kotlin/io/visio/mobile/ui/PreJoinScreen.kt
git commit -m "feat(android): background filter bottom sheet on PreJoin"
```

### Task 21: Android waiting room + join action

**Files:**
- Modify: `android/app/src/main/kotlin/io/visio/mobile/ui/PreJoinScreen.kt`

- [ ] **Step 1: Add waiting room state management**

Use a sealed interface for type-safe waiting state (not strings):

```kotlin
sealed interface WaitingState {
    data object Idle : WaitingState
    data object Waiting : WaitingState
    data object Denied : WaitingState
    data object Timeout : WaitingState
}

var waitingState by remember { mutableStateOf<WaitingState>(WaitingState.Idle) }
val coroutineScope = rememberCoroutineScope()

// Observe connection state
val connectionState by VisioManager.connectionState.collectAsState()
LaunchedEffect(connectionState) {
    if (connectionState is ConnectionState.Connected) {
        waitingState = WaitingState.Idle
        onJoin(displayName.trim())
    }
}

fun handleJoin() {
    waitingState = WaitingState.Waiting

    // Save settings + display name
    try {
        VisioManager.client.setDisplayName(displayName.trim().ifEmpty { null })
        VisioManager.client.setCameraEnabledOnJoin(isCameraOn)
        // If "no audio", disable mic before connecting
        if (audioMode == "none") {
            VisioManager.client.setMicEnabledOnJoin(false)
        } else {
            VisioManager.client.setMicEnabledOnJoin(isMicOn)
        }
    } catch (_: Exception) {}

    coroutineScope.launch {
        // 60s timeout
        launch {
            delay(60_000)
            if (waitingState is WaitingState.Waiting) waitingState = WaitingState.Timeout
        }

        withContext(Dispatchers.IO) {
            try {
                val user = displayName.trim().ifEmpty { null }
                VisioManager.client.connect(roomUrl, user)
            } catch (e: Exception) {
                waitingState = WaitingState.Idle
            }
        }
    }
}
```

- [ ] **Step 2: Update actions row**

```kotlin
Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
    OutlinedButton(onClick = onCancel, enabled = waitingState !is WaitingState.Waiting) {
        Text(Strings.t("prejoin.cancel", lang))
    }

    when (waitingState) {
        is WaitingState.Idle -> Button(
            onClick = { handleJoin() },
            colors = ButtonDefaults.buttonColors(containerColor = VisioColors.Primary500),
        ) { Text(Strings.t("prejoin.joinNow", lang)) }

        is WaitingState.Waiting -> Button(onClick = {}, enabled = false, colors = ButtonDefaults.buttonColors(containerColor = VisioColors.Primary500)) {
            CircularProgressIndicator(modifier = Modifier.size(16.dp), strokeWidth = 2.dp, color = Color.White)
            Spacer(modifier = Modifier.width(8.dp))
            Text(Strings.t("prejoin.waitingForApproval", lang))
        }

        is WaitingState.Denied -> Text(Strings.t("prejoin.accessDenied", lang), color = VisioColors.Error500)

        is WaitingState.Timeout -> Text(Strings.t("prejoin.requestTimeout", lang), color = VisioColors.Error500)
    }
}
```

- [ ] **Step 3: Commit**

```bash
git add android/app/src/main/kotlin/io/visio/mobile/ui/PreJoinScreen.kt
git commit -m "feat(android): PreJoin waiting room state and join action"
```

---

## Phase 5: Mobile Background Blur (Real-time Effects on Preview)

### Task 22: ONNX Runtime integration in visio-video for Android

**Files:**
- Modify: `crates/visio-video/Cargo.toml` (add ort dependency for Android)
- Modify: `crates/visio-video/src/android.rs`

- [ ] **Step 1: Design the preview-with-blur frame path for Android**

The current frame flow is: `CameraCapture.kt → nativePushCameraFrame (JNI) → CAMERA_SOURCE → LiveKit`.

For preview mode, create a new JNI function that processes frames through blur but renders to the preview surface only (no CAMERA_SOURCE needed):

```rust
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_io_visio_mobile_NativeVideo_nativeProcessPreviewFrame(
    env: *mut jni::sys::JNIEnv,
    _class: jni::sys::jobject,
    y_buf: jni::sys::jobject,
    u_buf: jni::sys::jobject,
    v_buf: jni::sys::jobject,
    y_stride: jni::sys::jint,
    u_stride: jni::sys::jint,
    v_stride: jni::sys::jint,
    u_pixel_stride: jni::sys::jint,
    v_pixel_stride: jni::sys::jint,
    width: jni::sys::jint,
    height: jni::sys::jint,
    rotation_degrees: jni::sys::jint,
) {
    // Same YUV extraction as nativePushCameraFrame
    // Apply blur processing
    // Render to LOCAL_PREVIEW_SURFACE only
    // Do NOT feed into CAMERA_SOURCE (skip source.capture_frame())
}
```

- [ ] **Step 2: Add NativeVideo.nativeProcessPreviewFrame to Kotlin**

In `NativeVideo.kt`:

```kotlin
external fun nativeProcessPreviewFrame(
    yBuf: ByteBuffer, uBuf: ByteBuffer, vBuf: ByteBuffer,
    yStride: Int, uStride: Int, vStride: Int,
    uPixelStride: Int, vPixelStride: Int,
    width: Int, height: Int, rotationDegrees: Int,
)
```

- [ ] **Step 3: Update CameraCapture for preview mode**

Add a `previewMode` flag to `CameraCapture.kt`. When true, call `nativeProcessPreviewFrame` instead of `nativePushCameraFrame` in the ImageReader callback.

- [ ] **Step 4: Commit**

```bash
git add crates/visio-video/Cargo.toml crates/visio-video/src/android.rs crates/visio-ffi/src/lib.rs android/app/src/main/kotlin/io/visio/mobile/NativeVideo.kt android/app/src/main/kotlin/io/visio/mobile/CameraCapture.kt
git commit -m "feat(android): preview-only frame processing with blur (no LiveKit source)"
```

### Task 23: ONNX Runtime integration in visio-video for iOS

**Files:**
- Modify: `crates/visio-video/src/ios.rs`

- [ ] **Step 1: Add preview frame rendering for iOS**

Add a C FFI function for preview-only frame processing:

```rust
/// Render a preview frame with blur processing, without feeding to LiveKit.
/// Called from Swift CameraCapture in preview mode.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn visio_video_process_preview_frame(
    y_ptr: *const u8, y_stride: u32,
    u_ptr: *const u8, u_stride: u32,
    v_ptr: *const u8, v_stride: u32,
    width: u32, height: u32,
    rotation: u32,
) {
    // Build I420 buffer from planes
    // Apply blur::BlurProcessor::process_i420()
    // Call the iOS frame callback with track_sid "local-preview"
}
```

- [ ] **Step 2: Update iOS CameraCapture for preview mode**

In `CameraCapture.swift`, add a `previewMode` property. When true, in the `captureOutput` delegate callback, call `visio_video_process_preview_frame()` instead of `visio_push_ios_camera_frame()`.

- [ ] **Step 3: Update LocalCameraPreviewView to use the processed frame path**

Replace the `AVCaptureVideoPreviewLayer` approach with a frame-by-frame approach: use `CameraCapture` in preview mode + a `VideoDisplayView` to render the processed frames (same as CallView does, but with a fixed "local-preview" track SID).

- [ ] **Step 4: Commit**

```bash
git add crates/visio-video/src/ios.rs ios/VisioMobile/CameraCapture.swift ios/VisioMobile/Views/LocalCameraPreviewView.swift
git commit -m "feat(ios): preview-only frame processing with blur (no LiveKit source)"
```

---

## Phase 6: Unit Tests & Integration Testing

### Task 24: Unit tests for new settings and blur-light fix

**Files:**
- Modify: `crates/visio-core/src/settings.rs` (add test)

- [ ] **Step 1: Add test for audio_mode default and persistence**

```rust
#[test]
fn test_audio_mode_defaults_to_computer() {
    let dir = tempfile::tempdir().unwrap();
    let store = SettingsStore::new(dir.path().join("settings.json"));
    let settings = store.get();
    assert_eq!(settings.audio_mode, "computer");
}

#[test]
fn test_audio_mode_round_trips() {
    let dir = tempfile::tempdir().unwrap();
    let store = SettingsStore::new(dir.path().join("settings.json"));
    store.set_audio_mode("none".to_string());

    let store2 = SettingsStore::new(dir.path().join("settings.json"));
    assert_eq!(store2.get().audio_mode, "none");
}
```

- [ ] **Step 2: Add test for blur-light mode mapping**

In `crates/visio-ffi/src/blur/` test module (or create one):

```rust
#[test]
fn test_blur_light_mode_mapping() {
    BlurProcessor::set_mode(BackgroundMode::BlurLight);
    assert!(matches!(BlurProcessor::get_mode(), BackgroundMode::BlurLight));
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p visio-core && cargo test -p visio-ffi`
Expected: All tests pass

- [ ] **Step 4: Commit**

```bash
git add crates/visio-core/src/settings.rs crates/visio-ffi/src/blur/
git commit -m "test(core): add unit tests for audio_mode settings and blur-light mode"
```

### Task 25: End-to-end smoke test

- [ ] **Step 1: Desktop — full flow test**

1. Launch desktop app: `cd crates/visio-desktop && cargo tauri dev`
2. Enter a room URL, click "Rejoindre"
3. Verify PreJoin screen appears with:
   - Camera preview active
   - Device selectors populated
   - VU meter animates when speaking
   - Background filter panel opens and applies effects
   - Speaker test plays sound
4. Click "Rejoindre maintenant" → verify transition to call
5. Verify cancel returns to home

- [ ] **Step 2: iOS — build and test on device/simulator**

Run: `scripts/build-ios.sh device` (or sim)
Test same flow as desktop: PreJoinView → camera preview → audio config → join

- [ ] **Step 3: Android — build and test**

Run: `cd android && ./gradlew assembleDebug`
Test same flow: PreJoinScreen → camera preview → audio config → join

- [ ] **Step 4: Commit any fixes found during testing**

```bash
git add -A
git commit -m "fix: address issues found during pre-join lobby smoke testing"
```
