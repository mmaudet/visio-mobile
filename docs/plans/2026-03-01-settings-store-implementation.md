# SettingsStore Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a persistent SettingsStore to visio-core that saves display name, language, and mic/camera-on-join preferences to a JSON file, exposed via UniFFI to all platforms.

**Architecture:** SettingsStore is an autonomous service in visio-core, independent of RoomManager. It owns a `std::sync::Mutex<Settings>` and a `PathBuf` pointing to `{data_dir}/settings.json`. Each setter updates in-memory state and writes to disk. `VisioClient::new(data_dir)` creates the store before anything else. Platform shells pass their app data directory at init.

**Tech Stack:** Rust (serde, serde_json, std::sync::Mutex, std::fs), UniFFI UDL, Kotlin/Compose, SwiftUI, Tauri 2.x

---

### Task 1: Create Settings struct and SettingsStore in visio-core

**Files:**
- Create: `crates/visio-core/src/settings.rs`
- Modify: `crates/visio-core/src/lib.rs:6-23`

**Step 1: Write the test module at the bottom of `settings.rs`**

Create `crates/visio-core/src/settings.rs` with tests first:

```rust
use std::path::PathBuf;
use std::sync::Mutex;
use serde::{Deserialize, Serialize};

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
}

fn default_true() -> bool { true }

impl Default for Settings {
    fn default() -> Self {
        Self {
            display_name: None,
            language: None,
            mic_enabled_on_join: true,
            camera_enabled_on_join: false,
        }
    }
}

pub struct SettingsStore {
    settings: Mutex<Settings>,
    file_path: PathBuf,
}

impl SettingsStore {
    pub fn new(data_dir: &str) -> Self {
        let file_path = PathBuf::from(data_dir).join("settings.json");
        let settings = Self::load(&file_path);
        Self {
            settings: Mutex::new(settings),
            file_path,
        }
    }

    pub fn get(&self) -> Settings {
        self.settings.lock().unwrap().clone()
    }

    pub fn set_display_name(&self, name: Option<String>) {
        self.settings.lock().unwrap().display_name = name;
        self.save();
    }

    pub fn set_language(&self, lang: Option<String>) {
        self.settings.lock().unwrap().language = lang;
        self.save();
    }

    pub fn set_mic_enabled_on_join(&self, enabled: bool) {
        self.settings.lock().unwrap().mic_enabled_on_join = enabled;
        self.save();
    }

    pub fn set_camera_enabled_on_join(&self, enabled: bool) {
        self.settings.lock().unwrap().camera_enabled_on_join = enabled;
        self.save();
    }

    fn save(&self) {
        let settings = self.settings.lock().unwrap().clone();
        if let Some(parent) = self.file_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(&settings) {
            let _ = std::fs::write(&self.file_path, json);
        }
    }

    fn load(path: &PathBuf) -> Settings {
        match std::fs::read_to_string(path) {
            Ok(contents) => serde_json::from_str(&contents).unwrap_or_default(),
            Err(_) => Settings::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_dir() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    #[test]
    fn test_default_settings() {
        let s = Settings::default();
        assert_eq!(s.display_name, None);
        assert_eq!(s.language, None);
        assert!(s.mic_enabled_on_join);
        assert!(!s.camera_enabled_on_join);
    }

    #[test]
    fn test_new_creates_defaults_when_no_file() {
        let dir = temp_dir();
        let store = SettingsStore::new(dir.path().to_str().unwrap());
        let s = store.get();
        assert_eq!(s, Settings::default());
    }

    #[test]
    fn test_set_display_name_persists() {
        let dir = temp_dir();
        let path = dir.path().to_str().unwrap();

        {
            let store = SettingsStore::new(path);
            store.set_display_name(Some("Alice".to_string()));
        }

        // Re-open from disk
        let store = SettingsStore::new(path);
        assert_eq!(store.get().display_name, Some("Alice".to_string()));
    }

    #[test]
    fn test_set_language_persists() {
        let dir = temp_dir();
        let path = dir.path().to_str().unwrap();

        {
            let store = SettingsStore::new(path);
            store.set_language(Some("fr".to_string()));
        }

        let store = SettingsStore::new(path);
        assert_eq!(store.get().language, Some("fr".to_string()));
    }

    #[test]
    fn test_set_mic_camera_persists() {
        let dir = temp_dir();
        let path = dir.path().to_str().unwrap();

        {
            let store = SettingsStore::new(path);
            store.set_mic_enabled_on_join(false);
            store.set_camera_enabled_on_join(true);
        }

        let store = SettingsStore::new(path);
        let s = store.get();
        assert!(!s.mic_enabled_on_join);
        assert!(s.camera_enabled_on_join);
    }

    #[test]
    fn test_clear_display_name() {
        let dir = temp_dir();
        let path = dir.path().to_str().unwrap();

        let store = SettingsStore::new(path);
        store.set_display_name(Some("Bob".to_string()));
        store.set_display_name(None);
        assert_eq!(store.get().display_name, None);
    }

    #[test]
    fn test_corrupt_file_falls_back_to_defaults() {
        let dir = temp_dir();
        let path = dir.path().to_str().unwrap();
        fs::write(dir.path().join("settings.json"), "not json!!!").unwrap();

        let store = SettingsStore::new(path);
        assert_eq!(store.get(), Settings::default());
    }

    #[test]
    fn test_partial_json_uses_serde_defaults() {
        let dir = temp_dir();
        let path = dir.path().to_str().unwrap();
        fs::write(
            dir.path().join("settings.json"),
            r#"{"display_name":"Eve"}"#,
        )
        .unwrap();

        let store = SettingsStore::new(path);
        let s = store.get();
        assert_eq!(s.display_name, Some("Eve".to_string()));
        assert!(s.mic_enabled_on_join); // serde default
        assert!(!s.camera_enabled_on_join); // serde default
    }
}
```

