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
            connection_quality: ConnectionQuality::Good,
        }
    }

    #[test]
    fn add_and_retrieve_participant() {
        let mut mgr = ParticipantManager::new();
        mgr.add_participant(make_participant("p1", "Alice"));
        assert_eq!(mgr.participant_count(), 1);
        assert_eq!(mgr.participant("p1").unwrap().name.as_deref(), Some("Alice"));
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
}
