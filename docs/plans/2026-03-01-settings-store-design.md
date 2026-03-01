# SettingsStore Design

## Overview

Persistent key-value settings store for Visio, managed entirely in Rust via a JSON file. Independent of room lifecycle — settings exist before, during, and after room connections.

## Data Model

```rust
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Settings {
    pub display_name: Option<String>,
    pub language: Option<String>,        // ISO 639-1: "fr", "en", etc.
    pub mic_enabled_on_join: bool,       // default: true
    pub camera_enabled_on_join: bool,    // default: false
}
```

Defaults: `display_name: None`, `language: None` (follow system locale), `mic_enabled_on_join: true`, `camera_enabled_on_join: false`.

## Service

```rust
pub struct SettingsStore {
    settings: std::sync::Mutex<Settings>,
    file_path: PathBuf,
}
```

### Constructor

`SettingsStore::new(data_dir: &str)` — builds path `{data_dir}/settings.json`, loads from file if it exists, otherwise uses defaults.

### Public API

| Method | Description |
|--------|-------------|
| `get() -> Settings` | Returns a clone of current settings |
| `set_display_name(name: Option<String>)` | Update display name, persist |
| `set_language(lang: Option<String>)` | Update language code, persist |
| `set_mic_enabled_on_join(enabled: bool)` | Update mic preference, persist |
| `set_camera_enabled_on_join(enabled: bool)` | Update camera preference, persist |

### Private

- `save(&self)` — serialize to JSON, write file atomically
- `load(path) -> Settings` — deserialize from file, fallback to defaults

Uses `std::sync::Mutex` (not tokio) — file I/O is fast and synchronous.

## FFI Layer

`VisioClient::new(data_dir: String)` receives the platform data directory. Creates `SettingsStore` before `RoomManager`.

UniFFI-exposed methods:
- `get_settings() -> Settings`
- `set_display_name(name: Option<String>)`
- `set_language(lang: Option<String>)`
- `set_mic_enabled_on_join(enabled: bool)`
- `set_camera_enabled_on_join(enabled: bool)`

`Settings` is exposed as a UniFFI record.

## Integration

### Display name

`RoomManager::connect()` already accepts `username: Option<&str>`. Platforms read `settings.display_name` and pass it to `connect()`. No direct coupling between RoomManager and SettingsStore.

### Mic/camera on join

Platforms read `mic_enabled_on_join` / `camera_enabled_on_join` after connect and call `toggle_mic()` / `toggle_camera()` accordingly.

## Platform Integration

| Platform | Data dir source |
|----------|----------------|
| Android | `context.filesDir.absolutePath` passed to `VisioClient(dataDir)` |
| iOS | `NSSearchPathForDirectoriesInDomains(.documentDirectory, ...)` |
| Desktop | `app.path().app_data_dir()` (Tauri API) |

## No Events

SettingsStore does not emit events. Settings are read on demand. Platform UIs observe state natively (StateFlow, @Published, React state).