**Step 2: Add tempfile dev-dependency to Cargo.toml**

In `crates/visio-core/Cargo.toml`, add:

```toml
[dev-dependencies]
tempfile = "3"
```

**Step 3: Register the module in lib.rs**

In `crates/visio-core/src/lib.rs`, add after line `pub mod room;`:

```rust
pub mod settings;
```

And add to the `pub use` section:

```rust
pub use settings::{Settings, SettingsStore};
```

**Step 4: Run tests**

Run: `cd /Users/mmaudet/work/visio-mobile-v2 && cargo test -p visio-core settings`
Expected: All 7 tests pass.

**Step 5: Commit**

```bash
git add crates/visio-core/src/settings.rs crates/visio-core/src/lib.rs crates/visio-core/Cargo.toml
git commit -m "feat(core): add SettingsStore with JSON persistence and tests"
```

---

### Task 2: Add Settings dictionary and SettingsStore methods to UniFFI UDL

**Files:**
- Modify: `crates/visio-ffi/src/visio.udl:55-114`
- Modify: `crates/visio-ffi/src/lib.rs:319-447`

**Step 1: Add Settings dictionary to UDL**

In `crates/visio-ffi/src/visio.udl`, add after the `ChatMessage` dictionary (after line 55):

```
dictionary Settings {
    string? display_name;
    string? language;
    boolean mic_enabled_on_join;
    boolean camera_enabled_on_join;
};
```

**Step 2: Add settings methods to VisioClient interface in UDL**

In the `interface VisioClient` block, change the constructor and add settings methods. Replace:

```
    constructor();
```

With:

```
    constructor(string data_dir);
```

Add after `void add_listener(...)`:

```
    Settings get_settings();

    void set_display_name(string? name);

    void set_language(string? lang);

    void set_mic_enabled_on_join(boolean enabled);

    void set_camera_enabled_on_join(boolean enabled);
```

**Step 3: Add FFI Settings struct and From impl in lib.rs**

In `crates/visio-ffi/src/lib.rs`, add `Settings as CoreSettings` to the core import at line 7-15. Then add after the `ChatMessage` impl (after line 217):

```rust
#[derive(Debug, Clone)]
pub struct Settings {
    pub display_name: Option<String>,
    pub language: Option<String>,
    pub mic_enabled_on_join: bool,
    pub camera_enabled_on_join: bool,
}

impl From<visio_core::Settings> for Settings {
    fn from(s: visio_core::Settings) -> Self {
        Self {
            display_name: s.display_name,
            language: s.language,
            mic_enabled_on_join: s.mic_enabled_on_join,
            camera_enabled_on_join: s.camera_enabled_on_join,
        }
    }
}
```

**Step 4: Update VisioClient to accept data_dir and hold SettingsStore**

In the `VisioClient` struct definition, add `settings` field:

```rust
pub struct VisioClient {
    room_manager: visio_core::RoomManager,
    controls: visio_core::MeetingControls,
    chat: visio_core::ChatService,
    settings: visio_core::SettingsStore,
    rt: tokio::runtime::Runtime,
}
```

