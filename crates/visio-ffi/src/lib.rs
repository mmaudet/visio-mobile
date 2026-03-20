//! UniFFI bindings for visio-core.
//!
//! Provides a VisioClient object that wraps RoomManager, MeetingControls,
//! and ChatService into a single FFI-safe interface.

use std::sync::{Arc, Mutex as StdMutex};
use visio_core::{
    self,
    adaptive::{
        AdaptiveMode as CoreAdaptiveMode, ContextSignal as CoreContextSignal,
        NetworkType as CoreNetworkType,
    },
    bandwidth::BandwidthMode as CoreBandwidthMode,
    events::{
        ChatMessage as CoreChatMessage, ConnectionQuality as CoreConnectionQuality,
        ConnectionState as CoreConnectionState, ParticipantInfo as CoreParticipantInfo,
        TrackInfo as CoreTrackInfo, TrackKind as CoreTrackKind, TrackSource as CoreTrackSource,
        VisioEvent as CoreVisioEvent,
    },
};

pub mod blur;

uniffi::include_scaffolding!("visio");

// ── Android WebRTC initialization ────────────────────────────────────
//
// Must be called from Kotlin AFTER System.loadLibrary, before connect().
// webrtc::InitAndroid needs a valid JNI class loader context, which is
// NOT available inside JNI_OnLoad.

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "C" fn Java_io_visio_mobile_VisioApplication_nativeInitWebrtc(
    env: *mut std::ffi::c_void,
    _class: *mut std::ffi::c_void,
) {
    visio_log("VISIO FFI: nativeInitWebrtc called");
    // Get JavaVM from JNIEnv
    let env = unsafe { jni::JNIEnv::from_raw(env as *mut jni::sys::JNIEnv) }
        .expect("nativeInitWebrtc: invalid JNIEnv");
    let jvm = env
        .get_java_vm()
        .expect("nativeInitWebrtc: failed to get JavaVM");

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
            fn __android_log_write(
                prio: i32,
                tag: *const std::ffi::c_char,
                text: *const std::ffi::c_char,
            ) -> i32;
        }
        let text = CString::new(msg).unwrap_or_else(|_| c"(invalid utf8)".into());
        unsafe {
            __android_log_write(4 /* INFO */, c"VISIO_FFI".as_ptr(), text.as_ptr());
        }
    }
    #[cfg(target_os = "ios")]
    {
        // Use syslog so messages appear in `xcrun simctl spawn ... log show`
        use std::ffi::CString;
        unsafe extern "C" {
            fn syslog(priority: i32, message: *const std::ffi::c_char, ...);
        }
        let text = CString::new(msg).unwrap_or_else(|_| c"(invalid utf8)".into());
        unsafe {
            syslog(6 /* LOG_INFO */, text.as_ptr());
        }
    }
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
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
                tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                    "visio_core=debug,visio_ffi=debug,visio_video=info"
                        .parse()
                        .unwrap()
                }),
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
    WaitingForHost,
}

