//! Visio Mobile core business logic.
//!
//! Pure Rust crate with no platform dependencies.
//! Consumed by native UI shells via UniFFI bindings.

pub mod auth;
pub mod errors;
pub mod events;
pub mod participants;
pub mod room;

pub use errors::VisioError;
pub use events::{
    ChatMessage, ConnectionQuality, ConnectionState, EventEmitter, ParticipantInfo, TrackInfo,
    TrackKind, TrackSource, VisioEvent, VisioEventListener,
};
