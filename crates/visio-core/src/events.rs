use std::sync::Arc;

/// A calendar meeting parsed from an iCal feed.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Meeting {
    pub id: String,
    pub summary: String,
    pub start_time: i64,
    pub end_time: i64,
    pub room_url: String,
    pub deep_link: String,
    pub server_name: String,
}

/// Events emitted by the core to native UI listeners.
#[derive(Debug, Clone)]
pub enum VisioEvent {
    ConnectionStateChanged(ConnectionState),
    ParticipantJoined(ParticipantInfo),
    ParticipantLeft(String), // participant SID
    TrackSubscribed(TrackInfo),
    TrackUnsubscribed(String), // track SID
    TrackMuted {
        participant_sid: String,
        source: TrackSource,
    },
    TrackUnmuted {
        participant_sid: String,
        source: TrackSource,
    },
    ActiveSpeakersChanged(Vec<String>), // participant SIDs
    ConnectionQualityChanged {
        participant_sid: String,
        quality: ConnectionQuality,
    },
    ChatMessageReceived(ChatMessage),
    HandRaisedChanged {
        participant_sid: String,
        raised: bool,
        position: u32,
    },
    UnreadCountChanged(u32),
    /// A participant is waiting in the lobby (host notification).
    LobbyParticipantJoined {
        id: String,
        username: String,
    },
    /// A waiting participant left the lobby.
    LobbyParticipantLeft {
        id: String,
    },
    /// Entry was denied by the host (participant notification).
    LobbyDenied,
    /// Lobby wait timed out (10 minutes without host approval).
    LobbyTimeout,
    /// A participant sent an animated reaction (emoji).
    ReactionReceived {
        participant_sid: String,
        participant_name: String,
        emoji: String,
    },
    /// The adaptive context mode changed (e.g. Office → Pedestrian).
    AdaptiveModeChanged {
        mode: crate::adaptive::AdaptiveMode,
    },
    /// Bandwidth mode changed due to network quality.
    BandwidthModeChanged {
        mode: crate::bandwidth::BandwidthMode,
    },
    /// Disconnected because another device connected with the same identity.
    DisconnectedDuplicateIdentity,
    /// Disconnected because the participant was removed by an admin.
    DisconnectedByAdmin,
    /// Connection lost unexpectedly — native UI should call reconnect().
    ConnectionLost,
    /// Local participant is alone in the room — UI may show countdown or auto-disconnect.
    /// `remaining_secs` counts down from 120 (2 minutes). 0 means time's up.
    AloneInRoom {
        remaining_secs: u32,
    },
    /// Alone-in-room countdown was cancelled (another participant joined).
    AloneInRoomCancelled,
    /// Admin requested muting all participants — native UI should mute the local mic.
    MuteRequested,
    /// Calendar feed was refreshed — full updated list of upcoming meetings.
    MeetingsUpdated(Vec<Meeting>),
    /// A meeting starts in ≤ 15 minutes (fired once per meeting).
    MeetingImminent(Meeting),
    /// A meeting starts in ≤ 5 minutes (fired once per meeting).
    MeetingStartingSoon(Meeting),
    /// A meeting's start time has been reached.
    MeetingStarted(Meeting),
    /// Calendar feed could not be fetched or parsed.
    CalendarError(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting { attempt: u32 },
    WaitingForHost,
}

#[derive(Debug, Clone)]
pub struct ParticipantInfo {
    pub sid: String,
    pub identity: String,
    pub name: Option<String>,
    pub is_muted: bool,
    pub has_video: bool,
    pub video_track_sid: Option<String>,
    pub has_screen_share: bool,
    pub screen_share_track_sid: Option<String>,
    pub connection_quality: ConnectionQuality,
    /// HSL color string from Meet server (participant attribute "color").
    pub color: Option<String>,
    /// Whether the participant is a room admin (attribute "room_admin").
    pub is_admin: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionQuality {
    Excellent,
    Good,
    Poor,
    Lost,
}

#[derive(Debug, Clone)]
pub struct TrackInfo {
    pub sid: String,
    pub participant_sid: String,
    pub participant_identity: String,
    pub kind: TrackKind,
    pub source: TrackSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackKind {
    Audio,
    Video,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackSource {
    Microphone,
    Camera,
    ScreenShare,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub id: String,
    pub sender_sid: String,
    pub sender_name: String,
    pub text: String,
    pub timestamp_ms: u64,
}

/// Trait for receiving events from the core.
/// Implementations must be Send + Sync (called from tokio tasks).
pub trait VisioEventListener: Send + Sync {
    fn on_event(&self, event: VisioEvent);
}

/// Internal event emitter that dispatches to registered listeners.
#[derive(Clone)]
pub struct EventEmitter {
    listeners: Arc<std::sync::RwLock<Vec<Arc<dyn VisioEventListener>>>>,
}

impl Default for EventEmitter {
    fn default() -> Self {
        Self::new()
    }
}

impl EventEmitter {
    pub fn new() -> Self {
        Self {
            listeners: Arc::new(std::sync::RwLock::new(Vec::new())),
        }
    }

    pub fn add_listener(&self, listener: Arc<dyn VisioEventListener>) {
        let mut guard = self
            .listeners
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        guard.push(listener);
    }

    pub fn emit(&self, event: VisioEvent) {
        let listeners = self
            .listeners
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        for listener in listeners.iter() {
            listener.on_event(event.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct CountingListener {
        count: Arc<AtomicUsize>,
    }

    impl VisioEventListener for CountingListener {
        fn on_event(&self, _event: VisioEvent) {
            self.count.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn emitter_dispatches_to_listener() {
        let emitter = EventEmitter::new();
        let count = Arc::new(AtomicUsize::new(0));
        let listener = Arc::new(CountingListener {
            count: count.clone(),
        });

        emitter.add_listener(listener);
        emitter.emit(VisioEvent::ConnectionStateChanged(
            ConnectionState::Connected,
        ));

        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn emitter_dispatches_to_multiple_listeners() {
        let emitter = EventEmitter::new();
        let count1 = Arc::new(AtomicUsize::new(0));
        let count2 = Arc::new(AtomicUsize::new(0));

        emitter.add_listener(Arc::new(CountingListener {
            count: count1.clone(),
        }));
        emitter.add_listener(Arc::new(CountingListener {
            count: count2.clone(),
        }));

        emitter.emit(VisioEvent::ConnectionStateChanged(
            ConnectionState::Connected,
        ));

        assert_eq!(count1.load(Ordering::SeqCst), 1);
        assert_eq!(count2.load(Ordering::SeqCst), 1);
    }

    struct EventCapture {
        events: Arc<std::sync::Mutex<Vec<VisioEvent>>>,
    }

    impl VisioEventListener for EventCapture {
        fn on_event(&self, event: VisioEvent) {
            self.events.lock().unwrap().push(event);
        }
    }

    #[test]
    fn emitter_delivers_correct_events() {
        let emitter = EventEmitter::new();
        let events = Arc::new(std::sync::Mutex::new(Vec::new()));
        let listener = Arc::new(EventCapture {
            events: events.clone(),
        });

        emitter.add_listener(listener);
        emitter.emit(VisioEvent::ParticipantLeft("p1".to_string()));

        let captured = events.lock().unwrap();
        assert_eq!(captured.len(), 1);
        match &captured[0] {
            VisioEvent::ParticipantLeft(sid) => assert_eq!(sid, "p1"),
            _ => panic!("expected ParticipantLeft"),
        }
    }
}
