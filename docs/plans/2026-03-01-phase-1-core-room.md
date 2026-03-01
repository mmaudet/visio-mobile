# Phase 1: Core Room Connection — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** The visio-core crate can connect to a LiveKit room via the Meet API, receive events, track participants, and manage room lifecycle.

**Architecture:** `RoomManager` wraps `livekit::Room`, runs an event loop on a tokio task, and pushes `VisioEvent`s to registered listeners via a callback trait. `AuthService` calls the Meet API to get a LiveKit token. `ParticipantManager` maintains the participant list from room events.

**Tech Stack:** Rust, livekit =0.7.32 (with rustls-tls-webpki-roots), livekit-api 0.4, tokio, reqwest, serde

**Reference:** `docs/plans/2026-03-01-v2-rewrite-design.md`, LiveKit Rust SDK API patterns from v1.

---

### Task 1: Add Missing Dependencies to visio-core

**Files:**
- Modify: `crates/visio-core/Cargo.toml`
- Modify: `Cargo.toml` (workspace root)

**Step 1: Add reqwest and other deps to workspace**

Add to `[workspace.dependencies]` in root `Cargo.toml`:
```toml
reqwest = { version = "0.12", features = ["json"] }
urlencoding = "2"
futures-util = "0.3"
```

**Step 2: Update visio-core Cargo.toml**

Add the new dependencies and the `rustls-tls-webpki-roots` feature for livekit:
```toml
[dependencies]
tokio = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
tracing = { workspace = true }
thiserror = { workspace = true }
uuid = { workspace = true }
livekit = { workspace = true, features = ["rustls-tls-webpki-roots"] }
livekit-api = { workspace = true }
reqwest = { workspace = true }
urlencoding = { workspace = true }
futures-util = { workspace = true }
```

**Step 3: Verify compilation**

Run: `cargo build -p visio-core`

**Step 4: Commit**

```bash
git add Cargo.toml crates/visio-core/Cargo.toml Cargo.lock
git commit -m "chore: add reqwest, urlencoding, futures-util dependencies"
```

---

### Task 2: Implement VisioEvent and EventEmitter

**Files:**
- Modify: `crates/visio-core/src/events.rs`
- Modify: `crates/visio-core/src/lib.rs`

**Step 1: Write the full events module**

`crates/visio-core/src/events.rs`:
```rust
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
```

**Step 2: Update lib.rs to export new types**

`crates/visio-core/src/lib.rs`:
```rust
//! Visio Mobile core business logic.
//!
//! Pure Rust crate with no platform dependencies.
//! Consumed by native UI shells via UniFFI bindings.

pub mod auth;
pub mod errors;
pub mod events;
pub mod participants;
pub mod room;

pub use errors::VisioError;
pub use events::{
    ChatMessage, ConnectionQuality, ConnectionState, EventEmitter, ParticipantInfo,
    TrackInfo, TrackKind, TrackSource, VisioEvent, VisioEventListener,
};
```

Create empty stub files so it compiles:
```bash
touch crates/visio-core/src/auth.rs
touch crates/visio-core/src/participants.rs
touch crates/visio-core/src/room.rs
```

**Step 3: Verify compilation**

Run: `cargo build -p visio-core`

**Step 4: Commit**

```bash
git add crates/visio-core/src/
git commit -m "feat: implement VisioEvent types and EventEmitter"
```

---

### Task 3: Implement AuthService

**Files:**
- Modify: `crates/visio-core/src/auth.rs`
- Modify: `crates/visio-core/src/errors.rs`

**Step 1: Expand error types**

Add to `crates/visio-core/src/errors.rs`:
```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum VisioError {
    #[error("connection failed: {0}")]
    Connection(String),
    #[error("room error: {0}")]
    Room(String),
    #[error("authentication failed: {0}")]
    Auth(String),
    #[error("HTTP request failed: {0}")]
    Http(String),
    #[error("invalid URL: {0}")]
    InvalidUrl(String),
}
```

**Step 2: Implement AuthService**

