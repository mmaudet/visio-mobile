use std::path::PathBuf;
use std::sync::Mutex;

use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use crate::room_display_name::strip_room_display_name_param;

/// A single entry in the visio join history.
///
/// Supports backward-compatible deserialization: old format stores bare strings,
/// new format stores `{"url": "...", "display_name": "..."}` objects.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct VisioHistoryEntry {
    pub url: String,
    pub display_name: Option<String>,
}

impl<'de> Deserialize<'de> for VisioHistoryEntry {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct VisioHistoryEntryVisitor;

        impl<'de> Visitor<'de> for VisioHistoryEntryVisitor {
            type Value = VisioHistoryEntry;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a string URL or an object with url and optional display_name")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(VisioHistoryEntry {
                    url: v.to_string(),
                    display_name: None,
                })
            }

            fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(VisioHistoryEntry {
                    url: v,
                    display_name: None,
                })
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: de::MapAccess<'de>,
            {
                let mut url: Option<String> = None;
                let mut display_name: Option<String> = None;

                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "url" => {
                            url = Some(map.next_value()?);
                        }
                        "display_name" => {
                            display_name = map.next_value()?;
                        }
                        _ => {
                            // Ignore unknown fields
                            let _ = map.next_value::<Value>()?;
                        }
                    }
                }

                let url = url.ok_or_else(|| de::Error::missing_field("url"))?;
                Ok(VisioHistoryEntry { url, display_name })
            }
        }

        deserializer.deserialize_any(VisioHistoryEntryVisitor)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub enum CalendarRefreshInterval {
    Minutes5,
    #[default]
    Minutes15,
    Hour1,
    Hours4,
    Manual,
}

/// A friendly alias mapping a human-readable name to a room slug.
///
/// Stored case-preserving but resolved case-insensitively.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct VisioAlias {
    /// Human-readable alias (e.g. "COMEX").
    pub name: String,
    /// Full canonical URL (e.g. "https://meet.example.com/abc-defg-hij").
    pub url: String,
}

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
    #[serde(default = "default_background_mode")]
    pub background_mode: String,
    #[serde(default = "default_audio_mode")]
    pub audio_mode: String,
    #[serde(default)]
    pub adaptive_mode_enabled: bool,
    #[serde(default, alias = "room_history")]
    pub visio_history: Vec<VisioHistoryEntry>,
    #[serde(default = "default_true")]
    pub noise_reduction_enabled: bool,
    /// Preferred audio input device name.
    #[serde(default)]
    pub audio_input_device: Option<String>,
    /// Preferred audio output device name.
    #[serde(default)]
    pub audio_output_device: Option<String>,
    /// Preferred camera device name.
    #[serde(default)]
    pub camera_device: Option<String>,
    /// Video resolution preset: "720p" (default), "360p", "180p".
    #[serde(default = "default_video_resolution")]
    pub video_resolution: String,
    /// iCal/CalDAV feed URL for calendar integration.
    #[serde(default)]
    pub calendar_url: Option<String>,
    /// How often to refresh the calendar feed.
    #[serde(default)]
    pub calendar_refresh_interval: CalendarRefreshInterval,
    /// Friendly URL aliases mapping display names to room slugs.
    #[serde(default)]
    pub visio_aliases: Vec<VisioAlias>,
}

fn default_video_resolution() -> String {
    "720p".to_string()
}

fn default_meet_instances() -> Vec<String> {
    vec![
        "visio.numerique.gouv.fr".to_string(),
        "meet.linagora.com".to_string(),
    ]
}

fn default_theme() -> String {
    "light".to_string()
}

fn default_background_mode() -> String {
    "off".to_string()
}

fn default_audio_mode() -> String {
    "computer".to_string()
}

