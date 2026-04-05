use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use hkdf::Hkdf;
use livekit::data_stream::StreamTextOptions;
use livekit::prelude::*;
use sha2::Sha256;
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

/// Wire format version byte for encrypted chat messages.
const ENCRYPTION_VERSION: u8 = 0x01;

/// AES-256-GCM nonce size in bytes.
const NONCE_SIZE: usize = 12;

/// Shared chat encryption key, accessible from both ChatService and room event loop.
pub type ChatKey = Arc<std::sync::Mutex<Option<[u8; 32]>>>;

/// Derive a 256-bit AES-GCM key from a room token using HKDF-SHA256.
pub fn derive_chat_key(room_token: &str) -> [u8; 32] {
    let hk = Hkdf::<Sha256>::new(Some(b"visio-chat-v1"), room_token.as_bytes());
    let mut key = [0u8; 32];
    hk.expand(b"aes-256-gcm", &mut key)
        .expect("HKDF expand failed for 32-byte key");
    key
}

/// Encrypt a plaintext message using AES-256-GCM.
///
/// Wire format: `[version=0x01][12-byte nonce][ciphertext+tag]`, then base64 encoded.
pub fn encrypt_message(plaintext: &str, key: &[u8; 32]) -> Result<String, VisioError> {
    use aes_gcm::AeadCore;
    use aes_gcm::aead::OsRng;

    let cipher = Aes256Gcm::new(key.into());
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ciphertext = cipher
        .encrypt(&nonce, plaintext.as_bytes())
        .map_err(|e| VisioError::Room(format!("encrypt failed: {e}")))?;

    let mut wire = Vec::with_capacity(1 + NONCE_SIZE + ciphertext.len());
    wire.push(ENCRYPTION_VERSION);
    wire.extend_from_slice(&nonce);
    wire.extend_from_slice(&ciphertext);

    Ok(BASE64.encode(&wire))
}

/// Decrypt a base64-encoded encrypted message.
///
/// Returns the plaintext on success, or an error if the version is unknown,
/// the data is corrupt, or the key is wrong.
pub fn decrypt_message(encoded: &str, key: &[u8; 32]) -> Result<String, VisioError> {
    let wire = BASE64
        .decode(encoded)
        .map_err(|e| VisioError::Room(format!("base64 decode failed: {e}")))?;

    if wire.is_empty() {
        return Err(VisioError::Room("empty encrypted message".into()));
    }

    let version = wire[0];
    if version != ENCRYPTION_VERSION {
        return Err(VisioError::Room(format!(
            "unknown encryption version: 0x{version:02x}"
        )));
    }

    if wire.len() < 1 + NONCE_SIZE + 1 {
        return Err(VisioError::Room("encrypted message too short".into()));
    }

    let nonce = Nonce::from_slice(&wire[1..1 + NONCE_SIZE]);
    let ciphertext = &wire[1 + NONCE_SIZE..];

    let cipher = Aes256Gcm::new(key.into());
    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| VisioError::Room(format!("decrypt failed: {e}")))?;

    String::from_utf8(plaintext).map_err(|e| VisioError::Room(format!("invalid UTF-8: {e}")))
}

/// Detect whether a message string is an encrypted chat message.
///
/// Checks if it base64-decodes to bytes starting with the version byte 0x01.
pub fn is_encrypted_message(text: &str) -> bool {
    BASE64
        .decode(text)
        .ok()
        .map(|wire| !wire.is_empty() && wire[0] == ENCRYPTION_VERSION)
        .unwrap_or(false)
}

/// Manages chat messaging via LiveKit data channels.
pub struct ChatService {
    room: Arc<Mutex<Option<Arc<Room>>>>,
    emitter: EventEmitter,
    messages: MessageStore,
    unread_count: Arc<AtomicU32>,
    chat_open: Arc<AtomicBool>,
    chat_key: ChatKey,
}