`crates/visio-core/src/auth.rs`:
```rust
use crate::errors::VisioError;
use serde::Deserialize;

/// Response from the Meet API.
#[derive(Debug, Deserialize)]
struct MeetApiResponse {
    livekit: LiveKitCredentials,
}

#[derive(Debug, Deserialize)]
struct LiveKitCredentials {
    url: String,
    token: String,
}

/// Token and connection info returned by the Meet API.
#[derive(Debug, Clone)]
pub struct TokenInfo {
    /// WebSocket URL for LiveKit (wss://)
    pub livekit_url: String,
    /// JWT access token
    pub token: String,
}

/// Requests a LiveKit token from the Meet API.
pub struct AuthService;

impl AuthService {
    /// Call the Meet API to get a LiveKit token for the given room.
    ///
    /// `meet_url` should be a full URL like `https://meet.example.com/room-slug`
    /// or just `meet.example.com/room-slug`.
    pub async fn request_token(
        meet_url: &str,
        username: Option<&str>,
    ) -> Result<TokenInfo, VisioError> {
        let (instance, slug) = Self::parse_meet_url(meet_url)?;

        let mut api_url = format!("https://{}/api/v1.0/rooms/{}/", instance, slug);
        if let Some(name) = username {
            let encoded = urlencoding::encode(name);
            api_url.push_str(&format!("?username={encoded}"));
        }

        tracing::info!("requesting token from Meet API: {}", api_url);

        let resp = reqwest::get(&api_url)
            .await
            .map_err(|e| VisioError::Http(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(VisioError::Auth(format!(
                "Meet API returned status {}",
                resp.status()
            )));
        }

        let data: MeetApiResponse = resp
            .json()
            .await
            .map_err(|e| VisioError::Auth(format!("invalid Meet API response: {e}")))?;

        // Convert URL to WebSocket
        let livekit_url = data
            .livekit
            .url
            .replace("https://", "wss://")
            .replace("http://", "ws://");

        Ok(TokenInfo {
            livekit_url,
            token: data.livekit.token,
        })
    }

    /// Parse a Meet URL into (instance, room_slug).
    fn parse_meet_url(url: &str) -> Result<(String, String), VisioError> {
        let url = url
            .trim()
            .trim_end_matches('/')
            .replace("https://", "")
            .replace("http://", "");

        let parts: Vec<&str> = url.splitn(2, '/').collect();
        if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
            return Err(VisioError::InvalidUrl(format!(
                "expected 'instance/room-slug', got '{url}'"
            )));
        }

        Ok((parts[0].to_string(), parts[1].to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_meet_url_with_https() {
        let (instance, slug) = AuthService::parse_meet_url("https://meet.example.com/my-room").unwrap();
        assert_eq!(instance, "meet.example.com");
        assert_eq!(slug, "my-room");
    }

    #[test]
    fn parse_meet_url_without_scheme() {
        let (instance, slug) = AuthService::parse_meet_url("meet.example.com/room-123").unwrap();
        assert_eq!(instance, "meet.example.com");
        assert_eq!(slug, "room-123");
    }

    #[test]
    fn parse_meet_url_with_trailing_slash() {
        let (instance, slug) = AuthService::parse_meet_url("https://meet.example.com/my-room/").unwrap();
        assert_eq!(instance, "meet.example.com");
        assert_eq!(slug, "my-room");
    }

    #[test]
    fn parse_meet_url_invalid() {
        assert!(AuthService::parse_meet_url("invalid").is_err());
        assert!(AuthService::parse_meet_url("").is_err());
    }
}
```

**Step 3: Verify tests pass**

Run: `cargo test -p visio-core`

**Step 4: Commit**

```bash
git add crates/visio-core/src/auth.rs crates/visio-core/src/errors.rs
git commit -m "feat: implement AuthService with Meet API token request"
```

---

### Task 4: Implement ParticipantManager

**Files:**
- Modify: `crates/visio-core/src/participants.rs`

**Step 1: Implement ParticipantManager**

`crates/visio-core/src/participants.rs`:
```rust
use crate::events::{ConnectionQuality, ParticipantInfo};

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
        // Avoid duplicates
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
```

**Step 2: Verify tests pass**

Run: `cargo test -p visio-core`

**Step 3: Commit**

```bash
git add crates/visio-core/src/participants.rs
git commit -m "feat: implement ParticipantManager with participant tracking"
```

---

### Task 5: Implement RoomManager with Event Loop

**Files:**
- Modify: `crates/visio-core/src/room.rs`
- Modify: `crates/visio-core/src/lib.rs`

This is the most complex task. The RoomManager wraps `livekit::Room`, manages connection lifecycle, and runs an event loop that dispatches `VisioEvent`s.

**Step 1: Implement RoomManager**

`crates/visio-core/src/room.rs`:
```rust
use std::sync::Arc;
use tokio::sync::Mutex;
use livekit::prelude::*;

use crate::auth::{AuthService, TokenInfo};
use crate::errors::VisioError;
use crate::events::*;
use crate::participants::ParticipantManager;

/// Manages the lifecycle of a LiveKit room connection.
pub struct RoomManager {
    room: Arc<Mutex<Option<Arc<Room>>>>,
    emitter: EventEmitter,
    participants: Arc<Mutex<ParticipantManager>>,
    connection_state: Arc<Mutex<ConnectionState>>,
}

impl RoomManager {
    pub fn new() -> Self {
        Self {
            room: Arc::new(Mutex::new(None)),
            emitter: EventEmitter::new(),
            participants: Arc::new(Mutex::new(ParticipantManager::new())),
            connection_state: Arc::new(Mutex::new(ConnectionState::Disconnected)),
        }
    }

    /// Register a listener for room events.
    pub fn add_listener(&self, listener: Arc<dyn VisioEventListener>) {
        self.emitter.add_listener(listener);
    }

    /// Get current connection state.
    pub async fn connection_state(&self) -> ConnectionState {
        self.connection_state.lock().await.clone()
    }

    /// Get a snapshot of current participants.
    pub async fn participants(&self) -> Vec<ParticipantInfo> {
        self.participants.lock().await.participants().to_vec()
    }

    /// Get current active speakers.
    pub async fn active_speakers(&self) -> Vec<String> {
        self.participants.lock().await.active_speakers().to_vec()
    }

    /// Connect to a room using the Meet API.
    ///
    /// Calls the Meet API to get a token, then connects to the LiveKit room.
    pub async fn connect(
        &self,
        meet_url: &str,
        username: Option<&str>,
    ) -> Result<(), VisioError> {
        // Update state
        self.set_connection_state(ConnectionState::Connecting).await;

        // Get token from Meet API
        let token_info = AuthService::request_token(meet_url, username).await?;

        // Connect to LiveKit
        self.connect_with_token(&token_info.livekit_url, &token_info.token)
            .await
    }

    /// Connect directly with a LiveKit URL and token (useful for testing).
    pub async fn connect_with_token(
        &self,
        livekit_url: &str,
        token: &str,
    ) -> Result<(), VisioError> {
        self.set_connection_state(ConnectionState::Connecting).await;

        let mut options = RoomOptions::default();
        options.auto_subscribe = true;

        let (room, events) = Room::connect(livekit_url, token, options)
            .await
            .map_err(|e| VisioError::Connection(e.to_string()))?;

        let room = Arc::new(room);

        // Store local participant SID
        {
            let local = room.local_participant();
            let mut pm = self.participants.lock().await;
            pm.set_local_sid(local.sid().to_string());
        }

        // Seed existing remote participants (they joined before us)
        {
            let mut pm = self.participants.lock().await;
            for (_, participant) in room.remote_participants() {
                let info = Self::remote_participant_to_info(&participant);
                pm.add_participant(info.clone());
                self.emitter.emit(VisioEvent::ParticipantJoined(info));
            }
        }

        // Store room reference
        *self.room.lock().await = Some(room.clone());

        // Update state to connected
        self.set_connection_state(ConnectionState::Connected).await;

        // Spawn event loop
        let emitter = self.emitter.clone();
        let participants = self.participants.clone();
        let connection_state = self.connection_state.clone();
        let room_ref = self.room.clone();

        tokio::spawn(async move {
            Self::event_loop(events, emitter, participants, connection_state, room_ref).await;
        });

        Ok(())
    }

    /// Disconnect from the current room.
    pub async fn disconnect(&self) {
        let room = self.room.lock().await.take();
        if let Some(room) = room {
            if let Err(e) = room.close().await {
                tracing::warn!("error closing room: {e}");
            }
        }
        self.participants.lock().await.clear();
        self.set_connection_state(ConnectionState::Disconnected).await;
    }

    async fn set_connection_state(&self, state: ConnectionState) {
        *self.connection_state.lock().await = state.clone();
        self.emitter.emit(VisioEvent::ConnectionStateChanged(state));
    }

    fn remote_participant_to_info(p: &RemoteParticipant) -> ParticipantInfo {
        let name = {
            let n = p.name().to_string();
            if n.is_empty() { None } else { Some(n) }
        };

        let has_video = p.track_publications().values().any(|pub_| {
            pub_.kind() == livekit::TrackKind::Video
        });

        let is_muted = p.track_publications().values().any(|pub_| {
            pub_.kind() == livekit::TrackKind::Audio && pub_.is_muted()
        });

        ParticipantInfo {
            sid: p.sid().to_string(),
            identity: p.identity().to_string(),
            name,
            is_muted,
            has_video,
            connection_quality: ConnectionQuality::Good,
        }
    }

    async fn event_loop(
        mut events: tokio::sync::mpsc::UnboundedReceiver<RoomEvent>,
        emitter: EventEmitter,
        participants: Arc<Mutex<ParticipantManager>>,
        connection_state: Arc<Mutex<ConnectionState>>,
        room_ref: Arc<Mutex<Option<Arc<Room>>>>,
    ) {
        let mut reconnect_attempt: u32 = 0;

        while let Some(event) = events.recv().await {
            match event {
                RoomEvent::Connected { .. } => {
                    reconnect_attempt = 0;
                    *connection_state.lock().await = ConnectionState::Connected;
                    emitter.emit(VisioEvent::ConnectionStateChanged(ConnectionState::Connected));
                }

                RoomEvent::Reconnecting => {
                    reconnect_attempt += 1;
                    let state = ConnectionState::Reconnecting { attempt: reconnect_attempt };
                    *connection_state.lock().await = state.clone();
                    emitter.emit(VisioEvent::ConnectionStateChanged(state));
                }

                RoomEvent::Reconnected => {
                    reconnect_attempt = 0;
                    *connection_state.lock().await = ConnectionState::Connected;
                    emitter.emit(VisioEvent::ConnectionStateChanged(ConnectionState::Connected));
                }

                RoomEvent::Disconnected { reason } => {
                    tracing::info!("room disconnected: {reason}");
                    *connection_state.lock().await = ConnectionState::Disconnected;
                    emitter.emit(VisioEvent::ConnectionStateChanged(ConnectionState::Disconnected));
                    participants.lock().await.clear();
                    *room_ref.lock().await = None;
                    break;
                }

                RoomEvent::ParticipantConnected(participant) => {
                    let info = Self::remote_participant_to_info(&participant);
                    participants.lock().await.add_participant(info.clone());
                    emitter.emit(VisioEvent::ParticipantJoined(info));
                }

                RoomEvent::ParticipantDisconnected(participant) => {
                    let sid = participant.sid().to_string();
                    participants.lock().await.remove_participant(&sid);
                    emitter.emit(VisioEvent::ParticipantLeft(sid));
                }

                RoomEvent::TrackSubscribed { track, publication, participant } => {
                    let track_source = match publication.source() {
                        livekit::TrackSource::Microphone => TrackSource::Microphone,
                        livekit::TrackSource::Camera => TrackSource::Camera,
                        livekit::TrackSource::Screenshare => TrackSource::ScreenShare,
                        _ => TrackSource::Unknown,
                    };
                    let track_kind = match publication.kind() {
                        livekit::TrackKind::Audio => TrackKind::Audio,
                        livekit::TrackKind::Video => TrackKind::Video,
                    };

                    let psid = participant.sid().to_string();

                    // Update participant state
                    {
                        let mut pm = participants.lock().await;
                        if let Some(p) = pm.participant_mut(&psid) {
                            if track_kind == TrackKind::Video {
                                p.has_video = true;
                            }
                        }
                    }

                    let info = TrackInfo {
                        sid: track.sid().to_string(),
                        participant_sid: psid,
                        kind: track_kind,
                        source: track_source,
                    };
                    emitter.emit(VisioEvent::TrackSubscribed(info));
                }

                RoomEvent::TrackUnsubscribed { track, publication, participant } => {
                    let psid = participant.sid().to_string();
                    let is_video = publication.kind() == livekit::TrackKind::Video;

                    if is_video {
                        let mut pm = participants.lock().await;
                        if let Some(p) = pm.participant_mut(&psid) {
                            p.has_video = false;
                        }
                    }

                    emitter.emit(VisioEvent::TrackUnsubscribed(track.sid().to_string()));
                }

                RoomEvent::TrackMuted { participant, publication } => {
                    let psid = participant.sid().to_string();
                    let source = match publication.source() {
                        livekit::TrackSource::Microphone => TrackSource::Microphone,
                        livekit::TrackSource::Camera => TrackSource::Camera,
                        _ => TrackSource::Unknown,
                    };

                    if source == TrackSource::Microphone {
                        let mut pm = participants.lock().await;
                        if let Some(p) = pm.participant_mut(&psid) {
                            p.is_muted = true;
                        }
                    }

                    emitter.emit(VisioEvent::TrackMuted { participant_sid: psid, source });
                }

                RoomEvent::TrackUnmuted { participant, publication } => {
                    let psid = participant.sid().to_string();
                    let source = match publication.source() {
                        livekit::TrackSource::Microphone => TrackSource::Microphone,
                        livekit::TrackSource::Camera => TrackSource::Camera,
                        _ => TrackSource::Unknown,
                    };

                    if source == TrackSource::Microphone {
                        let mut pm = participants.lock().await;
                        if let Some(p) = pm.participant_mut(&psid) {
                            p.is_muted = false;
                        }
                    }

                    emitter.emit(VisioEvent::TrackUnmuted { participant_sid: psid, source });
                }

                RoomEvent::ActiveSpeakersChanged { speakers } => {
                    let sids: Vec<String> = speakers.iter().map(|p| p.sid().to_string()).collect();
                    participants.lock().await.set_active_speakers(sids.clone());
                    emitter.emit(VisioEvent::ActiveSpeakersChanged(sids));
                }

                RoomEvent::ConnectionQualityChanged { quality, participant } => {
                    let psid = participant.sid().to_string();
                    let q = match quality {
                        livekit_protocol::ConnectionQuality::Excellent => ConnectionQuality::Excellent,
                        livekit_protocol::ConnectionQuality::Good => ConnectionQuality::Good,
                        livekit_protocol::ConnectionQuality::Poor => ConnectionQuality::Poor,
                        livekit_protocol::ConnectionQuality::Lost => ConnectionQuality::Lost,
                    };

                    {
                        let mut pm = participants.lock().await;
                        if let Some(p) = pm.participant_mut(&psid) {
                            p.connection_quality = q.clone();
                        }
                    }

                    emitter.emit(VisioEvent::ConnectionQualityChanged {
                        participant_sid: psid,
                        quality: q,
                    });
                }

                _ => {
                    // Ignore other events for now
                }
            }
        }

        tracing::info!("room event loop ended");
    }
}
```

**Step 2: Update lib.rs exports**

Ensure `pub mod room;` and `pub mod participants;` are already in lib.rs (done in Task 2).
Add to re-exports:
```rust
pub use auth::{AuthService, TokenInfo};
pub use participants::ParticipantManager;
pub use room::RoomManager;
```

**Step 3: Verify compilation**

Run: `cargo build -p visio-core`

This may require adjusting types to match the exact LiveKit SDK API. Common issues:
- `RoomEvent` variant field names may differ — check the actual SDK types
- `Room::connect` may need `&str` vs `String`
- `publication.source()` may return a different enum variant type
- `livekit_protocol::ConnectionQuality` may need the `livekit-protocol` crate added

If `livekit-protocol` is needed, add it to workspace deps:
```toml
livekit-protocol = "0.3"
```

Debug and fix any compilation errors.

**Step 4: Run tests**

Run: `cargo test -p visio-core`
All existing tests from Tasks 3-4 should still pass.

**Step 5: Commit**

```bash
git add crates/visio-core/src/ Cargo.toml Cargo.lock
git commit -m "feat: implement RoomManager with LiveKit event loop"
```

---

### Task 6: Unit Tests for RoomManager (Mock-based)

**Files:**
- Create: `crates/visio-core/tests/room_tests.rs` (or add to existing test modules)

Since integration tests require a running LiveKit server, write unit tests that verify the EventEmitter and ParticipantManager integration without a real connection.

**Step 1: Write EventEmitter tests**

Add tests to `crates/visio-core/src/events.rs`:
```rust
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
```

**Step 2: Verify all tests pass**

Run: `cargo test -p visio-core`

**Step 3: Commit**

```bash
git add crates/visio-core/
git commit -m "test: add unit tests for EventEmitter and ParticipantManager"
```

---

## Phase 1 Acceptance Criteria

- [ ] `cargo build -p visio-core` succeeds with all new modules
- [ ] `AuthService::parse_meet_url()` correctly parses URLs (unit tests pass)
- [ ] `ParticipantManager` add/remove/clear works (unit tests pass)
- [ ] `EventEmitter` dispatches to multiple listeners (unit tests pass)
- [ ] `RoomManager` compiles with full event loop
- [ ] `cargo test -p visio-core` — all tests pass
- [ ] All code committed with meaningful messages
