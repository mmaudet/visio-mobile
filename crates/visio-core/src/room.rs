use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use livekit::prelude::{Room, RoomEvent, RoomOptions, RemoteParticipant};
use livekit::track::{
    RemoteVideoTrack,
    TrackKind as LkTrackKind,
    TrackSource as LkTrackSource,
};
use livekit::participant::ConnectionQuality as LkConnectionQuality;
use livekit::data_stream::StreamReader;
use livekit::webrtc::audio_stream::native::NativeAudioStream;
use futures_util::StreamExt;

use crate::audio_playout::AudioPlayoutBuffer;
use crate::auth::AuthService;
use crate::chat::MessageStore;
use crate::errors::VisioError;
use crate::events::{
    ChatMessage, ConnectionQuality, ConnectionState, EventEmitter, ParticipantInfo, TrackInfo,
    TrackKind, TrackSource, VisioEvent, VisioEventListener,
};
use crate::hand_raise::HandRaiseManager;
use crate::participants::ParticipantManager;

/// Manages the lifecycle of a LiveKit room connection.
pub struct RoomManager {
    room: Arc<Mutex<Option<Arc<Room>>>>,
    emitter: EventEmitter,
    participants: Arc<Mutex<ParticipantManager>>,
    connection_state: Arc<Mutex<ConnectionState>>,
    subscribed_tracks: Arc<Mutex<HashMap<String, RemoteVideoTrack>>>,
    messages: MessageStore,
    playout_buffer: Arc<AudioPlayoutBuffer>,
    hand_raise: Arc<Mutex<Option<HandRaiseManager>>>,
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
        }
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

    /// Create MeetingControls bound to this room.
    pub fn controls(&self) -> crate::controls::MeetingControls {
        crate::controls::MeetingControls::new(self.room.clone(), self.emitter.clone())
    }

    /// Create a ChatService bound to this room.
    pub fn chat(&self) -> crate::chat::ChatService {
        crate::chat::ChatService::new(self.room.clone(), self.emitter.clone(), self.messages.clone())
    }

    /// Get current connection state.
    pub async fn connection_state(&self) -> ConnectionState {
        self.connection_state.lock().await.clone()
    }

    /// Get a snapshot of current participants.
    pub async fn participants(&self) -> Vec<ParticipantInfo> {
        self.participants.lock().await.participants().to_vec()
    }

    /// Get local participant info (for self-view tile).
    pub async fn local_participant_info(&self) -> Option<ParticipantInfo> {
        let room = self.room.lock().await;
        let room = room.as_ref()?;
        let local = room.local_participant();
        let name = {
            let n = local.name().to_string();
            if n.is_empty() { None } else { Some(n) }
        };
        let has_video = local.track_publications().values().any(|pub_| {
            pub_.kind() == LkTrackKind::Video
        });
        let is_muted = local.track_publications().values().any(|pub_| {
            pub_.kind() == LkTrackKind::Audio && pub_.is_muted()
        });
        Some(ParticipantInfo {
            sid: local.sid().to_string(),
            identity: local.identity().to_string(),
            name,
            is_muted,
            has_video,
            video_track_sid: if has_video { Some("local-camera".to_string()) } else { None },
            connection_quality: ConnectionQuality::Excellent,
        })
    }

    /// Get current active speakers.
    pub async fn active_speakers(&self) -> Vec<String> {
        self.participants.lock().await.active_speakers().to_vec()
    }

    /// Get a subscribed remote video track by its SID.
    ///
    /// Returns `None` if the track is not currently subscribed.
    pub async fn get_video_track(&self, track_sid: &str) -> Option<RemoteVideoTrack> {
        self.subscribed_tracks.lock().await.get(track_sid).cloned()
    }

    /// Get all currently subscribed video track SIDs.
    pub async fn video_track_sids(&self) -> Vec<String> {
        self.subscribed_tracks.lock().await.keys().cloned().collect()
    }

    /// Connect to a room using the Meet API.
    ///
    /// Calls the Meet API to get a token, then connects to the LiveKit room.
    pub async fn connect(
        &self,
        meet_url: &str,
        username: Option<&str>,
    ) -> Result<(), VisioError> {
        self.set_connection_state(ConnectionState::Connecting).await;

        let token_info = AuthService::request_token(meet_url, username).await?;

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

        tokio::spawn(async move {
            Self::event_loop(events, emitter, participants, connection_state, room_ref, subscribed_tracks, messages, playout_buffer, hand_raise).await;
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
        self.subscribed_tracks.lock().await.clear();
        self.messages.lock().await.clear();
        self.playout_buffer.clear();
        // Clear hand raise state
        if let Some(hm) = self.hand_raise.lock().await.take() {
            hm.clear().await;
        }
        self.set_connection_state(ConnectionState::Disconnected).await;
    }

    /// Raise the local participant's hand.
    pub async fn raise_hand(&self) -> Result<(), VisioError> {
        let hm = self.hand_raise.lock().await;
        hm.as_ref().ok_or(VisioError::Room("not connected".into()))?.raise_hand().await
    }

    /// Lower the local participant's hand.
    pub async fn lower_hand(&self) -> Result<(), VisioError> {
        let hm = self.hand_raise.lock().await;
        hm.as_ref().ok_or(VisioError::Room("not connected".into()))?.lower_hand().await
    }

    /// Check if the local participant's hand is currently raised.
    pub async fn is_hand_raised(&self) -> bool {
        let hm = self.hand_raise.lock().await;
        match hm.as_ref() {
            Some(hm) => hm.is_hand_raised().await,
            None => false,
        }
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
        let is_muted = p.track_publications().values().any(|pub_| {
            pub_.kind() == LkTrackKind::Audio && pub_.is_muted()
        });

        ParticipantInfo {
            sid: p.sid().to_string(),
            identity: p.identity().to_string(),
            name,
            is_muted,
            has_video: false,
            video_track_sid: None,
            connection_quality: ConnectionQuality::Good,
        }
    }

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
    ) {
        let mut reconnect_attempt: u32 = 0;
        // Track active audio stream tasks so they get cancelled on disconnect
        let mut audio_stream_tasks: HashMap<String, tokio::task::JoinHandle<()>> = HashMap::new();

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
                    tracing::info!("room disconnected: {reason:?}");
                    *connection_state.lock().await = ConnectionState::Disconnected;
                    emitter.emit(VisioEvent::ConnectionStateChanged(ConnectionState::Disconnected));
                    participants.lock().await.clear();
                    subscribed_tracks.lock().await.clear();
                    messages.lock().await.clear();
                    playout_buffer.clear();
                    // Clear hand raise state
                    if let Some(hm) = hand_raise.lock().await.take() {
                        hm.clear().await;
                    }
                    // Stop all audio playout streams
                    for (sid, handle) in audio_stream_tasks.drain() {
                        handle.abort();
                        tracing::info!("audio playout stream aborted on disconnect: {sid}");
                    }
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
                    let source = Self::lk_source_to_visio(publication.source());
                    let track_kind = match publication.kind() {
                        LkTrackKind::Audio => TrackKind::Audio,
                        LkTrackKind::Video => TrackKind::Video,
                    };

                    let psid = participant.sid().to_string();
                    let track_sid = track.sid().to_string();

                    {
                        let mut pm = participants.lock().await;
                        if let Some(p) = pm.participant_mut(&psid) {
                            if track_kind == TrackKind::Video {
                                p.has_video = true;
                                p.video_track_sid = Some(track_sid.clone());
                            }
                        }
                    }

                    // Store video tracks in the registry for later retrieval
                    if track_kind == TrackKind::Video {
                        if let livekit::track::RemoteTrack::Video(video_track) = &track {
                            subscribed_tracks.lock().await
                                .insert(track_sid.clone(), video_track.clone());
                        }
                    }

                    // Start audio playout: create NativeAudioStream and feed
                    // decoded PCM frames into the shared playout buffer.
                    if track_kind == TrackKind::Audio {
                        if let livekit::track::RemoteTrack::Audio(audio_track) = &track {
                            let rtc_track = audio_track.rtc_track();
                            let mut audio_stream = NativeAudioStream::new(
                                rtc_track,
                                48_000, // sample rate
                                1,      // mono
                            );
                            let buf = playout_buffer.clone();
                            let sid = track_sid.clone();
                            let handle = tokio::spawn(async move {
                                tracing::info!("audio playout stream started for track {sid}");
                                while let Some(frame) = audio_stream.next().await {
                                    buf.push_samples(&frame.data);
                                }
                                tracing::info!("audio playout stream ended for track {sid}");
                            });
                            audio_stream_tasks.insert(track_sid.clone(), handle);
                        }
                    }

                    let info = TrackInfo {
                        sid: track_sid,
                        participant_sid: psid,
                        kind: track_kind,
                        source,
                    };
                    emitter.emit(VisioEvent::TrackSubscribed(info));
                }

                RoomEvent::TrackUnsubscribed { track, publication, participant } => {
                    let psid = participant.sid().to_string();
                    let track_sid = track.sid().to_string();
                    let is_video = publication.kind() == LkTrackKind::Video;
                    let is_audio = publication.kind() == LkTrackKind::Audio;

                    if is_video {
                        let mut pm = participants.lock().await;
                        if let Some(p) = pm.participant_mut(&psid) {
                            p.has_video = false;
                            p.video_track_sid = None;
                        }
                        subscribed_tracks.lock().await.remove(&track_sid);
                    }

                    if is_audio {
                        if let Some(handle) = audio_stream_tasks.remove(&track_sid) {
                            handle.abort();
                            tracing::info!("audio playout stream aborted for track {track_sid}");
                        }
                    }

                    emitter.emit(VisioEvent::TrackUnsubscribed(track_sid));
                }

                RoomEvent::TrackMuted { participant, publication } => {
                    let psid = participant.sid().to_string();
                    let source = Self::lk_source_to_visio(publication.source());

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
                    let source = Self::lk_source_to_visio(publication.source());

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
                    // Auto-lower hand if local participant is speaking with hand raised
                    if let Some(hm) = hand_raise.lock().await.as_ref() {
                        hm.start_auto_lower(sids.clone());
                    }
                    emitter.emit(VisioEvent::ActiveSpeakersChanged(sids));
                }

                RoomEvent::ParticipantAttributesChanged { participant, changed_attributes } => {
                    let psid = participant.sid().to_string();
                    if let Some(hm) = hand_raise.lock().await.as_ref() {
                        hm.handle_participant_attributes(psid, &changed_attributes).await;
                    }
                }

                RoomEvent::ConnectionQualityChanged { quality, participant } => {
                    let psid = participant.sid().to_string();
                    let q = match quality {
                        LkConnectionQuality::Excellent => ConnectionQuality::Excellent,
                        LkConnectionQuality::Good => ConnectionQuality::Good,
                        LkConnectionQuality::Poor => ConnectionQuality::Poor,
                        LkConnectionQuality::Lost => ConnectionQuality::Lost,
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

                RoomEvent::ChatMessage { message, participant, .. } => {
                    tracing::info!("ChatMessage received: id={} text={}", message.id, message.message);
                    let sender_sid = participant
                        .as_ref()
                        .map(|p| p.sid().to_string())
                        .unwrap_or_default();
                    let sender_name = participant
                        .as_ref()
                        .map(|p| p.name().to_string())
                        .unwrap_or_default();

                    let msg = ChatMessage {
                        id: message.id,
                        sender_sid,
                        sender_name,
                        text: message.message,
                        timestamp_ms: message.timestamp as u64,
                    };
                    messages.lock().await.push(msg.clone());
                    emitter.emit(VisioEvent::ChatMessageReceived(msg));
                }

                RoomEvent::TextStreamOpened { reader, topic, participant_identity } => {
                    if topic == "lk.chat" {
                        let messages = messages.clone();
                        let emitter = emitter.clone();
                        let room_ref = room_ref.clone();
                        let identity = participant_identity.to_string();

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
                                    // Look up participant name from room
                                    let sender_name = {
                                        let room = room_ref.lock().await;
                                        room.as_ref()
                                            .and_then(|r| {
                                                r.remote_participants()
                                                    .values()
                                                    .find(|p| p.identity().to_string() == identity)
                                                    .map(|p| p.name().to_string())
                                            })
                                            .unwrap_or_else(|| identity.clone())
                                    };

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

                RoomEvent::DataReceived { payload, topic, kind, participant } => {
                    let psid = participant
                        .as_ref()
                        .map(|p| p.sid().to_string())
                        .unwrap_or_default();
                    let topic_str = topic.as_deref().unwrap_or("none");
                    tracing::debug!(
                        "DataReceived: from={psid} topic={topic_str} kind={kind:?} len={}",
                        payload.len()
                    );

                    // Legacy fallback: chat messages via DataReceived with topic "lk-chat-topic"
                    // New clients send both Stream + legacy; "ignoreLegacy" flag means
                    // the TextStreamOpened handler already processed it.
                    if topic_str == "lk-chat-topic" {
                        if let Ok(text) = std::str::from_utf8(&payload) {
                            if let Ok(json) = serde_json::from_str::<serde_json::Value>(text) {
                                // Skip if sender uses Stream API (we handle it in TextStreamOpened)
                                if json["ignoreLegacy"].as_bool() == Some(true) {
                                    tracing::debug!("Skipping legacy DataReceived (ignoreLegacy=true)");
                                    continue;
                                }

                                let sender_name = participant
                                    .as_ref()
                                    .map(|p| p.name().to_string())
                                    .unwrap_or_default();

                                let msg = ChatMessage {
                                    id: json["id"].as_str().unwrap_or("").to_string(),
                                    sender_sid: psid.clone(),
                                    sender_name,
                                    text: json["message"].as_str().unwrap_or("").to_string(),
                                    timestamp_ms: json["timestamp"].as_u64().unwrap_or(0),
                                };

                                if !msg.text.is_empty() {
                                    tracing::info!("Chat via DataReceived: from={psid} text={}", msg.text);
                                    messages.lock().await.push(msg.clone());
                                    emitter.emit(VisioEvent::ChatMessageReceived(msg));
                                }
                            }
                        }
                    }
                }

                _ => {
                    tracing::debug!("unhandled room event: {event:?}");
                }
            }
        }

        tracing::info!("room event loop ended");
    }
}
