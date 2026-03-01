//! UniFFI bindings for visio-core.
//!
//! Provides a VisioClient object that wraps RoomManager, MeetingControls,
//! and ChatService into a single FFI-safe interface.

use std::sync::{Arc, Mutex as StdMutex};
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
pub extern "C" fn Java_io_visio_mobile_VisioApplication_nativeInitWebrtc(
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
    #[cfg(target_os = "ios")]
    {
        // Use syslog so messages appear in `xcrun simctl spawn ... log show`
        use std::ffi::CString;
        unsafe extern "C" {
            fn syslog(priority: i32, message: *const std::ffi::c_char, ...);
        }
        let text = CString::new(msg).unwrap_or_else(|_| CString::new("(invalid utf8)").unwrap());
        unsafe { syslog(6 /* LOG_INFO */, text.as_ptr()); }
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
pub struct Settings {
    pub display_name: Option<String>,
    pub language: Option<String>,
    pub mic_enabled_on_join: bool,
    pub camera_enabled_on_join: bool,
    pub theme: String,
}

impl From<visio_core::Settings> for Settings {
    fn from(s: visio_core::Settings) -> Self {
        Self {
            display_name: s.display_name,
            language: s.language,
            mic_enabled_on_join: s.mic_enabled_on_join,
            camera_enabled_on_join: s.camera_enabled_on_join,
            theme: s.theme,
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
    HandRaisedChanged { participant_sid: String, raised: bool, position: u32 },
    UnreadCountChanged { count: u32 },
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
            CoreVisioEvent::HandRaisedChanged { participant_sid, raised, position } => {
                Self::HandRaisedChanged { participant_sid, raised, position }
            }
            CoreVisioEvent::UnreadCountChanged(count) => {
                Self::UnreadCountChanged { count }
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
    settings: visio_core::SettingsStore,
    rt: tokio::runtime::Runtime,
}

impl VisioClient {
    pub fn new(data_dir: String) -> Self {
        visio_log("VISIO FFI: VisioClient::new() called");
        let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
        visio_log("VISIO FFI: tokio runtime created successfully");
        let settings = visio_core::SettingsStore::new(&data_dir);
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

        visio_log("VISIO FFI: VisioClient::new() completed");
        Self {
            room_manager,
            controls,
            chat,
            settings,
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
            Ok(Ok(())) => {
                // Store self pointer for JNI video attach/detach
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
                .map_err(VisioError::from)?;

            #[cfg(target_os = "android")]
            {
                let mut guard = AUDIO_SOURCE.lock().unwrap();
                if enabled {
                    if let Some(source) = self.controls.audio_source().await {
                        visio_log("VISIO FFI: audio source stored for JNI pipeline");
                        *guard = Some(source);
                    }
                } else {
                    visio_log("VISIO FFI: audio source cleared");
                    *guard = None;
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

    pub fn raise_hand(&self) -> Result<(), VisioError> {
        self.rt.block_on(self.room_manager.raise_hand())
            .map_err(VisioError::from)
    }

    pub fn lower_hand(&self) -> Result<(), VisioError> {
        self.rt.block_on(self.room_manager.lower_hand())
            .map_err(VisioError::from)
    }

    pub fn is_hand_raised(&self) -> bool {
        self.rt.block_on(self.room_manager.is_hand_raised())
    }

    pub fn set_chat_open(&self, open: bool) {
        self.chat.set_chat_open(open);
    }

    pub fn unread_count(&self) -> u32 {
        self.chat.unread_count()
    }

    pub fn validate_room(&self, url: String, username: Option<String>) -> RoomValidationResult {
        if let Err(e) = visio_core::AuthService::extract_slug(&url) {
            return RoomValidationResult::InvalidFormat { message: e.to_string() };
        }
        match self.rt.block_on(visio_core::AuthService::validate_room(&url, username.as_deref())) {
            Ok(token_info) => RoomValidationResult::Valid {
                livekit_url: token_info.livekit_url,
                token: token_info.token,
            },
            Err(visio_core::VisioError::Auth(msg)) if msg.contains("404") => {
                RoomValidationResult::NotFound
            }
            Err(e) => RoomValidationResult::NetworkError { message: e.to_string() },
        }
    }

    pub fn start_video_renderer(&self, track_sid: String) {
        let track = self.rt.block_on(self.room_manager.get_video_track(&track_sid));
        if let Some(video_track) = track {
            visio_log(&format!("VISIO FFI: starting video renderer for {track_sid}"));
            visio_video::start_track_renderer(track_sid, video_track, std::ptr::null_mut(), Some(self.rt.handle().clone()));
        } else {
            visio_log(&format!("VISIO FFI: no video track found for {track_sid}"));
        }
    }

    pub fn stop_video_renderer(&self, track_sid: String) {
        visio_log(&format!("VISIO FFI: stopping video renderer for {track_sid}"));
        visio_video::stop_track_renderer(&track_sid);
    }
}

// ── Global camera video source (for Android Camera2 → Rust pipeline) ─

#[cfg(target_os = "android")]
use livekit::webrtc::prelude::*;
#[cfg(target_os = "android")]
use livekit::webrtc::video_source::native::NativeVideoSource;
#[cfg(target_os = "android")]
use livekit::webrtc::audio_source::native::NativeAudioSource;

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

/// Stores the ANativeWindow* for local camera self-view (as usize for Send).
/// Set when VideoSurfaceView attaches with track_sid "local-camera".
/// The nativePushCameraFrame JNI renders I420 frames directly to this surface.
#[cfg(target_os = "android")]
static LOCAL_PREVIEW_SURFACE: StdMutex<usize> = StdMutex::new(0);

/// Stores the NativeAudioSource after `set_microphone_enabled(true)` publishes
/// the audio track. The Android AudioCapture Kotlin class pushes PCM frames
/// into this source via JNI → `nativePushAudioFrame()`.
#[cfg(target_os = "android")]
static AUDIO_SOURCE: StdMutex<Option<NativeAudioSource>> = StdMutex::new(None);

/// Dedicated tokio runtime for async audio capture_frame calls.
#[cfg(target_os = "android")]
static AUDIO_RT: std::sync::OnceLock<tokio::runtime::Runtime> = std::sync::OnceLock::new();

#[cfg(target_os = "android")]
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
    let jni_env = unsafe { jni::JNIEnv::from_raw(env) }.expect("invalid JNIEnv");

    let y_ptr = unsafe {
        jni_env.get_direct_buffer_address(&jni::objects::JByteBuffer::from_raw(y_buf))
    };
    let u_ptr = unsafe {
        jni_env.get_direct_buffer_address(&jni::objects::JByteBuffer::from_raw(u_buf))
    };
    let v_ptr = unsafe {
        jni_env.get_direct_buffer_address(&jni::objects::JByteBuffer::from_raw(v_buf))
    };

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
        let preview_addr = *guard;
        if preview_addr != 0 {
            visio_video::render_i420_to_surface(
                &i420,
                preview_addr as *mut std::ffi::c_void,
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

    let jni_env = unsafe { jni::JNIEnv::from_raw(env) }.expect("invalid JNIEnv");
    let ptr = unsafe {
        jni_env.get_direct_buffer_address(&jni::objects::JByteBuffer::from_raw(data_buf))
    };
    let Ok(ptr) = ptr else { return; };

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

    let mut jni_env = unsafe { jni::JNIEnv::from_raw(env) }.expect("invalid JNIEnv");

    let len = jni_env.get_array_length(&unsafe { jni::objects::JShortArray::from_raw(buffer) })
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
static PLAYOUT_BUFFER_IOS: StdMutex<Option<Arc<visio_core::AudioPlayoutBuffer>>> = StdMutex::new(None);

/// Stores the NativeVideoSource after `set_camera_enabled(true)` publishes
/// the camera track. The iOS CameraCapture Swift class pushes I420 frames
/// into this source via C FFI → `visio_push_ios_camera_frame()`.
#[cfg(target_os = "ios")]
static CAMERA_SOURCE_IOS: StdMutex<Option<livekit::webrtc::video_source::native::NativeVideoSource>> = StdMutex::new(None);

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
    let Some(playout) = guard.as_ref() else { return 0 };
    let playout = playout.clone();
    drop(guard);

    let out = unsafe { std::slice::from_raw_parts_mut(buffer, capacity as usize) };
    playout.pull_samples(out) as i32
}

/// Push an I420 video frame from the iOS camera into the LiveKit NativeVideoSource.
///
/// # Safety
/// All pointers must be valid for the given dimensions and strides.
#[cfg(target_os = "ios")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn visio_push_ios_camera_frame(
    y_ptr: *const u8, y_stride: u32,
    u_ptr: *const u8, u_stride: u32,
    v_ptr: *const u8, v_stride: u32,
    width: u32, height: u32,
) {
    use livekit::webrtc::prelude::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    static IOS_FRAME_COUNT: AtomicU64 = AtomicU64::new(0);

    // Clone source and drop guard immediately (same pattern as visio_pull_audio_playback).
    let source = {
        let guard = CAMERA_SOURCE_IOS.lock().unwrap();
        match guard.as_ref() {
            Some(s) => s.clone(),
            None => {
                let n = IOS_FRAME_COUNT.fetch_add(1, Ordering::Relaxed);
                if n % 30 == 0 {
                    visio_log(&format!("visio_push_ios_camera_frame: no source (frame #{})", n));
                }
                return;
            },
        }
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
        let src = unsafe { std::slice::from_raw_parts(u_ptr.add(row * u_stride as usize), chroma_w) };
        let dst_start = row * strides.1 as usize;
        u_dst[dst_start..dst_start + chroma_w].copy_from_slice(src);
    }
    // Copy V plane
    for row in 0..chroma_h {
        let src = unsafe { std::slice::from_raw_parts(v_ptr.add(row * v_stride as usize), chroma_w) };
        let dst_start = row * strides.2 as usize;
        v_dst[dst_start..dst_start + chroma_w].copy_from_slice(src);
    }

    let frame = VideoFrame {
        rotation: VideoRotation::VideoRotation0,
        timestamp_us: 0,
        buffer: i420,
    };
    source.capture_frame(&frame);
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
            visio_video::start_track_renderer(sid_str, video_track, surface, Some(client.rt.handle().clone()));
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
    let native_window = unsafe {
        ndk_sys::ANativeWindow_fromSurface(env as *mut _, surface.as_raw() as *mut _)
    };
    if native_window.is_null() {
        visio_log("VISIO JNI: ANativeWindow_fromSurface returned null");
        return;
    }

    visio_log(&format!("VISIO JNI: attachSurface track={track_sid}"));

    // Local camera self-view: store the surface for direct rendering
    // in nativePushCameraFrame (bypasses NativeVideoStream which only
    // works with remote tracks).
    if track_sid == "local-camera" {
        visio_log("VISIO JNI: storing local preview surface for self-view");
        *LOCAL_PREVIEW_SURFACE.lock().unwrap() = native_window as usize;
        return;
    }

    // Remote tracks: look up the subscribed video track and start a renderer.
    let client_addr = *CLIENT_FOR_VIDEO.lock().unwrap();
    if client_addr == 0 {
        visio_log("VISIO JNI: no client pointer stored, cannot attach surface");
        unsafe { ndk_sys::ANativeWindow_release(native_window) };
        return;
    }

    let client = unsafe { &*(client_addr as *const VisioClient) };
    visio_log("VISIO JNI: about to block_on get_video_track");
    let track = client
        .rt
        .block_on(client.room_manager.get_video_track(&track_sid));
    visio_log(&format!("VISIO JNI: block_on done, track found={}", track.is_some()));

    match track {
        Some(video_track) => {
            visio_log(&format!("VISIO JNI: calling start_track_renderer for {track_sid}"));
            visio_video::start_track_renderer(
                track_sid.clone(),
                video_track,
                native_window as *mut std::ffi::c_void,
                Some(client.rt.handle().clone()),
            );
            visio_log(&format!("VISIO JNI: start_track_renderer returned for {track_sid}"));
        }
        None => {
            visio_log(&format!("VISIO JNI: no video track found for {track_sid}"));
            unsafe { ndk_sys::ANativeWindow_release(native_window) };
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

    // Local camera self-view: release the stored ANativeWindow.
    if track_sid == "local-camera" {
        let prev = std::mem::replace(&mut *LOCAL_PREVIEW_SURFACE.lock().unwrap(), 0);
        if prev != 0 {
            visio_log("VISIO JNI: releasing local preview surface");
            unsafe { ndk_sys::ANativeWindow_release(prev as *mut ndk_sys::ANativeWindow) };
        }
        return;
    }

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
