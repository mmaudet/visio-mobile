use std::sync::Arc;

/// Events emitted by the core to native UI listeners.
#[derive(Debug, Clone)]
pub enum VisioEvent {
    ConnectionStateChanged(ConnectionState),
    ParticipantJoined(ParticipantInfo),
    ParticipantLeft(String), // participant SID
    TrackSubscribed(TrackInfo),
    TrackUnsubscribed(String), // track SID
    TrackMuted { participant_sid: String, source: TrackSource },
    TrackUnmuted { participant_sid: String, source: TrackSource },
    ActiveSpeakersChanged(Vec<String>), // participant SIDs
    ConnectionQualityChanged { participant_sid: String, quality: ConnectionQuality },
    ChatMessageReceived(ChatMessage),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting { attempt: u32 },
}

#[derive(Debug, Clone)]
pub struct ParticipantInfo {
    pub sid: String,
    pub identity: String,
    pub name: Option<String>,
    pub is_muted: bool,
    pub has_video: bool,
    pub video_track_sid: Option<String>,
    pub connection_quality: ConnectionQuality,
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
    pub kind: TrackKind,
    pub source: TrackSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrackKind {
    Audio,
    Video,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

impl EventEmitter {
    pub fn new() -> Self {
        Self {
            listeners: Arc::new(std::sync::RwLock::new(Vec::new())),
        }
    }

    pub fn add_listener(&self, listener: Arc<dyn VisioEventListener>) {
        self.listeners.write().unwrap().push(listener);
    }

    pub fn emit(&self, event: VisioEvent) {
        let listeners = self.listeners.read().unwrap();
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
        let listener = Arc::new(CountingListener { count: count.clone() });

        emitter.add_listener(listener);
        emitter.emit(VisioEvent::ConnectionStateChanged(ConnectionState::Connected));

        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn emitter_dispatches_to_multiple_listeners() {
        let emitter = EventEmitter::new();
        let count1 = Arc::new(AtomicUsize::new(0));
        let count2 = Arc::new(AtomicUsize::new(0));

        emitter.add_listener(Arc::new(CountingListener { count: count1.clone() }));
        emitter.add_listener(Arc::new(CountingListener { count: count2.clone() }));

        emitter.emit(VisioEvent::ConnectionStateChanged(ConnectionState::Connected));

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
        let listener = Arc::new(EventCapture { events: events.clone() });

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
