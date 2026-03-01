//! Visio Mobile core business logic.
//!
//! Pure Rust crate with no platform dependencies.
//! Consumed by native UI shells via UniFFI bindings.

pub mod auth;
pub mod chat;
pub mod controls;
pub mod errors;
pub mod events;
pub mod participants;
pub mod room;

pub use auth::{AuthService, TokenInfo};
pub use chat::ChatService;
pub use controls::MeetingControls;
pub use errors::VisioError;
pub use events::{
    ChatMessage, ConnectionQuality, ConnectionState, EventEmitter, ParticipantInfo, TrackInfo,
    TrackKind, TrackSource, VisioEvent, VisioEventListener,
};
pub use participants::ParticipantManager;
pub use room::RoomManager;
