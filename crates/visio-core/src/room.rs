use futures_util::StreamExt;
use livekit::DisconnectReason;
use livekit::data_stream::StreamReader;
use livekit::participant::ConnectionQuality as LkConnectionQuality;
use livekit::prelude::{DataPacket, RemoteParticipant, Room, RoomEvent, RoomOptions};
use livekit::track::{
    RemoteVideoTrack, TrackKind as LkTrackKind, TrackSource as LkTrackSource, VideoQuality,
};
use livekit::webrtc::audio_stream::native::NativeAudioStream;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use tokio::sync::Mutex;

use crate::adaptive;
use crate::audio_playout::AudioPlayoutBuffer;
use crate::auth::AuthService;
use crate::bandwidth;
use crate::chat::MessageStore;
use crate::errors::VisioError;
use crate::events::{
    ChatMessage, ConnectionQuality, ConnectionState, EventEmitter, ParticipantInfo, TrackInfo,
    TrackKind, TrackSource, VisioEvent, VisioEventListener,
};
use crate::hand_raise::HandRaiseManager;
use crate::participants::ParticipantManager;

/// Returns true if the disconnect reason means we should NOT auto-reconnect.
#[allow(dead_code)]
fn should_not_reconnect(reason: DisconnectReason) -> bool {
    matches!(
        reason,
        DisconnectReason::ClientInitiated
            | DisconnectReason::DuplicateIdentity
            | DisconnectReason::ParticipantRemoved
    )
}

/// Manages the lifecycle of a LiveKit room connection.
#[derive(Clone)]
pub struct RoomManager {
    room: Arc<Mutex<Option<Arc<Room>>>>,
    emitter: EventEmitter,
    participants: Arc<Mutex<ParticipantManager>>,
    connection_state: Arc<Mutex<ConnectionState>>,
    subscribed_tracks: Arc<Mutex<HashMap<String, RemoteVideoTrack>>>,
    messages: MessageStore,
    playout_buffer: Arc<AudioPlayoutBuffer>,
    hand_raise: Arc<Mutex<Option<HandRaiseManager>>>,
    /// Shared with MeetingControls so local_participant_info() reads the
    /// authoritative camera state without depending on LiveKit publication
    /// mute-state timing.
    camera_enabled: Arc<Mutex<bool>>,
    /// Stored connection info for application-level reconnection.
    last_meet_url: Arc<Mutex<Option<String>>>,
    last_username: Arc<Mutex<Option<String>>>,
    /// Lobby (waiting room) state.
    lobby_cookie: Arc<Mutex<Option<String>>>,
    session_cookie: Arc<Mutex<Option<String>>>,
    lobby_cancel: Arc<tokio::sync::Notify>,
    /// Chat unread tracking (shared with event loop).
    chat_open: Arc<AtomicBool>,
    unread_count: Arc<AtomicU32>,
    /// Adaptive context engine (sync Mutex — methods are non-async).
    adaptive: Arc<std::sync::Mutex<adaptive::AdaptiveEngine>>,
    /// Bandwidth degradation controller (sync Mutex — methods are non-async).
    bandwidth: Arc<std::sync::Mutex<bandwidth::BandwidthController>>,
    /// When true, disables adaptive streaming and bandwidth degradation
    /// to always receive the highest quality video layers.
    high_quality_mode: Arc<AtomicBool>,
    /// Rate limit: last reaction timestamp.
    last_reaction_time: Arc<Mutex<Option<std::time::Instant>>>,
}

impl Default for RoomManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Build RoomOptions with our configuration.
/// Meet uses maxRetries=5, peerConnectionTimeout=60s.
fn create_room_options(high_quality: bool) -> RoomOptions {
    let mut options = RoomOptions::default();
    options.auto_subscribe = true;
    options.adaptive_stream = !high_quality;
    options.dynacast = true;
    options.join_retries = 5;
    options.connect_timeout = std::time::Duration::from_secs(60);
    options
}

impl RoomManager {
    pub fn new() -> Self {
        Self {
            room: Arc::new(Mutex::new(None)),
            emitter: EventEmitter::new(),
            participants: Arc::new(Mutex::new(ParticipantManager::new())),
            connection_state: Arc::new(Mutex::new(ConnectionState::Disconnected)),
            subscribed_tracks: Arc::new(Mutex::new(HashMap::new())),
            messages: Arc::new(Mutex::new(Vec::new())),
            playout_buffer: Arc::new(AudioPlayoutBuffer::new()),
            hand_raise: Arc::new(Mutex::new(None)),
            camera_enabled: Arc::new(Mutex::new(false)),
            last_meet_url: Arc::new(Mutex::new(None)),
            last_username: Arc::new(Mutex::new(None)),
            lobby_cookie: Arc::new(Mutex::new(None)),
            session_cookie: Arc::new(Mutex::new(None)),
            lobby_cancel: Arc::new(tokio::sync::Notify::new()),
            chat_open: Arc::new(AtomicBool::new(false)),
            unread_count: Arc::new(AtomicU32::new(0)),
            adaptive: Arc::new(std::sync::Mutex::new(adaptive::AdaptiveEngine::new())),
            bandwidth: Arc::new(std::sync::Mutex::new(bandwidth::BandwidthController::new())),
            high_quality_mode: Arc::new(AtomicBool::new(false)),
            last_reaction_time: Arc::new(Mutex::new(None)),
        }
    }

    /// Enable high-quality mode: disables adaptive streaming and bandwidth
    /// degradation to always receive the highest quality video.
    /// Intended for desktop clients with reliable connectivity.
    pub fn set_high_quality_mode(&self, enabled: bool) {
        self.high_quality_mode
            .store(enabled, std::sync::atomic::Ordering::Relaxed);
    }

    /// Get a reference to the audio playout buffer.
    ///
    /// Platform audio output (Android AudioTrack, desktop cpal) pulls
    /// decoded remote audio samples from this buffer.
    pub fn playout_buffer(&self) -> Arc<AudioPlayoutBuffer> {
        self.playout_buffer.clone()
    }

    /// Register a listener for room events.
    pub fn add_listener(&self, listener: Arc<dyn VisioEventListener>) {
        self.emitter.add_listener(listener);
    }

    /// Expose the internal event emitter so other services (e.g. CalendarService)
    /// can share the same emitter and thus reach all registered listeners.
    pub fn emitter(&self) -> EventEmitter {
        self.emitter.clone()
    }

    /// Create MeetingControls bound to this room.
    pub fn controls(&self) -> crate::controls::MeetingControls {
        crate::controls::MeetingControls::new(
            self.room.clone(),
            self.emitter.clone(),
            self.camera_enabled.clone(),
        )
    }

