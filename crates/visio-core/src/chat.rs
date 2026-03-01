use std::sync::Arc;
use tokio::sync::Mutex;
use livekit::prelude::*;

use crate::errors::VisioError;
use crate::events::{ChatMessage, EventEmitter, VisioEvent};

/// Manages chat messaging via LiveKit data channels.
pub struct ChatService {
    room: Arc<Mutex<Option<Arc<Room>>>>,
    emitter: EventEmitter,
    messages: Arc<Mutex<Vec<ChatMessage>>>,
}

impl ChatService {
    pub fn new(room: Arc<Mutex<Option<Arc<Room>>>>, emitter: EventEmitter) -> Self {
        Self {
            room,
            emitter,
            messages: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Send a chat message to all participants.
    pub async fn send_message(&self, text: &str) -> Result<ChatMessage, VisioError> {
        let room = self.room.lock().await;
        let room = room
            .as_ref()
            .ok_or_else(|| VisioError::Room("not connected".into()))?;

        let local = room.local_participant();

        let lk_msg = local
            .send_chat_message(text.to_string(), None, None)
            .await
            .map_err(|e| VisioError::Room(format!("send chat: {e}")))?;

        let msg = ChatMessage {
            id: lk_msg.id,
            sender_sid: local.sid().to_string(),
            sender_name: local.name().to_string(),
            text: lk_msg.message,
            timestamp_ms: lk_msg.timestamp as u64,
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
