use std::sync::Arc;
use tokio::sync::Mutex;

use visio_core::{ChatService, MeetingControls, RoomManager};

/// Shared application state managed by Tauri.
struct VisioState {
    room: Arc<Mutex<RoomManager>>,
    controls: Arc<Mutex<MeetingControls>>,
    chat: Arc<Mutex<ChatService>>,
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
                "connection_quality": format!("{:?}", p.connection_quality),
            })
        })
        .collect();
    Ok(result)
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

pub fn run() {
    let room_manager = RoomManager::new();
    let controls = room_manager.controls();
    let chat = room_manager.chat();

    let state = VisioState {
        room: Arc::new(Mutex::new(room_manager)),
        controls: Arc::new(Mutex::new(controls)),
        chat: Arc::new(Mutex::new(chat)),
    };

    tauri::Builder::default()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            connect,
            disconnect,
            get_connection_state,
            get_participants,
            toggle_mic,
            toggle_camera,
            send_chat,
            get_messages,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
