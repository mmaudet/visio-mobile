//! Visio Mobile core business logic.
//!
//! Pure Rust crate with no platform dependencies.
//! Consumed by native UI shells via UniFFI bindings.

pub mod access;
pub mod adaptive;
pub mod audio_capture_buffer;
pub mod audio_playout;
pub mod auth;
pub mod bandwidth;
pub mod calendar;
pub mod chat;
pub mod controls;
pub mod errors;
pub mod events;
pub mod features;
pub mod hand_raise;
pub mod http;
pub mod init;
pub mod layout;
pub mod lobby;
pub mod participants;
pub mod pkce;
pub mod room;
pub mod room_display_name;
pub mod session;
pub mod settings;
pub mod subscriptions;
pub mod tokens;

pub use access::{AccessService, RoomAccess, UserSearchResult};
pub use audio_capture_buffer::{AudioCaptureBuffer, CapturedFrame};
pub use audio_playout::AudioPlayoutBuffer;
pub use auth::{AuthService, TokenInfo};
pub use calendar::CalendarService;
pub use chat::ChatService;
pub use controls::MeetingControls;
pub use errors::VisioError;
pub use events::{
    ChatMessage, ConnectionQuality, ConnectionState, EventEmitter, InitPhase, InitPhaseError,
    InitResult, ParticipantInfo, TrackInfo, TrackKind, TrackSource, VisioEvent, VisioEventListener,
};
pub use features::FeatureService;
pub use hand_raise::HandRaiseManager;
pub use init::{InitProgressListener, InitSequence, NoOpListener};
pub use lobby::{LobbyPollResult, LobbyService, LobbyStatus, WaitingParticipant};
pub use participants::ParticipantManager;
pub use pkce::PkceChallenge;
pub use room::RoomManager;
pub use room_display_name::{
    extract_room_display_name, strip_room_display_name_param, validate_room_display_name,
};
pub use session::{CreateRoomLiveKit, CreateRoomResponse, SessionManager, SessionState, UserInfo};
pub use settings::{CalendarRefreshInterval, Settings, SettingsStore};
pub use tokens::{TokenPair, TokenStore, exchange_pkce_code, refresh_tokens};