    /// Create a ChatService bound to this room.
    pub fn chat(&self) -> crate::chat::ChatService {
        crate::chat::ChatService::new(
            self.room.clone(),
            self.emitter.clone(),
            self.messages.clone(),
        )
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

    /// Report a context signal from the platform layer.
    pub fn report_context_signal(
        &self,
        signal: adaptive::ContextSignal,
    ) -> Option<adaptive::AdaptiveMode> {
        let mut engine = self.adaptive.lock().unwrap_or_else(|p| p.into_inner());
        let changed = engine.update_signal(signal);
        if let Some(mode) = changed {
            self.emitter.emit(VisioEvent::AdaptiveModeChanged { mode });
        }
        changed
    }

    /// Get the current adaptive mode.
    pub fn adaptive_mode(&self) -> adaptive::AdaptiveMode {
        let engine = self.adaptive.lock().unwrap_or_else(|p| p.into_inner());
        engine.current_mode()
    }

    /// Set a manual mode override. Pass None to return to auto-detection.
    pub fn set_adaptive_mode_override(&self, mode: Option<adaptive::AdaptiveMode>) {
        let mut engine = self.adaptive.lock().unwrap_or_else(|p| p.into_inner());
        let old = engine.current_mode();
        engine.set_override(mode);
        let new = engine.current_mode();
        if new != old {
            self.emitter
                .emit(VisioEvent::AdaptiveModeChanged { mode: new });
        }
    }

    /// Get current connection state.
    pub async fn connection_state(&self) -> ConnectionState {
        self.connection_state.lock().await.clone()
    }

    /// Get a snapshot of current participants (sorted alphabetically, local first).
    pub async fn participants(&self) -> Vec<ParticipantInfo> {
        let mut list = self.participants.lock().await.sorted_participants();
        // Prepend local participant so the UI can render a self-view tile.
        if let Some(local) = self.local_participant_info().await {
            list.insert(0, local);
        }
        list
    }

    /// Get local participant info (for self-view tile).
    pub async fn local_participant_info(&self) -> Option<ParticipantInfo> {
        let room = self.room.lock().await;
        let room = room.as_ref()?;
        let local = room.local_participant();
        // Prefer the stored username (what the user entered) over LiveKit's
        // local.name() which comes from the JWT token and may be truncated
        // or corrupted by the server.
        let name = {
            let stored = self.last_username.lock().await.clone();
            if let Some(ref s) = stored {
                if !s.is_empty() {
                    stored
                } else {
                    let n = local.name().to_string();
                    if n.is_empty() { None } else { Some(n) }
                }
            } else {
                let n = local.name().to_string();
                if n.is_empty() { None } else { Some(n) }
            }
        };
        // Use the authoritative camera_enabled flag rather than checking
        // publication mute state, which may lag behind the actual user intent
        // (pub_.mute() is async and needs server ACK before is_muted() updates).
        let has_video = *self.camera_enabled.lock().await;
        let is_muted = local
            .track_publications()
            .values()
            .any(|pub_| pub_.kind() == LkTrackKind::Audio && pub_.is_muted());
        // "local-camera" is a sentinel SID recognised by the JNI layer:
        // attachSurface stores the ANativeWindow in LOCAL_PREVIEW_SURFACE
        // and nativePushCameraFrame renders I420 frames directly to it,
        // bypassing the NativeVideoStream path used for remote tracks.
        Some(ParticipantInfo {
            sid: local.sid().to_string(),
            identity: local.identity().to_string(),
            name,
            is_muted,
            has_video,
            video_track_sid: if has_video {
                Some("local-camera".to_string())
            } else {
                None
            },
            has_screen_share: false,
            screen_share_track_sid: None,
            connection_quality: ConnectionQuality::Excellent,
            color: None,
            is_admin: false,
        })
    }

    /// Get current active speakers.
    pub async fn active_speakers(&self) -> Vec<String> {
        self.participants.lock().await.active_speakers().to_vec()
    }

    /// Get a subscribed remote audio track by its SID.
    ///
    /// Returns `None` if the track is not currently subscribed.
    pub async fn get_audio_track(
        &self,
        track_sid: &str,
    ) -> Option<livekit::track::RemoteAudioTrack> {
        let room = self.room.lock().await;
        if let Some(lk_room) = room.as_ref() {
            for (_, participant) in lk_room.remote_participants() {
                for (sid, publication) in participant.track_publications() {
                    if sid.as_str() == track_sid
                        && let Some(livekit::track::RemoteTrack::Audio(audio_track)) =
                            publication.track()
                    {
                        return Some(audio_track);
                    }
                }
            }
        }
        None
    }

    /// Get a subscribed remote video track by its SID.
    ///
    /// Returns `None` if the track is not currently subscribed.
    pub async fn get_video_track(&self, track_sid: &str) -> Option<RemoteVideoTrack> {
        self.subscribed_tracks.lock().await.get(track_sid).cloned()
    }

    /// Check if a track SID belongs to a screen share (vs camera).
    pub async fn is_track_screencast(&self, track_sid: &str) -> bool {
        let pm = self.participants.lock().await;
        pm.participants()
            .iter()
            .any(|p| p.screen_share_track_sid.as_deref() == Some(track_sid))
    }

    /// Get all currently subscribed video track SIDs.
    pub async fn video_track_sids(&self) -> Vec<String> {
        self.subscribed_tracks
            .lock()
            .await
            .keys()
            .cloned()
            .collect()
    }

    /// Set a session cookie for authenticated Meet instances.
    pub async fn set_session_cookie(&self, cookie: Option<String>) {
        *self.session_cookie.lock().await = cookie;
    }

    /// Connect to a room using the Meet API.
    ///
    /// Calls the Meet API to get a token, then connects to the LiveKit room.
    pub async fn connect(
        &self,
        meet_url: &str,
        username: Option<&str>,
        session_cookie: Option<&str>,
    ) -> Result<(), VisioError> {
        // Store connection info for potential reconnection
        *self.last_meet_url.lock().await = Some(meet_url.to_string());
        *self.last_username.lock().await = username.map(|s| s.to_string());
        *self.session_cookie.lock().await = session_cookie.map(|s| s.to_string());

        self.set_connection_state(ConnectionState::Connecting).await;

        match AuthService::request_token(meet_url, username, session_cookie).await {
            Ok(token_info) => {
                self.connect_with_token(&token_info.livekit_url, &token_info.token)
                    .await?;

                // Start lobby polling for host (authenticated users)
                if session_cookie.is_some() {
                    tracing::info!("LOBBY: cookie present, starting host polling");
                    self.start_lobby_host_polling().await;
                } else {
                    tracing::info!("LOBBY: no cookie, skipping host polling");
                }

                Ok(())
            }
            Err(VisioError::WaitingForHost) => {
                tracing::info!("room requires host approval, entering lobby");
                let name = username.unwrap_or("Anonymous");
                self.enter_lobby(meet_url, name).await
            }
            Err(e) => Err(e),
        }
    }

    /// Connect directly with a LiveKit URL and token (useful for testing).
    pub async fn connect_with_token(
        &self,
        livekit_url: &str,
        token: &str,
    ) -> Result<(), VisioError> {
        self.set_connection_state(ConnectionState::Connecting).await;

        let high_quality = self
            .high_quality_mode
            .load(std::sync::atomic::Ordering::Relaxed);

        let options = create_room_options(high_quality);

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

        // Seed existing remote participants
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

        // Initialize HandRaiseManager now that we have a room
        {
            let hm = HandRaiseManager::new(room.clone(), self.emitter.clone());
            *self.hand_raise.lock().await = Some(hm);
        }

        // Update state to connected
        self.set_connection_state(ConnectionState::Connected).await;

        // Spawn event loop
        let emitter = self.emitter.clone();
        let participants = self.participants.clone();
        let connection_state = self.connection_state.clone();
        let room_ref = self.room.clone();
        let subscribed_tracks = self.subscribed_tracks.clone();
        let messages = self.messages.clone();
        let playout_buffer = self.playout_buffer.clone();
        let hand_raise = self.hand_raise.clone();
        let last_meet_url = self.last_meet_url.clone();
        let chat_open = self.chat_open.clone();
        let unread_count = self.unread_count.clone();
        let bandwidth_ctrl = self.bandwidth.clone();
        let high_quality_mode = self.high_quality_mode.clone();

        tokio::spawn(async move {
            Self::event_loop(
                events,
                emitter,
                participants,
                connection_state,
                room_ref,
                subscribed_tracks,
                messages,
                playout_buffer,
                hand_raise,
                last_meet_url,
                chat_open,
                unread_count,
                bandwidth_ctrl,
                high_quality_mode,
            )
            .await;
        });

        Ok(())
    }

    /// Disconnect from the current room.
    pub async fn disconnect(&self) {
        // Cancel any in-progress lobby polling
        self.lobby_cancel.notify_one();
        *self.lobby_cookie.lock().await = None;

        // Clear reconnection info BEFORE closing — so the event loop
        // knows this disconnect is intentional.
        *self.last_meet_url.lock().await = None;
        *self.last_username.lock().await = None;

        let room = self.room.lock().await.take();
        if let Some(room) = room
            && let Err(e) = room.close().await
        {
            tracing::warn!("error closing room: {e}");
        }
        self.participants.lock().await.clear();
        self.subscribed_tracks.lock().await.clear();
        self.messages.lock().await.clear();
        self.playout_buffer.clear();
        // Reset bandwidth controller
        self.bandwidth.lock().unwrap().reset();
        // Clear hand raise state
        if let Some(hm) = self.hand_raise.lock().await.take() {
            hm.clear().await;
        }
        self.set_connection_state(ConnectionState::Disconnected)
            .await;
    }

    /// Raise the local participant's hand.
    pub async fn raise_hand(&self) -> Result<(), VisioError> {
        let hm = self.hand_raise.lock().await;
        hm.as_ref()
            .ok_or(VisioError::Room("not connected".into()))?
            .raise_hand()
            .await
    }

    /// Lower the local participant's hand.
    pub async fn lower_hand(&self) -> Result<(), VisioError> {
        let hm = self.hand_raise.lock().await;
        hm.as_ref()
            .ok_or(VisioError::Room("not connected".into()))?
            .lower_hand()
            .await
    }

    /// Lower all raised hands (admin action).
    pub async fn lower_all_hands(&self) -> Result<(), VisioError> {
        let hm = self.hand_raise.lock().await;
        hm.as_ref()
            .ok_or(VisioError::Room("not connected".into()))?
            .lower_all_hands()
            .await
    }

    /// Allowed emoji IDs for reactions (matches Meet web client).
    const ALLOWED_EMOJIS: &'static [&'static str] =
        &["thumbsUp", "clap", "joy", "openMouth", "tada", "heart"];

