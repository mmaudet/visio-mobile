use crate::events::ParticipantInfo;

#[cfg(test)]
use crate::events::ConnectionQuality;

/// Manages the list of participants in a room.
///
/// Updated by the room event loop. Read by native UI layers.
#[derive(Debug, Clone)]
pub struct ParticipantManager {
    participants: Vec<ParticipantInfo>,
    active_speakers: Vec<String>,
    local_sid: Option<String>,
}

impl Default for ParticipantManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ParticipantManager {
    pub fn new() -> Self {
        Self {
            participants: Vec::new(),
            active_speakers: Vec::new(),
            local_sid: None,
        }
    }

    pub fn set_local_sid(&mut self, sid: String) {
        self.local_sid = Some(sid);
    }

    pub fn local_sid(&self) -> Option<&str> {
        self.local_sid.as_deref()
    }

    pub fn add_participant(&mut self, info: ParticipantInfo) {
        if !self.participants.iter().any(|p| p.sid == info.sid) {
            self.participants.push(info);
        }
    }

    pub fn remove_participant(&mut self, sid: &str) {
        self.participants.retain(|p| p.sid != sid);
        self.active_speakers.retain(|s| s != sid);
    }

    pub fn participants(&self) -> &[ParticipantInfo] {
        &self.participants
    }

    /// Get participants sorted: hand raised first, then alphabetical (case-insensitive).
    pub fn sorted_participants(&self) -> Vec<ParticipantInfo> {
        let mut sorted = self.participants.clone();
        sorted.sort_by(|a, b| {
            let a_name = a.name.as_deref().unwrap_or("").to_lowercase();
            let b_name = b.name.as_deref().unwrap_or("").to_lowercase();
            a_name.cmp(&b_name)
        });
        sorted
    }

    pub fn participant(&self, sid: &str) -> Option<&ParticipantInfo> {
        self.participants.iter().find(|p| p.sid == sid)
    }

    pub fn participant_mut(&mut self, sid: &str) -> Option<&mut ParticipantInfo> {
        self.participants.iter_mut().find(|p| p.sid == sid)
    }

    pub fn set_active_speakers(&mut self, sids: Vec<String>) {
        self.active_speakers = sids;
    }

    pub fn active_speakers(&self) -> &[String] {
        &self.active_speakers
    }

    pub fn participant_count(&self) -> usize {
        self.participants.len()
    }

    /// Compare current participant list with the room's actual state.
    /// Returns (joined, left) for participants that are out of sync.
    pub fn resync(
        &mut self,
        room_participants: Vec<ParticipantInfo>,
    ) -> (Vec<ParticipantInfo>, Vec<String>) {
        let current_sids: std::collections::HashSet<_> =
            self.participants.iter().map(|p| p.sid.clone()).collect();
        let room_sids: std::collections::HashSet<_> =
            room_participants.iter().map(|p| p.sid.clone()).collect();

        let joined: Vec<ParticipantInfo> = room_participants
            .into_iter()
            .filter(|p| !current_sids.contains(&p.sid))
            .collect();

        let left: Vec<String> = current_sids
            .into_iter()
            .filter(|sid| !room_sids.contains(sid))
            .collect();

        for p in &joined {
            self.add_participant(p.clone());
        }
        for sid in &left {
            self.remove_participant(sid);
        }

        (joined, left)
    }

