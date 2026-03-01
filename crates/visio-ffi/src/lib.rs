//! UniFFI bindings for visio-core.
//!
//! Provides a VisioClient object that wraps RoomManager, MeetingControls,
//! and ChatService into a single FFI-safe interface.

use std::sync::Arc;
use visio_core::{
    self,
    events::{
        ChatMessage as CoreChatMessage, ConnectionQuality as CoreConnectionQuality,
        ConnectionState as CoreConnectionState, ParticipantInfo as CoreParticipantInfo,
        TrackInfo as CoreTrackInfo, TrackKind as CoreTrackKind, TrackSource as CoreTrackSource,
        VisioEvent as CoreVisioEvent,
    },
};

uniffi::include_scaffolding!("visio");

// ── FFI-safe type conversions ──────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting { attempt: u32 },
}

impl From<CoreConnectionState> for ConnectionState {
    fn from(s: CoreConnectionState) -> Self {
        match s {
            CoreConnectionState::Disconnected => Self::Disconnected,
            CoreConnectionState::Connecting => Self::Connecting,
            CoreConnectionState::Connected => Self::Connected,
            CoreConnectionState::Reconnecting { attempt } => Self::Reconnecting { attempt },
        }
    }
}

#[derive(Debug, Clone)]
pub enum ConnectionQuality {
    Excellent,
    Good,
    Poor,
    Lost,
}

impl From<CoreConnectionQuality> for ConnectionQuality {
    fn from(q: CoreConnectionQuality) -> Self {
        match q {
            CoreConnectionQuality::Excellent => Self::Excellent,
            CoreConnectionQuality::Good => Self::Good,
            CoreConnectionQuality::Poor => Self::Poor,
            CoreConnectionQuality::Lost => Self::Lost,
        }
    }
}

#[derive(Debug, Clone)]
pub enum TrackKind {
    Audio,
    Video,
}

impl From<CoreTrackKind> for TrackKind {
    fn from(k: CoreTrackKind) -> Self {
        match k {
            CoreTrackKind::Audio => Self::Audio,
            CoreTrackKind::Video => Self::Video,
        }
    }
}

#[derive(Debug, Clone)]
pub enum TrackSource {
    Microphone,
    Camera,
    ScreenShare,
    Unknown,
}