    /// Send an animated reaction visible to all participants.
    ///
    /// The payload matches the Meet web client protocol:
    /// `{ "type": "reactionReceived", "data": { "emoji": "<id>" } }`
    /// Rate limited to 10/sec (100ms minimum interval).
    pub async fn send_reaction(&self, emoji: &str) -> Result<(), VisioError> {
        // Validate emoji
        if !Self::ALLOWED_EMOJIS.contains(&emoji) {
            return Err(VisioError::Room(format!("unknown reaction emoji: {emoji}")));
        }

        // Rate limit: 100ms between reactions
        {
            let mut last = self.last_reaction_time.lock().await;
            let now = std::time::Instant::now();
            if let Some(prev) = *last
                && now.duration_since(prev) < std::time::Duration::from_millis(100)
            {
                return Err(VisioError::Room("reaction rate limited".into()));
            }
            *last = Some(now);
        }

        let room = self.room.lock().await;
        let room = room
            .as_ref()
            .ok_or_else(|| VisioError::Room("not connected".into()))?;

        let payload = serde_json::json!({
            "type": "reactionReceived",
            "data": { "emoji": emoji }
        });
        let data = payload.to_string().into_bytes();

        room.local_participant()
            .publish_data(DataPacket {
                payload: data,
                reliable: true,
                ..Default::default()
            })
            .await
            .map_err(|e| VisioError::Room(format!("send reaction: {e}")))?;

        Ok(())
    }

    /// Request all participants to mute their microphones (admin action).
    /// Sends a data message that interoperable clients interpret as "mute yourself".
    pub async fn mute_everyone(&self) -> Result<(), VisioError> {
        let room = self.room.lock().await;
        let room = room
            .as_ref()
            .ok_or_else(|| VisioError::Room("not connected".into()))?;

        let payload = serde_json::json!({ "type": "muteEveryone" });
        let data = payload.to_string().into_bytes();
        room.local_participant()
            .publish_data(livekit::DataPacket {
                payload: data,
                reliable: true,
                ..Default::default()
            })
            .await
            .map_err(|e| VisioError::Room(format!("mute everyone: {e}")))?;

        tracing::info!("mute everyone request sent (admin action)");
        Ok(())
    }

    /// Request a specific participant to mute their microphone (admin action).
    /// Sends a targeted data message only to that participant.
    pub async fn mute_participant(&self, identity: &str) -> Result<(), VisioError> {
        let room = self.room.lock().await;
        let room = room
            .as_ref()
            .ok_or_else(|| VisioError::Room("not connected".into()))?;

        let payload = serde_json::json!({ "type": "muteEveryone" });
        let data = payload.to_string().into_bytes();
        room.local_participant()
            .publish_data(livekit::DataPacket {
                payload: data,
                topic: None,
                reliable: true,
                destination_identities: vec![livekit::id::ParticipantIdentity(
                    identity.to_string(),
                )],
            })
            .await
            .map_err(|e| VisioError::Room(format!("mute participant: {e}")))?;

        tracing::info!("mute request sent to participant {identity}");
        Ok(())
    }

    /// Set subscribe video quality for all remote video tracks.
    /// `quality` is "high", "medium", or "low".
    pub async fn set_subscribe_video_quality(&self, quality: &str) -> Result<(), VisioError> {
        use livekit::track::VideoQuality;

        let vq = match quality {
            "high" => VideoQuality::High,
            "medium" => VideoQuality::Medium,
            "low" => VideoQuality::Low,
            _ => return Err(VisioError::Room(format!("unknown quality: {quality}"))),
        };

        let room = self.room.lock().await;
        let room = room
            .as_ref()
            .ok_or_else(|| VisioError::Room("not connected".into()))?;

        for (_sid, participant) in room.remote_participants() {
            for (_tsid, pub_) in participant.track_publications() {
                if pub_.kind() == LkTrackKind::Video {
                    pub_.set_video_quality(vq);
                }
            }
        }

        tracing::info!("subscribe video quality set to {quality}");
        Ok(())
    }

    /// Check if the local participant's hand is currently raised.
    pub async fn is_hand_raised(&self) -> bool {
        let hm = self.hand_raise.lock().await;
        match hm.as_ref() {
            Some(hm) => hm.is_hand_raised().await,
            None => false,
        }
    }

    /// Get stored connection info for reconnection.
    pub async fn last_connection_info(&self) -> Option<(String, Option<String>)> {
        let url = self.last_meet_url.lock().await.clone();
        let username = self.last_username.lock().await.clone();
        url.map(|u| (u, username))
    }

