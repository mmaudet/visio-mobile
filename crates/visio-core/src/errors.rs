use thiserror::Error;

#[derive(Debug, Error)]
pub enum VisioError {
    #[error("connection failed: {0}")]
    Connection(String),
    #[error("room error: {0}")]
    Room(String),
    #[error("authentication failed: {0}")]
    Auth(String),
    #[error("HTTP request failed: {0}")]
    Http(String),
    #[error("invalid URL: {0}")]
    InvalidUrl(String),
}