impl From<CoreTrackSource> for TrackSource {
    fn from(s: CoreTrackSource) -> Self {
        match s {
            CoreTrackSource::Microphone => Self::Microphone,
            CoreTrackSource::Camera => Self::Camera,
            CoreTrackSource::ScreenShare => Self::ScreenShare,
            CoreTrackSource::Unknown => Self::Unknown,
        }
    }
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

impl From<CoreParticipantInfo> for ParticipantInfo {
    fn from(p: CoreParticipantInfo) -> Self {
        Self {
            sid: p.sid,
            identity: p.identity,
            name: p.name,
            is_muted: p.is_muted,
            has_video: p.has_video,
            video_track_sid: p.video_track_sid,
            connection_quality: p.connection_quality.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TrackInfo {
    pub sid: String,
    pub participant_sid: String,
    pub kind: TrackKind,
    pub source: TrackSource,
}

impl From<CoreTrackInfo> for TrackInfo {
    fn from(t: CoreTrackInfo) -> Self {
        Self {
            sid: t.sid,
            participant_sid: t.participant_sid,
            kind: t.kind.into(),
            source: t.source.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub id: String,
    pub sender_sid: String,
    pub sender_name: String,
    pub text: String,
    pub timestamp_ms: u64,
}

impl From<CoreChatMessage> for ChatMessage {
    fn from(m: CoreChatMessage) -> Self {
        Self {
            id: m.id,
            sender_sid: m.sender_sid,
            sender_name: m.sender_name,
            text: m.text,
            timestamp_ms: m.timestamp_ms,
        }
    }
}

#[derive(Debug, Clone)]
pub enum VisioEvent {
    ConnectionStateChanged { state: ConnectionState },
    ParticipantJoined { info: ParticipantInfo },
    ParticipantLeft { participant_sid: String },
    TrackSubscribed { info: TrackInfo },
    TrackUnsubscribed { track_sid: String },
    TrackMuted { participant_sid: String, source: TrackSource },
    TrackUnmuted { participant_sid: String, source: TrackSource },
    ActiveSpeakersChanged { participant_sids: Vec<String> },
    ConnectionQualityChanged { participant_sid: String, quality: ConnectionQuality },
    ChatMessageReceived { message: ChatMessage },
}

impl From<CoreVisioEvent> for VisioEvent {
    fn from(e: CoreVisioEvent) -> Self {
        match e {
            CoreVisioEvent::ConnectionStateChanged(s) => {
                Self::ConnectionStateChanged { state: s.into() }
            }
            CoreVisioEvent::ParticipantJoined(p) => {
                Self::ParticipantJoined { info: p.into() }
            }
            CoreVisioEvent::ParticipantLeft(sid) => {
                Self::ParticipantLeft { participant_sid: sid }
            }
            CoreVisioEvent::TrackSubscribed(t) => {
                Self::TrackSubscribed { info: t.into() }
            }
            CoreVisioEvent::TrackUnsubscribed(sid) => {
                Self::TrackUnsubscribed { track_sid: sid }
            }
            CoreVisioEvent::TrackMuted { participant_sid, source } => {
                Self::TrackMuted { participant_sid, source: source.into() }
            }
            CoreVisioEvent::TrackUnmuted { participant_sid, source } => {
                Self::TrackUnmuted { participant_sid, source: source.into() }
            }
            CoreVisioEvent::ActiveSpeakersChanged(sids) => {
                Self::ActiveSpeakersChanged { participant_sids: sids }
            }
            CoreVisioEvent::ConnectionQualityChanged { participant_sid, quality } => {
                Self::ConnectionQualityChanged { participant_sid, quality: quality.into() }
            }
            CoreVisioEvent::ChatMessageReceived(m) => {
                Self::ChatMessageReceived { message: m.into() }
            }
        }
    }
}

// ── Error conversion ──────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum VisioError {
    #[error("Connection error")]
    Connection,
    #[error("Room error")]
    Room,
    #[error("Auth error")]
    Auth,
    #[error("HTTP error")]
    Http,
    #[error("Invalid URL")]
    InvalidUrl,
}

impl From<visio_core::VisioError> for VisioError {
    fn from(e: visio_core::VisioError) -> Self {
        match e {
            visio_core::VisioError::Connection(_) => Self::Connection,
            visio_core::VisioError::Room(_) => Self::Room,
            visio_core::VisioError::Auth(_) => Self::Auth,
            visio_core::VisioError::Http(_) => Self::Http,
            visio_core::VisioError::InvalidUrl(_) => Self::InvalidUrl,
        }
    }
}

// ── Callback interface ────────────────────────────────────────────────

pub trait VisioEventListener: Send + Sync {
    fn on_event(&self, event: VisioEvent);
}

// ── Bridge listener: FFI callback → core listener ─────────────────────

struct BridgeListener {
    ffi_listener: Arc<dyn VisioEventListener>,
}

impl visio_core::VisioEventListener for BridgeListener {
    fn on_event(&self, event: CoreVisioEvent) {
        self.ffi_listener.on_event(event.into());
    }
}

// ── VisioClient: main FFI object ──────────────────────────────────────

pub struct VisioClient {
    room_manager: visio_core::RoomManager,
    controls: visio_core::MeetingControls,
    chat: visio_core::ChatService,
    rt: tokio::runtime::Runtime,
}

impl VisioClient {
    pub fn new() -> Self {
        let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
        let room_manager = visio_core::RoomManager::new();
        let controls = room_manager.controls();
        let chat = room_manager.chat();

        Self {
            room_manager,
            controls,
            chat,
            rt,
        }
    }

    pub fn connect(&self, meet_url: String, username: Option<String>) -> Result<(), VisioError> {
        self.rt.block_on(async {
            self.room_manager
                .connect(&meet_url, username.as_deref())
                .await
                .map_err(VisioError::from)
        })
    }

    pub fn disconnect(&self) {
        self.rt.block_on(self.room_manager.disconnect());
    }

    pub fn connection_state(&self) -> ConnectionState {
        self.rt.block_on(self.room_manager.connection_state()).into()
    }

    pub fn participants(&self) -> Vec<ParticipantInfo> {
        self.rt
            .block_on(self.room_manager.participants())
            .into_iter()
            .map(ParticipantInfo::from)
            .collect()
    }

    pub fn active_speakers(&self) -> Vec<String> {
        self.rt.block_on(self.room_manager.active_speakers())
    }

    pub fn set_microphone_enabled(&self, enabled: bool) -> Result<(), VisioError> {
        self.rt.block_on(async {
            self.controls
                .set_microphone_enabled(enabled)
                .await
                .map_err(VisioError::from)
        })
    }

    pub fn set_camera_enabled(&self, enabled: bool) -> Result<(), VisioError> {
        self.rt.block_on(async {
            self.controls
                .set_camera_enabled(enabled)
                .await
                .map_err(VisioError::from)
        })
    }

    pub fn is_microphone_enabled(&self) -> bool {
        self.rt.block_on(self.controls.is_microphone_enabled())
    }

    pub fn is_camera_enabled(&self) -> bool {
        self.rt.block_on(self.controls.is_camera_enabled())
    }

    pub fn send_chat_message(&self, text: String) -> Result<ChatMessage, VisioError> {
        self.rt.block_on(async {
            self.chat
                .send_message(&text)
                .await
                .map(ChatMessage::from)
                .map_err(VisioError::from)
        })
    }

    pub fn chat_messages(&self) -> Vec<ChatMessage> {
        self.rt
            .block_on(self.chat.messages())
            .into_iter()
            .map(ChatMessage::from)
            .collect()
    }

    pub fn add_listener(&self, listener: Box<dyn VisioEventListener>) {
        let bridge = Arc::new(BridgeListener {
            ffi_listener: Arc::from(listener),
        });
        self.room_manager.add_listener(bridge);
    }
}

// ── C FFI: video attach / detach ─────────────────────────────────────

/// Attach a native surface for video rendering.
///
/// Called from native code (Kotlin JNI / Swift C interop) to start
/// rendering frames from a subscribed video track onto a platform surface.
///
/// `client_ptr` must be a valid pointer to a `VisioClient` (obtained by
/// converting an `Arc<VisioClient>` via `Arc::into_raw`). The caller
/// retains ownership — this function does **not** consume the pointer.
///
/// # Safety
/// - `client_ptr` must point to a live `VisioClient`.
/// - `track_sid` must be a valid null-terminated UTF-8 C string.
/// - `surface` must be a valid platform surface handle that outlives the
///   renderer (until `visio_detach_video_surface` is called).
///
/// Returns 0 on success, -1 on invalid arguments, -2 if the track is not
/// found.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn visio_attach_video_surface(
    client_ptr: *const VisioClient,
    track_sid: *const std::ffi::c_char,
    surface: *mut std::ffi::c_void,
) -> i32 {
    if client_ptr.is_null() || track_sid.is_null() || surface.is_null() {
        return -1;
    }

    let client = unsafe { &*client_ptr };
    let sid = unsafe { std::ffi::CStr::from_ptr(track_sid) };
    let sid_str = match sid.to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return -1,
    };

    // Look up the track from the room manager
    let track = client
        .rt
        .block_on(client.room_manager.get_video_track(&sid_str));
    match track {
        Some(video_track) => {
            visio_video::start_track_renderer(sid_str, video_track, surface);
            0
        }
        None => {
            tracing::warn!("no video track found for SID {sid_str}");
            -2
        }
    }
}

/// Detach the video surface for a track, stopping frame rendering.
///
/// # Safety
/// `track_sid` must be a valid null-terminated UTF-8 C string.
///
/// Returns 0 on success, -1 on invalid arguments.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn visio_detach_video_surface(
    track_sid: *const std::ffi::c_char,
) -> i32 {
    if track_sid.is_null() {
        return -1;
    }
    let sid = unsafe { std::ffi::CStr::from_ptr(track_sid) };
    let sid_str = match sid.to_str() {
        Ok(s) => s,
        Err(_) => return -1,
    };
    visio_video::stop_track_renderer(sid_str);
    0
}
