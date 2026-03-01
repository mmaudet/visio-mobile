use std::sync::Arc;
use tokio::sync::Mutex;
use livekit::prelude::{Room, RoomEvent, RoomOptions, RemoteParticipant};
use livekit::track::{
    TrackKind as LkTrackKind,
    TrackSource as LkTrackSource,
};
use livekit::participant::ConnectionQuality as LkConnectionQuality;

use crate::auth::AuthService;
use crate::errors::VisioError;
use crate::events::{
    ConnectionQuality, ConnectionState, EventEmitter, ParticipantInfo, TrackInfo, TrackKind,
    TrackSource, VisioEvent, VisioEventListener,
};
use crate::participants::ParticipantManager;

/// Manages the lifecycle of a LiveKit room connection.
pub struct RoomManager {
    room: Arc<Mutex<Option<Arc<Room>>>>,
    emitter: EventEmitter,
    participants: Arc<Mutex<ParticipantManager>>,
    connection_state: Arc<Mutex<ConnectionState>>,
}

impl RoomManager {
    pub fn new() -> Self {
        Self {
            room: Arc::new(Mutex::new(None)),
            emitter: EventEmitter::new(),
            participants: Arc::new(Mutex::new(ParticipantManager::new())),
            connection_state: Arc::new(Mutex::new(ConnectionState::Disconnected)),
        }
    }

    /// Register a listener for room events.
    pub fn add_listener(&self, listener: Arc<dyn VisioEventListener>) {
        self.emitter.add_listener(listener);
    }

    /// Get current connection state.
    pub async fn connection_state(&self) -> ConnectionState {
        self.connection_state.lock().await.clone()
    }

    /// Get a snapshot of current participants.
    pub async fn participants(&self) -> Vec<ParticipantInfo> {
        self.participants.lock().await.participants().to_vec()
    }

    /// Get current active speakers.
    pub async fn active_speakers(&self) -> Vec<String> {
        self.participants.lock().await.active_speakers().to_vec()
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

        // Update state to connected
        self.set_connection_state(ConnectionState::Connected).await;

        // Spawn event loop
        let emitter = self.emitter.clone();
        let participants = self.participants.clone();
        let connection_state = self.connection_state.clone();
        let room_ref = self.room.clone();

        tokio::spawn(async move {
            Self::event_loop(events, emitter, participants, connection_state, room_ref).await;
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
        self.set_connection_state(ConnectionState::Disconnected).await;
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

        let has_video = p.track_publications().values().any(|pub_| {
            pub_.kind() == LkTrackKind::Video
        });

        let is_muted = p.track_publications().values().any(|pub_| {
            pub_.kind() == LkTrackKind::Audio && pub_.is_muted()
        });

        ParticipantInfo {
            sid: p.sid().to_string(),
            identity: p.identity().to_string(),
            name,
            is_muted,
            has_video,
            connection_quality: ConnectionQuality::Good,
        }
    }

    async fn event_loop(
        mut events: tokio::sync::mpsc::UnboundedReceiver<RoomEvent>,
        emitter: EventEmitter,
        participants: Arc<Mutex<ParticipantManager>>,
        connection_state: Arc<Mutex<ConnectionState>>,
        room_ref: Arc<Mutex<Option<Arc<Room>>>>,
    ) {
        let mut reconnect_attempt: u32 = 0;

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

                    {
                        let mut pm = participants.lock().await;
                        if let Some(p) = pm.participant_mut(&psid) {
                            if track_kind == TrackKind::Video {
                                p.has_video = true;
                            }
                        }
                    }

                    let info = TrackInfo {
                        sid: track.sid().to_string(),
                        participant_sid: psid,
                        kind: track_kind,
                        source,
                    };
                    emitter.emit(VisioEvent::TrackSubscribed(info));
                }

                RoomEvent::TrackUnsubscribed { track, publication, participant } => {
                    let psid = participant.sid().to_string();
                    let is_video = publication.kind() == LkTrackKind::Video;

                    if is_video {
                        let mut pm = participants.lock().await;
                        if let Some(p) = pm.participant_mut(&psid) {
                            p.has_video = false;
                        }
                    }

                    emitter.emit(VisioEvent::TrackUnsubscribed(track.sid().to_string()));
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
                    emitter.emit(VisioEvent::ActiveSpeakersChanged(sids));
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

                _ => {
                    // Ignore other events for now
                }
            }
        }

        tracing::info!("room event loop ended");
    }
}