impl ChatService {
    pub fn new(
        room: Arc<Mutex<Option<Arc<Room>>>>,
        emitter: EventEmitter,
        messages: MessageStore,
        chat_key: ChatKey,
    ) -> Self {
        Self {
            room,
            emitter,
            messages,
            unread_count: Arc::new(AtomicU32::new(0)),
            chat_open: Arc::new(AtomicBool::new(false)),
            chat_key,
        }
    }

    /// Derive and store the chat encryption key from the room token.
    pub fn set_room_token(&self, token: &str) {
        let key = derive_chat_key(token);
        *self.chat_key.lock().unwrap_or_else(|p| p.into_inner()) = Some(key);
    }

    /// Send a chat message to all participants using the Stream API (lk.chat topic).
    /// Messages are limited to 2000 characters (matching Meet web client).
    /// If a chat key is available, the message is encrypted before sending.
    pub async fn send_message(&self, text: &str) -> Result<ChatMessage, VisioError> {
        let text = Self::validate_message(text)?;

        let room = self.room.lock().await;
        let room = room
            .as_ref()
            .ok_or_else(|| VisioError::Room("not connected".into()))?;

        let local = room.local_participant();

        // Encrypt if a key is available
        let key = *self.chat_key.lock().unwrap_or_else(|p| p.into_inner());
        let (wire_text, encrypted) = match key {
            Some(ref k) => (encrypt_message(&text, k)?, true),
            None => (text.clone(), false),
        };

        let options = StreamTextOptions {
            topic: CHAT_TOPIC.to_string(),
            ..Default::default()
        };

        let info = local
            .send_text(&wire_text, options)
            .await
            .map_err(|e| VisioError::Room(format!("send chat: {e}")))?;

        let msg = ChatMessage {
            id: info.id,
            sender_sid: local.sid().to_string(),
            sender_name: local.name().to_string(),
            text,
            timestamp_ms: info.timestamp.timestamp_millis() as u64,
            encrypted,
            decryption_failed: false,
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

    // ── Encryption tests ────────────────────────────────────────────

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let key = derive_chat_key("test-room-token-abc123");
        let plaintext = "Hello, encrypted world!";
        let encrypted = encrypt_message(plaintext, &key).unwrap();
        let decrypted = decrypt_message(&encrypted, &key).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_wrong_key_fails() {
        let key1 = derive_chat_key("token-one");
        let key2 = derive_chat_key("token-two");
        let encrypted = encrypt_message("secret", &key1).unwrap();
        assert!(decrypt_message(&encrypted, &key2).is_err());
    }

    #[test]
    fn test_plaintext_detection() {
        assert!(!is_encrypted_message("Hello, plain text!"));
        assert!(!is_encrypted_message(""));
        assert!(!is_encrypted_message("not base64 at all !!!"));
    }

    #[test]
    fn test_encrypted_detection() {
        let key = derive_chat_key("detect-token");
        let encrypted = encrypt_message("test", &key).unwrap();
        assert!(is_encrypted_message(&encrypted));
    }

    #[test]
    fn test_long_message_roundtrip() {
        let key = derive_chat_key("long-msg-token");
        let plaintext = "x".repeat(2000);
        let encrypted = encrypt_message(&plaintext, &key).unwrap();
        // Overhead check: base64 of (1 + 12 + 2000 + 16) = 2029 bytes → ~2706 base64 chars
        assert!(
            encrypted.len() < 3000,
            "encrypted overhead too large: {} bytes",
            encrypted.len()
        );
        let decrypted = decrypt_message(&encrypted, &key).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_unknown_version_byte() {
        // Craft a message with version 0x02 instead of 0x01
        let mut wire = vec![0x02u8];
        wire.extend_from_slice(&[0u8; NONCE_SIZE]); // dummy nonce
        wire.extend_from_slice(b"dummy ciphertext");
        let encoded = BASE64.encode(&wire);
        let key = derive_chat_key("any-token");
        let err = decrypt_message(&encoded, &key).unwrap_err();
        assert!(err.to_string().contains("unknown encryption version"));
    }
}