impl From<CoreConnectionState> for ConnectionState {
    fn from(s: CoreConnectionState) -> Self {
        match s {
            CoreConnectionState::Disconnected => Self::Disconnected,
            CoreConnectionState::Connecting => Self::Connecting,
            CoreConnectionState::Connected => Self::Connected,
            CoreConnectionState::Reconnecting { attempt } => Self::Reconnecting { attempt },
            CoreConnectionState::WaitingForHost => Self::WaitingForHost,
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
pub enum AdaptiveMode {
    Office,
    Pedestrian,
    Car,
}

impl From<CoreAdaptiveMode> for AdaptiveMode {
    fn from(m: CoreAdaptiveMode) -> Self {
        match m {
            CoreAdaptiveMode::Office => Self::Office,
            CoreAdaptiveMode::Pedestrian => Self::Pedestrian,
            CoreAdaptiveMode::Car => Self::Car,
        }
    }
}

impl From<AdaptiveMode> for CoreAdaptiveMode {
    fn from(m: AdaptiveMode) -> Self {
        match m {
            AdaptiveMode::Office => Self::Office,
            AdaptiveMode::Pedestrian => Self::Pedestrian,
            AdaptiveMode::Car => Self::Car,
        }
    }
}

#[derive(Debug, Clone)]
pub enum BandwidthMode {
    Full,
    ReducedVideo,
    AudioOnly,
}

impl From<CoreBandwidthMode> for BandwidthMode {
    fn from(m: CoreBandwidthMode) -> Self {
        match m {
            CoreBandwidthMode::Full => Self::Full,
            CoreBandwidthMode::ReducedVideo => Self::ReducedVideo,
            CoreBandwidthMode::AudioOnly => Self::AudioOnly,
        }
    }
}

#[derive(Debug, Clone)]
pub enum NetworkType {
    Wifi,
    Cellular,
    Unknown,
}

impl From<NetworkType> for CoreNetworkType {
    fn from(n: NetworkType) -> Self {
        match n {
            NetworkType::Wifi => Self::Wifi,
            NetworkType::Cellular => Self::Cellular,
            NetworkType::Unknown => Self::Unknown,
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
    pub has_screen_share: bool,
    pub screen_share_track_sid: Option<String>,
    pub connection_quality: ConnectionQuality,
    pub color: Option<String>,
    pub is_admin: bool,
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
            has_screen_share: p.has_screen_share,
            screen_share_track_sid: p.screen_share_track_sid,
            connection_quality: p.connection_quality.into(),
            color: p.color,
            is_admin: p.is_admin,
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
pub struct Settings {
    pub display_name: Option<String>,
    pub language: Option<String>,
    pub mic_enabled_on_join: bool,
    pub camera_enabled_on_join: bool,
    pub theme: String,
    pub meet_instances: Vec<String>,
    pub notification_participant_join: bool,
    pub notification_hand_raised: bool,
    pub notification_message_received: bool,
    pub adaptive_mode_enabled: bool,
}

impl From<visio_core::Settings> for Settings {
    fn from(s: visio_core::Settings) -> Self {
        Self {
            display_name: s.display_name,
            language: s.language,
            mic_enabled_on_join: s.mic_enabled_on_join,
            camera_enabled_on_join: s.camera_enabled_on_join,
            theme: s.theme,
            meet_instances: s.meet_instances,
            notification_participant_join: s.notification_participant_join,
            notification_hand_raised: s.notification_hand_raised,
            notification_message_received: s.notification_message_received,
            adaptive_mode_enabled: s.adaptive_mode_enabled,
        }
    }
}

#[derive(Debug, Clone)]
pub enum RoomValidationResult {
    Valid { livekit_url: String, token: String },
    NotFound,
    InvalidFormat { message: String },
    NetworkError { message: String },
}

#[derive(Debug, Clone)]
pub enum SessionState {
    Anonymous,
    Authenticated {
        display_name: String,
        email: String,
        meet_instance: String,
    },
}

#[derive(Debug, Clone)]
pub struct CreateRoomResult {
    pub id: String,
    pub slug: String,
    pub name: String,
    pub access_level: String,
    pub livekit_url: String,
    pub livekit_token: String,
}

#[derive(Debug, Clone)]
pub struct WaitingParticipant {
    pub id: String,
    pub username: String,
}

impl From<visio_core::WaitingParticipant> for WaitingParticipant {
    fn from(w: visio_core::WaitingParticipant) -> Self {
        Self {
            id: w.id,
            username: w.username,
        }
    }
}

#[derive(Debug, Clone)]
pub struct UserSearchResult {
    pub id: String,
    pub email: String,
    pub full_name: Option<String>,
    pub short_name: Option<String>,
}

impl From<visio_core::UserSearchResult> for UserSearchResult {
    fn from(u: visio_core::UserSearchResult) -> Self {
        Self {
            id: u.id,
            email: u.email,
            full_name: u.full_name,
            short_name: u.short_name,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RoomAccess {
    pub id: String,
    pub user: UserSearchResult,
    pub resource: String,
    pub role: String,
}

impl From<visio_core::RoomAccess> for RoomAccess {
    fn from(a: visio_core::RoomAccess) -> Self {
        Self {
            id: a.id,
            user: a.user.into(),
            resource: a.resource,
            role: a.role,
        }
    }
}

#[derive(Debug, Clone)]
pub enum VisioEvent {
    ConnectionStateChanged {
        state: ConnectionState,
    },
    ParticipantJoined {
        info: ParticipantInfo,
    },
    ParticipantLeft {
        participant_sid: String,
    },
    TrackSubscribed {
        info: TrackInfo,
    },
    TrackUnsubscribed {
        track_sid: String,
    },
    TrackMuted {
        participant_sid: String,
        source: TrackSource,
    },
    TrackUnmuted {
        participant_sid: String,
        source: TrackSource,
    },
    ActiveSpeakersChanged {
        participant_sids: Vec<String>,
    },
    ConnectionQualityChanged {
        participant_sid: String,
        quality: ConnectionQuality,
    },
    ChatMessageReceived {
        message: ChatMessage,
    },
    HandRaisedChanged {
        participant_sid: String,
        raised: bool,
        position: u32,
    },
    UnreadCountChanged {
        count: u32,
    },
    LobbyParticipantJoined {
        id: String,
        username: String,
    },
    LobbyParticipantLeft {
        id: String,
    },
    LobbyDenied,
    LobbyTimeout,
    ReactionReceived {
        participant_sid: String,
        participant_name: String,
        emoji: String,
    },
    AdaptiveModeChanged {
        mode: AdaptiveMode,
    },
    BandwidthModeChanged {
        mode: BandwidthMode,
    },
    ConnectionLost,
    DisconnectedDuplicateIdentity,
    DisconnectedByAdmin,
    AloneInRoom { remaining_secs: u32 },
    AloneInRoomCancelled,
    MuteRequested,
}

impl From<CoreVisioEvent> for VisioEvent {
    fn from(e: CoreVisioEvent) -> Self {
        match e {
            CoreVisioEvent::ConnectionStateChanged(s) => {
                Self::ConnectionStateChanged { state: s.into() }
            }
            CoreVisioEvent::ParticipantJoined(p) => Self::ParticipantJoined { info: p.into() },
            CoreVisioEvent::ParticipantLeft(sid) => Self::ParticipantLeft {
                participant_sid: sid,
            },
            CoreVisioEvent::TrackSubscribed(t) => Self::TrackSubscribed { info: t.into() },
            CoreVisioEvent::TrackUnsubscribed(sid) => Self::TrackUnsubscribed { track_sid: sid },
            CoreVisioEvent::TrackMuted {
                participant_sid,
                source,
            } => Self::TrackMuted {
                participant_sid,
                source: source.into(),
            },
            CoreVisioEvent::TrackUnmuted {
                participant_sid,
                source,
            } => Self::TrackUnmuted {
                participant_sid,
                source: source.into(),
            },
            CoreVisioEvent::ActiveSpeakersChanged(sids) => Self::ActiveSpeakersChanged {
                participant_sids: sids,
            },
            CoreVisioEvent::ConnectionQualityChanged {
                participant_sid,
                quality,
            } => Self::ConnectionQualityChanged {
                participant_sid,
                quality: quality.into(),
            },
            CoreVisioEvent::ChatMessageReceived(m) => {
                Self::ChatMessageReceived { message: m.into() }
            }
            CoreVisioEvent::HandRaisedChanged {
                participant_sid,
                raised,
                position,
            } => Self::HandRaisedChanged {
                participant_sid,
                raised,
                position,
            },
            CoreVisioEvent::UnreadCountChanged(count) => Self::UnreadCountChanged { count },
            CoreVisioEvent::LobbyParticipantJoined { id, username } => {
                Self::LobbyParticipantJoined { id, username }
            }
            CoreVisioEvent::LobbyParticipantLeft { id } => Self::LobbyParticipantLeft { id },
            CoreVisioEvent::LobbyDenied => Self::LobbyDenied,
            CoreVisioEvent::LobbyTimeout => Self::LobbyTimeout,
            CoreVisioEvent::ReactionReceived {
                participant_sid,
                participant_name,
                emoji,
            } => Self::ReactionReceived {
                participant_sid,
                participant_name,
                emoji,
            },
            CoreVisioEvent::AdaptiveModeChanged { mode } => {
                Self::AdaptiveModeChanged { mode: mode.into() }
            }
            CoreVisioEvent::BandwidthModeChanged { mode } => {
                Self::BandwidthModeChanged { mode: mode.into() }
            }
            CoreVisioEvent::ConnectionLost => Self::ConnectionLost,
            CoreVisioEvent::DisconnectedDuplicateIdentity => Self::DisconnectedDuplicateIdentity,
            CoreVisioEvent::DisconnectedByAdmin => Self::DisconnectedByAdmin,
            CoreVisioEvent::AloneInRoom { remaining_secs } => {
                Self::AloneInRoom { remaining_secs }
            }
            CoreVisioEvent::AloneInRoomCancelled => Self::AloneInRoomCancelled,
            CoreVisioEvent::MuteRequested => Self::MuteRequested,
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
    #[error("Session error: {msg}")]
    Session { msg: String },
    #[error("{msg}")]
    Generic { msg: String },
}

impl From<visio_core::VisioError> for VisioError {
    fn from(e: visio_core::VisioError) -> Self {
        tracing::error!("VisioError: {e}");
        match e {
            visio_core::VisioError::Connection(msg) => Self::Connection { msg },
            visio_core::VisioError::Room(msg) => Self::Room { msg },
            visio_core::VisioError::Auth(msg) => Self::Auth { msg },
            visio_core::VisioError::AuthRequired => Self::Auth {
                msg: "authentication required".to_string(),
            },
            visio_core::VisioError::Http(msg) => Self::Http { msg },
            visio_core::VisioError::InvalidUrl(msg) => Self::InvalidUrl { msg },
            visio_core::VisioError::Session(msg) => Self::Session { msg },
            visio_core::VisioError::WaitingForHost => Self::Auth {
                msg: "waiting for host approval".to_string(),
            },
            visio_core::VisioError::DevicePermissionDenied(msg) => Self::Generic { msg },
            visio_core::VisioError::DeviceInUse(msg) => Self::Generic { msg },
            visio_core::VisioError::DeviceNotFound(msg) => Self::Generic { msg },
        }
    }
}

// ── Callback interface ────────────────────────────────────────────────

pub trait VisioEventListener: Send + Sync {
    fn on_event(&self, event: VisioEvent);
}

// ── Bridge listener: FFI callback → core listener ─────────────────────

/// Send-safe wrapper for raw VisioClient pointer (used in async spawn on Android).
#[cfg(target_os = "android")]
struct SendClientPtr(usize);
#[cfg(target_os = "android")]
unsafe impl Send for SendClientPtr {}
#[cfg(target_os = "android")]
impl SendClientPtr {
    fn as_client(&self) -> &VisioClient {
        unsafe { &*(self.0 as *const VisioClient) }
    }
}

/// Send-safe wrapper for raw surface pointer (used in async spawn on Android).
#[cfg(target_os = "android")]
struct SendSurfacePtr(usize);
#[cfg(target_os = "android")]
unsafe impl Send for SendSurfacePtr {}
#[cfg(target_os = "android")]
impl SendSurfacePtr {
    fn as_ptr(&self) -> *mut std::ffi::c_void {
        self.0 as *mut std::ffi::c_void
    }
}

struct BridgeListener {
    ffi_listener: Arc<dyn VisioEventListener>,
    room_manager: visio_core::RoomManager,
    settings: Arc<visio_core::SettingsStore>,
}

impl visio_core::VisioEventListener for BridgeListener {
    fn on_event(&self, event: CoreVisioEvent) {
        // Clean up pending surfaces when a track is unsubscribed (Android only).
        #[cfg(target_os = "android")]
        if let CoreVisioEvent::TrackUnsubscribed(ref track_sid) = event {
            pending::remove(track_sid);
        }

        // Check for pending surfaces when a video track is subscribed (Android only).
        // Spawn on the runtime to avoid block_on inside a tokio context.
        #[cfg(target_os = "android")]
        if let CoreVisioEvent::TrackSubscribed(ref info) = event {
            if info.kind == visio_core::events::TrackKind::Video {
                if let Some(surface_ptr) = pending::take(&info.sid) {
                    visio_log(&format!(
                        "VISIO JNI: attaching pending surface for track {}",
                        info.sid
                    ));
                    let client_addr = *CLIENT_FOR_VIDEO.lock().unwrap();
                    if client_addr != 0 {
                        let client = unsafe { &*(client_addr as *const VisioClient) };
                        let sid = info.sid.clone();
                        let is_screencast = info.source == visio_core::events::TrackSource::ScreenShare;
                        let rt_handle = client.rt.handle().clone();
                        // Wrap raw pointers in Send-able newtypes for the async block.
                        let send_client = SendClientPtr(client_addr);
                        let send_surface = SendSurfacePtr(surface_ptr as usize);
                        rt_handle.spawn(async move {
                            // SAFETY: client_addr is valid while connected (cleared on disconnect).
                            let client = send_client.as_client();
                            let track = client.room_manager.get_video_track(&sid).await;
                            if let Some(video_track) = track {
                                visio_video::start_track_renderer(
                                    sid.clone(),
                                    video_track,
                                    send_surface.as_ptr(),
                                    Some(client.rt.handle().clone()),
                                    is_screencast,
                                );
                                visio_log(&format!(
                                    "VISIO JNI: pending surface attached for track {sid}"
                                ));
                            }
                        });
                    }
                }
            }
        }

        // Record room in history on successful connection (covers lobby acceptance path
        // which bypasses VisioClient::connect's return-value path at line 805).
        if let CoreVisioEvent::ConnectionStateChanged(visio_core::ConnectionState::Connected) = &event {
            let rm = self.room_manager.clone();
            let settings = self.settings.clone();
            tokio::spawn(async move {
                if let Some((url, _)) = rm.last_connection_info().await {
                    settings.add_room_to_history(url);
                }
            });
        }

        self.ffi_listener.on_event(event.into());
    }
}

// ── VisioClient: main FFI object ──────────────────────────────────────

pub struct VisioClient {
    room_manager: visio_core::RoomManager,
    controls: visio_core::MeetingControls,
    chat: visio_core::ChatService,
    settings: Arc<visio_core::SettingsStore>,
    session_manager: Arc<StdMutex<visio_core::SessionManager>>,
    rt: tokio::runtime::Runtime,
}

impl VisioClient {
    pub fn new(data_dir: String) -> Self {
        visio_log("VISIO FFI: VisioClient::new() called");
        let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
        visio_log("VISIO FFI: tokio runtime created successfully");
        let settings = Arc::new(visio_core::SettingsStore::new(&data_dir));
        let room_manager = visio_core::RoomManager::new();

        // Store playout buffer for Android JNI audio pull
        #[cfg(target_os = "android")]
        {
            let buf = room_manager.playout_buffer();
            *PLAYOUT_BUFFER.lock().unwrap() = Some(buf);
            visio_log("VISIO FFI: playout buffer stored for Android audio output");
        }

        // Store playout buffer for iOS C FFI audio pull
        #[cfg(target_os = "ios")]
        {
            let buf = room_manager.playout_buffer();
            *PLAYOUT_BUFFER_IOS.lock().unwrap() = Some(buf);
            visio_log("VISIO FFI: playout buffer stored for iOS audio output");
        }

        let controls = room_manager.controls();
        let chat = room_manager.chat();

        let session_manager = Arc::new(StdMutex::new(visio_core::SessionManager::new()));

        visio_log("VISIO FFI: VisioClient::new() completed");
        Self {
            room_manager,
            controls,
            chat,
            settings,
            session_manager,
            rt,
        }
    }

    pub fn connect(&self, meet_url: String, username: Option<String>) -> Result<(), VisioError> {
        visio_log(&format!("VISIO FFI: connect() entered, url={meet_url}"));

        let cookie = {
            let session = self.session_manager.lock().unwrap();
            session.cookie()
        };

        visio_log(&format!(
            "VISIO FFI: connect() cookie present={}",
            cookie.is_some()
        ));

        // Wrap in catch_unwind to prevent panics from crossing FFI boundary (UB → SIGSEGV).
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            visio_log("VISIO FFI: about to call block_on");
            let res = self.rt.block_on(async {
                visio_log("VISIO FFI: inside block_on async block");
                self.room_manager
                    .connect(&meet_url, username.as_deref(), cookie.as_deref())
                    .await
                    .map_err(VisioError::from)
            });
            visio_log(&format!(
                "VISIO FFI: block_on completed, success={}",
                res.is_ok()
            ));
            res
        }));

        match result {
            Ok(Ok(())) => {
                // Store self pointer for JNI video attach/detach
                #[cfg(target_os = "android")]
                {
                    *CLIENT_FOR_VIDEO.lock().unwrap() = self as *const VisioClient as usize;
                }

                self.settings.add_room_to_history(meet_url.clone());

                Ok(())
            }
            Ok(Err(e)) => Err(e),
            Err(panic_info) => {
                let msg = if let Some(s) = panic_info.downcast_ref::<&str>() {
                    s.to_string()
                } else if let Some(s) = panic_info.downcast_ref::<String>() {
                    s.clone()
                } else {
                    "unknown panic".to_string()
                };
                visio_log(&format!("VISIO FFI: connect() PANIC caught: {msg}"));
                Err(VisioError::Connection {
                    msg: format!("panic in connect: {msg}"),
                })
            }
        }
    }

    pub fn connect_with_token(
        &self,
        livekit_url: String,
        token: String,
    ) -> Result<(), VisioError> {
        visio_log(&format!(
            "VISIO FFI: connect_with_token() entered, url={livekit_url}"
        ));

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.rt.block_on(async {
                self.room_manager
                    .connect_with_token(&livekit_url, &token)
                    .await
                    .map_err(VisioError::from)
            })
        }));

        match result {
            Ok(Ok(())) => {
                #[cfg(target_os = "android")]
                {
                    *CLIENT_FOR_VIDEO.lock().unwrap() = self as *const VisioClient as usize;
                }

                Ok(())
            }
            Ok(Err(e)) => Err(e),
            Err(panic_info) => {
                let msg = if let Some(s) = panic_info.downcast_ref::<&str>() {
                    s.to_string()
                } else if let Some(s) = panic_info.downcast_ref::<String>() {
                    s.clone()
                } else {
                    "unknown panic".to_string()
                };
                visio_log(&format!(
                    "VISIO FFI: connect_with_token() PANIC caught: {msg}"
                ));
                Err(VisioError::Connection {
                    msg: format!("panic in connect_with_token: {msg}"),
                })
            }
        }
    }

    pub fn disconnect(&self) {
        // Clear the client pointer BEFORE disconnecting so no JNI call
        // can dereference a stale pointer while teardown is in progress.
        #[cfg(target_os = "android")]
        {
            *CLIENT_FOR_VIDEO.lock().unwrap() = 0;
            // Release the local preview surface (detachSurface is a no-op for
            // local-camera to avoid a recomposition race, so we clean up here).
            LOCAL_PREVIEW_SURFACE.lock().unwrap().take();
            // Drain pending surfaces to release any ANativeWindow that was never attached.
            pending::drain();
        }
        self.rt.block_on(self.room_manager.disconnect());
    }

    pub fn reconnect(&self) -> Result<(), VisioError> {
        self.rt
            .block_on(self.room_manager.reconnect())
            .map_err(Into::into)
    }

    pub fn connection_state(&self) -> ConnectionState {
        self.rt
            .block_on(self.room_manager.connection_state())
            .into()
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
                .map_err(VisioError::from)?;

            // Store/clear the audio source for platform-native capture pipelines
            #[cfg(any(target_os = "android", target_os = "ios"))]
            {
                #[cfg(target_os = "android")]
                let guard = &AUDIO_SOURCE;
                #[cfg(target_os = "ios")]
                let guard = &AUDIO_SOURCE_IOS;
                let mut lock = guard.lock().unwrap();
                if enabled {
                    if let Some(source) = self.controls.audio_source().await {
                        visio_log("VISIO FFI: audio source stored for native pipeline");
                        *lock = Some(source);
                    }
                } else {
                    visio_log("VISIO FFI: audio source cleared");
                    *lock = None;
                }
            }

            Ok(())
        })
    }

    pub fn set_camera_enabled(&self, enabled: bool) -> Result<(), VisioError> {
        self.rt.block_on(async {
            self.controls
                .set_camera_enabled(enabled)
                .await
                .map_err(VisioError::from)?;

            // On Android, store/clear the video source for the Camera2 → JNI pipeline
            #[cfg(target_os = "android")]
            {
                let mut guard = CAMERA_SOURCE.lock().unwrap();
                if enabled {
                    if let Some(source) = self.controls.video_source().await {
                        visio_log("VISIO FFI: camera source stored for JNI pipeline");
                        *guard = Some(source);
                    } else {
                        visio_log("VISIO FFI: ERROR — video_source() returned None, CAMERA_SOURCE not set!");
                    }
                } else {
                    visio_log("VISIO FFI: camera source cleared");
                    *guard = None;
                }
            }

            // On iOS, store/clear the video source for the AVCaptureSession → C FFI pipeline
            #[cfg(target_os = "ios")]
            {
                let mut guard = CAMERA_SOURCE_IOS.lock().unwrap();
                if enabled {
                    if let Some(source) = self.controls.video_source().await {
                        visio_log("VISIO FFI: camera source stored for iOS capture pipeline");
                        *guard = Some(source);
                    }
                } else {
                    visio_log("VISIO FFI: camera source cleared");
                    *guard = None;
                }
            }

            Ok(())
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
            room_manager: self.room_manager.clone(),
            settings: self.settings.clone(),
        });
        self.room_manager.add_listener(bridge);
    }

    pub fn get_settings(&self) -> Settings {
        self.settings.get().into()
    }

    pub fn set_display_name(&self, name: Option<String>) {
        self.settings.set_display_name(name);
    }

    pub fn set_language(&self, lang: Option<String>) {
        self.settings.set_language(lang);
    }

    pub fn set_mic_enabled_on_join(&self, enabled: bool) {
        self.settings.set_mic_enabled_on_join(enabled);
    }

    pub fn set_camera_enabled_on_join(&self, enabled: bool) {
        self.settings.set_camera_enabled_on_join(enabled);
    }

    pub fn set_theme(&self, theme: String) {
        self.settings.set_theme(theme);
    }

    pub fn get_meet_instances(&self) -> Vec<String> {
        self.settings.get_meet_instances()
    }

    pub fn set_meet_instances(&self, instances: Vec<String>) {
        self.settings.set_meet_instances(instances);
    }

    pub fn set_notification_participant_join(&self, enabled: bool) {
        self.settings.set_notification_participant_join(enabled);
    }

    pub fn set_notification_hand_raised(&self, enabled: bool) {
        self.settings.set_notification_hand_raised(enabled);
    }

    pub fn set_notification_message_received(&self, enabled: bool) {
        self.settings.set_notification_message_received(enabled);
    }

    pub fn is_adaptive_mode_enabled(&self) -> bool {
        self.settings.is_adaptive_mode_enabled()
    }

    pub fn set_adaptive_mode_enabled(&self, enabled: bool) {
        self.settings.set_adaptive_mode_enabled(enabled);
    }

    pub fn add_room_to_history(&self, url: String) {
        self.settings.add_room_to_history(url);
    }

    pub fn get_room_history(&self) -> Vec<String> {
        self.settings.get_room_history()
    }

    pub fn clear_room_history(&self) {
        self.settings.clear_room_history();
    }

    pub fn raise_hand(&self) -> Result<(), VisioError> {
        self.rt
            .block_on(self.room_manager.raise_hand())
            .map_err(VisioError::from)
    }

    pub fn lower_hand(&self) -> Result<(), VisioError> {
        self.rt
            .block_on(self.room_manager.lower_hand())
            .map_err(VisioError::from)
    }

    pub fn lower_all_hands(&self) -> Result<(), VisioError> {
        self.rt
            .block_on(self.room_manager.lower_all_hands())
            .map_err(VisioError::from)
    }

    pub fn mute_everyone(&self) -> Result<(), VisioError> {
        self.rt
            .block_on(self.room_manager.mute_everyone())
            .map_err(VisioError::from)
    }

    pub fn is_hand_raised(&self) -> bool {
        self.rt.block_on(self.room_manager.is_hand_raised())
    }

    pub fn send_reaction(&self, emoji: String) -> Result<(), VisioError> {
        self.rt
            .block_on(self.room_manager.send_reaction(&emoji))
            .map_err(VisioError::from)
    }

    pub fn set_chat_open(&self, open: bool) {
        self.room_manager.set_chat_open(open);
    }

    pub fn unread_count(&self) -> u32 {
        self.room_manager.unread_count()
    }

    pub fn validate_room(&self, url: String, username: Option<String>) -> RoomValidationResult {
        let cookie = {
            let session = self.session_manager.lock().unwrap();
            session.cookie()
        };
        if let Err(e) = visio_core::AuthService::extract_slug(&url) {
            return RoomValidationResult::InvalidFormat {
                message: e.to_string(),
            };
        }
        match self.rt.block_on(visio_core::AuthService::validate_room(
            &url,
            username.as_deref(),
            cookie.as_deref(),
        )) {
            Ok(token_info) => RoomValidationResult::Valid {
                livekit_url: token_info.livekit_url,
                token: token_info.token,
            },
            Err(visio_core::VisioError::Auth(msg)) if msg.contains("404") => {
                RoomValidationResult::NotFound
            }
            Err(e) => RoomValidationResult::NetworkError {
                message: e.to_string(),
            },
        }
    }

    /// Set session cookie after OIDC flow, validate with backend
    pub fn authenticate(&self, meet_url: String, cookie: String) -> Result<(), VisioError> {
        let user = self
            .rt
            .block_on(visio_core::SessionManager::fetch_user(&meet_url, &cookie))
            .map_err(VisioError::from)?;

        let instance = meet_url
            .trim_end_matches('/')
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .to_string();
        let mut session = self.session_manager.lock().unwrap();
        session.set_authenticated(user, cookie, instance);
        Ok(())
    }

    /// Get current session state
    pub fn get_session_state(&self) -> SessionState {
        let session = self.session_manager.lock().unwrap();
        match session.state() {
            visio_core::SessionState::Anonymous => SessionState::Anonymous,
            visio_core::SessionState::Authenticated {
                user,
                meet_instance,
                ..
            } => SessionState::Authenticated {
                display_name: user.display_name(),
                email: user.email.clone(),
                meet_instance: meet_instance.clone(),
            },
        }
    }

    /// Logout and clear session
    pub fn logout(&self, meet_url: String) -> Result<(), VisioError> {
        let mut session = self.session_manager.lock().unwrap();
        self.rt
            .block_on(session.logout(&meet_url))
            .map_err(VisioError::from)?;
        Ok(())
    }

    /// Validate existing session cookie (returns true if still valid)
    pub fn validate_session(&self, meet_url: String) -> Result<bool, VisioError> {
        let mut session = self.session_manager.lock().unwrap();
        self.rt
            .block_on(session.validate_session(&meet_url))
            .map_err(VisioError::from)
    }

    /// Create a new room via the Meet backend API
    pub fn create_room(
        &self,
        meet_url: String,
        name: String,
        access_level: String,
    ) -> Result<CreateRoomResult, VisioError> {
        let cookie = {
            let session = self.session_manager.lock().unwrap();
            session.cookie().ok_or_else(|| VisioError::Session {
                msg: "Not authenticated".to_string(),
            })?
        };

        let result = self
            .rt
            .block_on(visio_core::SessionManager::create_room(
                &meet_url,
                &cookie,
                &name,
                &access_level,
            ))
            .map_err(VisioError::from)?;

        let (livekit_url, livekit_token) = match result.livekit {
            Some(lk) => (
                lk.url
                    .replace("https://", "wss://")
                    .replace("http://", "ws://"),
                lk.token,
            ),
            None => (String::new(), String::new()),
        };

        Ok(CreateRoomResult {
            id: result.id,
            slug: result.slug,
            name: result.name,
            access_level: result.access_level,
            livekit_url,
            livekit_token,
        })
    }

    pub fn list_waiting_participants(&self) -> Result<Vec<WaitingParticipant>, VisioError> {
        self.rt
            .block_on(self.room_manager.list_waiting_participants())
            .map(|v| v.into_iter().map(Into::into).collect())
            .map_err(Into::into)
    }

    pub fn admit_participant(&self, participant_id: String) -> Result<(), VisioError> {
        self.rt
            .block_on(self.room_manager.handle_lobby_entry(&participant_id, true))
            .map_err(Into::into)
    }

    pub fn deny_participant(&self, participant_id: String) -> Result<(), VisioError> {
        self.rt
            .block_on(self.room_manager.handle_lobby_entry(&participant_id, false))
            .map_err(Into::into)
    }

    pub fn cancel_lobby(&self) {
        self.rt.block_on(self.room_manager.cancel_lobby());
    }

    pub fn search_users(&self, query: String) -> Result<Vec<UserSearchResult>, VisioError> {
        let (cookie, meet_instance) = {
            let session = self.session_manager.lock().unwrap();
            let cookie = session.cookie().ok_or_else(|| VisioError::Session {
                msg: "Not authenticated".to_string(),
            })?;
            let instance = session
                .meet_instance()
                .ok_or_else(|| VisioError::Session {
                    msg: "No meet instance".to_string(),
                })?
                .to_string();
            (cookie, instance)
        };
        let meet_url = format!("https://{}/room", meet_instance);

        let results = self
            .rt
            .block_on(visio_core::AccessService::search_users(
                &meet_url, &cookie, &query,
            ))
            .map_err(VisioError::from)?;

        Ok(results.into_iter().map(|u| u.into()).collect())
    }

    pub fn list_accesses(&self, room_id: String) -> Result<Vec<RoomAccess>, VisioError> {
        let (cookie, meet_instance) = {
            let session = self.session_manager.lock().unwrap();
            let cookie = session.cookie().ok_or_else(|| VisioError::Session {
                msg: "Not authenticated".to_string(),
            })?;
            let instance = session
                .meet_instance()
                .ok_or_else(|| VisioError::Session {
                    msg: "No meet instance".to_string(),
                })?
                .to_string();
            (cookie, instance)
        };
        let meet_url = format!("https://{}/room", meet_instance);

        let results = self
            .rt
            .block_on(visio_core::AccessService::list_accesses(
                &meet_url, &cookie, &room_id,
            ))
            .map_err(VisioError::from)?;

        Ok(results.into_iter().map(|a| a.into()).collect())
    }

    pub fn add_access(&self, user_id: String, room_id: String) -> Result<RoomAccess, VisioError> {
        let (cookie, meet_instance) = {
            let session = self.session_manager.lock().unwrap();
            let cookie = session.cookie().ok_or_else(|| VisioError::Session {
                msg: "Not authenticated".to_string(),
            })?;
            let instance = session
                .meet_instance()
                .ok_or_else(|| VisioError::Session {
                    msg: "No meet instance".to_string(),
                })?
                .to_string();
            (cookie, instance)
        };
        let meet_url = format!("https://{}/room", meet_instance);

        let result = self
            .rt
            .block_on(visio_core::AccessService::add_access(
                &meet_url, &cookie, &user_id, &room_id,
            ))
            .map_err(VisioError::from)?;

        Ok(result.into())
    }

    pub fn remove_access(&self, access_id: String) -> Result<(), VisioError> {
        let (cookie, meet_instance) = {
            let session = self.session_manager.lock().unwrap();
            let cookie = session.cookie().ok_or_else(|| VisioError::Session {
                msg: "Not authenticated".to_string(),
            })?;
            let instance = session
                .meet_instance()
                .ok_or_else(|| VisioError::Session {
                    msg: "No meet instance".to_string(),
                })?
                .to_string();
            (cookie, instance)
        };
        let meet_url = format!("https://{}/room", meet_instance);

        self.rt
            .block_on(visio_core::AccessService::remove_access(
                &meet_url, &cookie, &access_id,
            ))
            .map_err(VisioError::from)?;

        Ok(())
    }

    pub fn report_network_type(&self, network_type: NetworkType) {
        self.room_manager
            .report_context_signal(CoreContextSignal::NetworkType(network_type.into()));
    }

    pub fn report_motion_detected(&self, detected: bool) {
        self.room_manager
            .report_context_signal(CoreContextSignal::MotionDetected(detected));
    }

    pub fn report_bluetooth_car_kit(&self, connected: bool) {
        self.room_manager
            .report_context_signal(CoreContextSignal::BluetoothCarKit(connected));
    }

    pub fn adaptive_mode(&self) -> AdaptiveMode {
        self.room_manager.adaptive_mode().into()
    }

    pub fn set_adaptive_mode_override(&self, mode: Option<AdaptiveMode>) {
        self.room_manager
            .set_adaptive_mode_override(mode.map(CoreAdaptiveMode::from));
    }

    pub fn start_video_renderer(&self, track_sid: String) {
        let is_screencast = self.rt.block_on(self.room_manager.is_track_screencast(&track_sid));
        let track = self
            .rt
            .block_on(self.room_manager.get_video_track(&track_sid));
        if let Some(video_track) = track {
            visio_log(&format!(
                "VISIO FFI: starting video renderer for {track_sid} screencast={is_screencast}"
            ));
            visio_video::start_track_renderer(
                track_sid,
                video_track,
                std::ptr::null_mut(),
                Some(self.rt.handle().clone()),
                is_screencast,
            );
        } else {
            visio_log(&format!("VISIO FFI: no video track found for {track_sid}"));
        }
    }

    pub fn stop_video_renderer(&self, track_sid: String) {
        visio_log(&format!(
            "VISIO FFI: stopping video renderer for {track_sid}"
        ));
        visio_video::stop_track_renderer(&track_sid);
    }

    pub fn set_background_mode(&self, mode: String) {
        // 1. Persist in settings
        self.settings.set_background_mode(mode.clone());

        // 2. Update BlurProcessor mode
        let bg_mode = match mode.as_str() {
            "blur" => blur::process::BackgroundMode::Blur,
            m if m.starts_with("image:") => {
                if let Ok(id) = m[6..].parse::<u8>() {
                    blur::process::BackgroundMode::Image(id)
                } else {
                    blur::process::BackgroundMode::Off
                }
            }
            _ => blur::process::BackgroundMode::Off,
        };
        blur::BlurProcessor::set_mode(bg_mode);
    }

    pub fn get_background_mode(&self) -> String {
        self.settings.get_background_mode()
    }

    pub fn load_background_image(&self, id: u8, jpeg_path: String) -> Result<(), VisioError> {
        let jpeg_bytes = std::fs::read(&jpeg_path).map_err(|e| VisioError::Generic {
            msg: format!("Failed to read image: {e}"),
        })?;
        // Use 640x480 as default target — will be re-loaded at actual frame dimensions if needed
        blur::BlurProcessor::load_replacement_image(id, &jpeg_bytes, 640, 480)
            .map_err(|e| VisioError::Generic { msg: e })
    }

    pub fn load_blur_model(&self, model_path: String) -> Result<(), VisioError> {
        blur::model::load_model(std::path::Path::new(&model_path))
            .map_err(|e| VisioError::Generic { msg: e })
    }
}

// ── Global camera video source (for Android Camera2 → Rust pipeline) ─

#[cfg(target_os = "android")]
use livekit::webrtc::audio_source::native::NativeAudioSource;
#[cfg(target_os = "android")]
use livekit::webrtc::prelude::*;
#[cfg(target_os = "android")]
use livekit::webrtc::video_source::native::NativeVideoSource;

/// Stores the AudioPlayoutBuffer from RoomManager so the Android AudioPlayout
/// Kotlin class can pull decoded remote audio via JNI.
#[cfg(target_os = "android")]
static PLAYOUT_BUFFER: StdMutex<Option<Arc<visio_core::AudioPlayoutBuffer>>> = StdMutex::new(None);

/// Global VisioClient pointer (as usize) for JNI video attach/detach.
/// Set in `connect()` so the JNI attachSurface can look up video tracks.
#[cfg(target_os = "android")]
static CLIENT_FOR_VIDEO: StdMutex<usize> = StdMutex::new(0);

/// Stores the NativeVideoSource after `set_camera_enabled(true)` publishes
/// the camera track. The Android CameraCapture Kotlin class pushes YUV frames
/// into this source via JNI → `visio_push_camera_frame()`.
#[cfg(target_os = "android")]
static CAMERA_SOURCE: StdMutex<Option<NativeVideoSource>> = StdMutex::new(None);

/// RAII wrapper around `ANativeWindow*` that calls `ANativeWindow_release` on drop.
///
/// Prevents leaks and double-frees on error paths in JNI surface management.
#[cfg(target_os = "android")]
struct NativeWindowHandle(*mut ndk_sys::ANativeWindow);

#[cfg(target_os = "android")]
impl NativeWindowHandle {
    /// Wrap a non-null `ANativeWindow*` obtained from `ANativeWindow_fromSurface`.
    ///
    /// # Safety
    /// `ptr` must be a valid, non-null `ANativeWindow*` with an outstanding reference.
    unsafe fn from_raw(ptr: *mut ndk_sys::ANativeWindow) -> Self {
        debug_assert!(!ptr.is_null());
        Self(ptr)
    }

    /// Return the raw pointer.
    fn as_ptr(&self) -> *mut ndk_sys::ANativeWindow {
        self.0
    }

    /// Consume `self` without calling `ANativeWindow_release`, transferring
    /// ownership to the caller (e.g. into `start_track_renderer`).
    fn into_raw(self) -> *mut ndk_sys::ANativeWindow {
        let ptr = self.0;
        std::mem::forget(self);
        ptr
    }
}

#[cfg(target_os = "android")]
impl Drop for NativeWindowHandle {
    fn drop(&mut self) {
        unsafe { ndk_sys::ANativeWindow_release(self.0) };
    }
}

// SAFETY: ANativeWindow is thread-safe per Android documentation.
// All ANativeWindow functions are safe to call from any thread.
#[cfg(target_os = "android")]
unsafe impl Send for NativeWindowHandle {}

/// Stores the ANativeWindow for local camera self-view.
/// Set when VideoSurfaceView attaches with track_sid "local-camera".
/// The nativePushCameraFrame JNI renders I420 frames directly to this surface.
#[cfg(target_os = "android")]
static LOCAL_PREVIEW_SURFACE: StdMutex<Option<NativeWindowHandle>> = StdMutex::new(None);

/// Stores the NativeAudioSource after `set_microphone_enabled(true)` publishes
/// the audio track. The Android AudioCapture Kotlin class pushes PCM frames
/// into this source via JNI → `nativePushAudioFrame()`.
#[cfg(target_os = "android")]
static AUDIO_SOURCE: StdMutex<Option<NativeAudioSource>> = StdMutex::new(None);

/// Pending surfaces: track SID → native surface pointer.
/// When attachSurface is called before the track is in the registry,
/// the surface is stored here and attached when TrackSubscribed arrives.
#[cfg(target_os = "android")]
mod pending {
    use std::collections::HashMap;
    use std::sync::Mutex;

    struct RawSurface(*mut std::ffi::c_void);
    unsafe impl Send for RawSurface {}

    static SURFACES: std::sync::LazyLock<Mutex<HashMap<String, RawSurface>>> =
        std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

    pub fn store(track_sid: String, surface: *mut std::ffi::c_void) {
        if let Ok(mut map) = SURFACES.lock() {
            map.insert(track_sid, RawSurface(surface));
        }
    }

    pub fn take(track_sid: &str) -> Option<*mut std::ffi::c_void> {
        if let Ok(mut map) = SURFACES.lock() {
            map.remove(track_sid).map(|rs| rs.0)
        } else {
            None
        }
    }

    /// Remove a specific pending surface, releasing its ANativeWindow.
    /// Call when a track is unsubscribed or detached before it was claimed.
    pub fn remove(track_sid: &str) {
        if let Ok(mut map) = SURFACES.lock() {
            if let Some(rs) = map.remove(track_sid) {
                unsafe { ndk_sys::ANativeWindow_release(rs.0 as *mut _) };
            }
        }
    }

    /// Drain all pending surfaces, releasing the underlying ANativeWindows.
    /// Call on disconnect to avoid leaking native resources.
    pub fn drain() {
        if let Ok(mut map) = SURFACES.lock() {
            for (_sid, rs) in map.drain() {
                unsafe { ndk_sys::ANativeWindow_release(rs.0 as *mut _) };
            }
        }
    }
}

/// Dedicated tokio runtime for async audio capture_frame calls.
/// Used by both Android JNI and iOS C FFI audio capture, which run on
/// non-tokio threads (AudioCapture thread / Swift DispatchQueue).
#[cfg(any(target_os = "android", target_os = "ios"))]
static AUDIO_RT: std::sync::OnceLock<tokio::runtime::Runtime> = std::sync::OnceLock::new();

#[cfg(any(target_os = "android", target_os = "ios"))]
fn audio_runtime() -> &'static tokio::runtime::Runtime {
    AUDIO_RT.get_or_init(|| {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("failed to create audio runtime")
    })
}

/// Receive a YUV_420_888 frame from the Android Camera2 pipeline and feed it
/// into the LiveKit NativeVideoSource.
///
/// Called from Kotlin via JNI on the ImageReader callback thread.
/// ByteBuffer parameters are direct buffers from `Image.Plane.getBuffer()`.
///
/// # Safety
/// - `env` must be a valid JNI environment pointer.
/// - `y_buf`, `u_buf`, `v_buf` must be valid direct ByteBuffer jobjects.
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_io_visio_mobile_NativeVideo_nativePushCameraFrame(
    env: *mut jni::sys::JNIEnv,
    _class: jni::sys::jobject,
    y_buf: jni::sys::jobject,
    u_buf: jni::sys::jobject,
    v_buf: jni::sys::jobject,
    y_stride: jni::sys::jint,
    u_stride: jni::sys::jint,
    v_stride: jni::sys::jint,
    u_pixel_stride: jni::sys::jint,
    v_pixel_stride: jni::sys::jint,
    width: jni::sys::jint,
    height: jni::sys::jint,
    rotation_degrees: jni::sys::jint,
) {
    let guard = CAMERA_SOURCE.lock().unwrap();
    let Some(source) = guard.as_ref() else {
        visio_log("VISIO FFI: CAMERA_SOURCE is None — discarding frame");
        return;
    };

    // Get direct buffer addresses from ByteBuffer objects
    let Ok(jni_env) = (unsafe { jni::JNIEnv::from_raw(env) }) else {
        return;
    };

    let y_ptr =
        unsafe { jni_env.get_direct_buffer_address(&jni::objects::JByteBuffer::from_raw(y_buf)) };
    let u_ptr =
        unsafe { jni_env.get_direct_buffer_address(&jni::objects::JByteBuffer::from_raw(u_buf)) };
    let v_ptr =
        unsafe { jni_env.get_direct_buffer_address(&jni::objects::JByteBuffer::from_raw(v_buf)) };

    let (Ok(y_ptr), Ok(u_ptr), Ok(v_ptr)) = (y_ptr, u_ptr, v_ptr) else {
        visio_log("VISIO FFI: failed to get direct buffer addresses from ByteBuffers");
        return;
    };

    let w = width as u32;
    let h = height as u32;
    let ys = y_stride as usize;
    let us = u_stride as usize;
    let vs = v_stride as usize;
    let ups = u_pixel_stride as usize;
    let vps = v_pixel_stride as usize;
    let wu = w as usize;
    let hu = h as usize;
    let chroma_h = hu / 2;
    let chroma_w = wu / 2;

    let mut i420 = I420Buffer::new(w, h);
    let strides = i420.strides();
    let (y_dst, u_dst, v_dst) = i420.data_mut();

    // Copy Y plane row-by-row (Y always has pixelStride=1)
    for row in 0..hu {
        let src = unsafe { std::slice::from_raw_parts(y_ptr.add(row * ys), wu) };
        let dst_start = row * strides.0 as usize;
        y_dst[dst_start..dst_start + wu].copy_from_slice(src);
    }

    // Copy U plane — handle pixelStride (1 = planar I420, 2 = semi-planar NV12)
    for row in 0..chroma_h {
        let row_base = unsafe { u_ptr.add(row * us) };
        let dst_start = row * strides.1 as usize;
        if ups == 1 {
            let src = unsafe { std::slice::from_raw_parts(row_base, chroma_w) };
            u_dst[dst_start..dst_start + chroma_w].copy_from_slice(src);
        } else {
            for col in 0..chroma_w {
                u_dst[dst_start + col] = unsafe { *row_base.add(col * ups) };
            }
        }
    }

    // Copy V plane — same pixel stride handling
    for row in 0..chroma_h {
        let row_base = unsafe { v_ptr.add(row * vs) };
        let dst_start = row * strides.2 as usize;
        if vps == 1 {
            let src = unsafe { std::slice::from_raw_parts(row_base, chroma_w) };
            v_dst[dst_start..dst_start + chroma_w].copy_from_slice(src);
        } else {
            for col in 0..chroma_w {
                v_dst[dst_start + col] = unsafe { *row_base.add(col * vps) };
            }
        }
    }

    // Apply background processing (blur/replacement) if enabled
    {
        let strides = i420.strides();
        let (y_data, u_data, v_data) = i420.data_mut();
        blur::BlurProcessor::process_i420(
            y_data,
            u_data,
            v_data,
            w as usize,
            h as usize,
            strides.0 as usize,
            strides.1 as usize,
            strides.2 as usize,
            rotation_degrees as u32,
        );
    }

    let rotation = match rotation_degrees {
        90 => VideoRotation::VideoRotation90,
        180 => VideoRotation::VideoRotation180,
        270 => VideoRotation::VideoRotation270,
        _ => VideoRotation::VideoRotation0,
    };

    // Render to local preview surface (self-view) BEFORE moving i420 into VideoFrame.
    // The guard MUST be kept alive during rendering so that detachSurface cannot
    // release the ANativeWindow while we are writing to it (prevents SIGSEGV).
    {
        let guard = LOCAL_PREVIEW_SURFACE.lock().unwrap();
        if let Some(ref handle) = *guard {
            visio_video::render_i420_to_surface(
                &i420,
                handle.as_ptr() as *mut std::ffi::c_void,
                rotation_degrees as u32,
                true, // mirror for front-camera self-view
            );
        }
        drop(guard);
    }

    let frame = VideoFrame {
        rotation,
        timestamp_us: 0,
        buffer: i420,
    };
    source.capture_frame(&frame);
    drop(guard);

    // Prevent Drop from calling DestroyJavaVM
    std::mem::forget(jni_env);
}

/// Clear the global camera source (called when camera is disabled).
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "C" fn Java_io_visio_mobile_NativeVideo_nativeStopCameraCapture(
    _env: *mut jni::sys::JNIEnv,
    _class: jni::sys::jobject,
) {
    visio_log("VISIO FFI: nativeStopCameraCapture — clearing camera source");
    let mut guard = CAMERA_SOURCE.lock().unwrap();
    *guard = None;
}

// ── JNI: audio capture pipeline ──────────────────────────────────────

/// Receive a PCM audio frame from Android AudioRecord and feed it into
/// the LiveKit NativeAudioSource.
///
/// Called from Kotlin via JNI on the AudioCapture recording thread.
/// `data` is a direct ByteBuffer containing 16-bit signed PCM samples.
///
/// # Safety
/// - `env` must be a valid JNI environment pointer.
/// - `data_buf` must be a valid direct ByteBuffer jobject.
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_io_visio_mobile_NativeVideo_nativePushAudioFrame(
    env: *mut jni::sys::JNIEnv,
    _class: jni::sys::jobject,
    data_buf: jni::sys::jobject,
    num_samples: jni::sys::jint,
    sample_rate: jni::sys::jint,
    num_channels: jni::sys::jint,
) {
    let guard = AUDIO_SOURCE.lock().unwrap();
    let Some(source) = guard.as_ref() else {
        return;
    };
    let source = source.clone();
    drop(guard);

    let Ok(jni_env) = (unsafe { jni::JNIEnv::from_raw(env) }) else {
        return;
    };
    let ptr = unsafe {
        jni_env.get_direct_buffer_address(&jni::objects::JByteBuffer::from_raw(data_buf))
    };
    let Ok(ptr) = ptr else {
        return;
    };

    let sample_count = num_samples as usize;
    let pcm_data = unsafe { std::slice::from_raw_parts(ptr as *const i16, sample_count) };

    let frame = AudioFrame {
        data: pcm_data.into(),
        sample_rate: sample_rate as u32,
        num_channels: num_channels as u32,
        samples_per_channel: sample_count as u32 / num_channels as u32,
    };

    // capture_frame is async — run on dedicated single-thread runtime
    let _ = audio_runtime().block_on(source.capture_frame(&frame));

    std::mem::forget(jni_env);
}

/// Clear the global audio source (called when mic capture stops).
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "C" fn Java_io_visio_mobile_NativeVideo_nativeStopAudioCapture(
    _env: *mut jni::sys::JNIEnv,
    _class: jni::sys::jobject,
) {
    visio_log("VISIO FFI: nativeStopAudioCapture — clearing audio source");
    let mut guard = AUDIO_SOURCE.lock().unwrap();
    *guard = None;
}

// ── JNI: audio playout pipeline (remote audio → speakers) ───────────

/// Pull decoded remote audio samples from the playout buffer.
///
/// Called from Kotlin's AudioPlayout polling thread. Fills the provided
/// ShortArray with PCM samples. Returns the number of samples actually
/// available (rest is filled with silence).
///
/// # Safety
/// - `env` must be a valid JNI environment pointer.
/// - `buffer` must be a valid jshortArray.
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_io_visio_mobile_NativeVideo_nativePullAudioPlayback(
    env: *mut jni::sys::JNIEnv,
    _class: jni::sys::jobject,
    buffer: jni::sys::jshortArray,
) -> jni::sys::jint {
    let guard = PLAYOUT_BUFFER.lock().unwrap();
    let Some(playout) = guard.as_ref() else {
        return 0;
    };
    let playout = playout.clone();
    drop(guard);

    let Ok(mut jni_env) = (unsafe { jni::JNIEnv::from_raw(env) }) else {
        return 0;
    };

    let len = jni_env
        .get_array_length(&unsafe { jni::objects::JShortArray::from_raw(buffer) })
        .unwrap_or(0) as usize;
    if len == 0 {
        std::mem::forget(jni_env);
        return 0;
    }

    let mut tmp = vec![0i16; len];
    let pulled = playout.pull_samples(&mut tmp) as jni::sys::jint;

    let _ = jni_env.set_short_array_region(
        &unsafe { jni::objects::JShortArray::from_raw(buffer) },
        0,
        &tmp,
    );

    std::mem::forget(jni_env);
    pulled
}

// ── iOS: statics for audio playout + camera capture ──────────────────

/// Stores the AudioPlayoutBuffer from RoomManager so the iOS AudioPlayout
/// Swift class can pull decoded remote audio via C FFI.
#[cfg(target_os = "ios")]
static PLAYOUT_BUFFER_IOS: StdMutex<Option<Arc<visio_core::AudioPlayoutBuffer>>> =
    StdMutex::new(None);

/// Stores the NativeAudioSource after `set_microphone_enabled(true)` publishes
/// the audio track. The iOS SyntheticAudioCapture pushes PCM frames
/// into this source via C FFI → `visio_push_ios_audio_frame()`.
#[cfg(target_os = "ios")]
static AUDIO_SOURCE_IOS: StdMutex<
    Option<livekit::webrtc::audio_source::native::NativeAudioSource>,
> = StdMutex::new(None);

/// Stores the NativeVideoSource after `set_camera_enabled(true)` publishes
/// the camera track. The iOS CameraCapture Swift class pushes I420 frames
/// into this source via C FFI → `visio_push_ios_camera_frame()`.
#[cfg(target_os = "ios")]
static CAMERA_SOURCE_IOS: StdMutex<
    Option<livekit::webrtc::video_source::native::NativeVideoSource>,
> = StdMutex::new(None);

/// Pull decoded remote audio samples from the playout buffer.
///
/// Called from Swift's AVAudioSourceNode render callback. Fills the provided
/// buffer with PCM i16 samples. Returns the number of samples actually
/// available (rest is filled with silence by AudioPlayoutBuffer::pull_samples).
///
/// # Safety
/// - `buffer` must point to a valid i16 array of at least `capacity` elements.
#[cfg(target_os = "ios")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn visio_pull_audio_playback(buffer: *mut i16, capacity: u32) -> i32 {
    let guard = PLAYOUT_BUFFER_IOS.lock().unwrap();
    let Some(playout) = guard.as_ref() else {
        return 0;
    };
    let playout = playout.clone();
    drop(guard);

    let out = unsafe { std::slice::from_raw_parts_mut(buffer, capacity as usize) };
    playout.pull_samples(out) as i32
}

/// Push a PCM audio frame from iOS into the LiveKit NativeAudioSource.
///
/// Used by SyntheticAudioCapture to push generated sine wave frames.
///
/// # Safety
/// - `data` must point to a valid i16 array of at least `num_samples` elements.
#[cfg(target_os = "ios")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn visio_push_ios_audio_frame(
    data: *const i16,
    num_samples: u32,
    sample_rate: u32,
    num_channels: u32,
) {
    let source = {
        let guard = AUDIO_SOURCE_IOS.lock().unwrap();
        match guard.as_ref() {
            Some(s) => s.clone(),
            None => return,
        }
    };

    let samples = unsafe { std::slice::from_raw_parts(data, num_samples as usize) };
    let frame = livekit::webrtc::prelude::AudioFrame {
        data: samples.to_vec().into(),
        sample_rate,
        num_channels,
        samples_per_channel: num_samples / num_channels,
    };

    // capture_frame is async — run on dedicated single-thread runtime
    // (same pattern as Android JNI audio capture)
    let _ = audio_runtime().block_on(source.capture_frame(&frame));
}