fn default_true() -> bool {
    true
}

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
            background_mode: "off".to_string(),
            audio_mode: "computer".to_string(),
            adaptive_mode_enabled: false,
            visio_history: Vec::new(),
            noise_reduction_enabled: true,
            audio_input_device: None,
            audio_output_device: None,
            camera_device: None,
            video_resolution: "720p".to_string(),
            calendar_url: None,
            calendar_refresh_interval: CalendarRefreshInterval::Minutes15,
            visio_aliases: Vec::new(),
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
        self.settings
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    pub fn set_display_name(&self, name: Option<String>) {
        self.settings
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .display_name = name;
        self.save();
    }

    pub fn set_language(&self, lang: Option<String>) {
        self.settings
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .language = lang;
        self.save();
    }

    pub fn set_mic_enabled_on_join(&self, enabled: bool) {
        self.settings
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .mic_enabled_on_join = enabled;
        self.save();
    }

    pub fn set_camera_enabled_on_join(&self, enabled: bool) {
        self.settings
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .camera_enabled_on_join = enabled;
        self.save();
    }

    pub fn set_theme(&self, theme: String) {
        self.settings
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .theme = theme;
        self.save();
    }

    pub fn get_meet_instances(&self) -> Vec<String> {
        self.settings
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .meet_instances
            .clone()
    }

    pub fn set_meet_instances(&self, instances: Vec<String>) {
        self.settings
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .meet_instances = instances;
        self.save();
    }

    pub fn set_notification_participant_join(&self, enabled: bool) {
        self.settings
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .notification_participant_join = enabled;
        self.save();
    }

    pub fn set_notification_hand_raised(&self, enabled: bool) {
        self.settings
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .notification_hand_raised = enabled;
        self.save();
    }

    pub fn set_notification_message_received(&self, enabled: bool) {
        self.settings
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .notification_message_received = enabled;
        self.save();
    }

    pub fn get_background_mode(&self) -> String {
        self.settings
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .background_mode
            .clone()
    }

    pub fn set_background_mode(&self, mode: String) {
        self.settings
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .background_mode = mode;
        self.save();
    }

    pub fn get_audio_mode(&self) -> String {
        self.settings
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .audio_mode
            .clone()
    }

    pub fn set_audio_mode(&self, mode: String) {
        self.settings
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .audio_mode = mode;
        self.save();
    }

    pub fn is_adaptive_mode_enabled(&self) -> bool {
        self.settings
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .adaptive_mode_enabled
    }

    pub fn set_adaptive_mode_enabled(&self, enabled: bool) {
        self.settings
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .adaptive_mode_enabled = enabled;
        self.save();
    }

    pub fn add_visio_to_history(&self, url: String, display_name: Option<String>) {
        let canonical = strip_room_display_name_param(&url);
        let mut s = self.settings.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(pos) = s.visio_history.iter().position(|e| e.url == canonical) {
            let existing_name = s.visio_history[pos].display_name.take();
            let merged_name = display_name.or(existing_name);
            s.visio_history.remove(pos);
            s.visio_history.insert(
                0,
                VisioHistoryEntry {
                    url: canonical,
                    display_name: merged_name,
                },
            );
        } else {
            s.visio_history.insert(
                0,
                VisioHistoryEntry {
                    url: canonical,
                    display_name,
                },
            );
        }
        s.visio_history.truncate(10);
        drop(s);
        self.save();
    }

    pub fn get_visio_history(&self) -> Vec<VisioHistoryEntry> {
        self.settings
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .visio_history
            .clone()
    }

    pub fn is_noise_reduction_enabled(&self) -> bool {
        self.settings
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .noise_reduction_enabled
    }

    pub fn set_noise_reduction_enabled(&self, enabled: bool) {
        self.settings
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .noise_reduction_enabled = enabled;
        self.save();
    }

    pub fn get_audio_input_device(&self) -> Option<String> {
        self.settings
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .audio_input_device
            .clone()
    }

    pub fn set_audio_input_device(&self, device: Option<String>) {
        self.settings
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .audio_input_device = device;
        self.save();
    }

    pub fn get_audio_output_device(&self) -> Option<String> {
        self.settings
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .audio_output_device
            .clone()
    }

    pub fn set_audio_output_device(&self, device: Option<String>) {
        self.settings
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .audio_output_device = device;
        self.save();
    }

    pub fn get_camera_device(&self) -> Option<String> {
        self.settings
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .camera_device
            .clone()
    }

    pub fn set_camera_device(&self, device: Option<String>) {
        self.settings
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .camera_device = device;
        self.save();
    }

    pub fn get_video_resolution(&self) -> String {
        self.settings
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .video_resolution
            .clone()
    }

    /// Set video resolution preset. Valid values: "720p", "360p", "180p".
    pub fn set_video_resolution(&self, resolution: String) {
        self.settings
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .video_resolution = resolution;
        self.save();
    }

    pub fn get_calendar_url(&self) -> Option<String> {
        self.settings
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .calendar_url
            .clone()
    }

    pub fn set_calendar_url(&self, url: Option<String>) {
        self.settings
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .calendar_url = url;
        self.save();
    }

    pub fn get_calendar_refresh_interval(&self) -> CalendarRefreshInterval {
        self.settings
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .calendar_refresh_interval
            .clone()
    }

    pub fn set_calendar_refresh_interval(&self, interval: CalendarRefreshInterval) {
        self.settings
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .calendar_refresh_interval = interval;
        self.save();
    }

    pub fn clear_visio_history(&self) {
        self.settings
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .visio_history
            .clear();
        self.save();
    }

    /// Store or update a friendly alias mapping a display name to a canonical URL.
    ///
    /// If an alias with the same name (case-insensitive) already exists, its URL
    /// is updated to the new value.
    pub fn add_visio_alias(&self, name: String, url: String) {
        let canonical = strip_room_display_name_param(&url);
        let mut s = self.settings.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(existing) = s
            .visio_aliases
            .iter_mut()
            .find(|a| a.name.eq_ignore_ascii_case(&name))
        {
            existing.url = canonical;
            existing.name = name;
        } else {
            s.visio_aliases.push(VisioAlias {
                name,
                url: canonical,
            });
        }
        drop(s);
        self.save();
    }

    /// Resolve a friendly alias to its canonical URL.
    ///
    /// Matching is case-insensitive.
    pub fn resolve_visio_alias(&self, name: &str) -> Option<String> {
        self.settings
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .visio_aliases
            .iter()
            .find(|a| a.name.eq_ignore_ascii_case(name))
            .map(|a| a.url.clone())
    }

    /// Check whether an alias name already maps to a different URL.
    ///
    /// Returns `Some(existing_url)` when a conflict exists, `None` otherwise.
    pub fn check_visio_alias_conflict(&self, name: &str, url: &str) -> Option<String> {
        let canonical = strip_room_display_name_param(url);
        self.settings
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .visio_aliases
            .iter()
            .find(|a| a.name.eq_ignore_ascii_case(name) && a.url != canonical)
            .map(|a| a.url.clone())
    }

    /// Return all stored aliases.
    pub fn get_visio_aliases(&self) -> Vec<VisioAlias> {
        self.settings
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .visio_aliases
            .clone()
    }

    fn save(&self) {
        let settings = self
            .settings
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
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
    fn test_default_theme_is_light() {
        assert_eq!(Settings::default().theme, "light");
    }

    #[test]
    fn test_set_theme_persists() {
        let dir = temp_dir();
        let path = dir.path().to_str().unwrap();
        {
            let store = SettingsStore::new(path);
            assert_eq!(store.get().theme, "light");
            store.set_theme("dark".to_string());
        }
        let store = SettingsStore::new(path);
        assert_eq!(store.get().theme, "dark");
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
        assert!(s.mic_enabled_on_join);
        assert!(!s.camera_enabled_on_join);
    }

    #[test]
    fn test_default_meet_instances() {
        let s = Settings::default();
        assert_eq!(
            s.meet_instances,
            vec![
                "visio.numerique.gouv.fr".to_string(),
                "meet.linagora.com".to_string()
            ]
        );
    }

    #[test]
    fn test_set_meet_instances_persists() {
        let dir = temp_dir();
        let path = dir.path().to_str().unwrap();
        {
            let store = SettingsStore::new(path);
            store.set_meet_instances(vec![
                "meet.numerique.gouv.fr".to_string(),
                "meet.example.com".to_string(),
            ]);
        }
        let store = SettingsStore::new(path);
        assert_eq!(
            store.get().meet_instances,
            vec![
                "meet.numerique.gouv.fr".to_string(),
                "meet.example.com".to_string(),
            ]
        );
    }

    #[test]
    fn test_default_notification_settings() {
        let s = Settings::default();
        assert!(s.notification_participant_join);
        assert!(s.notification_hand_raised);
        assert!(s.notification_message_received);
    }

    #[test]
    fn test_set_notification_settings_persist() {
        let dir = temp_dir();
        let path = dir.path().to_str().unwrap();
        {
            let store = SettingsStore::new(path);
            store.set_notification_participant_join(false);
            store.set_notification_hand_raised(false);
            store.set_notification_message_received(false);
        }
        let store = SettingsStore::new(path);
        let s = store.get();
        assert!(!s.notification_participant_join);
        assert!(!s.notification_hand_raised);
        assert!(!s.notification_message_received);
    }

    #[test]
    fn test_background_mode_defaults_to_off() {
        let s = Settings::default();
        assert_eq!(s.background_mode, "off");
    }

    #[test]
    fn test_set_background_mode_persists() {
        let dir = temp_dir();
        let path = dir.path().to_str().unwrap();
        {
            let store = SettingsStore::new(path);
            store.set_background_mode("blur".to_string());
        }
        let store = SettingsStore::new(path);
        assert_eq!(store.get_background_mode(), "blur");
    }

    #[test]
    fn test_set_background_mode_image() {
        let dir = temp_dir();
        let path = dir.path().to_str().unwrap();
        {
            let store = SettingsStore::new(path);
            store.set_background_mode("image:3".to_string());
        }
        let store = SettingsStore::new(path);
        assert_eq!(store.get_background_mode(), "image:3");
    }

    #[test]
    fn test_adaptive_mode_enabled_default_false() {
        let s = Settings::default();
        assert!(!s.adaptive_mode_enabled);
    }

    #[test]
    fn test_set_adaptive_mode_enabled_persists() {
        let dir = temp_dir();
        let path = dir.path().to_str().unwrap();
        {
            let store = SettingsStore::new(path);
            assert!(!store.is_adaptive_mode_enabled());
            store.set_adaptive_mode_enabled(true);
        }
        let store = SettingsStore::new(path);
        assert!(store.is_adaptive_mode_enabled());
    }

    #[test]
    fn test_room_history() {
        let dir = temp_dir();
        let store = SettingsStore::new(dir.path().to_str().unwrap());

        store.add_visio_to_history("https://meet.example.com/room1".to_string(), None);
        store.add_visio_to_history("https://meet.example.com/room2".to_string(), None);
        store.add_visio_to_history("https://meet.example.com/room1".to_string(), None); // dedup, moves to front

        let history = store.get_visio_history();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].url, "https://meet.example.com/room1");
        assert_eq!(history[1].url, "https://meet.example.com/room2");

        store.clear_visio_history();
        assert!(store.get_visio_history().is_empty());
    }

    #[test]
    fn test_room_history_max_10() {
        let dir = temp_dir();
        let store = SettingsStore::new(dir.path().to_str().unwrap());

        for i in 0..15 {
            store.add_visio_to_history(format!("https://meet.example.com/room{i}"), None);
        }

        let history = store.get_visio_history();
        assert_eq!(history.len(), 10);
        assert_eq!(history[0].url, "https://meet.example.com/room14");
    }

    #[test]
    fn test_room_history_persists() {
        let dir = temp_dir();
        let path = dir.path().to_str().unwrap();
        {
            let store = SettingsStore::new(path);
            store.add_visio_to_history("https://meet.example.com/room1".to_string(), None);
        }
        let store = SettingsStore::new(path);
        let history = store.get_visio_history();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].url, "https://meet.example.com/room1");
    }

    #[test]
    fn noise_reduction_enabled_by_default() {
        let settings = Settings::default();
        assert!(settings.noise_reduction_enabled);
    }

    #[test]
    fn test_set_noise_reduction_persists() {
        let dir = temp_dir();
        let path = dir.path().to_str().unwrap();
        {
            let store = SettingsStore::new(path);
            assert!(store.is_noise_reduction_enabled());
            store.set_noise_reduction_enabled(false);
        }
        let store = SettingsStore::new(path);
        assert!(!store.is_noise_reduction_enabled());
    }

    #[test]
    fn test_device_selection_persists() {
        let dir = temp_dir();
        let path = dir.path().to_str().unwrap();
        {
            let store = SettingsStore::new(path);
            store.set_audio_input_device(Some("Blue Yeti".to_string()));
            store.set_audio_output_device(Some("Speakers".to_string()));
            store.set_camera_device(Some("FaceTime HD".to_string()));
        }
        let store = SettingsStore::new(path);
        assert_eq!(
            store.get_audio_input_device(),
            Some("Blue Yeti".to_string())
        );
        assert_eq!(
            store.get_audio_output_device(),
            Some("Speakers".to_string())
        );
        assert_eq!(store.get_camera_device(), Some("FaceTime HD".to_string()));
    }

    #[test]
    fn test_device_selection_defaults_to_none() {
        let s = Settings::default();
        assert!(s.audio_input_device.is_none());
        assert!(s.audio_output_device.is_none());
        assert!(s.camera_device.is_none());
    }

    #[test]
    fn test_partial_json_defaults_meet_instances() {
        let dir = temp_dir();
        let path = dir.path().to_str().unwrap();
        std::fs::write(
            dir.path().join("settings.json"),
            r#"{"display_name":"Eve"}"#,
        )
        .unwrap();
        let store = SettingsStore::new(path);
        assert_eq!(
            store.get().meet_instances,
            vec![
                "visio.numerique.gouv.fr".to_string(),
                "meet.linagora.com".to_string()
            ]
        );
    }

    #[test]
    fn test_audio_mode_defaults_to_computer() {
        let dir = temp_dir();
        let store = SettingsStore::new(dir.path().to_str().unwrap());
        let settings = store.get();
        assert_eq!(settings.audio_mode, "computer");
    }

    #[test]
    fn test_audio_mode_round_trips() {
        let dir = temp_dir();
        let path = dir.path().to_str().unwrap();
        {
            let store = SettingsStore::new(path);
            store.set_audio_mode("none".to_string());
        }
        // Re-read from disk
        let store2 = SettingsStore::new(path);
        assert_eq!(store2.get().audio_mode, "none");
    }

    #[test]
    fn test_calendar_url_default_none() {
        let s = Settings::default();
        assert_eq!(s.calendar_url, None);
    }

    #[test]
    fn test_calendar_refresh_interval_default() {
        let s = Settings::default();
        assert_eq!(
            s.calendar_refresh_interval,
            CalendarRefreshInterval::Minutes15
        );
    }

    #[test]
    fn test_set_calendar_url_persists() {
        let dir = temp_dir();
        let path = dir.path().to_str().unwrap();
        {
            let store = SettingsStore::new(path);
            store.set_calendar_url(Some("https://cal.example.com/feed.ics".to_string()));
        }
        let store = SettingsStore::new(path);
        assert_eq!(
            store.get_calendar_url(),
            Some("https://cal.example.com/feed.ics".to_string())
        );
    }

    #[test]
    fn test_clear_calendar_url() {
        let dir = temp_dir();
        let path = dir.path().to_str().unwrap();
        let store = SettingsStore::new(path);
        store.set_calendar_url(Some("https://cal.example.com/feed.ics".to_string()));
        store.set_calendar_url(None);
        assert_eq!(store.get_calendar_url(), None);
    }

    #[test]
    fn test_set_calendar_refresh_interval_persists() {
        let dir = temp_dir();
        let path = dir.path().to_str().unwrap();
        {
            let store = SettingsStore::new(path);
            store.set_calendar_refresh_interval(CalendarRefreshInterval::Hour1);
        }
        let store = SettingsStore::new(path);
        assert_eq!(
            store.get_calendar_refresh_interval(),
            CalendarRefreshInterval::Hour1
        );
    }

    #[test]
    fn test_partial_json_defaults_calendar_fields() {
        let dir = temp_dir();
        let path = dir.path().to_str().unwrap();
        std::fs::write(
            dir.path().join("settings.json"),
            r#"{"display_name":"Eve"}"#,
        )
        .unwrap();
        let store = SettingsStore::new(path);
        let s = store.get();
        assert_eq!(s.calendar_url, None);
        assert_eq!(
            s.calendar_refresh_interval,
            CalendarRefreshInterval::Minutes15
        );
    }

    // --- VisioHistoryEntry and migration tests ---

    #[test]
    fn room_history_entry_round_trip() {
        let entry = VisioHistoryEntry {
            url: "https://meet.example.com/room".to_string(),
            display_name: Some("My Room".to_string()),
        };
        let json = serde_json::to_string(&entry).unwrap();
        let decoded: VisioHistoryEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.url, "https://meet.example.com/room");
        assert_eq!(decoded.display_name, Some("My Room".to_string()));
    }

    #[test]
    fn room_history_entry_without_name() {
        let entry = VisioHistoryEntry {
            url: "https://meet.example.com/room".to_string(),
            display_name: None,
        };
        let json = serde_json::to_string(&entry).unwrap();
        let decoded: VisioHistoryEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.url, "https://meet.example.com/room");
        assert_eq!(decoded.display_name, None);
    }

    #[test]
    fn room_history_migrates_old_string_format() {
        let dir = temp_dir();
        let path = dir.path().to_str().unwrap();
        fs::write(
            dir.path().join("settings.json"),
            r#"{"room_history": ["https://meet.example.com/room1", "https://meet.example.com/room2"]}"#,
        )
        .unwrap();
        let store = SettingsStore::new(path);
        let history = store.get_visio_history();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].url, "https://meet.example.com/room1");
        assert_eq!(history[0].display_name, None);
        assert_eq!(history[1].url, "https://meet.example.com/room2");
        assert_eq!(history[1].display_name, None);
    }

    #[test]
    fn room_history_new_format() {
        let dir = temp_dir();
        let path = dir.path().to_str().unwrap();
        fs::write(
            dir.path().join("settings.json"),
            r#"{"room_history": [{"url": "https://meet.example.com/room", "display_name": "My Room"}]}"#,
        )
        .unwrap();
        let store = SettingsStore::new(path);
        let history = store.get_visio_history();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].url, "https://meet.example.com/room");
        assert_eq!(history[0].display_name, Some("My Room".to_string()));
    }

    #[test]
    fn room_history_mixed_format() {
        let dir = temp_dir();
        let path = dir.path().to_str().unwrap();
        fs::write(
            dir.path().join("settings.json"),
            r#"{"room_history": ["https://meet.example.com/room1", {"url": "https://meet.example.com/room2", "display_name": "Room Two"}]}"#,
        )
        .unwrap();
        let store = SettingsStore::new(path);
        let history = store.get_visio_history();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].url, "https://meet.example.com/room1");
        assert_eq!(history[0].display_name, None);
        assert_eq!(history[1].url, "https://meet.example.com/room2");
        assert_eq!(history[1].display_name, Some("Room Two".to_string()));
    }

    #[test]
    fn add_room_updates_display_name() {
        let dir = temp_dir();
        let store = SettingsStore::new(dir.path().to_str().unwrap());
        store.add_visio_to_history(
            "https://meet.example.com/room".to_string(),
            Some("Old Name".to_string()),
        );
        store.add_visio_to_history(
            "https://meet.example.com/room".to_string(),
            Some("New Name".to_string()),
        );
        let history = store.get_visio_history();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].url, "https://meet.example.com/room");
        assert_eq!(history[0].display_name, Some("New Name".to_string()));
    }

    #[test]
    fn add_room_without_name_preserves_existing() {
        let dir = temp_dir();
        let store = SettingsStore::new(dir.path().to_str().unwrap());
        store.add_visio_to_history(
            "https://meet.example.com/room".to_string(),
            Some("Keep Me".to_string()),
        );
        store.add_visio_to_history("https://meet.example.com/room".to_string(), None);
        let history = store.get_visio_history();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].url, "https://meet.example.com/room");
        assert_eq!(history[0].display_name, Some("Keep Me".to_string()));
    }

    #[test]
    fn add_room_caps_at_10() {
        let dir = temp_dir();
        let store = SettingsStore::new(dir.path().to_str().unwrap());
        for i in 0..12 {
            store.add_visio_to_history(
                format!("https://meet.example.com/room{i}"),
                Some(format!("Room {i}")),
            );
        }
        let history = store.get_visio_history();
        assert_eq!(history.len(), 10);
        assert_eq!(history[0].url, "https://meet.example.com/room11");
        assert_eq!(history[0].display_name, Some("Room 11".to_string()));
    }

    // --- VisioAlias tests ---

    #[test]
    fn alias_add_and_resolve() {
        let dir = temp_dir();
        let store = SettingsStore::new(dir.path().to_str().unwrap());
        store.add_visio_alias(
            "COMEX".to_string(),
            "https://meet.example.com/abc-defg-hij".to_string(),
        );
        assert_eq!(
            store.resolve_visio_alias("COMEX"),
            Some("https://meet.example.com/abc-defg-hij".to_string())
        );
    }

    #[test]
    fn alias_case_insensitive_resolve() {
        let dir = temp_dir();
        let store = SettingsStore::new(dir.path().to_str().unwrap());
        store.add_visio_alias(
            "COMEX".to_string(),
            "https://meet.example.com/abc-defg-hij".to_string(),
        );
        assert_eq!(
            store.resolve_visio_alias("comex"),
            Some("https://meet.example.com/abc-defg-hij".to_string())
        );
        assert_eq!(
            store.resolve_visio_alias("Comex"),
            Some("https://meet.example.com/abc-defg-hij".to_string())
        );
    }

    #[test]
    fn alias_update_existing() {
        let dir = temp_dir();
        let store = SettingsStore::new(dir.path().to_str().unwrap());
        store.add_visio_alias(
            "COMEX".to_string(),
            "https://meet.example.com/old-slug-xxx".to_string(),
        );
        store.add_visio_alias(
            "comex".to_string(),
            "https://meet.example.com/new-slug-yyy".to_string(),
        );
        let aliases = store.get_visio_aliases();
        assert_eq!(aliases.len(), 1);
        assert_eq!(aliases[0].name, "comex");
        assert_eq!(aliases[0].url, "https://meet.example.com/new-slug-yyy");
    }

    #[test]
    fn alias_conflict_detection() {
        let dir = temp_dir();
        let store = SettingsStore::new(dir.path().to_str().unwrap());
        store.add_visio_alias(
            "COMEX".to_string(),
            "https://meet.example.com/abc-defg-hij".to_string(),
        );
        // Same name, same URL => no conflict
        assert_eq!(
            store.check_visio_alias_conflict("COMEX", "https://meet.example.com/abc-defg-hij"),
            None
        );
        // Same name, different URL => conflict
        assert_eq!(
            store.check_visio_alias_conflict("comex", "https://meet.example.com/xyz-abcd-efg"),
            Some("https://meet.example.com/abc-defg-hij".to_string())
        );
    }

    #[test]
    fn alias_persists_across_reload() {
        let dir = temp_dir();
        let path = dir.path().to_str().unwrap();
        {
            let store = SettingsStore::new(path);
            store.add_visio_alias(
                "Weekly".to_string(),
                "https://meet.example.com/abc-defg-hij".to_string(),
            );
        }
        let store = SettingsStore::new(path);
        assert_eq!(
            store.resolve_visio_alias("weekly"),
            Some("https://meet.example.com/abc-defg-hij".to_string())
        );
    }
}
