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
pub mod chat;
pub mod controls;
pub mod errors;
pub mod events;
pub mod hand_raise;
pub mod lobby;
pub mod participants;
pub mod room;
pub mod session;
pub mod settings;

pub use access::{AccessService, RoomAccess, UserSearchResult};
pub use audio_capture_buffer::{AudioCaptureBuffer, CapturedFrame};
pub use audio_playout::AudioPlayoutBuffer;
pub use auth::{AuthService, TokenInfo};
pub use chat::ChatService;
pub use controls::MeetingControls;
pub use errors::VisioError;
pub use events::{
    ChatMessage, ConnectionQuality, ConnectionState, EventEmitter, ParticipantInfo, TrackInfo,
    TrackKind, TrackSource, VisioEvent, VisioEventListener,
};
pub use hand_raise::HandRaiseManager;
pub use lobby::{LobbyPollResult, LobbyService, LobbyStatus, WaitingParticipant};
pub use participants::ParticipantManager;
pub use room::RoomManager;
pub use session::{CreateRoomLiveKit, CreateRoomResponse, SessionManager, SessionState, UserInfo};
pub use settings::{RoomHistoryEntry, Settings, SettingsStore};