Update `VisioClient::new()` to accept `data_dir: String`:

```rust
    pub fn new(data_dir: String) -> Self {
        visio_log("VISIO FFI: VisioClient::new() called");
        let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
        visio_log("VISIO FFI: tokio runtime created successfully");
        let settings = visio_core::SettingsStore::new(&data_dir);
        let room_manager = visio_core::RoomManager::new();
        let controls = room_manager.controls();
        let chat = room_manager.chat();

        visio_log("VISIO FFI: VisioClient::new() completed");
        Self {
            room_manager,
            controls,
            chat,
            settings,
            rt,
        }
    }
```

**Step 5: Add settings methods to VisioClient impl**

Add after `add_listener`:

```rust
    pub fn get_settings(&self) -> Settings {
        self.settings.get().into()
    }

    pub fn set_display_name(&self, name: Option<String>) {
        self.settings.set_display_name(name);
    }

    pub fn set_language(&self, lang: Option<String>) {
        self.settings.set_language(lang);
    }

    pub fn set_mic_enabled_on_join(&self, enabled: bool) {
        self.settings.set_mic_enabled_on_join(enabled);
    }

    pub fn set_camera_enabled_on_join(&self, enabled: bool) {
        self.settings.set_camera_enabled_on_join(enabled);
    }
```

**Step 6: Update the FFI test to pass data_dir**

In the `test_visioclient_new_and_connect_smoke` test, change:

```rust
        let client = VisioClient::new();
```

To:

```rust
        let dir = std::env::temp_dir().join("visio-test");
        let client = VisioClient::new(dir.to_str().unwrap().to_string());
```

**Step 7: Verify it compiles**

Run: `cd /Users/mmaudet/work/visio-mobile-v2 && cargo build -p visio-ffi`
Expected: Compiles successfully.

**Step 8: Commit**

```bash
git add crates/visio-ffi/src/visio.udl crates/visio-ffi/src/lib.rs
git commit -m "feat(ffi): expose SettingsStore via UniFFI with data_dir constructor"
```

---

### Task 3: Update Desktop (Tauri) to pass data_dir and expose settings commands

**Files:**
- Modify: `crates/visio-desktop/src/lib.rs`
- Modify: `crates/visio-desktop/Cargo.toml` (if needed for tauri path API)

**Step 1: Add SettingsStore to VisioState and settings Tauri commands**

In `crates/visio-desktop/src/lib.rs`:

Add `SettingsStore` to the import from visio-core (line 6):

```rust
use visio_core::{
    ChatService, MeetingControls, RoomManager, Settings, SettingsStore, TrackInfo, TrackKind,
    VisioEvent, VisioEventListener,
};
```

Add `settings: SettingsStore` field to the `VisioState` struct:

```rust
struct VisioState {
    room: Arc<Mutex<RoomManager>>,
    controls: Arc<Mutex<MeetingControls>>,
    chat: Arc<Mutex<ChatService>>,
    settings: SettingsStore,
    #[cfg(target_os = "macos")]
    camera_capture: std::sync::Mutex<Option<camera_macos::MacCameraCapture>>,
}
```

Note: `SettingsStore` uses `std::sync::Mutex` internally, no `Arc<Mutex>` wrapper needed.

Add Tauri commands after `get_messages`:

```rust
#[tauri::command]
fn get_settings(state: tauri::State<'_, VisioState>) -> Result<serde_json::Value, String> {
    let s = state.settings.get();
    Ok(serde_json::json!({
        "display_name": s.display_name,
        "language": s.language,
        "mic_enabled_on_join": s.mic_enabled_on_join,
        "camera_enabled_on_join": s.camera_enabled_on_join,
    }))
}

#[tauri::command]
fn set_display_name(state: tauri::State<'_, VisioState>, name: Option<String>) {
    state.settings.set_display_name(name);
}

#[tauri::command]
fn set_language(state: tauri::State<'_, VisioState>, lang: Option<String>) {
    state.settings.set_language(lang);
}

#[tauri::command]
fn set_mic_enabled_on_join(state: tauri::State<'_, VisioState>, enabled: bool) {
    state.settings.set_mic_enabled_on_join(enabled);
}

#[tauri::command]
fn set_camera_enabled_on_join(state: tauri::State<'_, VisioState>, enabled: bool) {
    state.settings.set_camera_enabled_on_join(enabled);
}
```