    /// Attempt to reconnect to the last room with exponential backoff.
    ///
    /// Called by native UI when ConnectionLost is received.
    pub async fn reconnect(&self) -> Result<(), VisioError> {
        let (meet_url, username) = self
            .last_connection_info()
            .await
            .ok_or_else(|| VisioError::Connection("no previous connection info".into()))?;

        let max_attempts: u32 = 10;
        let base_delay = std::time::Duration::from_secs(1);
        let max_delay = std::time::Duration::from_secs(30);

        for attempt in 1..=max_attempts {
            self.set_connection_state(ConnectionState::Reconnecting { attempt })
                .await;

            tracing::info!("reconnection attempt {attempt}/{max_attempts}");

            match self.connect(&meet_url, username.as_deref(), None).await {
                Ok(()) => {
                    tracing::info!("reconnection successful on attempt {attempt}");
                    return Ok(());
                }
                Err(e) => {
                    tracing::warn!("reconnection attempt {attempt}/{max_attempts} failed: {e}");
                    if attempt < max_attempts {
                        let delay = base_delay
                            .checked_mul(2u32.pow(attempt - 1))
                            .unwrap_or(max_delay)
                            .min(max_delay);
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }

        // All attempts failed — clear connection info and report disconnect
        *self.last_meet_url.lock().await = None;
        *self.last_username.lock().await = None;
        self.set_connection_state(ConnectionState::Disconnected)
            .await;
        Err(VisioError::Connection(
            "reconnection failed after all attempts".into(),
        ))
    }

    /// Enter the waiting room lobby and start polling for entry approval.
    async fn enter_lobby(&self, meet_url: &str, username: &str) -> Result<(), VisioError> {
        use crate::lobby::{LobbyPollResult, LobbyService};

        let (participant_id, lobby_cookie, poll_result) =
            LobbyService::request_entry(meet_url, username).await?;

        tracing::info!("lobby entry requested: participant_id={participant_id}");

        *self.lobby_cookie.lock().await = Some(lobby_cookie.clone());

        match poll_result {
            LobbyPollResult::Accepted { livekit_url, token } => {
                tracing::info!("immediately accepted into room");
                return self.connect_with_token(&livekit_url, &token).await;
            }
            LobbyPollResult::Denied => {
                self.emitter.emit(VisioEvent::LobbyDenied);
                self.set_connection_state(ConnectionState::Disconnected)
                    .await;
                return Err(VisioError::Auth("entry denied by host".to_string()));
            }
            LobbyPollResult::Waiting => {
                // Fall through to start polling
            }
        }

        self.set_connection_state(ConnectionState::WaitingForHost)
            .await;

        // Clone Arcs for the spawned polling task
        let meet_url = meet_url.to_string();
        let username = username.to_string();
        let lobby_cookie_arc = self.lobby_cookie.clone();
        let lobby_cancel = self.lobby_cancel.clone();
        let room = self.room.clone();
        let participants = self.participants.clone();
        let subscribed_tracks = self.subscribed_tracks.clone();
        let messages = self.messages.clone();
        let playout_buffer = self.playout_buffer.clone();
        let hand_raise = self.hand_raise.clone();
        let _camera_enabled = self.camera_enabled.clone();
        let connection_state = self.connection_state.clone();
        let emitter = self.emitter.clone();
        let last_meet_url = self.last_meet_url.clone();
        let chat_open = self.chat_open.clone();
        let unread_count = self.unread_count.clone();
        let bandwidth_ctrl = self.bandwidth.clone();
        let high_quality_mode = self.high_quality_mode.clone();

        tokio::spawn(async move {
            let lobby_deadline =
                tokio::time::Instant::now() + std::time::Duration::from_secs(10 * 60); // 10 minutes

            loop {
                tokio::select! {
                    _ = lobby_cancel.notified() => {
                        tracing::info!("lobby polling cancelled");
                        break;
                    }
                    _ = tokio::time::sleep_until(lobby_deadline) => {
                        tracing::warn!("lobby wait timed out after 10 minutes");
                        emitter.emit(VisioEvent::LobbyTimeout);
                        *connection_state.lock().await = ConnectionState::Disconnected;
                        emitter.emit(VisioEvent::ConnectionStateChanged(
                            ConnectionState::Disconnected,
                        ));
                        break;
                    }
                    _ = tokio::time::sleep(std::time::Duration::from_secs(3)) => {
                        let cookie = lobby_cookie_arc.lock().await.clone().unwrap_or_default();
                        if cookie.is_empty() {
                            tracing::warn!("lobby cookie missing, stopping poll");
                            break;
                        }

                        match LobbyService::poll_entry(&meet_url, &username, &cookie).await {
                            Ok(LobbyPollResult::Accepted { livekit_url, token }) => {
                                tracing::info!("lobby entry accepted, connecting to room");
                                *connection_state.lock().await = ConnectionState::Connecting;
                                emitter.emit(VisioEvent::ConnectionStateChanged(
                                    ConnectionState::Connecting,
                                ));

                                let mut options = RoomOptions::default();
                                options.auto_subscribe = true;
                                options.adaptive_stream = true;
                                options.dynacast = true;

                                match Room::connect(&livekit_url, &token, options).await {
                                    Ok((lk_room, events)) => {
                                        let lk_room = Arc::new(lk_room);

                                        // Store local participant SID
                                        {
                                            let local = lk_room.local_participant();
                                            let mut pm = participants.lock().await;
                                            pm.set_local_sid(local.sid().to_string());
                                        }

                                        // Seed existing remote participants
                                        {
                                            let mut pm = participants.lock().await;
                                            for (_, participant) in lk_room.remote_participants() {
                                                let info = RoomManager::remote_participant_to_info(&participant);
                                                pm.add_participant(info.clone());
                                                emitter.emit(VisioEvent::ParticipantJoined(info));
                                            }
                                        }

                                        // Store room reference
                                        *room.lock().await = Some(lk_room.clone());

                                        // Initialize HandRaiseManager
                                        {
                                            let hm = HandRaiseManager::new(
                                                lk_room.clone(),
                                                emitter.clone(),
                                            );
                                            *hand_raise.lock().await = Some(hm);
                                        }

                                        // Update state to connected
                                        *connection_state.lock().await = ConnectionState::Connected;
                                        emitter.emit(VisioEvent::ConnectionStateChanged(
                                            ConnectionState::Connected,
                                        ));

                                        // Spawn event loop
                                        let ev_emitter = emitter.clone();
                                        let ev_participants = participants.clone();
                                        let ev_connection_state = connection_state.clone();
                                        let ev_room_ref = room.clone();
                                        let ev_subscribed_tracks = subscribed_tracks.clone();
                                        let ev_messages = messages.clone();
                                        let ev_playout_buffer = playout_buffer.clone();
                                        let ev_hand_raise = hand_raise.clone();
                                        let ev_last_meet_url = last_meet_url.clone();
                                        let ev_chat_open = chat_open.clone();
                                        let ev_unread_count = unread_count.clone();
                                        let ev_bandwidth_ctrl = bandwidth_ctrl.clone();
                                        let ev_high_quality_mode = high_quality_mode.clone();

                                        tokio::spawn(async move {
                                            RoomManager::event_loop(
                                                events,
                                                ev_emitter,
                                                ev_participants,
                                                ev_connection_state,
                                                ev_room_ref,
                                                ev_subscribed_tracks,
                                                ev_messages,
                                                ev_playout_buffer,
                                                ev_hand_raise,
                                                ev_last_meet_url,
                                                ev_chat_open,
                                                ev_unread_count,
                                                ev_bandwidth_ctrl,
                                                ev_high_quality_mode,
                                            )
                                            .await;
                                        });
                                    }
                                    Err(e) => {
                                        tracing::error!("failed to connect after lobby acceptance: {e}");
                                        *connection_state.lock().await = ConnectionState::Disconnected;
                                        emitter.emit(VisioEvent::ConnectionStateChanged(
                                            ConnectionState::Disconnected,
                                        ));
                                    }
                                }
                                break;
                            }
                            Ok(LobbyPollResult::Denied) => {
                                tracing::info!("lobby entry denied by host");
                                emitter.emit(VisioEvent::LobbyDenied);
                                *connection_state.lock().await = ConnectionState::Disconnected;
                                emitter.emit(VisioEvent::ConnectionStateChanged(
                                    ConnectionState::Disconnected,
                                ));
                                break;
                            }
                            Ok(LobbyPollResult::Waiting) => {
                                tracing::debug!("still waiting in lobby...");
                            }
                            Err(e) => {
                                tracing::warn!("lobby poll error (will retry): {e}");
                            }
                        }
                    }
                }
            }
        });

        Ok(())
    }

    /// List participants currently waiting in the lobby (host only).
    pub async fn list_waiting_participants(
        &self,
    ) -> Result<Vec<crate::lobby::WaitingParticipant>, VisioError> {
        let meet_url = self
            .last_meet_url
            .lock()
            .await
            .clone()
            .ok_or_else(|| VisioError::Room("not connected".to_string()))?;
        let cookie = self
            .session_cookie
            .lock()
            .await
            .clone()
            .ok_or_else(|| VisioError::Room("not authenticated".to_string()))?;
        crate::lobby::LobbyService::list_waiting(&meet_url, &cookie).await
    }

    /// Allow or deny a waiting participant (host only).
    pub async fn handle_lobby_entry(
        &self,
        participant_id: &str,
        allow: bool,
    ) -> Result<(), VisioError> {
        let meet_url = self
            .last_meet_url
            .lock()
            .await
            .clone()
            .ok_or_else(|| VisioError::Room("not connected".to_string()))?;
        let cookie = self
            .session_cookie
            .lock()
            .await
            .clone()
            .ok_or_else(|| VisioError::Room("not authenticated".to_string()))?;
        crate::lobby::LobbyService::handle_entry(&meet_url, &cookie, participant_id, allow).await
    }

    /// Cancel lobby polling and clear lobby state.
    pub async fn cancel_lobby(&self) {
        self.lobby_cancel.notify_one();
        *self.lobby_cookie.lock().await = None;
    }

    /// Start polling the Meet API for waiting lobby participants (host side).
    /// Emits LobbyParticipantJoined/Left events when changes are detected.
    async fn start_lobby_host_polling(&self) {
        let meet_url = self.last_meet_url.lock().await.clone();
        let cookie = self.session_cookie.lock().await.clone();

        let (meet_url, cookie) = match (meet_url, cookie) {
            (Some(u), Some(c)) => (u, c),
            _ => return, // Not authenticated, skip
        };

        let emitter = self.emitter.clone();
        // Use the room reference to detect disconnection instead of lobby_cancel
        // (lobby_cancel is shared with the guest lobby polling and may already be notified)
        let connection_state = self.connection_state.clone();

        tracing::info!("starting lobby host polling for {}", meet_url);

        tokio::spawn(async move {
            use std::collections::HashSet;
            let mut known_ids: HashSet<String> = HashSet::new();

            loop {
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;

                // Stop polling if disconnected
                let state = connection_state.lock().await.clone();
                if matches!(state, ConnectionState::Disconnected) {
                    tracing::info!("lobby host polling stopped (disconnected)");
                    break;
                }

                match crate::lobby::LobbyService::list_waiting(&meet_url, &cookie).await {
                    Ok(participants) => {
                        let current_ids: HashSet<String> =
                            participants.iter().map(|p| p.id.clone()).collect();

                        // Detect new participants
                        for p in &participants {
                            if !known_ids.contains(&p.id) {
                                tracing::info!(
                                    "lobby: new waiting participant: {} ({})",
                                    p.username,
                                    p.id
                                );
                                emitter.emit(VisioEvent::LobbyParticipantJoined {
                                    id: p.id.clone(),
                                    username: p.username.clone(),
                                });
                            }
                        }

                        // Detect departed participants
                        for id in &known_ids {
                            if !current_ids.contains(id) {
                                tracing::info!("lobby: participant left: {}", id);
                                emitter.emit(VisioEvent::LobbyParticipantLeft { id: id.clone() });
                            }
                        }

                        known_ids = current_ids;
                    }
                    Err(e) => {
                        tracing::debug!("lobby host poll error (will retry): {e}");
                    }
                }
            }
        });
    }

    async fn set_connection_state(&self, state: ConnectionState) {
        *self.connection_state.lock().await = state.clone();
        self.emitter.emit(VisioEvent::ConnectionStateChanged(state));
    }

    fn lk_source_to_visio(source: LkTrackSource) -> TrackSource {
        match source {
            LkTrackSource::Microphone => TrackSource::Microphone,
            LkTrackSource::Camera => TrackSource::Camera,
            LkTrackSource::Screenshare => TrackSource::ScreenShare,
            _ => TrackSource::Unknown,
        }
    }

    fn remote_participant_to_info(p: &RemoteParticipant) -> ParticipantInfo {
        let name = {
            let n = p.name().to_string();
            if n.is_empty() { None } else { Some(n) }
        };

        // Only use publication metadata for audio mute state.
        // Video state (has_video / video_track_sid) is set exclusively by
        // TrackSubscribed events to avoid a race where the UI creates a
        // VideoSurfaceView before the track is actually subscribed, leading
        // to a permanent black tile (attachSurface finds no track in the
        // subscribed_tracks registry).
        let is_muted = p
            .track_publications()
            .values()
            .any(|pub_| pub_.kind() == LkTrackKind::Audio && pub_.is_muted());

        let attrs = p.attributes();
        let color = attrs.get("color").cloned().filter(|s| !s.is_empty());
        let is_admin = attrs.get("room_admin").is_some_and(|v| v == "true");

        ParticipantInfo {
            sid: p.sid().to_string(),
            identity: p.identity().to_string(),
            name,
            is_muted,
            has_video: false,
            video_track_sid: None,
            has_screen_share: false,
            screen_share_track_sid: None,
            connection_quality: ConnectionQuality::Good,
            color,
            is_admin,
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn event_loop(
        mut events: tokio::sync::mpsc::UnboundedReceiver<RoomEvent>,
        emitter: EventEmitter,
        participants: Arc<Mutex<ParticipantManager>>,
        connection_state: Arc<Mutex<ConnectionState>>,
        room_ref: Arc<Mutex<Option<Arc<Room>>>>,
        subscribed_tracks: Arc<Mutex<HashMap<String, RemoteVideoTrack>>>,
        messages: MessageStore,
        playout_buffer: Arc<AudioPlayoutBuffer>,
        hand_raise: Arc<Mutex<Option<HandRaiseManager>>>,
        last_meet_url: Arc<Mutex<Option<String>>>,
        chat_open: Arc<AtomicBool>,
        unread_count: Arc<AtomicU32>,
        bandwidth_ctrl: Arc<std::sync::Mutex<bandwidth::BandwidthController>>,
        high_quality_mode: Arc<AtomicBool>,
    ) {
        let mut ctx = EventLoopContext {
            emitter,
            participants,
            connection_state,
            room_ref,
            subscribed_tracks,
            messages,
            playout_buffer,
            hand_raise,
            last_meet_url,
            chat_open,
            unread_count,
            bandwidth_ctrl,
            high_quality_mode,
            reconnect_attempt: 0,
            audio_stream_tasks: HashMap::new(),
            idle_timer: None,
        };

        while let Some(event) = events.recv().await {
            match event {
                RoomEvent::Connected { .. } => ctx.handle_connected().await,
                RoomEvent::Reconnecting => ctx.handle_reconnecting().await,
                RoomEvent::Reconnected => ctx.handle_reconnected().await,
                RoomEvent::Disconnected { reason } => {
                    ctx.handle_disconnected(reason).await;
                    break;
                }
                RoomEvent::ParticipantConnected(participant) => {
                    ctx.handle_participant_connected(&participant).await;
                }
                RoomEvent::ParticipantDisconnected(participant) => {
                    ctx.handle_participant_disconnected(&participant).await;
                }
                RoomEvent::TrackSubscribed {
                    track,
                    publication,
                    participant,
                } => {
                    let source = Self::lk_source_to_visio(publication.source());
                    let track_kind = match publication.kind() {
                        LkTrackKind::Audio => TrackKind::Audio,
                        LkTrackKind::Video => TrackKind::Video,
                    };
                    let psid = participant.sid().to_string();
                    let track_sid = track.sid().to_string();
                    let identity = participant.identity().to_string();

                    ctx.update_participant_video_on_subscribe(&psid, &track_sid, track_kind, source).await;
                    ctx.store_video_track(&track_sid, &track, track_kind, source).await;
                    ctx.start_audio_playout(&track_sid, &track, track_kind);

                    let info = TrackInfo {
                        sid: track_sid,
                        participant_sid: psid,
                        participant_identity: identity,
                        kind: track_kind,
                        source,
                    };
                    ctx.emitter.emit(VisioEvent::TrackSubscribed(info));
                }
                RoomEvent::TrackUnsubscribed {
                    track,
                    publication,
                    participant,
                } => {
                    let psid = participant.sid().to_string();
                    let track_sid = track.sid().to_string();
                    let is_video = publication.kind() == LkTrackKind::Video;
                    let is_audio = publication.kind() == LkTrackKind::Audio;
                    let lk_source = publication.source();

                    if is_video {
                        ctx.clear_participant_video(&psid, lk_source).await;
                        ctx.subscribed_tracks.lock().await.remove(&track_sid);
                    }
                    if is_audio {
                        if let Some(handle) = ctx.audio_stream_tasks.remove(&track_sid) {
                            handle.abort();
                            tracing::info!("audio playout stream aborted for track {track_sid}");
                        }
                    }
                    ctx.emitter.emit(VisioEvent::TrackUnsubscribed(track_sid));
                }
                RoomEvent::TrackMuted {
                    participant,
                    publication,
                } => {
                    let psid = participant.sid().to_string();
                    let track_sid = publication.sid().to_string();
                    let source = Self::lk_source_to_visio(publication.source());
                    tracing::info!(
                        participant_sid = %psid,
                        track_sid = %track_sid,
                        source = ?source,
                        "TrackMuted: track still in subscribed_tracks={}",
                        ctx.subscribed_tracks.lock().await.contains_key(&track_sid),
                    );
                    ctx.apply_mute_state(&psid, source).await;
                    ctx.emitter.emit(VisioEvent::TrackMuted {
                        participant_sid: psid,
                        source,
                    });
                }
                RoomEvent::TrackUnmuted {
                    participant,
                    publication,
                } => {
                    let psid = participant.sid().to_string();
                    let source = Self::lk_source_to_visio(publication.source());
                    let track_sid = publication.sid().to_string();
                    tracing::info!(
                        participant_sid = %psid,
                        track_sid = %track_sid,
                        source = ?source,
                        "TrackUnmuted: track in subscribed_tracks={}",
                        ctx.subscribed_tracks.lock().await.contains_key(&track_sid),
                    );
                    ctx.apply_unmute_state(&psid, source, track_sid).await;
                    ctx.emitter.emit(VisioEvent::TrackUnmuted {
                        participant_sid: psid,
                        source,
                    });
                }
                RoomEvent::ActiveSpeakersChanged { speakers } => {
                    let sids: Vec<String> = speakers.iter().map(|p| p.sid().to_string()).collect();
                    ctx.handle_active_speakers_changed(sids).await;
                }
                RoomEvent::ParticipantAttributesChanged {
                    participant,
                    changed_attributes,
                } => {
                    let psid = participant.sid().to_string();
                    ctx.handle_participant_attributes_changed(psid, changed_attributes).await;
                }
                RoomEvent::ConnectionQualityChanged {
                    quality,
                    participant,
                } => {
                    let psid = participant.sid().to_string();
                    let q = convert_connection_quality(quality);
                    ctx.handle_connection_quality_changed(psid, q).await;
                }
                RoomEvent::ChatMessage {
                    message,
                    participant,
                    ..
                } => {
                    let sender_sid = participant
                        .as_ref()
                        .map(|p| p.sid().to_string())
                        .unwrap_or_default();
                    let sender_name = participant
                        .as_ref()
                        .map(|p| p.name().to_string())
                        .unwrap_or_default();
                    ctx.handle_chat_message(message.id, sender_sid, sender_name, message.message, message.timestamp as u64).await;
                }
                RoomEvent::TextStreamOpened {
                    reader,
                    topic,
                    participant_identity,
                } => {
                    if topic == "lk.chat" {
                        let messages = ctx.messages.clone();
                        let emitter = ctx.emitter.clone();
                        let room_ref = ctx.room_ref.clone();
                        let identity = participant_identity.to_string();
                        let chat_open = ctx.chat_open.clone();
                        let unread_count = ctx.unread_count.clone();
                        tokio::spawn(async move {
                            let reader = reader.take();
                            if reader.is_none() {
                                tracing::warn!("TextStreamOpened: reader already taken");
                                return;
                            }
                            let reader = reader.unwrap();
                            let stream_id = reader.info().id.clone();
                            let timestamp_ms = reader.info().timestamp.timestamp_millis() as u64;
                            match reader.read_all().await {
                                Ok(text) => {
                                    let sender_name = lookup_participant_name(&room_ref, &identity).await;
                                    let msg = ChatMessage {
                                        id: stream_id,
                                        sender_sid: identity,
                                        sender_name,
                                        text,
                                        timestamp_ms,
                                    };
                                    tracing::info!("Chat via TextStream: from={} text={}", msg.sender_name, msg.text);
                                    messages.lock().await.push(msg.clone());
                                    emitter.emit(VisioEvent::ChatMessageReceived(msg));
                                    if !chat_open.load(Ordering::Relaxed) {
                                        let count = unread_count.fetch_add(1, Ordering::Relaxed) + 1;
                                        emitter.emit(VisioEvent::UnreadCountChanged(count));
                                    }
                                }
                                Err(e) => {
                                    tracing::warn!("Failed to read chat text stream: {e}");
                                }
                            }
                        });
                    } else {
                        tracing::debug!("TextStreamOpened: topic={topic} (ignored)");
                    }
                }
                RoomEvent::DataReceived {
                    payload,
                    topic,
                    kind,
                    participant,
                } => {
                    let psid = participant
                        .as_ref()
                        .map(|p| p.sid().to_string())
                        .unwrap_or_default();
                    let sender_name = participant
                        .as_ref()
                        .map(|p| p.name().to_string())
                        .unwrap_or_default();
                    let topic_str = topic.as_deref().unwrap_or("none");
                    tracing::debug!(
                        "DataReceived: from={psid} topic={topic_str} kind={kind:?} len={}",
                        payload.len()
                    );
                    ctx.handle_data_received(&psid, &sender_name, topic_str, &payload, participant.is_none()).await;
                }
                _ => {
                    tracing::debug!("unhandled room event: {event:?}");
                }
            }
        }

        tracing::info!("room event loop ended");
    }
}

/// Holds shared state for the room event loop, reducing parameter passing.
struct EventLoopContext {
    emitter: EventEmitter,
    participants: Arc<Mutex<ParticipantManager>>,
    connection_state: Arc<Mutex<ConnectionState>>,
    room_ref: Arc<Mutex<Option<Arc<Room>>>>,
    subscribed_tracks: Arc<Mutex<HashMap<String, RemoteVideoTrack>>>,
    messages: MessageStore,
    playout_buffer: Arc<AudioPlayoutBuffer>,
    hand_raise: Arc<Mutex<Option<HandRaiseManager>>>,
    last_meet_url: Arc<Mutex<Option<String>>>,
    chat_open: Arc<AtomicBool>,
    unread_count: Arc<AtomicU32>,
    bandwidth_ctrl: Arc<std::sync::Mutex<bandwidth::BandwidthController>>,
    high_quality_mode: Arc<AtomicBool>,
    reconnect_attempt: u32,
    audio_stream_tasks: HashMap<String, tokio::task::JoinHandle<()>>,
    idle_timer: Option<tokio::task::JoinHandle<()>>,
}

impl EventLoopContext {
    async fn handle_connected(&mut self) {
        self.reconnect_attempt = 0;
        *self.connection_state.lock().await = ConnectionState::Connected;
        self.emitter
            .emit(VisioEvent::ConnectionStateChanged(ConnectionState::Connected));
    }

    async fn handle_reconnecting(&mut self) {
        self.reconnect_attempt += 1;
        let state = ConnectionState::Reconnecting {
            attempt: self.reconnect_attempt,
        };
        *self.connection_state.lock().await = state.clone();
        self.emitter
            .emit(VisioEvent::ConnectionStateChanged(state));
    }

    async fn handle_reconnected(&mut self) {
        self.reconnect_attempt = 0;
        *self.connection_state.lock().await = ConnectionState::Connected;
        self.emitter
            .emit(VisioEvent::ConnectionStateChanged(ConnectionState::Connected));
    }

    async fn handle_disconnected(&mut self, reason: DisconnectReason) {
        tracing::info!("room disconnected: {reason:?}");

        let is_intentional = self.last_meet_url.lock().await.is_none();

        *self.connection_state.lock().await = ConnectionState::Disconnected;
        self.participants.lock().await.clear();
        self.subscribed_tracks.lock().await.clear();
        self.messages.lock().await.clear();
        self.playout_buffer.clear();
        if let Some(hm) = self.hand_raise.lock().await.take() {
            hm.clear().await;
        }
        for (sid, handle) in self.audio_stream_tasks.drain() {
            handle.abort();
            tracing::info!("audio playout stream aborted on disconnect: {sid}");
        }
        *self.room_ref.lock().await = None;

        if is_intentional {
            self.emitter.emit(VisioEvent::ConnectionStateChanged(
                ConnectionState::Disconnected,
            ));
        } else {
            emit_disconnect_reason(&self.emitter, reason);
        }
    }

    async fn handle_participant_connected(&mut self, participant: &RemoteParticipant) {
        let info = RoomManager::remote_participant_to_info(participant);
        self.participants
            .lock()
            .await
            .add_participant(info.clone());
        self.emitter.emit(VisioEvent::ParticipantJoined(info));
        if let Some(handle) = self.idle_timer.take() {
            handle.abort();
            self.emitter.emit(VisioEvent::AloneInRoomCancelled);
        }
    }

    async fn handle_participant_disconnected(&mut self, participant: &RemoteParticipant) {
        let sid = participant.sid().to_string();
        self.participants
            .lock()
            .await
            .remove_participant(&sid);
        self.emitter
            .emit(VisioEvent::ParticipantLeft(sid.clone()));
        self.maybe_start_idle_timer().await;
    }

    async fn maybe_start_idle_timer(&mut self) {
        let count = self.participants.lock().await.participant_count();
        if count == 0 && self.idle_timer.is_none() {
            let emitter_idle = self.emitter.clone();
            self.idle_timer = Some(tokio::spawn(async move {
                const IDLE_SECS: u32 = 120;
                let mut remaining = IDLE_SECS;
                while remaining > 0 {
                    emitter_idle.emit(VisioEvent::AloneInRoom {
                        remaining_secs: remaining,
                    });
                    tokio::time::sleep(std::time::Duration::from_secs(10)).await;
                    remaining = remaining.saturating_sub(10);
                }
                emitter_idle.emit(VisioEvent::AloneInRoom { remaining_secs: 0 });
            }));
        }
    }

    async fn update_participant_video_on_subscribe(
        &self,
        psid: &str,
        track_sid: &str,
        track_kind: TrackKind,
        source: TrackSource,
    ) {
        if track_kind != TrackKind::Video {
            return;
        }
        let mut pm = self.participants.lock().await;
        if let Some(p) = pm.participant_mut(psid) {
            match source {
                TrackSource::ScreenShare => {
                    p.has_screen_share = true;
                    p.screen_share_track_sid = Some(track_sid.to_string());
                    tracing::info!(
                        "screen share track subscribed: participant={psid}, track_sid={track_sid}"
                    );
                }
                _ => {
                    p.has_video = true;
                    p.video_track_sid = Some(track_sid.to_string());
                }
            }
        }
    }

    async fn store_video_track(
        &self,
        track_sid: &str,
        track: &livekit::track::RemoteTrack,
        track_kind: TrackKind,
        source: TrackSource,
    ) {
        if track_kind != TrackKind::Video {
            return;
        }
        if let livekit::track::RemoteTrack::Video(video_track) = track {
            self.subscribed_tracks
                .lock()
                .await
                .insert(track_sid.to_string(), video_track.clone());
            tracing::info!(
                "video track stored in registry: track_sid={}, source={:?}",
                track_sid,
                source
            );
        }
    }

    fn start_audio_playout(
        &mut self,
        track_sid: &str,
        track: &livekit::track::RemoteTrack,
        track_kind: TrackKind,
    ) {
        if track_kind != TrackKind::Audio {
            return;
        }
        if let livekit::track::RemoteTrack::Audio(audio_track) = track {
            let rtc_track = audio_track.rtc_track();
            let mut audio_stream = NativeAudioStream::new(rtc_track, 48_000, 1);
            let buf = self.playout_buffer.clone();
            let sid = track_sid.to_string();
            let handle = tokio::spawn(async move {
                tracing::info!("audio playout stream started for track {sid}");
                while let Some(frame) = audio_stream.next().await {
                    buf.push_samples_for_track(&sid, &frame.data);
                }
                buf.remove_track(&sid);
                tracing::info!("audio playout stream ended for track {sid}");
            });
            self.audio_stream_tasks
                .insert(track_sid.to_string(), handle);
        }
    }

    async fn clear_participant_video(&self, psid: &str, source: LkTrackSource) {
        let is_screen_share = source == LkTrackSource::Screenshare;
        let mut pm = self.participants.lock().await;
        if let Some(p) = pm.participant_mut(psid) {
            if is_screen_share {
                p.has_screen_share = false;
                p.screen_share_track_sid = None;
            } else {
                p.has_video = false;
                p.video_track_sid = None;
            }
        }
    }

    async fn apply_mute_state(&self, psid: &str, source: TrackSource) {
        let mut pm = self.participants.lock().await;
        if let Some(p) = pm.participant_mut(psid) {
            match source {
                TrackSource::Microphone => p.is_muted = true,
                TrackSource::Camera => {
                    p.has_video = false;
                    p.video_track_sid = None;
                }
                TrackSource::ScreenShare => {
                    p.has_screen_share = false;
                    p.screen_share_track_sid = None;
                }
                _ => {}
            }
        }
    }

    async fn apply_unmute_state(&self, psid: &str, source: TrackSource, track_sid: String) {
        let mut pm = self.participants.lock().await;
        if let Some(p) = pm.participant_mut(psid) {
            match source {
                TrackSource::Microphone => p.is_muted = false,
                TrackSource::Camera => {
                    p.has_video = true;
                    p.video_track_sid = Some(track_sid);
                }
                TrackSource::ScreenShare => {
                    p.has_screen_share = true;
                    p.screen_share_track_sid = Some(track_sid);
                }
                _ => {}
            }
        }
    }

    async fn handle_active_speakers_changed(&self, sids: Vec<String>) {
        self.participants
            .lock()
            .await
            .set_active_speakers(sids.clone());
        if let Some(hm) = self.hand_raise.lock().await.as_ref() {
            hm.start_auto_lower(sids.clone());
        }
        self.emitter
            .emit(VisioEvent::ActiveSpeakersChanged(sids));
    }

    async fn handle_participant_attributes_changed(
        &self,
        psid: String,
        changed_attributes: HashMap<String, String>,
    ) {
        {
            let mut pm = self.participants.lock().await;
            if let Some(p) = pm.participant_mut(&psid) {
                if let Some(color) = changed_attributes.get("color") {
                    p.color = if color.is_empty() {
                        None
                    } else {
                        Some(color.clone())
                    };
                }
                if let Some(admin) = changed_attributes.get("room_admin") {
                    p.is_admin = admin == "true";
                }
            }
        }

        if let Some(hm) = self.hand_raise.lock().await.as_ref() {
            hm.handle_participant_attributes(psid, &changed_attributes)
                .await;
        }
    }

    async fn handle_connection_quality_changed(&self, psid: String, q: ConnectionQuality) {
        {
            let mut pm = self.participants.lock().await;
            if let Some(p) = pm.participant_mut(&psid) {
                p.connection_quality = q.clone();
            }
        }

        self.emitter.emit(VisioEvent::ConnectionQualityChanged {
            participant_sid: psid.clone(),
            quality: q.clone(),
        });

        self.maybe_adapt_bandwidth(&psid, q).await;
    }

    async fn maybe_adapt_bandwidth(&self, psid: &str, quality: ConnectionQuality) {
        if self
            .high_quality_mode
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            return;
        }

        let local_sid_opt = self
            .participants
            .lock()
            .await
            .local_sid()
            .map(|s| s.to_string());
        if local_sid_opt.as_deref() != Some(psid) {
            return;
        }

        let new_mode = {
            let mut bw = self.bandwidth_ctrl.lock().unwrap();
            bw.update(quality)
        };
        if let Some(new_mode) = new_mode {
            tracing::info!("bandwidth mode changed to {:?}", new_mode);
            self.emitter
                .emit(VisioEvent::BandwidthModeChanged { mode: new_mode });
            self.apply_bandwidth_mode(new_mode).await;
        }
    }

    async fn apply_bandwidth_mode(&self, mode: bandwidth::BandwidthMode) {
        let lk_room = self.room_ref.lock().await;
        let Some(lk_room) = lk_room.as_ref() else {
            return;
        };

        let remote_participants = lk_room.remote_participants();
        let active = self
            .participants
            .lock()
            .await
            .active_speakers()
            .first()
            .cloned();

        for rp in remote_participants.values() {
            let rp_sid = rp.sid().to_string();
            for (_sid, pub_) in rp.track_publications() {
                if pub_.kind() != LkTrackKind::Video {
                    continue;
                }
                match mode {
                    bandwidth::BandwidthMode::Full => {
                        pub_.set_enabled(true);
                        pub_.set_video_quality(VideoQuality::High);
                    }
                    bandwidth::BandwidthMode::ReducedVideo => {
                        let is_active = active.as_deref() == Some(&rp_sid);
                        pub_.set_enabled(is_active);
                        if is_active {
                            pub_.set_video_quality(VideoQuality::Low);
                        }
                    }
                    bandwidth::BandwidthMode::AudioOnly => {
                        pub_.set_enabled(false);
                    }
                }
            }
        }
    }

    async fn handle_chat_message(
        &self,
        id: String,
        sender_sid: String,
        sender_name: String,
        text: String,
        timestamp_ms: u64,
    ) {
        tracing::info!("ChatMessage received: id={} text={}", id, text);
        let msg = ChatMessage {
            id,
            sender_sid,
            sender_name,
            text,
            timestamp_ms,
        };
        self.messages.lock().await.push(msg.clone());
        self.emitter.emit(VisioEvent::ChatMessageReceived(msg));
    }

    async fn handle_data_received(
        &self,
        psid: &str,
        sender_name: &str,
        topic_str: &str,
        payload: &[u8],
        is_self: bool,
    ) {
        if try_handle_lobby_notification(&self.emitter, topic_str, payload) {
            return;
        }
        if self.try_handle_admin_command(psid, payload).await {
            return;
        }
        if self.try_handle_reaction(psid, sender_name, payload, is_self).await {
            return;
        }
        self.try_handle_legacy_chat(psid, sender_name, topic_str, payload).await;
    }

    async fn try_handle_admin_command(&self, psid: &str, payload: &[u8]) -> bool {
        let Ok(text) = std::str::from_utf8(payload) else {
            return false;
        };
        let Ok(json) = serde_json::from_str::<serde_json::Value>(text) else {
            return false;
        };

        match json["type"].as_str() {
            Some("lowerAllHands") => {
                tracing::info!("received lowerAllHands from {psid}");
                if let Some(hm) = self.hand_raise.lock().await.as_ref() {
                    if hm.is_hand_raised().await {
                        let _ = hm.lower_hand().await;
                    }
                }
                true
            }
            Some("muteEveryone") => {
                tracing::info!("received muteEveryone from {psid}");
                self.emitter.emit(VisioEvent::MuteRequested);
                true
            }
            _ => false,
        }
    }

    async fn try_handle_reaction(
        &self,
        psid: &str,
        sender_name: &str,
        payload: &[u8],
        is_self: bool,
    ) -> bool {
        let Ok(text) = std::str::from_utf8(payload) else {
            return false;
        };
        let Ok(json) = serde_json::from_str::<serde_json::Value>(text) else {
            return false;
        };
        if json["type"].as_str() != Some("reactionReceived") {
            return false;
        }

        if is_self {
            tracing::debug!("ignoring self-echoed reaction (no participant)");
            return true;
        }
        let local_sid = self
            .participants
            .lock()
            .await
            .local_sid()
            .map(|s| s.to_string());
        if local_sid.as_deref() == Some(psid) {
            tracing::debug!("ignoring self-echoed reaction (SID match)");
            return true;
        }

        if let Some(emoji) = json["data"]["emoji"].as_str() {
            if !RoomManager::ALLOWED_EMOJIS.contains(&emoji) {
                tracing::debug!("ignoring unknown reaction emoji: {emoji}");
                return true;
            }
            self.emitter.emit(VisioEvent::ReactionReceived {
                participant_sid: psid.to_string(),
                participant_name: sender_name.to_string(),
                emoji: emoji.to_string(),
            });
        }
        true
    }

    async fn try_handle_legacy_chat(
        &self,
        psid: &str,
        sender_name: &str,
        topic_str: &str,
        payload: &[u8],
    ) {
        if topic_str != "lk-chat-topic" {
            return;
        }
        let Ok(text) = std::str::from_utf8(payload) else {
            return;
        };
        let Ok(json) = serde_json::from_str::<serde_json::Value>(text) else {
            return;
        };

        if json["ignoreLegacy"].as_bool() == Some(true) {
            tracing::debug!("Skipping legacy DataReceived (ignoreLegacy=true)");
            return;
        }

        let msg = ChatMessage {
            id: json["id"].as_str().unwrap_or("").to_string(),
            sender_sid: psid.to_string(),
            sender_name: sender_name.to_string(),
            text: json["message"].as_str().unwrap_or("").to_string(),
            timestamp_ms: json["timestamp"].as_u64().unwrap_or(0),
        };

        if !msg.text.is_empty() {
            tracing::info!("Chat via DataReceived: from={psid} text={}", msg.text);
            self.messages.lock().await.push(msg.clone());
            self.emitter.emit(VisioEvent::ChatMessageReceived(msg));
            if !self.chat_open.load(Ordering::Relaxed) {
                let count = self.unread_count.fetch_add(1, Ordering::Relaxed) + 1;
                self.emitter.emit(VisioEvent::UnreadCountChanged(count));
            }
        }
    }
}

/// Emit the appropriate disconnect event based on the reason.
fn emit_disconnect_reason(emitter: &EventEmitter, reason: DisconnectReason) {
    if reason == DisconnectReason::DuplicateIdentity {
        tracing::warn!("disconnected: duplicate identity (connected from another device)");
        emitter.emit(VisioEvent::DisconnectedDuplicateIdentity);
    } else if reason == DisconnectReason::ParticipantRemoved {
        tracing::warn!("disconnected: removed by admin");
        emitter.emit(VisioEvent::DisconnectedByAdmin);
    } else {
        emitter.emit(VisioEvent::ConnectionLost);
    }
}

/// Convert LiveKit connection quality to Visio connection quality.
fn convert_connection_quality(quality: LkConnectionQuality) -> ConnectionQuality {
    match quality {
        LkConnectionQuality::Excellent => ConnectionQuality::Excellent,
        LkConnectionQuality::Good => ConnectionQuality::Good,
        LkConnectionQuality::Poor => ConnectionQuality::Poor,
        LkConnectionQuality::Lost => ConnectionQuality::Lost,
    }
}

/// Handle lobby/waiting room data channel notifications.
fn try_handle_lobby_notification(emitter: &EventEmitter, topic_str: &str, payload: &[u8]) -> bool {
    if !topic_str.contains("lobby") && !topic_str.contains("waiting") {
        return false;
    }
    if let Ok(text) = std::str::from_utf8(payload) {
        tracing::info!("lobby notification received: {}", text);
        if let Ok(data) = serde_json::from_str::<serde_json::Value>(text) {
            let id = data
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let username = data
                .get("username")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown")
                .to_string();
            emitter.emit(VisioEvent::LobbyParticipantJoined { id, username });
        }
    }
    true
}

/// Look up a participant's display name from the room by identity.
async fn lookup_participant_name(
    room_ref: &Arc<Mutex<Option<Arc<Room>>>>,
    identity: &str,
) -> String {
    let room = room_ref.lock().await;
    room.as_ref()
        .and_then(|r| {
            r.remote_participants()
                .values()
                .find(|p| p.identity().to_string() == identity)
                .map(|p| p.name().to_string())
        })
        .unwrap_or_else(|| identity.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn local_participant_info_returns_none_when_disconnected() {
        let rm = RoomManager::new();
        // No room connected, so local_participant_info returns None
        assert!(rm.local_participant_info().await.is_none());
    }

    #[tokio::test]
    async fn camera_enabled_shared_with_controls() {
        let rm = RoomManager::new();
        let controls = rm.controls();

        // Default: camera disabled
        assert!(!controls.is_camera_enabled().await);

        // Modify camera_enabled via the shared Arc inside RoomManager
        *rm.camera_enabled.lock().await = true;

        // Controls should see the updated value
        assert!(controls.is_camera_enabled().await);
    }

    #[tokio::test]
    async fn initial_connection_state_is_disconnected() {
        let rm = RoomManager::new();
        assert_eq!(rm.connection_state().await, ConnectionState::Disconnected);
    }

    #[tokio::test]
    async fn participants_empty_when_disconnected() {
        let rm = RoomManager::new();
        // No room means no local participant, no remote participants
        let participants = rm.participants().await;
        assert!(participants.is_empty());
    }

    #[test]
    fn disconnect_reason_should_not_reconnect() {
        use livekit::DisconnectReason;
        // These reasons should NOT trigger reconnection
        let no_reconnect = [
            DisconnectReason::DuplicateIdentity,
            DisconnectReason::ParticipantRemoved,
            DisconnectReason::ClientInitiated,
        ];
        for reason in &no_reconnect {
            assert!(
                should_not_reconnect(*reason),
                "should not reconnect for {reason:?}"
            );
        }
        // These reasons SHOULD trigger reconnection
        let reconnect = [
            DisconnectReason::ServerShutdown,
            DisconnectReason::StateMismatch,
            DisconnectReason::UnknownReason,
            DisconnectReason::SignalClose,
            DisconnectReason::Migration,
            DisconnectReason::ConnectionTimeout,
            DisconnectReason::MediaFailure,
        ];
        for reason in &reconnect {
            assert!(
                !should_not_reconnect(*reason),
                "should reconnect for {reason:?}"
            );
        }
    }

    #[test]
    fn room_options_have_extended_timeouts() {
        // Meet: maxRetries=5, peerConnectionTimeout=60s
        let options = create_room_options(false);
        assert_eq!(options.join_retries, 5);
        assert_eq!(options.connect_timeout, std::time::Duration::from_secs(60));
    }

    #[test]
    fn allowed_emojis_are_valid() {
        for emoji in RoomManager::ALLOWED_EMOJIS {
            assert!(!emoji.is_empty());
        }
        assert!(RoomManager::ALLOWED_EMOJIS.contains(&"thumbsUp"));
        assert!(RoomManager::ALLOWED_EMOJIS.contains(&"heart"));
        assert!(!RoomManager::ALLOWED_EMOJIS.contains(&"invalid"));
    }

    #[tokio::test]
    async fn send_reaction_rejects_unknown_emoji() {
        let rm = RoomManager::new();
        let result = rm.send_reaction("unknown_emoji").await;
        assert!(result.is_err());
        let err_msg = format!("{:?}", result.unwrap_err());
        assert!(err_msg.contains("unknown reaction emoji"));
    }
}
