use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub enum CalendarRefreshInterval {
    Minutes5,
    #[default]
    Minutes15,
    Hour1,
    Hours4,
    Manual,
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
    #[serde(default)]
    pub room_history: Vec<String>,
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
}

fn default_video_resolution() -> String {
    "720p".to_string()
}

fn default_meet_instances() -> Vec<String> {
    vec![
        "meet.linagora.com".to_string(),
        "meet.numerique.gouv.fr".to_string(),
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
            room_history: Vec::new(),
            noise_reduction_enabled: true,
            audio_input_device: None,
            audio_output_device: None,
            camera_device: None,
            video_resolution: "720p".to_string(),
            calendar_url: None,
            calendar_refresh_interval: CalendarRefreshInterval::Minutes15,
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

    pub fn add_room_to_history(&self, url: String) {
        let mut s = self.settings.lock().unwrap_or_else(|e| e.into_inner());
        s.room_history.retain(|u| u != &url);
        s.room_history.insert(0, url);
        s.room_history.truncate(10);
        drop(s);
        self.save();
    }

    pub fn get_room_history(&self) -> Vec<String> {
        self.settings
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .room_history
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

    pub fn clear_room_history(&self) {
        self.settings
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .room_history
            .clear();
        self.save();
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
                "meet.linagora.com".to_string(),
                "meet.numerique.gouv.fr".to_string()
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

        store.add_room_to_history("https://meet.example.com/room1".to_string());
        store.add_room_to_history("https://meet.example.com/room2".to_string());
        store.add_room_to_history("https://meet.example.com/room1".to_string()); // dedup, moves to front

        let history = store.get_room_history();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0], "https://meet.example.com/room1");
        assert_eq!(history[1], "https://meet.example.com/room2");

        store.clear_room_history();
        assert!(store.get_room_history().is_empty());
    }

    #[test]
    fn test_room_history_max_10() {
        let dir = temp_dir();
        let store = SettingsStore::new(dir.path().to_str().unwrap());

        for i in 0..15 {
            store.add_room_to_history(format!("https://meet.example.com/room{i}"));
        }

        let history = store.get_room_history();
        assert_eq!(history.len(), 10);
        assert_eq!(history[0], "https://meet.example.com/room14");
    }

    #[test]
    fn test_room_history_persists() {
        let dir = temp_dir();
        let path = dir.path().to_str().unwrap();
        {
            let store = SettingsStore::new(path);
            store.add_room_to_history("https://meet.example.com/room1".to_string());
        }
        let store = SettingsStore::new(path);
        let history = store.get_room_history();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0], "https://meet.example.com/room1");
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
                "meet.linagora.com".to_string(),
                "meet.numerique.gouv.fr".to_string()
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
}
