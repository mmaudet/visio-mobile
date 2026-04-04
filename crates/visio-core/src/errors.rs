use thiserror::Error;

#[derive(Debug, Error)]
pub enum VisioError {
    #[error("connection failed: {0}")]
    Connection(String),
    #[error("room error: {0}")]
    Room(String),
    #[error("authentication failed: {0}")]
    Auth(String),
    #[error("authentication required")]
    AuthRequired,
    #[error("HTTP request failed: {0}")]
    Http(String),
    #[error("invalid URL: {0}")]
    InvalidUrl(String),
    #[error("Session error: {0}")]
    Session(String),
    #[error("waiting for host approval")]
    WaitingForHost,
    #[error("device permission denied: {0}")]
    DevicePermissionDenied(String),
    #[error("device in use: {0}")]
    DeviceInUse(String),
    #[error("device not found: {0}")]
    DeviceNotFound(String),
    #[error("network unreachable: {0}")]
    NetworkUnreachable(String),
}