    pub fn clear(&mut self) {
        self.participants.clear();
        self.active_speakers.clear();
        self.local_sid = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_participant(sid: &str, name: &str) -> ParticipantInfo {
        ParticipantInfo {
            sid: sid.to_string(),
            identity: format!("identity-{sid}"),
            name: Some(name.to_string()),
            is_muted: false,
            has_video: false,
            video_track_sid: None,
            has_screen_share: false,
            screen_share_track_sid: None,
            connection_quality: ConnectionQuality::Good,
            color: None,
            is_admin: false,
        }
    }

    #[test]
    fn add_and_retrieve_participant() {
        let mut mgr = ParticipantManager::new();
        mgr.add_participant(make_participant("p1", "Alice"));
        assert_eq!(mgr.participant_count(), 1);
        assert_eq!(
            mgr.participant("p1").unwrap().name.as_deref(),
            Some("Alice")
        );
    }

    #[test]
    fn no_duplicate_participants() {
        let mut mgr = ParticipantManager::new();
        mgr.add_participant(make_participant("p1", "Alice"));
        mgr.add_participant(make_participant("p1", "Alice"));
        assert_eq!(mgr.participant_count(), 1);
    }

    #[test]
    fn remove_participant() {
        let mut mgr = ParticipantManager::new();
        mgr.add_participant(make_participant("p1", "Alice"));
        mgr.add_participant(make_participant("p2", "Bob"));
        mgr.remove_participant("p1");
        assert_eq!(mgr.participant_count(), 1);
        assert!(mgr.participant("p1").is_none());
        assert!(mgr.participant("p2").is_some());
    }

    #[test]
    fn active_speakers() {
        let mut mgr = ParticipantManager::new();
        mgr.add_participant(make_participant("p1", "Alice"));
        mgr.set_active_speakers(vec!["p1".to_string()]);
        assert_eq!(mgr.active_speakers(), &["p1"]);
    }

    #[test]
    fn clear_resets_everything() {
        let mut mgr = ParticipantManager::new();
        mgr.set_local_sid("local".to_string());
        mgr.add_participant(make_participant("p1", "Alice"));
        mgr.set_active_speakers(vec!["p1".to_string()]);
        mgr.clear();
        assert_eq!(mgr.participant_count(), 0);
        assert!(mgr.active_speakers().is_empty());
        assert!(mgr.local_sid().is_none());
    }

    #[test]
    fn track_muted_camera_clears_video() {
        let mut mgr = ParticipantManager::new();
        let mut p = make_participant("p1", "Alice");
        p.has_video = true;
        p.video_track_sid = Some("TR_CAM_1".to_string());
        mgr.add_participant(p);

        // Simulate TrackMuted for camera
        if let Some(p) = mgr.participant_mut("p1") {
            p.has_video = false;
            p.video_track_sid = None;
        }

        let p = mgr.participant("p1").unwrap();
        assert!(!p.has_video);
        assert!(p.video_track_sid.is_none());
    }

    #[test]
    fn track_unmuted_camera_restores_video() {
        let mut mgr = ParticipantManager::new();
        let p = make_participant("p1", "Alice");
        mgr.add_participant(p);

        // Simulate TrackUnmuted for camera
        if let Some(p) = mgr.participant_mut("p1") {
            p.has_video = true;
            p.video_track_sid = Some("TR_CAM_1".to_string());
        }

        let p = mgr.participant("p1").unwrap();
        assert!(p.has_video);
        assert_eq!(p.video_track_sid.as_deref(), Some("TR_CAM_1"));
    }

    #[test]
    fn mute_mic_preserves_video() {
        let mut mgr = ParticipantManager::new();
        let mut p = make_participant("p1", "Alice");
        p.has_video = true;
        p.video_track_sid = Some("TR_CAM_1".to_string());
        mgr.add_participant(p);

        // Simulate TrackMuted for microphone (only changes is_muted)
        if let Some(p) = mgr.participant_mut("p1") {
            p.is_muted = true;
        }

        let p = mgr.participant("p1").unwrap();
        assert!(p.is_muted);
        assert!(p.has_video, "muting mic should not clear video");
        assert_eq!(p.video_track_sid.as_deref(), Some("TR_CAM_1"));
    }

    #[test]
    fn track_subscribed_screen_share_sets_fields() {
        let mut mgr = ParticipantManager::new();
        mgr.add_participant(make_participant("p1", "Alice"));

        if let Some(p) = mgr.participant_mut("p1") {
            p.has_screen_share = true;
            p.screen_share_track_sid = Some("TR_SCREEN_1".to_string());
        }

        let p = mgr.participant("p1").unwrap();
        assert!(p.has_screen_share);
        assert_eq!(p.screen_share_track_sid.as_deref(), Some("TR_SCREEN_1"));
    }

    #[test]
    fn track_muted_screen_share_clears_fields() {
        let mut mgr = ParticipantManager::new();
        let mut p = make_participant("p1", "Alice");
        p.has_screen_share = true;
        p.screen_share_track_sid = Some("TR_SCREEN_1".to_string());
        p.has_video = true;
        p.video_track_sid = Some("TR_CAM_1".to_string());
        mgr.add_participant(p);

        if let Some(p) = mgr.participant_mut("p1") {
            p.has_screen_share = false;
            p.screen_share_track_sid = None;
        }

        let p = mgr.participant("p1").unwrap();
        assert!(!p.has_screen_share);
        assert!(p.screen_share_track_sid.is_none());
        assert!(p.has_video);
        assert_eq!(p.video_track_sid.as_deref(), Some("TR_CAM_1"));
    }

    #[test]
    fn sorted_participants_alphabetical() {
        let mut mgr = ParticipantManager::new();
        mgr.add_participant(make_participant("p3", "Charlie"));
        mgr.add_participant(make_participant("p1", "Alice"));
        mgr.add_participant(make_participant("p2", "Bob"));

        let sorted = mgr.sorted_participants();
        let names: Vec<_> = sorted.iter().map(|p| p.name.as_deref().unwrap()).collect();
        assert_eq!(names, vec!["Alice", "Bob", "Charlie"]);
    }

    #[test]
    fn sorted_participants_case_insensitive() {
        let mut mgr = ParticipantManager::new();
        mgr.add_participant(make_participant("p1", "bob"));
        mgr.add_participant(make_participant("p2", "Alice"));

        let sorted = mgr.sorted_participants();
        let names: Vec<_> = sorted.iter().map(|p| p.name.as_deref().unwrap()).collect();
        assert_eq!(names, vec!["Alice", "bob"]);
    }

    #[test]
    fn resync_detects_joined_and_left() {
        let mut pm = ParticipantManager::new();
        pm.add_participant(make_participant("sid1", "Alice"));
        pm.add_participant(make_participant("sid2", "Bob"));

        // Room now has sid2 + sid3 (sid1 left, sid3 joined)
        let p2_again = make_participant("sid2", "Bob");
        let p3 = make_participant("sid3", "Charlie");

        let (joined, left) = pm.resync(vec![p2_again, p3]);
        assert_eq!(joined.len(), 1);
        assert_eq!(joined[0].sid, "sid3");
        assert_eq!(left, vec!["sid1".to_string()]);
        assert_eq!(pm.participants().len(), 2);
        assert!(pm.participant("sid2").is_some());
        assert!(pm.participant("sid3").is_some());
        assert!(pm.participant("sid1").is_none());
    }

    #[test]
    fn track_unmuted_screen_share_restores_fields() {
        let mut mgr = ParticipantManager::new();
        mgr.add_participant(make_participant("p1", "Alice"));

        if let Some(p) = mgr.participant_mut("p1") {
            p.has_screen_share = true;
            p.screen_share_track_sid = Some("TR_SCREEN_1".to_string());
        }

        let p = mgr.participant("p1").unwrap();
        assert!(p.has_screen_share);
        assert_eq!(p.screen_share_track_sid.as_deref(), Some("TR_SCREEN_1"));
    }
}
