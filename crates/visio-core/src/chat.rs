use livekit::data_stream::StreamTextOptions;
use livekit::prelude::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use tokio::sync::Mutex;

use crate::errors::VisioError;
use crate::events::{ChatMessage, EventEmitter, VisioEvent};

/// Shared message store between RoomManager event loop and ChatService.
pub type MessageStore = Arc<Mutex<Vec<ChatMessage>>>;

/// The topic used by LiveKit Meet / LaSuite Meet for chat messages.
const CHAT_TOPIC: &str = "lk.chat";

/// Maximum chat message length (matches Meet web client).
const MAX_MESSAGE_LENGTH: usize = 2000;

/// Manages chat messaging via LiveKit data channels.
pub struct ChatService {
    room: Arc<Mutex<Option<Arc<Room>>>>,
    emitter: EventEmitter,
    messages: MessageStore,
    unread_count: Arc<AtomicU32>,
    chat_open: Arc<AtomicBool>,
}

impl ChatService {
    pub fn new(
        room: Arc<Mutex<Option<Arc<Room>>>>,
        emitter: EventEmitter,
        messages: MessageStore,
    ) -> Self {
        Self {
            room,
            emitter,
            messages,
            unread_count: Arc::new(AtomicU32::new(0)),
            chat_open: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Send a chat message to all participants using the Stream API (lk.chat topic).
    /// Messages are limited to 2000 characters (matching Meet web client).
    pub async fn send_message(&self, text: &str) -> Result<ChatMessage, VisioError> {
        let text = Self::validate_message(text)?;

        let room = self.room.lock().await;
        let room = room
            .as_ref()
            .ok_or_else(|| VisioError::Room("not connected".into()))?;

        let local = room.local_participant();

        let options = StreamTextOptions {
            topic: CHAT_TOPIC.to_string(),
            ..Default::default()
        };

        let info = local
            .send_text(&text, options)
            .await
            .map_err(|e| VisioError::Room(format!("send chat: {e}")))?;

        let msg = ChatMessage {
            id: info.id,
            sender_sid: local.sid().to_string(),
            sender_name: local.name().to_string(),
            text,
            timestamp_ms: info.timestamp.timestamp_millis() as u64,
        };

        self.messages.lock().await.push(msg.clone());
        self.emitter
            .emit(VisioEvent::ChatMessageReceived(msg.clone()));

        Ok(msg)
    }

    /// Get all messages in the current session.
    pub async fn messages(&self) -> Vec<ChatMessage> {
        self.messages.lock().await.clone()
    }

    /// Handle an incoming chat message from the event loop.
    pub async fn handle_incoming(&self, msg: ChatMessage) {
        self.messages.lock().await.push(msg.clone());
        self.emitter.emit(VisioEvent::ChatMessageReceived(msg));

        if !self.chat_open.load(Ordering::Relaxed) {
            let count = self.unread_count.fetch_add(1, Ordering::Relaxed) + 1;
            self.emitter.emit(VisioEvent::UnreadCountChanged(count));
        }
    }

    /// Clear all messages (on disconnect).
    pub async fn clear(&self) {
        self.messages.lock().await.clear();
        self.unread_count.store(0, Ordering::Relaxed);
    }

    /// Mark the chat panel as open or closed.
    /// When opened, resets the unread count to zero.
    pub fn set_chat_open(&self, open: bool) {
        self.chat_open.store(open, Ordering::Relaxed);
        if open {
            self.unread_count.store(0, Ordering::Relaxed);
            self.emitter.emit(VisioEvent::UnreadCountChanged(0));
        }
    }

    /// Get the current unread message count.
    pub fn unread_count(&self) -> u32 {
        self.unread_count.load(Ordering::Relaxed)
    }

    /// Validate message text before sending. Returns trimmed text or error.
    pub fn validate_message(text: &str) -> Result<String, VisioError> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Err(VisioError::Room("message is empty".into()));
        }
        if trimmed.len() > MAX_MESSAGE_LENGTH {
            return Err(VisioError::Room(format!(
                "message too long ({} chars, max {MAX_MESSAGE_LENGTH})",
                trimmed.len()
            )));
        }

        // Basic XSS sanitization: strip HTML tags and script-like patterns
        Ok(Self::sanitize_xss(trimmed))
    }

    /// Strip potentially dangerous script patterns from message text.
    /// Only removes dangerous patterns (javascript:, vbscript:, event handlers),
    /// preserves safe content like URLs in angle brackets.
    fn sanitize_xss(text: &str) -> String {
        use regex::Regex;
        use std::sync::OnceLock;

        static SCRIPT_RE: OnceLock<Regex> = OnceLock::new();

        let script_re = SCRIPT_RE.get_or_init(|| {
            Regex::new(
                r"(?i)(^|\s)(javascript|vbscript):|data:text/html|<script[^>]*>.*?</script>|<script[^>]*/?\s*>|on\w+\s*=|&#\d+;|&#x[0-9a-f]+;",
            )
            .expect("invalid regex pattern for XSS filtering")
        });

        script_re.replace_all(text, "").to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_message_rejected() {
        assert!(ChatService::validate_message("").is_err());
        assert!(ChatService::validate_message("   ").is_err());
    }

    #[test]
    fn long_message_rejected() {
        let long = "a".repeat(2001);
        assert!(ChatService::validate_message(&long).is_err());
    }

    #[test]
    fn valid_message_accepted() {
        assert!(ChatService::validate_message("hello").is_ok());
        assert!(ChatService::validate_message(&"a".repeat(2000)).is_ok());
    }

    #[test]
    fn message_trimmed() {
        let result = ChatService::validate_message("  hello  ").unwrap();
        assert_eq!(result, "hello");
    }

    #[test]
    fn javascript_pattern_stripped() {
        let result =
            ChatService::validate_message("click here javascript:void(0) to continue").unwrap();
        assert!(!result.to_lowercase().contains("javascript:"));
    }

    #[test]
    fn event_handler_stripped() {
        let result = ChatService::validate_message("user onclick='evil()' clicked").unwrap();
        assert!(!result.to_lowercase().contains("onclick"));
    }

    #[test]
    fn compact_event_handler_stripped() {
        // Test event handlers without spaces before =
        let result = ChatService::validate_message("<img onerror=alert(1) src=x>").unwrap();
        assert!(!result.to_lowercase().contains("onerror"));
    }

    #[test]
    fn script_tags_stripped() {
        let result = ChatService::validate_message("<script>alert('xss')</script>").unwrap();
        assert!(!result.to_lowercase().contains("<script"));
        assert!(!result.to_lowercase().contains("</script>"));
    }

    #[test]
    fn urls_in_angle_brackets_preserved() {
        let result = ChatService::validate_message("Check <https://example.com> for info").unwrap();
        assert!(result.contains("https://example.com"));
    }
}
