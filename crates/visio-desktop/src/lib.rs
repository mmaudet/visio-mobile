use std::sync::{Arc, OnceLock};
use tokio::sync::Mutex;

use tauri::{AppHandle, Emitter};
use visio_core::{
    ChatService, MeetingControls, RoomManager, TrackInfo, TrackKind, VisioEvent,
    VisioEventListener,
};

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
}

// ---------------------------------------------------------------------------
// Event listener — auto-starts/stops video renderers
// ---------------------------------------------------------------------------

struct VideoAutoStarter {
    room: Arc<Mutex<RoomManager>>,
}

impl VisioEventListener for VideoAutoStarter {
    fn on_event(&self, event: VisioEvent) {
        match event {
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
                        );
                    }
                });
            }
            VisioEvent::TrackUnsubscribed(track_sid) => {
                tracing::info!("auto-stopping video renderer for track {track_sid}");
                visio_video::stop_track_renderer(&track_sid);
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

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
        .map_err(|e| e.to_string())
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
            let _source = controls
                .publish_camera()
                .await
                .map_err(|e| e.to_string())?;
            tracing::info!("camera track published via toggle_camera");
            // TODO: start native camera capture and feed frames into _source
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

    let room_manager = RoomManager::new();
    let controls = room_manager.controls();
    let chat = room_manager.chat();

    let room_arc = Arc::new(Mutex::new(room_manager));

    // Register event listener for auto-starting video renderers
    {
        let listener = Arc::new(VideoAutoStarter {
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
    };

    tauri::Builder::default()
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
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            connect,
            disconnect,
            get_connection_state,
            get_participants,
            get_video_tracks,
            toggle_mic,
            toggle_camera,
            send_chat,
            get_messages,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
