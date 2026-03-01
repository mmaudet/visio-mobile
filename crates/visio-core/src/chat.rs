use std::sync::Arc;
use tokio::sync::Mutex;
use livekit::prelude::*;
use livekit::data_stream::StreamTextOptions;

use crate::errors::VisioError;
use crate::events::{ChatMessage, EventEmitter, VisioEvent};

/// Shared message store between RoomManager event loop and ChatService.
pub type MessageStore = Arc<Mutex<Vec<ChatMessage>>>;

/// The topic used by LiveKit Meet / LaSuite Meet for chat messages.
const CHAT_TOPIC: &str = "lk.chat";

/// Manages chat messaging via LiveKit data channels.
pub struct ChatService {
    room: Arc<Mutex<Option<Arc<Room>>>>,
    emitter: EventEmitter,
    messages: MessageStore,
}

impl ChatService {
    pub fn new(room: Arc<Mutex<Option<Arc<Room>>>>, emitter: EventEmitter, messages: MessageStore) -> Self {
        Self {
            room,
            emitter,
            messages,
        }
    }

    /// Send a chat message to all participants using the Stream API (lk.chat topic).
    pub async fn send_message(&self, text: &str) -> Result<ChatMessage, VisioError> {
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
            .send_text(text, options)
            .await
            .map_err(|e| VisioError::Room(format!("send chat: {e}")))?;

        let msg = ChatMessage {
            id: info.id,
            sender_sid: local.sid().to_string(),
            sender_name: local.name().to_string(),
            text: text.to_string(),
            timestamp_ms: info.timestamp.timestamp_millis() as u64,
        };

        self.messages.lock().await.push(msg.clone());
        self.emitter.emit(VisioEvent::ChatMessageReceived(msg.clone()));

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
    }

    /// Clear all messages (on disconnect).
    pub async fn clear(&self) {
        self.messages.lock().await.clear();
    }
}
