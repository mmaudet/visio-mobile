use std::sync::{Arc, OnceLock};
use tokio::sync::Mutex;

use tauri::{AppHandle, Emitter, Listener, Manager};
use visio_core::{
    ChatService, MeetingControls, RoomManager, SettingsStore, TrackInfo, TrackKind, TrackSource,
    VisioEvent, VisioEventListener,
};

#[cfg(target_os = "macos")]
mod camera_macos;
mod audio_cpal;

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
    let Ok(b64_str) = std::str::from_utf8(b64) else { return };

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
    settings: SettingsStore,
    #[cfg(target_os = "macos")]
    camera_capture: std::sync::Mutex<Option<camera_macos::MacCameraCapture>>,
    _audio_playout: audio_cpal::CpalAudioPlayout,
    audio_capture: std::sync::Mutex<Option<audio_cpal::CpalAudioCapture>>,
}

// ---------------------------------------------------------------------------
// Event listener — auto-starts/stops video renderers
// ---------------------------------------------------------------------------

struct DesktopEventListener {
    room: Arc<Mutex<RoomManager>>,
}

fn source_to_str(source: &TrackSource) -> &'static str {
    match source {
        TrackSource::Microphone => "microphone",
        TrackSource::Camera => "camera",
        TrackSource::ScreenShare => "screen_share",
        TrackSource::Unknown => "unknown",
    }
}

