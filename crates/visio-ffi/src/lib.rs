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

// ── Android WebRTC initialization ────────────────────────────────────
//
// Must be called from Kotlin AFTER System.loadLibrary, before connect().
// webrtc::InitAndroid needs a valid JNI class loader context, which is
// NOT available inside JNI_OnLoad.

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "C" fn Java_io_visio_mobile_VisioManager_nativeInitWebrtc(
    env: *mut std::ffi::c_void,
    _class: *mut std::ffi::c_void,
) {
    visio_log("VISIO FFI: nativeInitWebrtc called");
    // Get JavaVM from JNIEnv
    let env = unsafe { jni::JNIEnv::from_raw(env as *mut jni::sys::JNIEnv) }
        .expect("nativeInitWebrtc: invalid JNIEnv");
    let jvm = env.get_java_vm().expect("nativeInitWebrtc: failed to get JavaVM");

    libwebrtc::android::initialize_android(&jvm);

    // Prevent Drop from calling DestroyJavaVM
    std::mem::forget(jvm);
    visio_log("VISIO FFI: WebRTC initialized successfully");
}

// ── Android logcat helper ────────────────────────────────────────────

/// Write a message to logcat on Android, or stderr on other platforms.
fn visio_log(msg: &str) {
    #[cfg(target_os = "android")]
    {
        use std::ffi::CString;
        unsafe extern "C" {
            fn __android_log_write(prio: i32, tag: *const std::ffi::c_char, text: *const std::ffi::c_char) -> i32;
        }
        let tag = CString::new("VISIO_FFI").unwrap();
        let text = CString::new(msg).unwrap_or_else(|_| CString::new("(invalid utf8)").unwrap());
        unsafe { __android_log_write(4 /* INFO */, tag.as_ptr(), text.as_ptr()); }
    }
    #[cfg(not(target_os = "android"))]
    eprintln!("{msg}");
}

// ── Namespace functions ──────────────────────────────────────────────

/// Initialize tracing/logging. Call once from the host before using VisioClient.
/// On Android, stderr goes to logcat for debuggable builds.
fn init_logging() {
    use std::sync::Once;
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| "visio_core=debug,visio_ffi=debug,visio_video=info".parse().unwrap()),
            )
            .with_ansi(false)
            .init();
    });
}

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
    #[error("Connection error: {msg}")]
    Connection { msg: String },
    #[error("Room error: {msg}")]
    Room { msg: String },
    #[error("Auth error: {msg}")]
    Auth { msg: String },
    #[error("HTTP error: {msg}")]
    Http { msg: String },
    #[error("Invalid URL: {msg}")]
    InvalidUrl { msg: String },
}

impl From<visio_core::VisioError> for VisioError {
    fn from(e: visio_core::VisioError) -> Self {
        tracing::error!("VisioError: {e}");
        match e {
            visio_core::VisioError::Connection(msg) => Self::Connection { msg },
            visio_core::VisioError::Room(msg) => Self::Room { msg },
            visio_core::VisioError::Auth(msg) => Self::Auth { msg },
            visio_core::VisioError::Http(msg) => Self::Http { msg },
            visio_core::VisioError::InvalidUrl(msg) => Self::InvalidUrl { msg },
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
        visio_log("VISIO FFI: VisioClient::new() called");
        let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
        visio_log("VISIO FFI: tokio runtime created successfully");
        let room_manager = visio_core::RoomManager::new();
        let controls = room_manager.controls();
        let chat = room_manager.chat();

        visio_log("VISIO FFI: VisioClient::new() completed");
        Self {
            room_manager,
            controls,
            chat,
            rt,
        }
    }

    pub fn connect(&self, meet_url: String, username: Option<String>) -> Result<(), VisioError> {
        visio_log(&format!("VISIO FFI: connect() entered, url={meet_url}"));

        // Wrap in catch_unwind to prevent panics from crossing FFI boundary (UB → SIGSEGV).
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            visio_log("VISIO FFI: about to call block_on");
            let res = self.rt.block_on(async {
                visio_log("VISIO FFI: inside block_on async block");
                self.room_manager
                    .connect(&meet_url, username.as_deref())
                    .await
                    .map_err(VisioError::from)
            });
            visio_log(&format!("VISIO FFI: block_on completed, success={}", res.is_ok()));
            res
        }));

        match result {
            Ok(res) => res,
            Err(panic_info) => {
                let msg = if let Some(s) = panic_info.downcast_ref::<&str>() {
                    s.to_string()
                } else if let Some(s) = panic_info.downcast_ref::<String>() {
                    s.clone()
                } else {
                    "unknown panic".to_string()
                };
                visio_log(&format!("VISIO FFI: connect() PANIC caught: {msg}"));
                Err(VisioError::Connection { msg: format!("panic in connect: {msg}") })
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_visioclient_new_and_connect_smoke() {
        let client = VisioClient::new();
        eprintln!("TEST: VisioClient created successfully");

        let result = client.connect(
            "https://meet.linagora.com/test-desktop-debug".to_string(),
            Some("desktop-test".to_string()),
        );

        match &result {
            Ok(()) => eprintln!("TEST: connect() succeeded (unexpected but ok)"),
            Err(e) => eprintln!("TEST: connect() returned error (expected): {e}"),
        }

        eprintln!("TEST: no crash - connect() returned normally");
    }

    #[test]
    fn test_block_on_works() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(async { 42 });
        assert_eq!(result, 42);
    }
}
