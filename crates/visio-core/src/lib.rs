//! Visio Mobile core business logic.
//!
//! Pure Rust crate with no platform dependencies.
//! Consumed by native UI shells via UniFFI bindings.

pub mod errors;
pub mod events;

pub use errors::VisioError;
pub use events::VisioEvent;