**Step 2: Create SettingsStore in the `run()` function using Tauri app data dir**

In the `run()` function, the SettingsStore needs to be created before `VisioState`. Since we need the Tauri app data dir, which is only available after `tauri::Builder::default().setup(|app| ...)`, we have two options. The simplest: use a known default path first, then move it into setup.

Actually, the cleanest approach: create SettingsStore inside `setup()` and use `app.manage()` to add it. But since `VisioState` is a single struct, let's use a temp dir first then override, or better: compute the path before Tauri starts using `tauri::api` or use `dirs::data_dir()`.

Best approach for desktop: use `dirs::data_dir()` + app name since Tauri's `app_data_dir()` requires a running app handle.

Add `dirs = "6"` to `crates/visio-desktop/Cargo.toml` dependencies.

Then in `run()`, before creating state:

```rust
    let data_dir = dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("io.visio.desktop");
    std::fs::create_dir_all(&data_dir).ok();
    let settings = SettingsStore::new(data_dir.to_str().unwrap());
```

Add `settings` to the `VisioState` construction:

```rust
    let state = VisioState {
        room: room_arc,
        controls: Arc::new(Mutex::new(controls)),
        chat: Arc::new(Mutex::new(chat)),
        settings,
        #[cfg(target_os = "macos")]
        camera_capture: std::sync::Mutex::new(None),
    };
```

**Step 3: Register new commands in invoke_handler**

Add to the `generate_handler!` macro:

```rust
        .invoke_handler(tauri::generate_handler![
            connect,
            disconnect,
            get_connection_state,
            get_participants,
            get_video_tracks,
            toggle_mic,
            toggle_camera,
            send_chat,
            get_messages,
            get_settings,
            set_display_name,
            set_language,
            set_mic_enabled_on_join,
            set_camera_enabled_on_join,
        ])
```

**Step 4: Verify it compiles**

Run: `cd /Users/mmaudet/work/visio-mobile-v2 && cargo build -p visio-desktop`
Expected: Compiles.

**Step 5: Commit**

```bash
git add crates/visio-desktop/src/lib.rs crates/visio-desktop/Cargo.toml
git commit -m "feat(desktop): expose SettingsStore via Tauri commands"
```

---

### Task 4: Update Android VisioManager to pass data_dir

**Files:**
- Modify: `android/app/src/main/kotlin/io/visio/mobile/VisioManager.kt:16`

**Step 1: Update VisioClient construction**

The UniFFI-generated `VisioClient` constructor now requires a `dataDir: String` parameter.

In `VisioManager.kt`, change line 16 from:

```kotlin
    val client: VisioClient = VisioClient()
```

To:

```kotlin
    lateinit var client: VisioClient
        private set
```

Update `initialize()` to accept context and create the client:

```kotlin
    fun initialize(context: android.content.Context) {
        client = VisioClient(context.filesDir.absolutePath)
        client.addListener(this)
    }
```

**Step 2: Update VisioApplication to pass context**

Find and update `VisioApplication.kt` (or wherever `VisioManager.initialize()` is called) to pass `applicationContext`:

```kotlin
VisioManager.initialize(applicationContext)
```

**Step 3: Commit**

```bash
git add android/
git commit -m "feat(android): pass filesDir to VisioClient for settings persistence"
```

---

### Task 5: Update iOS VisioManager to pass data_dir

**Files:**
- Modify: `ios/VisioMobile/VisioManager.swift:24-28`

**Step 1: Update VisioClient construction**

Change the `init()` in `VisioManager.swift` from:

```swift
    init() {
        client = VisioClient()
        client.addListener(listener: self)
    }
```

To:

```swift
    init() {
        let documentsDir = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first!
        client = VisioClient(dataDir: documentsDir.path)
        client.addListener(listener: self)
    }
```

**Step 2: Commit**

```bash
git add ios/
git commit -m "feat(ios): pass documentDirectory to VisioClient for settings persistence"
```

---

### Task 6: Run full test suite and final verification

**Step 1: Run visio-core tests**

Run: `cd /Users/mmaudet/work/visio-mobile-v2 && cargo test -p visio-core`
Expected: All tests pass (existing 12 + 7 new settings tests = 19).

**Step 2: Build all targets**

Run: `cargo build -p visio-core -p visio-ffi -p visio-desktop`
Expected: All compile successfully.

**Step 3: Commit any fixes if needed**

If any compilation issues arise, fix and commit.
