use std::sync::{Arc, OnceLock};
use tokio::sync::Mutex;

use tauri::{AppHandle, Emitter, Listener, Manager};
use visio_core::{
    AudioPlayoutBuffer, CalendarService, ChatService, MeetingControls, RoomManager, SessionManager,
    SessionState, SettingsStore, TrackInfo, TrackKind, TrackSource, VisioEvent, VisioEventListener,
};

mod audio_engine;
#[cfg(target_os = "linux")]
mod audio_linux;
#[cfg(target_os = "macos")]
mod audio_macos;
#[cfg(target_os = "windows")]
mod audio_windows;
#[cfg(target_os = "linux")]
mod camera_linux;
#[cfg(target_os = "macos")]
mod camera_macos;
mod noise_reduction;
#[cfg(target_os = "macos")]
mod screen_audio_macos;
mod screen_capture;

// ---------------------------------------------------------------------------
// Global AppHandle for the C video callback
// ---------------------------------------------------------------------------

static APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();

/// C callback invoked by visio-video for each rendered desktop frame.
/// Emits a Tauri "video-frame" event to the frontend.
unsafe extern "C" fn on_desktop_frame(
    track_sid: *const std::ffi::c_char,
    data: *const u8,
    data_len: usize,
    width: u32,
    height: u32,
    _user_data: *mut std::ffi::c_void,
) {
    let Some(app) = APP_HANDLE.get() else { return };
    let sid = unsafe { std::ffi::CStr::from_ptr(track_sid) };
    let Ok(sid_str) = sid.to_str() else { return };
    let b64 = unsafe { std::slice::from_raw_parts(data, data_len) };
    let Ok(b64_str) = std::str::from_utf8(b64) else {
        return;
    };

    let _ = app.emit(
        "video-frame",
        serde_json::json!({
            "track_sid": sid_str,
            "data": b64_str,
            "width": width,
            "height": height,
        }),
    );
}

// ---------------------------------------------------------------------------
// Shared state
// ---------------------------------------------------------------------------

struct VisioState {
    room: Arc<Mutex<RoomManager>>,
    controls: Arc<Mutex<MeetingControls>>,
    chat: Arc<Mutex<ChatService>>,
    session: Mutex<SessionManager>,
    settings: Arc<SettingsStore>,
    calendar: Arc<CalendarService>,
    #[cfg(target_os = "macos")]
    camera_capture: std::sync::Mutex<Option<camera_macos::MacCameraCapture>>,
    #[cfg(target_os = "linux")]
    camera_capture: std::sync::Mutex<Option<camera_linux::LinuxCameraCapture>>,
    audio_engine: std::sync::Mutex<Box<dyn audio_engine::VoiceAudioEngine>>,
    playout_buffer: Arc<AudioPlayoutBuffer>,
    selected_input_device: std::sync::Mutex<Option<String>>,
    selected_output_device: std::sync::Mutex<Option<String>>,
    screen_capture: std::sync::Mutex<Option<screen_capture::ScreenCapture>>,
    #[cfg(target_os = "macos")]
    screen_audio_capture: std::sync::Mutex<Option<screen_audio_macos::ScreenAudioCapture>>,
}

// ---------------------------------------------------------------------------
// Event listener — auto-starts/stops video renderers
// ---------------------------------------------------------------------------

struct DesktopEventListener {
    room: Arc<Mutex<RoomManager>>,
    settings: Arc<SettingsStore>,
}

fn source_to_str(source: &TrackSource) -> &'static str {
    match source {
        TrackSource::Microphone => "microphone",
        TrackSource::Camera => "camera",
        TrackSource::ScreenShare => "screen_share",
        TrackSource::Unknown => "unknown",
    }
}

/// Emit a Tauri event to the frontend if the app handle is available.
fn emit_event<S: serde::Serialize + Clone>(event_name: &str, payload: S) {
    if let Some(app) = APP_HANDLE.get() {
        let _ = app.emit(event_name, payload);
    }
}

fn handle_connection_state_changed(
    state: visio_core::ConnectionState,
    room: &Arc<Mutex<RoomManager>>,
    settings: &Arc<SettingsStore>,
) {
    let name = match &state {
        visio_core::ConnectionState::Disconnected => "disconnected",
        visio_core::ConnectionState::Connecting => "connecting",
        visio_core::ConnectionState::Connected => "connected",
        visio_core::ConnectionState::Reconnecting { .. } => "reconnecting",
        visio_core::ConnectionState::WaitingForHost => "waiting_for_host",
    };
    tracing::info!("connection state changed: {name}");

    // Record room in history on successful connection (covers
    // both direct connect and lobby acceptance paths).
    if matches!(state, visio_core::ConnectionState::Connected) {
        let room = room.clone();
        let settings = settings.clone();
        tokio::spawn(async move {
            if let Some((url, _)) = room.lock().await.last_connection_info().await {
                let display_name = visio_core::extract_room_display_name(&url);
                let clean_url = visio_core::strip_room_display_name_param(&url);
                settings.add_room_to_history(clean_url, display_name);
            }
        });
    }

    emit_event("connection-state-changed", name);
}

fn handle_track_subscribed_video(
    track_sid: &str,
    participant_sid: &str,
    source: &TrackSource,
    room: &Arc<Mutex<RoomManager>>,
) {
    let room = room.clone();
    let sid = track_sid.to_owned();
    let is_screencast = *source == TrackSource::ScreenShare;
    tokio::spawn(async move {
        let rm = room.lock().await;
        if let Some(video_track) = rm.get_video_track(&sid).await {
            tracing::info!(
                "auto-starting video renderer for track {sid} screencast={is_screencast}"
            );
            visio_video::start_track_renderer(
                sid,
                video_track,
                std::ptr::null_mut(),
                None,
                is_screencast,
            );
        }
    });
    emit_event(
        "track-subscribed",
        serde_json::json!({
            "trackSid": track_sid,
            "participantSid": participant_sid,
            "source": source_to_str(source),
        }),
    );
}

fn handle_connection_lost(room: &Arc<Mutex<RoomManager>>) {
    emit_event("connection-lost", ());
    let room = room.clone();
    tokio::spawn(async move {
        let rm = room.lock().await;
        tracing::info!("connection lost, attempting reconnection");
        if let Err(e) = rm.reconnect().await {
            tracing::error!("desktop reconnection failed: {e}");
        }
    });
}

fn handle_meetings_updated(meetings: Vec<visio_core::events::Meeting>) {
    let payload: Vec<serde_json::Value> = meetings
        .iter()
        .map(|m| {
            serde_json::json!({
                "id": m.id,
                "summary": m.summary,
                "start_time": m.start_time,
                "end_time": m.end_time,
                "room_url": m.room_url,
                "deep_link": m.deep_link,
                "server_name": m.server_name,
            })
        })
        .collect();
    emit_event("meetings-updated", payload);
}

fn handle_meeting_reminder(m: visio_core::events::Meeting) {
    emit_event(
        "meeting-reminder",
        serde_json::json!({
            "id": m.id,
            "summary": m.summary,
            "start_time": m.start_time,
            "room_url": m.room_url,
        }),
    );
}