/// Push an I420 video frame from the iOS camera into the LiveKit NativeVideoSource.
///
/// # Safety
/// All pointers must be valid for the given dimensions and strides.
#[cfg(target_os = "ios")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn visio_push_ios_camera_frame(
    y_ptr: *const u8,
    y_stride: u32,
    u_ptr: *const u8,
    u_stride: u32,
    v_ptr: *const u8,
    v_stride: u32,
    width: u32,
    height: u32,
) {
    use livekit::webrtc::prelude::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    static IOS_FRAME_COUNT: AtomicU64 = AtomicU64::new(0);

    // Clone source (may be None before connect() finishes).
    let source = {
        let guard = CAMERA_SOURCE_IOS.lock().unwrap();
        guard.as_ref().cloned()
    };

    let n = IOS_FRAME_COUNT.fetch_add(1, Ordering::Relaxed);
    if n % 30 == 0 {
        visio_log(&format!(
            "visio_push_ios_camera_frame: pushing frame #{} ({}x{})",
            n, width, height
        ));
    }

    let mut i420 = I420Buffer::new(width, height);
    let strides = i420.strides();
    let (y_dst, u_dst, v_dst) = i420.data_mut();
    let w = width as usize;
    let h = height as usize;
    let chroma_h = h / 2;
    let chroma_w = w / 2;

    // Copy Y plane
    for row in 0..h {
        let src = unsafe { std::slice::from_raw_parts(y_ptr.add(row * y_stride as usize), w) };
        let dst_start = row * strides.0 as usize;
        y_dst[dst_start..dst_start + w].copy_from_slice(src);
    }
    // Copy U plane
    for row in 0..chroma_h {
        let src =
            unsafe { std::slice::from_raw_parts(u_ptr.add(row * u_stride as usize), chroma_w) };
        let dst_start = row * strides.1 as usize;
        u_dst[dst_start..dst_start + chroma_w].copy_from_slice(src);
    }
    // Copy V plane
    for row in 0..chroma_h {
        let src =
            unsafe { std::slice::from_raw_parts(v_ptr.add(row * v_stride as usize), chroma_w) };
        let dst_start = row * strides.2 as usize;
        v_dst[dst_start..dst_start + chroma_w].copy_from_slice(src);
    }

    // Apply background processing (blur/replacement) if enabled
    {
        let strides = i420.strides();
        let (y_data, u_data, v_data) = i420.data_mut();
        blur::BlurProcessor::process_i420(
            y_data,
            u_data,
            v_data,
            width as usize,
            height as usize,
            strides.0 as usize,
            strides.1 as usize,
            strides.2 as usize,
            0, // iOS frames are pre-rotated by AVCaptureConnection
        );
    }

    // Deliver post-blur frame to local preview.
    {
        let strides = i420.strides();
        let (y_data, u_data, v_data) = i420.data();
        visio_video::deliver_i420_to_ios_callback(
            width,
            height,
            y_data.as_ptr(),
            strides.0,
            u_data.as_ptr(),
            strides.1,
            v_data.as_ptr(),
            strides.2,
            "local-camera",
        );
    }

    // Publish to WebRTC if connected.
    if let Some(source) = source {
        let frame = VideoFrame {
            rotation: VideoRotation::VideoRotation0,
            timestamp_us: 0,
            buffer: i420,
        };
        source.capture_frame(&frame);
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
    let is_screencast = client.rt.block_on(client.room_manager.is_track_screencast(&sid_str));
    let track = client
        .rt
        .block_on(client.room_manager.get_video_track(&sid_str));
    match track {
        Some(video_track) => {
            visio_video::start_track_renderer(
                sid_str,
                video_track,
                surface,
                Some(client.rt.handle().clone()),
                is_screencast,
            );
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
pub unsafe extern "C" fn visio_detach_video_surface(track_sid: *const std::ffi::c_char) -> i32 {
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

// ── JNI: video surface attach/detach for Android ────────────────────

/// JNI: NativeVideo.attachSurface(trackSid: String, surface: Surface)
/// Gets the ANativeWindow from the Java Surface, looks up the video track
/// from the stored VisioClient, and starts the renderer.
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_io_visio_mobile_NativeVideo_attachSurface(
    env: *mut jni::sys::JNIEnv,
    _class: jni::sys::jobject,
    track_sid_jstr: jni::sys::jstring,
    surface_obj: jni::sys::jobject,
) {
    use jni::objects::{JObject, JString};

    let mut jni_env = match unsafe { jni::JNIEnv::from_raw(env) } {
        Ok(e) => e,
        Err(_) => return,
    };

    // Extract track_sid String
    let jstr = unsafe { JString::from_raw(track_sid_jstr) };
    let track_sid: String = match jni_env.get_string(&jstr) {
        Ok(s) => s.into(),
        Err(_) => return,
    };

    // Get ANativeWindow from Surface
    let surface = unsafe { JObject::from_raw(surface_obj) };
    let native_window =
        unsafe { ndk_sys::ANativeWindow_fromSurface(env as *mut _, surface.as_raw() as *mut _) };
    if native_window.is_null() {
        visio_log("VISIO JNI: ANativeWindow_fromSurface returned null");
        return;
    }

    visio_log(&format!("VISIO JNI: attachSurface track={track_sid}"));

    // Paint the surface black immediately to replace the uninitialized green
    // TextureView buffer. This runs before any WebRTC negotiation.
    visio_video::paint_surface_black(native_window as *mut std::ffi::c_void);

    // Wrap in RAII handle — Drop calls ANativeWindow_release on early return.
    let window_handle = unsafe { NativeWindowHandle::from_raw(native_window) };

    // Local camera self-view: store the surface for direct rendering
    // in nativePushCameraFrame (bypasses NativeVideoStream which only
    // works with remote tracks).
    if track_sid == "local-camera" {
        visio_log("VISIO JNI: storing local preview surface for self-view");
        *LOCAL_PREVIEW_SURFACE.lock().unwrap() = Some(window_handle);
        return;
    }

    // Remote tracks: look up the subscribed video track and start a renderer.
    let client_addr = *CLIENT_FOR_VIDEO.lock().unwrap();
    if client_addr == 0 {
        visio_log("VISIO JNI: no client pointer stored, cannot attach surface");
        // window_handle is dropped here → ANativeWindow_release called automatically
        return;
    }

    let client = unsafe { &*(client_addr as *const VisioClient) };
    visio_log("VISIO JNI: about to block_on get_video_track");
    let is_screencast = client.rt.block_on(client.room_manager.is_track_screencast(&track_sid));
    let track = client
        .rt
        .block_on(client.room_manager.get_video_track(&track_sid));
    visio_log(&format!(
        "VISIO JNI: block_on done, track found={}, screencast={}",
        track.is_some(), is_screencast
    ));

    match track {
        Some(video_track) => {
            visio_log(&format!(
                "VISIO JNI: calling start_track_renderer for {track_sid}"
            ));
            // Transfer ownership — start_track_renderer/frame_loop holds the surface.
            visio_video::start_track_renderer(
                track_sid.clone(),
                video_track,
                window_handle.into_raw() as *mut std::ffi::c_void,
                Some(client.rt.handle().clone()),
                is_screencast,
            );
            visio_log(&format!(
                "VISIO JNI: start_track_renderer returned for {track_sid}"
            ));
        }
        None => {
            visio_log(&format!(
                "VISIO JNI: track {track_sid} not in registry yet, storing as pending surface"
            ));
            pending::store(track_sid, window_handle.into_raw() as *mut std::ffi::c_void);
        }
    }
}

/// JNI: NativeVideo.detachSurface(trackSid: String)
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_io_visio_mobile_NativeVideo_detachSurface(
    env: *mut jni::sys::JNIEnv,
    _class: jni::sys::jobject,
    track_sid_jstr: jni::sys::jstring,
) {
    use jni::objects::JString;

    let mut jni_env = match unsafe { jni::JNIEnv::from_raw(env) } {
        Ok(e) => e,
        Err(_) => return,
    };

    let jstr = unsafe { JString::from_raw(track_sid_jstr) };
    let track_sid: String = match jni_env.get_string(&jstr) {
        Ok(s) => s.into(),
        Err(_) => return,
    };

    visio_log(&format!("VISIO JNI: detachSurface track={track_sid}"));

    // Local camera self-view: do NOT clear the surface here.
    // On Android, Compose may destroy the old TextureView AFTER creating a new
    // one when recomposing (e.g. returning from chat).  Clearing here would
    // remove the freshly-attached surface, freezing the local video.
    // The old ANativeWindow is released automatically by the RAII wrapper when
    // attachSurface replaces it.  Final cleanup happens in disconnect().
    if track_sid == "local-camera" {
        visio_log(
            "VISIO JNI: detachSurface(local-camera) — skipped (surface replaced on next attach)",
        );
        return;
    }

    // Clean up any pending surface that was never claimed by TrackSubscribed.
    pending::remove(&track_sid);
    visio_video::stop_track_renderer(&track_sid);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_visioclient_new_and_connect_smoke() {
        let dir = std::env::temp_dir().join("visio-test");
        let client = VisioClient::new(dir.to_str().unwrap().to_string());
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