impl VisioEventListener for DesktopEventListener {
    fn on_event(&self, event: VisioEvent) {
        match event {
            VisioEvent::ConnectionStateChanged(state) => {
                let name = match &state {
                    visio_core::ConnectionState::Disconnected => "disconnected",
                    visio_core::ConnectionState::Connecting => "connecting",
                    visio_core::ConnectionState::Connected => "connected",
                    visio_core::ConnectionState::Reconnecting { .. } => "reconnecting",
                };
                tracing::info!("connection state changed: {name}");
                if let Some(app) = APP_HANDLE.get() {
                    let _ = app.emit("connection-state-changed", name);
                }
            }
            VisioEvent::ParticipantJoined(info) => {
                tracing::info!("participant joined: {} ({})", info.identity, info.sid);
                if let Some(app) = APP_HANDLE.get() {
                    let _ = app.emit(
                        "participant-joined",
                        serde_json::json!({
                            "sid": info.sid,
                            "identity": info.identity,
                            "name": info.name,
                        }),
                    );
                }
            }
            VisioEvent::ParticipantLeft(sid) => {
                tracing::info!("participant left: {sid}");
                if let Some(app) = APP_HANDLE.get() {
                    let _ = app.emit("participant-left", &sid);
                }
            }
            VisioEvent::TrackSubscribed(TrackInfo {
                sid: track_sid,
                kind: TrackKind::Video,
                ..
            }) => {
                let room = self.room.clone();
                let sid = track_sid.clone();
                tokio::spawn(async move {
                    let rm = room.lock().await;
                    if let Some(video_track) = rm.get_video_track(&sid).await {
                        tracing::info!("auto-starting video renderer for track {sid}");
                        visio_video::start_track_renderer(
                            sid,
                            video_track,
                            std::ptr::null_mut(),
                            None,
                        );
                    }
                });
            }
            VisioEvent::TrackSubscribed(_) => {}
            VisioEvent::TrackUnsubscribed(track_sid) => {
                tracing::info!("auto-stopping video renderer for track {track_sid}");
                visio_video::stop_track_renderer(&track_sid);
            }
            VisioEvent::TrackMuted {
                participant_sid,
                source,
            } => {
                if let Some(app) = APP_HANDLE.get() {
                    let _ = app.emit(
                        "track-muted",
                        serde_json::json!({
                            "participantSid": participant_sid,
                            "source": source_to_str(&source),
                        }),
                    );
                }
            }
            VisioEvent::TrackUnmuted {
                participant_sid,
                source,
            } => {
                if let Some(app) = APP_HANDLE.get() {
                    let _ = app.emit(
                        "track-unmuted",
                        serde_json::json!({
                            "participantSid": participant_sid,
                            "source": source_to_str(&source),
                        }),
                    );
                }
            }
            VisioEvent::HandRaisedChanged {
                participant_sid,
                raised,
                position,
            } => {
                tracing::info!(
                    "DesktopEventListener: HandRaisedChanged sid={participant_sid} raised={raised} position={position}"
                );
                if let Some(app) = APP_HANDLE.get() {
                    let _ = app.emit(
                        "hand-raised-changed",
                        serde_json::json!({
                            "participantSid": participant_sid,
                            "raised": raised,
                            "position": position,
                        }),
                    );
                }
            }
            VisioEvent::UnreadCountChanged(count) => {
                if let Some(app) = APP_HANDLE.get() {
                    let _ = app.emit("unread-count-changed", count);
                }
            }
            VisioEvent::ActiveSpeakersChanged(sids) => {
                if let Some(app) = APP_HANDLE.get() {
                    let _ = app.emit("active-speakers-changed", &sids);
                }
            }
            VisioEvent::ConnectionQualityChanged {
                participant_sid,
                quality,
            } => {
                if let Some(app) = APP_HANDLE.get() {
                    let _ = app.emit(
                        "connection-quality-changed",
                        serde_json::json!({
                            "participantSid": participant_sid,
                            "quality": format!("{:?}", quality),
                        }),
                    );
                }
            }
            VisioEvent::ChatMessageReceived(msg) => {
                if let Some(app) = APP_HANDLE.get() {
                    let _ = app.emit(
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
            }
            VisioEvent::ConnectionLost => {
                if let Some(app) = APP_HANDLE.get() {
                    let _ = app.emit("connection-lost", ());
                }
                let room = self.room.clone();
                tokio::spawn(async move {
                    let rm = room.lock().await;
                    tracing::info!("connection lost, attempting reconnection");
                    if let Err(e) = rm.reconnect().await {
                        tracing::error!("desktop reconnection failed: {e}");
                    }
                });
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

#[tauri::command]
async fn validate_room(
    _state: tauri::State<'_, VisioState>,
    url: String,
    username: Option<String>,
) -> Result<serde_json::Value, String> {
    if let Err(e) = visio_core::AuthService::extract_slug(&url) {
        return Ok(serde_json::json!({ "status": "invalid_format", "message": e.to_string() }));
    }
    match visio_core::AuthService::validate_room(&url, username.as_deref()).await {
        Ok(token_info) => Ok(serde_json::json!({
            "status": "valid",
            "livekit_url": token_info.livekit_url,
            "token": token_info.token,
        })),
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
    let room = state.room.lock().await;
    room.connect(&meet_url, username.as_deref())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn disconnect(state: tauri::State<'_, VisioState>) -> Result<(), String> {
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
        })
    }))
}

#[tauri::command]
async fn get_video_tracks(
    state: tauri::State<'_, VisioState>,
) -> Result<Vec<String>, String> {
    let room = state.room.lock().await;
    let sids = room.video_track_sids().await;
    Ok(sids)
}

#[tauri::command]
async fn toggle_mic(
    state: tauri::State<'_, VisioState>,
    enabled: bool,
) -> Result<(), String> {
    let controls = state.controls.lock().await;
    controls
        .set_microphone_enabled(enabled)
        .await
        .map_err(|e| e.to_string())?;

    if enabled {
        // Start capture if not already running
        let already_running = state.audio_capture.lock().unwrap().is_some();
        if !already_running {
            if let Some(source) = controls.audio_source().await {
                let capture = audio_cpal::CpalAudioCapture::start(source)
                    .map_err(|e| format!("audio capture: {e}"))?;
                *state.audio_capture.lock().unwrap() = Some(capture);
            }
        }
    } else {
        // Stop capture
        let mut cap = state.audio_capture.lock().unwrap();
        if let Some(capture) = cap.take() {
            capture.stop();
        }
    }

    Ok(())
}

#[tauri::command]
async fn toggle_camera(
    state: tauri::State<'_, VisioState>,
    enabled: bool,
) -> Result<(), String> {
    let controls = state.controls.lock().await;
    if enabled {
        // Publish camera track if not yet published
        if controls.video_source().await.is_none() {
            let source = controls
                .publish_camera()
                .await
                .map_err(|e| e.to_string())?;
            tracing::info!("camera track published via toggle_camera");

            // Start native camera capture
            #[cfg(target_os = "macos")]
            {
                let capture = camera_macos::MacCameraCapture::start(source)
                    .map_err(|e| format!("camera capture: {e}"))?;
                let mut cam = state.camera_capture.lock().unwrap();
                *cam = Some(capture);
            }
        }
    } else {
        // Stop camera capture when disabling
        #[cfg(target_os = "macos")]
        {
            let mut cam = state.camera_capture.lock().unwrap();
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
fn get_translations(
    app: AppHandle,
    lang: String,
) -> Result<serde_json::Value, String> {
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

    serde_json::from_str(&content)
        .map_err(|e| format!("invalid i18n JSON: {e}"))
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
    let _ = app.emit("settings-changed", serde_json::json!({"display_name": name}));
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
fn set_mic_enabled_on_join(
    app: AppHandle,
    state: tauri::State<'_, VisioState>,
    enabled: bool,
) {
    state.settings.set_mic_enabled_on_join(enabled);
    let _ = app.emit("settings-changed", serde_json::json!({"mic_enabled_on_join": enabled}));
}

#[tauri::command]
fn set_camera_enabled_on_join(
    app: AppHandle,
    state: tauri::State<'_, VisioState>,
    enabled: bool,
) {
    state.settings.set_camera_enabled_on_join(enabled);
    let _ = app.emit("settings-changed", serde_json::json!({"camera_enabled_on_join": enabled}));
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

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                "visio_core=info,visio_video=info,visio_desktop=info".parse().unwrap()
            }),
        )
        .init();

    let data_dir = dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("io.visio.desktop");
    std::fs::create_dir_all(&data_dir).ok();
    let settings = SettingsStore::new(data_dir.to_str().unwrap());

    let room_manager = RoomManager::new();
    let playout_buffer = room_manager.playout_buffer();
    let controls = room_manager.controls();
    let chat = room_manager.chat();

    let audio_playout = audio_cpal::CpalAudioPlayout::start(playout_buffer)
        .expect("failed to start audio playout");

    let room_arc = Arc::new(Mutex::new(room_manager));

    // Register event listener for auto-starting video renderers
    {
        let listener = Arc::new(DesktopEventListener {
            room: room_arc.clone(),
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

    let state = VisioState {
        room: room_arc,
        controls: Arc::new(Mutex::new(controls)),
        chat: Arc::new(Mutex::new(chat)),
        settings,
        #[cfg(target_os = "macos")]
        camera_capture: std::sync::Mutex::new(None),
        _audio_playout: audio_playout,
        audio_capture: std::sync::Mutex::new(None),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_deep_link::init())
        .manage(state)
        .setup(|app| {
            // Store handle globally for the C video callback
            let _ = APP_HANDLE.set(app.handle().clone());

            // Register the desktop video frame callback
            unsafe {
                visio_video::visio_video_set_desktop_callback(
                    on_desktop_frame,
                    std::ptr::null_mut(),
                );
            }

            tracing::info!("Visio desktop app started, video callback registered");

            // Log deep link events on the Rust side
            app.listen("deep-link://new-url", |event: tauri::Event| {
                tracing::info!("Deep link received (Rust): {:?}", event.payload());
            });

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                tracing::info!("window close requested, disconnecting gracefully");
                let state: tauri::State<'_, VisioState> = window.state();
                let room = state.room.clone();
                // Stop audio/camera capture before disconnect
                {
                    let mut cap = state.audio_capture.lock().unwrap_or_else(|e| e.into_inner());
                    if let Some(capture) = cap.take() {
                        capture.stop();
                    }
                }
                #[cfg(target_os = "macos")]
                {
                    let mut cam = state.camera_capture.lock().unwrap_or_else(|e| e.into_inner());
                    if let Some(mut capture) = cam.take() {
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
            set_theme,
            get_meet_instances,
            set_meet_instances,
            raise_hand,
            lower_hand,
            is_hand_raised,
            set_chat_open,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