impl VisioEventListener for DesktopEventListener {
    fn on_event(&self, event: VisioEvent) {
        match event {
            VisioEvent::ConnectionStateChanged(state) => {
                handle_connection_state_changed(state, &self.room, &self.settings);
            }
            VisioEvent::ParticipantJoined(info) => {
                tracing::info!("participant joined: {} ({})", info.identity, info.sid);
                emit_event(
                    "participant-joined",
                    serde_json::json!({
                        "sid": info.sid,
                        "identity": info.identity,
                        "name": info.name,
                    }),
                );
            }
            VisioEvent::ParticipantLeft(sid) => {
                tracing::info!("participant left: {sid}");
                emit_event("participant-left", sid);
            }
            VisioEvent::TrackSubscribed(TrackInfo {
                sid: ref track_sid,
                ref participant_sid,
                kind: TrackKind::Video,
                ref source,
                ..
            }) => {
                handle_track_subscribed_video(track_sid, participant_sid, source, &self.room);
            }
            VisioEvent::TrackSubscribed(_) => {}
            VisioEvent::TrackUnsubscribed(track_sid) => {
                tracing::info!("auto-stopping video renderer for track {track_sid}");
                visio_video::stop_track_renderer(&track_sid);
                emit_event("track-unsubscribed", track_sid);
            }
            VisioEvent::TrackMuted {
                participant_sid,
                source,
            } => {
                emit_event(
                    "track-muted",
                    serde_json::json!({
                        "participantSid": participant_sid,
                        "source": source_to_str(&source),
                    }),
                );
            }
            VisioEvent::TrackUnmuted {
                participant_sid,
                source,
            } => {
                emit_event(
                    "track-unmuted",
                    serde_json::json!({
                        "participantSid": participant_sid,
                        "source": source_to_str(&source),
                    }),
                );
            }
            VisioEvent::HandRaisedChanged {
                participant_sid,
                raised,
                position,
            } => {
                tracing::info!(
                    "DesktopEventListener: HandRaisedChanged sid={participant_sid} raised={raised} position={position}"
                );
                emit_event(
                    "hand-raised-changed",
                    serde_json::json!({
                        "participantSid": participant_sid,
                        "raised": raised,
                        "position": position,
                    }),
                );
            }
            VisioEvent::UnreadCountChanged(count) => {
                emit_event("unread-count-changed", count);
            }
            VisioEvent::ActiveSpeakersChanged(sids) => {
                emit_event("active-speakers-changed", sids);
            }
            VisioEvent::ConnectionQualityChanged {
                participant_sid,
                quality,
            } => {
                emit_event(
                    "connection-quality-changed",
                    serde_json::json!({
                        "participantSid": participant_sid,
                        "quality": format!("{:?}", quality),
                    }),
                );
            }
            VisioEvent::ChatMessageReceived(msg) => {
                emit_event(
                    "chat-message-received",
                    serde_json::json!({
                        "id": msg.id,
                        "senderSid": msg.sender_sid,
                        "senderName": msg.sender_name,
                        "text": msg.text,
                        "timestampMs": msg.timestamp_ms,
                    }),
                );
            }
            VisioEvent::LobbyParticipantJoined { id, username } => {
                tracing::info!("lobby participant joined: {username} (id={id})");
                emit_event(
                    "lobby-participant-joined",
                    serde_json::json!({ "id": id, "username": username }),
                );
            }
            VisioEvent::LobbyParticipantLeft { id } => {
                emit_event("lobby-participant-left", id);
            }
            VisioEvent::LobbyDenied => {
                emit_event("lobby-denied", ());
            }
            VisioEvent::LobbyTimeout => {
                emit_event("lobby-timeout", ());
            }
            VisioEvent::ReactionReceived {
                participant_sid,
                participant_name,
                emoji,
            } => {
                emit_event(
                    "reaction-received",
                    serde_json::json!({
                        "participantSid": participant_sid,
                        "participantName": participant_name,
                        "emoji": emoji,
                    }),
                );
            }
            VisioEvent::AdaptiveModeChanged { mode } => {
                let mode_str = match mode {
                    visio_core::adaptive::AdaptiveMode::Office => "office",
                    visio_core::adaptive::AdaptiveMode::Pedestrian => "pedestrian",
                    visio_core::adaptive::AdaptiveMode::Car => "car",
                };
                tracing::info!("adaptive mode changed: {mode_str}");
                emit_event("adaptive-mode-changed", mode_str);
            }
            VisioEvent::BandwidthModeChanged { mode } => {
                let mode_str = match mode {
                    visio_core::bandwidth::BandwidthMode::Full => "full",
                    visio_core::bandwidth::BandwidthMode::ReducedVideo => "reduced_video",
                    visio_core::bandwidth::BandwidthMode::AudioOnly => "audio_only",
                };
                tracing::info!("bandwidth mode changed: {mode_str}");
                emit_event("bandwidth-mode-changed", mode_str);
            }
            VisioEvent::ConnectionLost => {
                handle_connection_lost(&self.room);
            }
            VisioEvent::DisconnectedDuplicateIdentity => {
                tracing::warn!("disconnected: duplicate identity");
                emit_event("disconnected-reason", "duplicate_identity");
            }
            VisioEvent::DisconnectedByAdmin => {
                tracing::warn!("disconnected: removed by admin");
                emit_event("disconnected-reason", "removed_by_admin");
            }
            VisioEvent::AloneInRoom { remaining_secs } => {
                emit_event("alone-in-room", remaining_secs);
            }
            VisioEvent::AloneInRoomCancelled => {
                emit_event("alone-in-room-cancelled", ());
            }
            VisioEvent::MuteRequested => {
                emit_event("mute-requested", ());
            }
            VisioEvent::MeetingsUpdated(meetings) => {
                handle_meetings_updated(meetings);
            }
            VisioEvent::MeetingImminent(m)
            | VisioEvent::MeetingStartingSoon(m)
            | VisioEvent::MeetingStarted(m) => {
                handle_meeting_reminder(m);
            }
            VisioEvent::CalendarError(msg) => {
                tracing::warn!("calendar error: {msg}");
                emit_event("calendar-error", msg);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

#[tauri::command]
async fn validate_room(
    state: tauri::State<'_, VisioState>,
    url: String,
    username: Option<String>,
) -> Result<serde_json::Value, String> {
    if let Err(e) = visio_core::AuthService::extract_slug(&url) {
        return Ok(serde_json::json!({ "status": "invalid_format", "message": e.to_string() }));
    }
    let cookie = {
        let session = state.session.lock().await;
        session.cookie()
    };
    match visio_core::AuthService::validate_room(&url, username.as_deref(), cookie.as_deref()).await
    {
        Ok(token_info) => Ok(serde_json::json!({
            "status": "valid",
            "livekit_url": token_info.livekit_url,
            "token": token_info.token,
        })),
        Err(visio_core::VisioError::AuthRequired) => {
            Ok(serde_json::json!({ "status": "auth_required" }))
        }
        Err(visio_core::VisioError::Auth(msg)) if msg.contains("404") => {
            Ok(serde_json::json!({ "status": "not_found" }))
        }
        Err(e) => Ok(serde_json::json!({ "status": "error", "message": e.to_string() })),
    }
}

#[tauri::command]
async fn connect(
    state: tauri::State<'_, VisioState>,
    meet_url: String,
    username: Option<String>,
) -> Result<(), String> {
    // Start audio playout on connect (deferred from app startup to avoid
    // grabbing the microphone before the user joins a room).
    {
        let mut engine = state.audio_engine.lock().unwrap_or_else(|e| e.into_inner());
        // Stop first for idempotency (no-op if not running).
        engine.stop_playout();
        engine.start_playout(state.playout_buffer.clone())?;
        engine.set_device_change_callback(Arc::new(|| {
            tracing::info!("audio devices changed — re-enumerating");
            if let Some(app) = APP_HANDLE.get() {
                let _ = app.emit("audio-devices-changed", ());
            }
        }));
    }

    let cookie = {
        let session = state.session.lock().await;
        session.cookie()
    };
    let room = state.room.lock().await;
    room.connect(&meet_url, username.as_deref(), cookie.as_deref())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn connect_with_token(
    state: tauri::State<'_, VisioState>,
    livekit_url: String,
    token: String,
) -> Result<(), String> {
    tracing::info!("connect_with_token called: url={}", livekit_url);

    // Start audio playout on connect (deferred from app startup).
    {
        let mut engine = state.audio_engine.lock().unwrap_or_else(|e| e.into_inner());
        // Stop first for idempotency (no-op if not running).
        engine.stop_playout();
        engine.start_playout(state.playout_buffer.clone())?;
        engine.set_device_change_callback(Arc::new(|| {
            tracing::info!("audio devices changed — re-enumerating");
            if let Some(app) = APP_HANDLE.get() {
                let _ = app.emit("audio-devices-changed", ());
            }
        }));
    }

    let room = state.room.lock().await;
    room.connect_with_token(&livekit_url, &token)
        .await
        .map_err(|e| {
            tracing::error!("connect_with_token failed: {}", e);
            e.to_string()
        })?;
    tracing::info!("connect_with_token succeeded");
    Ok(())
}

#[tauri::command]
async fn disconnect(state: tauri::State<'_, VisioState>) -> Result<(), String> {
    // Stop audio engine (capture + playout) so the microphone is released.
    {
        let mut engine = state.audio_engine.lock().unwrap_or_else(|e| e.into_inner());
        engine.stop_capture();
        engine.stop_playout();
    }

    let room = state.room.lock().await;
    room.disconnect().await;
    Ok(())
}

#[tauri::command]
async fn get_connection_state(state: tauri::State<'_, VisioState>) -> Result<String, String> {
    let room = state.room.lock().await;
    let cs = room.connection_state().await;
    let name = match cs {
        visio_core::ConnectionState::Disconnected => "disconnected",
        visio_core::ConnectionState::Connecting => "connecting",
        visio_core::ConnectionState::Connected => "connected",
        visio_core::ConnectionState::Reconnecting { .. } => "reconnecting",
        visio_core::ConnectionState::WaitingForHost => "waiting_for_host",
    };
    Ok(name.to_string())
}

#[tauri::command]
async fn get_participants(
    state: tauri::State<'_, VisioState>,
) -> Result<Vec<serde_json::Value>, String> {
    let room = state.room.lock().await;
    let participants = room.participants().await;
    let result: Vec<serde_json::Value> = participants
        .into_iter()
        .map(|p| {
            serde_json::json!({
                "sid": p.sid,
                "identity": p.identity,
                "name": p.name,
                "is_muted": p.is_muted,
                "has_video": p.has_video,
                "video_track_sid": p.video_track_sid,
                "connection_quality": format!("{:?}", p.connection_quality),
                "has_screen_share": p.has_screen_share,
                "screen_share_track_sid": p.screen_share_track_sid,
                "is_admin": p.is_admin,
            })
        })
        .collect();
    Ok(result)
}

#[tauri::command]
async fn get_local_participant(
    state: tauri::State<'_, VisioState>,
) -> Result<Option<serde_json::Value>, String> {
    let room = state.room.lock().await;
    let info = room.local_participant_info().await;
    Ok(info.map(|p| {
        serde_json::json!({
            "sid": p.sid,
            "identity": p.identity,
            "name": p.name,
            "is_muted": p.is_muted,
            "has_video": p.has_video,
            "video_track_sid": p.video_track_sid,
            "connection_quality": format!("{:?}", p.connection_quality),
            "has_screen_share": p.has_screen_share,
            "screen_share_track_sid": p.screen_share_track_sid,
            "is_admin": p.is_admin,
        })
    }))
}

#[tauri::command]
async fn get_video_tracks(state: tauri::State<'_, VisioState>) -> Result<Vec<String>, String> {
    let room = state.room.lock().await;
    let sids = room.video_track_sids().await;
    Ok(sids)
}

#[tauri::command]
async fn toggle_mic(state: tauri::State<'_, VisioState>, enabled: bool) -> Result<(), String> {
    let controls = state.controls.lock().await;
    controls
        .set_microphone_enabled(enabled)
        .await
        .map_err(|e| e.to_string())?;
    if enabled {
        if let Some(source) = controls.audio_source().await {
            let mut engine = state.audio_engine.lock().unwrap_or_else(|e| e.into_inner());
            // Stop first to ensure idempotency (avoids leaking drain thread
            // if toggle_mic(true) is called twice without an intervening false)
            engine.stop_capture();
            let nr = state.settings.is_noise_reduction_enabled();
            engine
                .start_capture(source, nr)
                .map_err(|e| format!("audio capture: {e}"))?;
        }
    } else {
        let mut engine = state.audio_engine.lock().unwrap_or_else(|e| e.into_inner());
        engine.stop_capture();
    }
    Ok(())
}

#[tauri::command]
async fn toggle_camera(state: tauri::State<'_, VisioState>, enabled: bool) -> Result<(), String> {
    tracing::info!("toggle_camera called with enabled={}", enabled);
    let controls = state.controls.lock().await;
    if enabled {
        // Publish camera track if not yet published, or reuse existing source
        let source = match controls.video_source().await {
            Some(existing) => Some(existing),
            None => {
                let res = state.settings.get_video_resolution();
                let source = controls
                    .publish_camera(&res)
                    .await
                    .map_err(|e| e.to_string())?;
                tracing::info!("camera track published via toggle_camera");
                Some(source)
            }
        };

        // Start native camera capture if not already running
        #[cfg(target_os = "macos")]
        {
            let mut cam = state
                .camera_capture
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            if cam.is_none() {
                if let Some(source) = source {
                    // Must request permission before starting AVCaptureSession,
                    // otherwise startRunning silently produces no frames.
                    if !camera_macos::request_camera_permission() {
                        return Err("Camera permission denied".into());
                    }
                    let capture = camera_macos::MacCameraCapture::start(source)
                        .map_err(|e| format!("camera capture: {e}"))?;
                    *cam = Some(capture);
                    tracing::info!("camera capture (re)started");
                }
            }
        }

        #[cfg(target_os = "linux")]
        {
            tracing::info!("Linux camera: attempting to start capture");
            let mut cam = state
                .camera_capture
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            if cam.is_none() {
                tracing::info!(
                    "Linux camera: no existing capture, source={}",
                    source.is_some()
                );
                if let Some(source) = source {
                    tracing::info!("Linux camera: starting LinuxCameraCapture");
                    let capture = camera_linux::LinuxCameraCapture::start(source).map_err(|e| {
                        tracing::error!("Linux camera capture failed: {}", e);
                        format!("camera capture: {e}")
                    })?;
                    *cam = Some(capture);
                    tracing::info!("Linux camera capture (re)started successfully");
                }
            } else {
                tracing::info!("Linux camera: capture already running");
            }
        }
    } else {
        // Stop camera capture when disabling
        #[cfg(target_os = "macos")]
        {
            let mut cam = state
                .camera_capture
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            if let Some(mut capture) = cam.take() {
                capture.stop();
            }
        }

        #[cfg(target_os = "linux")]
        {
            let mut cam = state
                .camera_capture
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            if let Some(mut capture) = cam.take() {
                capture.stop();
            }
        }
    }
    controls
        .set_camera_enabled(enabled)
        .await
        .map_err(|e| e.to_string())
}

/// Check microphone and camera permissions on macOS.
/// Returns `{ "microphone": "granted"|"denied"|"undetermined", "camera": "granted"|"denied"|"undetermined" }`.
#[tauri::command]
fn check_media_permissions() -> serde_json::Value {
    #[cfg(target_os = "macos")]
    {
        use objc2::msg_send;
        use objc2::runtime::AnyClass;

        fn auth_status(media_type: &std::ffi::CStr) -> &'static str {
            unsafe {
                let cls = match AnyClass::get(c"AVCaptureDevice") {
                    Some(c) => c,
                    None => return "undetermined",
                };
                let nsstring_cls = match AnyClass::get(c"NSString") {
                    Some(c) => c,
                    None => return "undetermined",
                };
                let mt: *const objc2::runtime::AnyObject =
                    msg_send![nsstring_cls, stringWithUTF8String: media_type.as_ptr()];
                let status: i64 = msg_send![cls, authorizationStatusForMediaType: mt];
                // 0 = notDetermined, 1 = restricted, 2 = denied, 3 = authorized
                match status {
                    3 => "granted",
                    2 | 1 => "denied",
                    _ => "undetermined",
                }
            }
        }

        serde_json::json!({
            "microphone": auth_status(c"soun"),
            "camera": auth_status(c"vide"),
        })
    }

    #[cfg(not(target_os = "macos"))]
    {
        // On non-macOS platforms, assume granted (OS handles permissions natively)
        serde_json::json!({
            "microphone": "granted",
            "camera": "granted",
        })
    }
}

#[tauri::command]
async fn send_chat(
    state: tauri::State<'_, VisioState>,
    text: String,
) -> Result<serde_json::Value, String> {
    let chat = state.chat.lock().await;
    let msg = chat.send_message(&text).await.map_err(|e| e.to_string())?;
    Ok(serde_json::json!({
        "id": msg.id,
        "sender_sid": msg.sender_sid,
        "sender_name": msg.sender_name,
        "text": msg.text,
        "timestamp_ms": msg.timestamp_ms,
    }))
}

#[tauri::command]
async fn get_messages(
    state: tauri::State<'_, VisioState>,
) -> Result<Vec<serde_json::Value>, String> {
    let chat = state.chat.lock().await;
    let messages = chat.messages().await;
    let result: Vec<serde_json::Value> = messages
        .into_iter()
        .map(|m| {
            serde_json::json!({
                "id": m.id,
                "sender_sid": m.sender_sid,
                "sender_name": m.sender_name,
                "text": m.text,
                "timestamp_ms": m.timestamp_ms,
            })
        })
        .collect();
    Ok(result)
}

#[tauri::command]
fn get_translations(app: AppHandle, lang: String) -> Result<serde_json::Value, String> {
    let supported = ["en", "fr", "de", "es", "it", "nl"];
    let lang = if supported.contains(&lang.as_str()) {
        lang
    } else {
        "en".to_string()
    };

    let resource_path = app
        .path()
        .resource_dir()
        .map_err(|e| format!("resource dir: {e}"))?
        .join("i18n")
        .join(format!("{lang}.json"));

    let content = std::fs::read_to_string(&resource_path).map_err(|e| {
        tracing::warn!("failed to load i18n/{lang}.json from {resource_path:?}: {e}");
        format!("i18n file not found: {lang}.json")
    })?;

    serde_json::from_str(&content).map_err(|e| format!("invalid i18n JSON: {e}"))
}

#[tauri::command]
fn get_system_language() -> String {
    sys_locale::get_locale()
        .and_then(|l| l.split(['-', '_']).next().map(String::from))
        .unwrap_or_else(|| "en".to_string())
}

#[tauri::command]
fn get_settings(state: tauri::State<'_, VisioState>) -> Result<serde_json::Value, String> {
    let s = state.settings.get();
    Ok(serde_json::json!({
        "display_name": s.display_name,
        "language": s.language,
        "mic_enabled_on_join": s.mic_enabled_on_join,
        "camera_enabled_on_join": s.camera_enabled_on_join,
        "theme": s.theme,
        "adaptive_mode_enabled": s.adaptive_mode_enabled,
        "audio_mode": s.audio_mode,
    }))
}

#[tauri::command]
fn set_display_name(
    app: AppHandle,
    state: tauri::State<'_, VisioState>,
    name: Option<String>,
) -> Result<(), String> {
    if let Some(ref n) = name {
        let trimmed = n.trim();
        if trimmed.is_empty() || trimmed.len() > 100 {
            return Err("display name must be 1-100 characters".into());
        }
    }
    state.settings.set_display_name(name.clone());
    let _ = app.emit(
        "settings-changed",
        serde_json::json!({"display_name": name}),
    );
    Ok(())
}

#[tauri::command]
fn set_language(
    app: AppHandle,
    state: tauri::State<'_, VisioState>,
    lang: Option<String>,
) -> Result<(), String> {
    let supported = ["en", "fr", "de", "es", "it", "nl"];
    if let Some(ref l) = lang {
        if !supported.contains(&l.as_str()) {
            return Err(format!("unsupported language: {l}"));
        }
    }
    state.settings.set_language(lang.clone());
    let _ = app.emit("settings-changed", serde_json::json!({"language": lang}));
    Ok(())
}

#[tauri::command]
fn set_mic_enabled_on_join(app: AppHandle, state: tauri::State<'_, VisioState>, enabled: bool) {
    state.settings.set_mic_enabled_on_join(enabled);
    let _ = app.emit(
        "settings-changed",
        serde_json::json!({"mic_enabled_on_join": enabled}),
    );
}

#[tauri::command]
fn set_camera_enabled_on_join(app: AppHandle, state: tauri::State<'_, VisioState>, enabled: bool) {
    state.settings.set_camera_enabled_on_join(enabled);
    let _ = app.emit(
        "settings-changed",
        serde_json::json!({"camera_enabled_on_join": enabled}),
    );
}

#[tauri::command]
fn set_audio_mode(app: AppHandle, state: tauri::State<'_, VisioState>, mode: String) {
    state.settings.set_audio_mode(mode.clone());
    let _ = app.emit("settings-changed", serde_json::json!({"audio_mode": mode}));
}

#[tauri::command]
fn set_theme(
    app: AppHandle,
    state: tauri::State<'_, VisioState>,
    theme: String,
) -> Result<(), String> {
    let valid = ["light", "dark", "system"];
    if !valid.contains(&theme.as_str()) {
        return Err(format!("invalid theme: {theme}"));
    }
    state.settings.set_theme(theme.clone());
    let _ = app.emit("settings-changed", serde_json::json!({"theme": theme}));
    Ok(())
}

#[tauri::command]
fn get_meet_instances(state: tauri::State<'_, VisioState>) -> Result<Vec<String>, String> {
    Ok(state.settings.get_meet_instances())
}

#[tauri::command]
fn set_meet_instances(state: tauri::State<'_, VisioState>, instances: Vec<String>) {
    state.settings.set_meet_instances(instances);
}

#[derive(serde::Serialize)]
struct RoomHistoryEntryJs {
    url: String,
    display_name: Option<String>,
}

#[tauri::command]
fn get_room_history(state: tauri::State<'_, VisioState>) -> Result<Vec<RoomHistoryEntryJs>, String> {
    Ok(state
        .settings
        .get_room_history()
        .into_iter()
        .map(|e| RoomHistoryEntryJs {
            url: e.url,
            display_name: e.display_name,
        })
        .collect())
}

#[tauri::command]
fn validate_room_display_name_cmd(name: String) -> Option<String> {
    visio_core::validate_room_display_name(&name)
}

#[tauri::command]
fn clear_room_history(state: tauri::State<'_, VisioState>) {
    state.settings.clear_room_history();
}

// ---------------------------------------------------------------------------
// Calendar commands
// ---------------------------------------------------------------------------

#[tauri::command]
fn get_calendar_url(state: tauri::State<'_, VisioState>) -> Option<String> {
    state.settings.get_calendar_url()
}

#[tauri::command]
fn set_calendar_url(state: tauri::State<'_, VisioState>, url: Option<String>) {
    state.settings.set_calendar_url(url.clone());
    if url.is_some() {
        let calendar = state.calendar.clone();
        tauri::async_runtime::spawn(async move {
            calendar.refresh().await;
        });
    } else {
        state.calendar.clear_cache();
    }
}

#[tauri::command]
fn get_calendar_refresh_interval(state: tauri::State<'_, VisioState>) -> String {
    match state.settings.get_calendar_refresh_interval() {
        visio_core::CalendarRefreshInterval::Minutes5 => "Minutes5".to_string(),
        visio_core::CalendarRefreshInterval::Minutes15 => "Minutes15".to_string(),
        visio_core::CalendarRefreshInterval::Hour1 => "Hour1".to_string(),
        visio_core::CalendarRefreshInterval::Hours4 => "Hours4".to_string(),
        visio_core::CalendarRefreshInterval::Manual => "Manual".to_string(),
    }
}

#[tauri::command]
fn set_calendar_refresh_interval(state: tauri::State<'_, VisioState>, interval: String) {
    let parsed = match interval.as_str() {
        "Minutes5" => visio_core::CalendarRefreshInterval::Minutes5,
        "Minutes15" => visio_core::CalendarRefreshInterval::Minutes15,
        "Hour1" => visio_core::CalendarRefreshInterval::Hour1,
        "Hours4" => visio_core::CalendarRefreshInterval::Hours4,
        _ => visio_core::CalendarRefreshInterval::Manual,
    };
    state.settings.set_calendar_refresh_interval(parsed);
}

#[tauri::command]
fn get_upcoming_meetings(state: tauri::State<'_, VisioState>) -> Vec<serde_json::Value> {
    state
        .calendar
        .get_meetings()
        .into_iter()
        .map(|m| {
            serde_json::json!({
                "id": m.id,
                "summary": m.summary,
                "start_time": m.start_time,
                "end_time": m.end_time,
                "room_url": m.room_url,
                "deep_link": m.deep_link,
                "server_name": m.server_name,
            })
        })
        .collect()
}

#[tauri::command]
async fn refresh_calendar_now(state: tauri::State<'_, VisioState>) -> Result<(), String> {
    state.calendar.refresh().await;
    Ok(())
}

#[tauri::command]
async fn raise_hand(state: tauri::State<'_, VisioState>) -> Result<(), String> {
    tracing::info!("Tauri command: raise_hand");
    let room = state.room.lock().await;
    room.raise_hand().await.map_err(|e| {
        tracing::error!("raise_hand command failed: {e}");
        e.to_string()
    })
}

#[tauri::command]
async fn lower_hand(state: tauri::State<'_, VisioState>) -> Result<(), String> {
    tracing::info!("Tauri command: lower_hand");
    let room = state.room.lock().await;
    room.lower_hand().await.map_err(|e| {
        tracing::error!("lower_hand command failed: {e}");
        e.to_string()
    })
}

#[tauri::command]
async fn is_hand_raised(state: tauri::State<'_, VisioState>) -> Result<bool, String> {
    let room = state.room.lock().await;
    Ok(room.is_hand_raised().await)
}

#[tauri::command]
async fn set_chat_open(state: tauri::State<'_, VisioState>, open: bool) -> Result<(), String> {
    let chat = state.chat.lock().await;
    chat.set_chat_open(open);
    Ok(())
}

#[tauri::command]
async fn send_reaction(state: tauri::State<'_, VisioState>, emoji: String) -> Result<(), String> {
    let room = state.room.lock().await;
    room.send_reaction(&emoji).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn lower_all_hands(state: tauri::State<'_, VisioState>) -> Result<(), String> {
    let room = state.room.lock().await;
    room.lower_all_hands().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn mute_everyone(state: tauri::State<'_, VisioState>) -> Result<(), String> {
    let room = state.room.lock().await;
    room.mute_everyone().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn mute_participant(
    state: tauri::State<'_, VisioState>,
    identity: String,
) -> Result<(), String> {
    let room = state.room.lock().await;
    room.mute_participant(&identity)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn list_screen_sources() -> Vec<screen_capture::ScreenSource> {
    screen_capture::list_sources()
}

#[tauri::command]
async fn start_screen_share(
    state: tauri::State<'_, VisioState>,
    source_id: String,
) -> Result<(), String> {
    tracing::info!("start_screen_share called with source_id={source_id}");
    let controls = state.controls.lock().await;
    let source = controls
        .publish_screen_share()
        .await
        .map_err(|e| e.to_string())?;
    tracing::info!("screen share track published, starting capture");

    let capture = screen_capture::ScreenCapture::start(&source_id, source)
        .map_err(|e| format!("screen capture: {e}"))?;

    {
        let mut cap = state
            .screen_capture
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        *cap = Some(capture);
    }

    // Start screen audio capture (macOS only)
    #[cfg(target_os = "macos")]
    {
        match controls.publish_screen_share_audio().await {
            Ok(audio_source) => match screen_audio_macos::ScreenAudioCapture::start(audio_source) {
                Ok(audio_cap) => {
                    let mut sa = state
                        .screen_audio_capture
                        .lock()
                        .unwrap_or_else(|e| e.into_inner());
                    *sa = Some(audio_cap);
                    tracing::info!("screen audio capture started");
                }
                Err(e) => {
                    tracing::warn!("screen audio capture failed (continuing without): {e}");
                }
            },
            Err(e) => {
                tracing::warn!("publish screen share audio failed (continuing without): {e}");
            }
        }
    }

    Ok(())
}

#[tauri::command]
async fn stop_screen_share(state: tauri::State<'_, VisioState>) -> Result<(), String> {
    {
        let mut cap = state
            .screen_capture
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if let Some(mut capture) = cap.take() {
            capture.stop();
        }
    }

    // Stop screen audio capture (macOS only)
    #[cfg(target_os = "macos")]
    {
        let mut sa = state
            .screen_audio_capture
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if let Some(mut audio_cap) = sa.take() {
            audio_cap.stop();
        }
    }

    let controls = state.controls.lock().await;
    // Stop screen share audio track
    let _ = controls.stop_screen_share_audio().await;
    controls
        .stop_screen_share()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn set_subscribe_quality(
    state: tauri::State<'_, VisioState>,
    quality: String,
) -> Result<(), String> {
    let room = state.room.lock().await;
    room.set_subscribe_video_quality(&quality)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn set_background_mode(
    state: tauri::State<'_, VisioState>,
    app: AppHandle,
    mode: String,
) -> Result<(), String> {
    // Validate mode
    if mode != "off" && mode != "blur" && mode != "blur-light" && !mode.starts_with("image:") {
        return Err("Invalid background mode".into());
    }
    // Update BlurProcessor
    let bg_mode = match mode.as_str() {
        "blur" => visio_ffi::blur::process::BackgroundMode::Blur,
        "blur-light" => visio_ffi::blur::process::BackgroundMode::BlurLight,
        m if m.starts_with("image:") => {
            if let Ok(id) = m[6..].parse::<u8>() {
                visio_ffi::blur::process::BackgroundMode::Image(id)
            } else {
                return Err("Invalid image ID".into());
            }
        }
        _ => visio_ffi::blur::process::BackgroundMode::Off,
    };
    visio_ffi::blur::BlurProcessor::set_mode(bg_mode);
    // Persist
    state.settings.set_background_mode(mode);
    let _ = app.emit("settings-changed", ());
    Ok(())
}

#[tauri::command]
fn get_background_mode(state: tauri::State<'_, VisioState>) -> String {
    state.settings.get_background_mode()
}

#[tauri::command]
fn set_adaptive_mode_enabled(
    app: AppHandle,
    state: tauri::State<'_, VisioState>,
    enabled: bool,
) -> Result<(), String> {
    state.settings.set_adaptive_mode_enabled(enabled);
    let _ = app.emit(
        "settings-changed",
        serde_json::json!({"adaptive_mode_enabled": enabled}),
    );
    Ok(())
}

#[tauri::command]
fn load_blur_model(model_path: String) -> Result<(), String> {
    visio_ffi::blur::model::load_model(std::path::Path::new(&model_path))
}

#[tauri::command]
fn load_background_image(id: u8, jpeg_path: String) -> Result<(), String> {
    let jpeg_bytes = std::fs::read(&jpeg_path).map_err(|e| e.to_string())?;
    visio_ffi::blur::BlurProcessor::load_replacement_image(id, &jpeg_bytes, 640, 480)
}

#[tauri::command]
fn set_camera_device(state: tauri::State<'_, VisioState>, unique_id: Option<String>) {
    state.settings.set_camera_device(unique_id);
}

#[tauri::command]
fn list_audio_input_devices() -> Vec<audio_engine::AudioDeviceInfo> {
    audio_engine::list_input_devices()
}

#[tauri::command]
fn list_audio_output_devices() -> Vec<audio_engine::AudioDeviceInfo> {
    audio_engine::list_output_devices()
}

#[cfg(target_os = "macos")]
#[tauri::command]
fn list_video_input_devices() -> Vec<camera_macos::VideoDeviceInfo> {
    camera_macos::list_cameras()
}

#[cfg(target_os = "linux")]
#[tauri::command]
fn list_video_input_devices() -> Vec<camera_linux::VideoDeviceInfo> {
    camera_linux::list_cameras()
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn list_video_input_devices() -> Vec<serde_json::Value> {
    Vec::new()
}

// ---------------------------------------------------------------------------
// Device selection commands
// ---------------------------------------------------------------------------

#[tauri::command]
async fn select_audio_input(
    state: tauri::State<'_, VisioState>,
    device_name: String,
) -> Result<(), String> {
    // Get the audio source before locking the engine (async)
    let controls = state.controls.lock().await;
    let audio_source = controls.audio_source().await;
    drop(controls);

    // Store selected device name
    *state
        .selected_input_device
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = Some(device_name.clone());

    // Recreate engine with both device names preserved
    let output_device = state
        .selected_output_device
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    let mut engine = state.audio_engine.lock().unwrap_or_else(|e| e.into_inner());
    engine.stop_playout();
    let new_engine =
        audio_engine::create_audio_engine(Some(&device_name), output_device.as_deref());
    *engine = new_engine;
    engine.set_device_change_callback(Arc::new(|| {
        tracing::info!("audio devices changed — re-enumerating");
        if let Some(app) = APP_HANDLE.get() {
            let _ = app.emit("audio-devices-changed", ());
        }
    }));
    engine.start_playout(state.playout_buffer.clone())?;
    if let Some(source) = audio_source {
        let nr = state.settings.is_noise_reduction_enabled();
        engine.start_capture(source, nr)?;
    }
    Ok(())
}

#[tauri::command]
async fn select_audio_output(
    state: tauri::State<'_, VisioState>,
    device_name: String,
) -> Result<(), String> {
    let controls = state.controls.lock().await;
    let audio_source = controls.audio_source().await;
    drop(controls);

    *state
        .selected_output_device
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = Some(device_name.clone());

    let input_device = state
        .selected_input_device
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    let mut engine = state.audio_engine.lock().unwrap_or_else(|e| e.into_inner());
    engine.stop_playout();
    let new_engine = audio_engine::create_audio_engine(input_device.as_deref(), Some(&device_name));
    *engine = new_engine;
    engine.set_device_change_callback(Arc::new(|| {
        tracing::info!("audio devices changed — re-enumerating");
        if let Some(app) = APP_HANDLE.get() {
            let _ = app.emit("audio-devices-changed", ());
        }
    }));
    engine.start_playout(state.playout_buffer.clone())?;
    if let Some(source) = audio_source {
        let nr = state.settings.is_noise_reduction_enabled();
        engine.start_capture(source, nr)?;
    }
    Ok(())
}

#[cfg(target_os = "macos")]
#[tauri::command]
async fn select_video_input(
    state: tauri::State<'_, VisioState>,
    unique_id: String,
) -> Result<(), String> {
    let controls = state.controls.lock().await;
    let source = controls
        .video_source()
        .await
        .ok_or("No video source — enable camera first")?;

    // Stop existing camera
    {
        let mut cam = state
            .camera_capture
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if let Some(mut capture) = cam.take() {
            capture.stop();
        }
    }

    // Start with selected camera
    let capture = camera_macos::MacCameraCapture::start_with_unique_id(&unique_id, source)
        .map_err(|e| format!("camera: {e}"))?;
    let mut cam = state
        .camera_capture
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    *cam = Some(capture);

    Ok(())
}

#[cfg(target_os = "linux")]
#[tauri::command]
async fn select_video_input(
    state: tauri::State<'_, VisioState>,
    unique_id: String,
) -> Result<(), String> {
    let controls = state.controls.lock().await;
    let source = controls
        .video_source()
        .await
        .ok_or("No video source — enable camera first")?;

    // Stop existing camera
    {
        let mut cam = state
            .camera_capture
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if let Some(mut capture) = cam.take() {
            capture.stop();
        }
    }

    // Start with selected camera
    let capture = camera_linux::LinuxCameraCapture::start_with_unique_id(&unique_id, source)
        .map_err(|e| format!("camera: {e}"))?;
    let mut cam = state
        .camera_capture
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    *cam = Some(capture);

    Ok(())
}

#[cfg(target_os = "windows")]
#[tauri::command]
async fn select_video_input(
    _state: tauri::State<'_, VisioState>,
    _unique_id: String,
) -> Result<(), String> {
    Err("Video device selection not implemented on Windows".into())
}

// ---------------------------------------------------------------------------
// Camera preview commands (pre-join lobby, no LiveKit connection)
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
#[tauri::command]
async fn start_camera_preview(state: tauri::State<'_, VisioState>) -> Result<(), String> {
    // No-op if camera is already running
    {
        let cam_guard = state
            .camera_capture
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if cam_guard.is_some() {
            return Ok(());
        }
    }

    if !camera_macos::request_camera_permission() {
        return Err("Camera permission denied".into());
    }

    let settings = state.settings.get();
    let capture = if let Some(ref device_id) = settings.camera_device {
        camera_macos::MacCameraCapture::start_preview_with_unique_id(device_id)
    } else {
        camera_macos::MacCameraCapture::start_preview()
    };

    let mut cam_guard = state
        .camera_capture
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    *cam_guard = Some(capture.map_err(|e| e)?);
    Ok(())
}

#[cfg(target_os = "macos")]
#[tauri::command]
async fn stop_camera_preview(state: tauri::State<'_, VisioState>) -> Result<(), String> {
    let mut cam = state
        .camera_capture
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    if let Some(mut capture) = cam.take() {
        capture.stop();
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
#[tauri::command]
async fn start_camera_preview() -> Result<(), String> {
    Err("Camera preview not supported on this platform".to_string())
}

#[cfg(not(target_os = "macos"))]
#[tauri::command]
async fn stop_camera_preview() -> Result<(), String> {
    Ok(())
}

// ---------------------------------------------------------------------------
// Mic level / VU meter commands
// ---------------------------------------------------------------------------

/// Returns the current mic RMS level as 0.0–1.0 for VU meter display.
/// Cheap — reads a single atomic; safe to call at high frequency.
#[tauri::command]
fn get_mic_level() -> f32 {
    audio_engine::get_mic_level()
}

/// Start a lightweight preview capture on the currently selected input device.
/// Opens a cpal input stream independent of any LiveKit session so the VU
/// meter works in the pre-join lobby.
#[tauri::command]
async fn start_mic_preview(state: tauri::State<'_, VisioState>) -> Result<(), String> {
    let device_name = state
        .selected_input_device
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    let mut engine = state.audio_engine.lock().unwrap_or_else(|e| e.into_inner());
    engine.set_device_change_callback(Arc::new(|| {
        tracing::info!("audio devices changed (lobby) — re-enumerating");
        if let Some(app) = APP_HANDLE.get() {
            let _ = app.emit("audio-devices-changed", ());
        }
    }));
    engine.start_preview_capture(device_name.as_deref())
}

/// Stop the preview capture stream and clear the VU meter level.
#[tauri::command]
async fn stop_mic_preview(state: tauri::State<'_, VisioState>) -> Result<(), String> {
    let mut engine = state.audio_engine.lock().unwrap_or_else(|e| e.into_inner());
    engine.stop_preview_capture();
    Ok(())
}

/// Play a short 440 Hz test tone on the currently selected output device.
/// Runs on a blocking thread so the async runtime is not stalled.
#[tauri::command]
async fn play_speaker_test(state: tauri::State<'_, VisioState>) -> Result<(), String> {
    let device_name = state
        .selected_output_device
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    tokio::task::spawn_blocking(move || audio_engine::play_speaker_test(device_name.as_deref()))
        .await
        .map_err(|e| e.to_string())?
}

// ---------------------------------------------------------------------------
// Lobby commands
// ---------------------------------------------------------------------------

#[tauri::command]
async fn list_waiting_participants(
    state: tauri::State<'_, VisioState>,
) -> Result<serde_json::Value, String> {
    let room = state.room.lock().await;
    let participants = room
        .list_waiting_participants()
        .await
        .map_err(|e| e.to_string())?;
    let json: Vec<_> = participants
        .iter()
        .map(|p| serde_json::json!({ "id": p.id, "username": p.username }))
        .collect();
    Ok(serde_json::json!(json))
}

#[tauri::command]
async fn admit_participant(
    state: tauri::State<'_, VisioState>,
    participant_id: String,
) -> Result<(), String> {
    let room = state.room.lock().await;
    room.handle_lobby_entry(&participant_id, true)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn deny_participant(
    state: tauri::State<'_, VisioState>,
    participant_id: String,
) -> Result<(), String> {
    let room = state.room.lock().await;
    room.handle_lobby_entry(&participant_id, false)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn cancel_lobby(state: tauri::State<'_, VisioState>) -> Result<(), String> {
    let room = state.room.lock().await;
    room.cancel_lobby().await;
    Ok(())
}

// ---------------------------------------------------------------------------
// Access management commands
// ---------------------------------------------------------------------------

#[tauri::command]
async fn search_users(
    state: tauri::State<'_, VisioState>,
    query: String,
) -> Result<serde_json::Value, String> {
    let session = state.session.lock().await;
    let cookie = session.cookie().ok_or("Not authenticated")?;
    let meet_instance = session
        .meet_instance()
        .ok_or("No meet instance")?
        .to_string();
    drop(session);

    let meet_url = format!("https://{}/room", meet_instance);
    let results = visio_core::AccessService::search_users(&meet_url, &cookie, &query)
        .await
        .map_err(|e| e.to_string())?;

    serde_json::to_value(&results).map_err(|e| e.to_string())
}

#[tauri::command]
async fn list_accesses(
    state: tauri::State<'_, VisioState>,
    room_id: String,
) -> Result<serde_json::Value, String> {
    let session = state.session.lock().await;
    let cookie = session.cookie().ok_or("Not authenticated")?;
    let meet_instance = session
        .meet_instance()
        .ok_or("No meet instance")?
        .to_string();
    drop(session);

    let meet_url = format!("https://{}/room", meet_instance);
    let results = visio_core::AccessService::list_accesses(&meet_url, &cookie, &room_id)
        .await
        .map_err(|e| e.to_string())?;

    serde_json::to_value(&results).map_err(|e| e.to_string())
}

#[tauri::command]
async fn add_access(
    state: tauri::State<'_, VisioState>,
    user_id: String,
    room_id: String,
) -> Result<serde_json::Value, String> {
    let session = state.session.lock().await;
    let cookie = session.cookie().ok_or("Not authenticated")?;
    let meet_instance = session
        .meet_instance()
        .ok_or("No meet instance")?
        .to_string();
    drop(session);

    let meet_url = format!("https://{}/room", meet_instance);
    let result = visio_core::AccessService::add_access(&meet_url, &cookie, &user_id, &room_id)
        .await
        .map_err(|e| e.to_string())?;

    serde_json::to_value(&result).map_err(|e| e.to_string())
}

#[tauri::command]
async fn remove_access(
    state: tauri::State<'_, VisioState>,
    access_id: String,
) -> Result<(), String> {
    let session = state.session.lock().await;
    let cookie = session.cookie().ok_or("Not authenticated")?;
    let meet_instance = session
        .meet_instance()
        .ok_or("No meet instance")?
        .to_string();
    drop(session);

    let meet_url = format!("https://{}/room", meet_instance);
    visio_core::AccessService::remove_access(&meet_url, &cookie, &access_id)
        .await
        .map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// OIDC authentication commands
// ---------------------------------------------------------------------------

/// Open the system browser for OIDC authentication.
/// The browser has the user's existing SSO session, so login is seamless.
/// After auth, the server redirects to visio://auth-callback?code={uuid}
/// which the deep link plugin delivers to the frontend.
#[tauri::command]
fn launch_oidc_browser(meet_instance: String) -> Result<(), String> {
    let auth_url = format!(
        "https://{}/api/v1.0/authenticate/?returnTo=visio%3A%2F%2Fauth-callback&prompt=login",
        meet_instance
    );
    tracing::info!("Opening system browser for OIDC: {}", auth_url);
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&auth_url)
            .spawn()
            .map_err(|e| format!("failed to open browser: {e}"))?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&auth_url)
            .spawn()
            .map_err(|e| format!("failed to open browser: {e}"))?;
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", &auth_url])
            .spawn()
            .map_err(|e| format!("failed to open browser: {e}"))?;
    }
    Ok(())
}

/// Exchange a one-time OIDC code for a session, completing authentication.
/// Called by the frontend after receiving the visio://auth-callback?code= deep link.
#[tauri::command]
async fn exchange_oidc_code(
    state: tauri::State<'_, VisioState>,
    meet_instance: String,
    code: String,
) -> Result<serde_json::Value, String> {
    tracing::info!("Exchanging OIDC code for session on {}", meet_instance);

    let session_cookie = SessionManager::exchange_oidc_code(&meet_instance, &code)
        .await
        .map_err(|e| e.to_string())?;

    let meet_url = format!("https://{}", meet_instance);
    let user = SessionManager::fetch_user(&meet_url, &session_cookie)
        .await
        .map_err(|e| e.to_string())?;

    let mut session = state.session.lock().await;
    session.set_authenticated(user.clone(), session_cookie, meet_instance.clone());

    Ok(serde_json::json!({
        "display_name": user.display_name(),
        "email": user.email,
        "meet_instance": meet_instance,
    }))
}

#[tauri::command]
async fn authenticate(
    state: tauri::State<'_, VisioState>,
    meet_url: String,
    cookie: String,
) -> Result<serde_json::Value, String> {
    let user = SessionManager::fetch_user(&meet_url, &cookie)
        .await
        .map_err(|e| e.to_string())?;
    let mut session = state.session.lock().await;
    let instance = meet_url
        .trim_end_matches('/')
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .to_string();
    session.set_authenticated(user.clone(), cookie, instance);
    Ok(serde_json::json!({
        "display_name": user.display_name(),
        "email": user.email,
    }))
}

#[tauri::command]
async fn logout_session(
    state: tauri::State<'_, VisioState>,
    meet_url: String,
) -> Result<(), String> {
    let mut session = state.session.lock().await;
    session.logout(&meet_url).await.map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn create_room(
    state: tauri::State<'_, VisioState>,
    meet_url: String,
    name: String,
    access_level: String,
) -> Result<serde_json::Value, String> {
    let session = state.session.lock().await;
    let cookie = session.cookie().ok_or("Not authenticated")?;
    drop(session);

    let result = visio_core::SessionManager::create_room(&meet_url, &cookie, &name, &access_level)
        .await
        .map_err(|e| e.to_string())?;

    let (livekit_url, livekit_token) = match result.livekit {
        Some(lk) => (
            lk.url
                .replace("https://", "wss://")
                .replace("http://", "ws://"),
            lk.token,
        ),
        None => (String::new(), String::new()),
    };

    Ok(serde_json::json!({
        "id": result.id,
        "slug": result.slug,
        "name": result.name,
        "access_level": result.access_level,
        "livekit_url": livekit_url,
        "livekit_token": livekit_token,
    }))
}

#[tauri::command]
async fn get_session_state(
    state: tauri::State<'_, VisioState>,
) -> Result<serde_json::Value, String> {
    let session = state.session.lock().await;
    match session.state() {
        SessionState::Anonymous => Ok(serde_json::json!({ "state": "anonymous" })),
        SessionState::Authenticated {
            user,
            meet_instance,
            ..
        } => Ok(serde_json::json!({
            "state": "authenticated",
            "display_name": user.display_name(),
            "email": user.email,
            "meet_instance": meet_instance,
        })),
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                "visio_core=info,visio_video=info,visio_desktop=info"
                    .parse()
                    .unwrap()
            }),
        )
        .init();

    let data_dir = dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("io.visio.desktop");
    std::fs::create_dir_all(&data_dir).ok();
    let settings = Arc::new(SettingsStore::new(data_dir.to_str().unwrap()));

    let room_manager = RoomManager::new();
    room_manager.set_high_quality_mode(true);
    let playout_buffer = room_manager.playout_buffer();
    let controls = room_manager.controls();
    let chat = room_manager.chat();

    // Audio engine is created eagerly (so device enumeration works) but
    // playout is NOT started until the user actually connects to a room.
    // This avoids grabbing the microphone / audio device at app startup.
    let audio_engine_instance = audio_engine::create_audio_engine(None, None);

    let room_arc = Arc::new(Mutex::new(room_manager));

    // Create the CalendarService, sharing the room's emitter so it can reach
    // the DesktopEventListener and forward calendar events to the frontend.
    let calendar_emitter = {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let emitter = rt.block_on(async {
            let rm = room_arc.lock().await;
            rm.emitter()
        });
        drop(rt);
        emitter
    };
    let calendar = Arc::new(CalendarService::new(
        settings.clone(),
        calendar_emitter,
        data_dir.to_str().unwrap().to_string(),
    ));

    // Register event listener for auto-starting video renderers
    {
        let listener = Arc::new(DesktopEventListener {
            room: room_arc.clone(),
            settings: settings.clone(),
        });
        // We need to add the listener while we can still access room_manager
        // But room_manager is now behind Arc<Mutex>. We'll do it via block_on.
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let rm = room_arc.lock().await;
            rm.add_listener(listener);
        });
        // Drop the temp runtime — Tauri will create its own
        drop(rt);
    }

    // Calendar periodic refresh is started in the Tauri setup() callback
    // where the async runtime is available.
    let calendar_for_setup = calendar.clone();

    let state = VisioState {
        room: room_arc,
        controls: Arc::new(Mutex::new(controls)),
        chat: Arc::new(Mutex::new(chat)),
        session: Mutex::new(SessionManager::new()),
        settings,
        calendar,
        #[cfg(target_os = "macos")]
        camera_capture: std::sync::Mutex::new(None),
        #[cfg(target_os = "linux")]
        camera_capture: std::sync::Mutex::new(None),
        audio_engine: std::sync::Mutex::new(audio_engine_instance),
        playout_buffer,
        selected_input_device: std::sync::Mutex::new(None),
        selected_output_device: std::sync::Mutex::new(None),
        screen_capture: std::sync::Mutex::new(None),
        #[cfg(target_os = "macos")]
        screen_audio_capture: std::sync::Mutex::new(None),
    };

    // Parse CLI args for auto-connect (--livekit-url <url> --token <token>)
    let mut cli_livekit_url: Option<String> = None;
    let mut cli_token: Option<String> = None;
    {
        let args: Vec<String> = std::env::args().collect();
        let mut i = 1;
        while i < args.len() {
            match args[i].as_str() {
                "--livekit-url" if i + 1 < args.len() => {
                    cli_livekit_url = Some(args[i + 1].clone());
                    i += 2;
                }
                "--token" if i + 1 < args.len() => {
                    cli_token = Some(args[i + 1].clone());
                    i += 2;
                }
                _ => {
                    i += 1;
                }
            }
        }
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_deep_link::init())
        .manage(state)
        .setup(move |app| {
            // Set the macOS dock icon via objc2-app-kit
            #[cfg(target_os = "macos")]
            {
                use objc2::AllocAnyThread;
                use objc2::MainThreadMarker;
                use objc2_app_kit::{NSApplication, NSImage};
                use objc2_foundation::NSData;

                let png_bytes = include_bytes!("../icons/icon.png");
                let data = NSData::with_bytes(png_bytes);
                let ns_image = NSImage::initWithData(NSImage::alloc(), &data);
                if let Some(ns_image) = ns_image {
                    // We're in the Tauri setup which runs on the main thread
                    let mtm = unsafe { MainThreadMarker::new_unchecked() };
                    let ns_app = NSApplication::sharedApplication(mtm);
                    unsafe { ns_app.setApplicationIconImage(Some(&ns_image)) };
                    tracing::info!("macOS dock icon set via NSApplication (objc2)");
                } else {
                    tracing::warn!("Failed to create NSImage from icon PNG");
                }
            }

            // Store handle globally for the C video callback
            let _ = APP_HANDLE.set(app.handle().clone());

            // Store handle for audio error event emission
            audio_engine::set_app_handle(app.handle().clone());

            // Register the desktop video frame callback
            unsafe {
                visio_video::visio_video_set_desktop_callback(
                    on_desktop_frame,
                    std::ptr::null_mut(),
                );
            }

            tracing::info!("Visio desktop app started, video callback registered");

            // Note: devtools can be opened with Ctrl+Shift+I if needed

            // Start calendar periodic refresh (Tauri runtime provides the async executor)
            tauri::async_runtime::spawn(async move {
                calendar_for_setup.refresh().await;
                calendar_for_setup.start_periodic_refresh().await;
            });

            // Log deep link events on the Rust side
            app.listen("deep-link://new-url", |event: tauri::Event| {
                tracing::info!("Deep link received (Rust): {:?}", event.payload());
            });

            // Emit auto-connect event if CLI args were provided
            if let (Some(url), Some(token)) = (cli_livekit_url, cli_token) {
                tracing::info!("Auto-connect requested via CLI: {}", url);
                let handle = app.handle().clone();
                // Emit after a short delay to let the frontend mount
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(2000));
                    let _ = handle.emit(
                        "auto-connect",
                        serde_json::json!({
                            "livekit_url": url,
                            "token": token,
                        }),
                    );
                });
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                tracing::info!("window close requested, disconnecting gracefully");
                let state: tauri::State<'_, VisioState> = window.state();
                let room = state.room.clone();
                // Stop audio engine (capture + playout) before disconnect
                {
                    let mut engine = state.audio_engine.lock().unwrap_or_else(|e| e.into_inner());
                    engine.stop_capture();
                    engine.stop_playout();
                }
                #[cfg(target_os = "macos")]
                {
                    let mut cam = state
                        .camera_capture
                        .lock()
                        .unwrap_or_else(|e| e.into_inner());
                    if let Some(mut capture) = cam.take() {
                        capture.stop();
                    }
                }
                #[cfg(target_os = "linux")]
                {
                    let mut cam = state
                        .camera_capture
                        .lock()
                        .unwrap_or_else(|e| e.into_inner());
                    if let Some(mut capture) = cam.take() {
                        capture.stop();
                    }
                }
                {
                    let mut sc = state
                        .screen_capture
                        .lock()
                        .unwrap_or_else(|e| e.into_inner());
                    if let Some(mut capture) = sc.take() {
                        capture.stop();
                    }
                }
                // Gracefully disconnect the room
                tauri::async_runtime::block_on(async {
                    let rm = room.lock().await;
                    rm.disconnect().await;
                });
                tracing::info!("graceful disconnect complete");
            }
        })
        .invoke_handler(tauri::generate_handler![
            validate_room,
            connect,
            connect_with_token,
            disconnect,
            get_connection_state,
            get_participants,
            get_local_participant,
            get_video_tracks,
            toggle_mic,
            toggle_camera,
            send_chat,
            get_messages,
            get_translations,
            get_system_language,
            get_settings,
            set_display_name,
            set_language,
            set_mic_enabled_on_join,
            set_camera_enabled_on_join,
            set_audio_mode,
            set_theme,
            get_meet_instances,
            set_meet_instances,
            raise_hand,
            lower_hand,
            is_hand_raised,
            set_chat_open,
            list_waiting_participants,
            admit_participant,
            deny_participant,
            cancel_lobby,
            search_users,
            list_accesses,
            add_access,
            remove_access,
            launch_oidc_browser,
            exchange_oidc_code,
            authenticate,
            logout_session,
            create_room,
            get_session_state,
            send_reaction,
            lower_all_hands,
            mute_everyone,
            mute_participant,
            list_screen_sources,
            start_screen_share,
            stop_screen_share,
            set_subscribe_quality,
            set_background_mode,
            get_background_mode,
            load_blur_model,
            load_background_image,
            list_audio_input_devices,
            list_audio_output_devices,
            list_video_input_devices,
            select_audio_input,
            select_audio_output,
            set_camera_device,
            select_video_input,
            start_camera_preview,
            stop_camera_preview,
            get_mic_level,
            start_mic_preview,
            stop_mic_preview,
            play_speaker_test,
            set_adaptive_mode_enabled,
            check_media_permissions,
            get_room_history,
            clear_room_history,
            validate_room_display_name_cmd,
            get_calendar_url,
            set_calendar_url,
            get_calendar_refresh_interval,
            set_calendar_refresh_interval,
            get_upcoming_meetings,
            refresh_calendar_now,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
