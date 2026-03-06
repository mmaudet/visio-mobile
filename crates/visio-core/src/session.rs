//! Session management for OIDC SSO authentication.
//!
//! Handles storage, validation, and expiration of authenticated sessions
//! across multiple Meet instances.

use crate::errors::VisioError;
use crate::secure_storage::SecureStorage;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// An authenticated session for a Meet instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthSession {
    /// The Meet instance hostname (e.g., "meet.example.com")
    pub instance: String,
    /// The session cookie value
    pub session_token: String,
    /// Unix timestamp (milliseconds) when the session expires
    pub expires_at_ms: u64,
    /// The authenticated user's display name (if available)
    pub user_name: Option<String>,
    /// The authenticated user's email (if available)
    pub user_email: Option<String>,
}

impl AuthSession {
    /// Check if the session has expired.
    pub fn is_expired(&self) -> bool {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        now_ms >= self.expires_at_ms
    }

    /// Returns the remaining time until expiration in seconds.
    /// Returns 0 if already expired.
    pub fn remaining_seconds(&self) -> u64 {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        if now_ms >= self.expires_at_ms {
            0
        } else {
            (self.expires_at_ms - now_ms) / 1000
        }
    }
}

/// Pending authentication state for CSRF protection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingAuth {
    /// Random state parameter
    pub state: String,
    /// The instance being authenticated
    pub instance: String,
    /// Unix timestamp (milliseconds) when this pending auth expires
    pub expires_at_ms: u64,
}

impl PendingAuth {
    /// Create a new pending auth with a random state.
    pub fn new(instance: &str) -> Self {
        let state = uuid::Uuid::new_v4().to_string();
        // Pending auth expires in 5 minutes
        let expires_at_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
            + 5 * 60 * 1000;

        Self {
            state,
            instance: instance.to_string(),
            expires_at_ms,
        }
    }

    /// Check if the pending auth has expired.
    pub fn is_expired(&self) -> bool {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        now_ms >= self.expires_at_ms
    }
}

/// Manages authenticated sessions across multiple Meet instances.
pub struct SessionManager {
    storage: Arc<dyn SecureStorage>,
    /// In-memory cache of sessions (populated from storage on load)
    sessions: RwLock<HashMap<String, AuthSession>>,
    /// Pending authentication states (for CSRF protection)
    pending_auths: RwLock<HashMap<String, PendingAuth>>,
}

impl SessionManager {
    const SESSIONS_KEY: &'static str = "visio_sessions";

    /// Create a new SessionManager with the given secure storage backend.
    pub fn new(storage: Arc<dyn SecureStorage>) -> Self {
        let manager = Self {
            storage,
            sessions: RwLock::new(HashMap::new()),
            pending_auths: RwLock::new(HashMap::new()),
        };
        // Load sessions from storage
        if let Err(e) = manager.load_sessions() {
            tracing::warn!("failed to load sessions from storage: {e}");
        }
        manager
    }

    /// Load sessions from secure storage into memory.
    fn load_sessions(&self) -> Result<(), VisioError> {
        if let Some(json) = self.storage.retrieve(Self::SESSIONS_KEY)? {
            let sessions: HashMap<String, AuthSession> = serde_json::from_str(&json)
                .map_err(|e| VisioError::Auth(format!("failed to parse sessions: {e}")))?;
            // Filter out expired sessions
            let valid_sessions: HashMap<String, AuthSession> = sessions
                .into_iter()
                .filter(|(_, s)| !s.is_expired())
                .collect();
            *self
                .sessions
                .write()
                .map_err(|e| VisioError::Auth(format!("lock error: {e}")))? = valid_sessions;
        }
        Ok(())
    }

    /// Persist sessions to secure storage.
    fn save_sessions(&self) -> Result<(), VisioError> {
        let sessions = self
            .sessions
            .read()
            .map_err(|e| VisioError::Auth(format!("lock error: {e}")))?;
        let json = serde_json::to_string(&*sessions)
            .map_err(|e| VisioError::Auth(format!("failed to serialize sessions: {e}")))?;
        self.storage.store(Self::SESSIONS_KEY, &json)
    }

    /// Start an authentication flow for an instance.
    /// Returns the state parameter to include in the auth URL.
    pub fn start_auth(&self, instance: &str) -> Result<String, VisioError> {
        let pending = PendingAuth::new(instance);
        let state = pending.state.clone();
        self.pending_auths
            .write()
            .map_err(|e| VisioError::Auth(format!("lock error: {e}")))?
            .insert(state.clone(), pending);
        // Clean up old pending auths
        self.cleanup_pending_auths()?;
        Ok(state)
    }

    /// Clean up expired pending auths.
    fn cleanup_pending_auths(&self) -> Result<(), VisioError> {
        let mut pending = self
            .pending_auths
            .write()
            .map_err(|e| VisioError::Auth(format!("lock error: {e}")))?;
        pending.retain(|_, p| !p.is_expired());
        Ok(())
    }

    /// Validate a state parameter and return the associated instance.
    /// Consumes the pending auth (can only be used once).
    pub fn validate_state(&self, state: &str) -> Result<String, VisioError> {
        let mut pending = self
            .pending_auths
            .write()
            .map_err(|e| VisioError::Auth(format!("lock error: {e}")))?;

        let auth = pending
            .remove(state)
            .ok_or_else(|| VisioError::Oidc("invalid or expired state parameter".to_string()))?;

        if auth.is_expired() {
            return Err(VisioError::Oidc("state parameter has expired".to_string()));
        }

        Ok(auth.instance)
    }

    /// Store a new session for an instance.
    pub fn store_session(&self, session: AuthSession) -> Result<(), VisioError> {
        let instance = session.instance.clone();
        self.sessions
            .write()
            .map_err(|e| VisioError::Auth(format!("lock error: {e}")))?
            .insert(instance, session);
        self.save_sessions()
    }

    /// Get the session for an instance (if valid and not expired).
    pub fn get_session(&self, instance: &str) -> Result<Option<AuthSession>, VisioError> {
        let sessions = self
            .sessions
            .read()
            .map_err(|e| VisioError::Auth(format!("lock error: {e}")))?;
        match sessions.get(instance) {
            Some(session) if !session.is_expired() => Ok(Some(session.clone())),
            Some(_) => {
                // Session expired, clean it up
                drop(sessions);
                self.remove_session(instance)?;
                Ok(None)
            }
            None => Ok(None),
        }
    }

    /// Check if we have a valid session for an instance.
    pub fn is_authenticated(&self, instance: &str) -> bool {
        self.get_session(instance).ok().flatten().is_some()
    }

    /// Remove the session for an instance (logout).
    pub fn remove_session(&self, instance: &str) -> Result<(), VisioError> {
        self.sessions
            .write()
            .map_err(|e| VisioError::Auth(format!("lock error: {e}")))?
            .remove(instance);
        self.save_sessions()
    }

    /// Get all valid (non-expired) sessions.
    pub fn get_all_sessions(&self) -> Result<Vec<AuthSession>, VisioError> {
        let sessions = self
            .sessions
            .read()
            .map_err(|e| VisioError::Auth(format!("lock error: {e}")))?;
        Ok(sessions
            .values()
            .filter(|s| !s.is_expired())
            .cloned()
            .collect())
    }

    /// Clear all sessions (logout from all instances).
    pub fn clear_all_sessions(&self) -> Result<(), VisioError> {
        self.sessions
            .write()
            .map_err(|e| VisioError::Auth(format!("lock error: {e}")))?
            .clear();
        self.storage.delete(Self::SESSIONS_KEY)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secure_storage::MemoryStorage;

    #[test]
    fn test_auth_session_expiration() {
        let session = AuthSession {
            instance: "test.example.com".to_string(),
            session_token: "token123".to_string(),
            expires_at_ms: 0, // Already expired
            user_name: Some("Test User".to_string()),
            user_email: Some("test@example.com".to_string()),
        };
        assert!(session.is_expired());
        assert_eq!(session.remaining_seconds(), 0);

        let future_session = AuthSession {
            instance: "test.example.com".to_string(),
            session_token: "token123".to_string(),
            expires_at_ms: u64::MAX, // Far in the future
            user_name: None,
            user_email: None,
        };
        assert!(!future_session.is_expired());
        assert!(future_session.remaining_seconds() > 0);
    }

    #[test]
    fn test_pending_auth() {
        let pending = PendingAuth::new("meet.example.com");
        assert!(!pending.is_expired());
        assert!(!pending.state.is_empty());
        assert_eq!(pending.instance, "meet.example.com");
    }

    #[test]
    fn test_session_manager_store_and_retrieve() {
        let storage = Arc::new(MemoryStorage::new());
        let manager = SessionManager::new(storage);

        let session = AuthSession {
            instance: "meet.example.com".to_string(),
            session_token: "token123".to_string(),
            expires_at_ms: u64::MAX,
            user_name: Some("Test User".to_string()),
            user_email: None,
        };

        manager.store_session(session.clone()).unwrap();
        let retrieved = manager.get_session("meet.example.com").unwrap();
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.session_token, "token123");
        assert!(manager.is_authenticated("meet.example.com"));
    }

    #[test]
    fn test_session_manager_expired_session() {
        let storage = Arc::new(MemoryStorage::new());
        let manager = SessionManager::new(storage);

        let session = AuthSession {
            instance: "meet.example.com".to_string(),
            session_token: "token123".to_string(),
            expires_at_ms: 0, // Already expired
            user_name: None,
            user_email: None,
        };

        manager.store_session(session).unwrap();
        // Expired session should be cleaned up on retrieval
        let retrieved = manager.get_session("meet.example.com").unwrap();
        assert!(retrieved.is_none());
        assert!(!manager.is_authenticated("meet.example.com"));
    }

    #[test]
    fn test_session_manager_auth_flow() {
        let storage = Arc::new(MemoryStorage::new());
        let manager = SessionManager::new(storage);

        // Start auth flow
        let state = manager.start_auth("meet.example.com").unwrap();
        assert!(!state.is_empty());

        // Validate state (consumes it)
        let instance = manager.validate_state(&state).unwrap();
        assert_eq!(instance, "meet.example.com");

        // Can't use the same state twice
        assert!(manager.validate_state(&state).is_err());
    }

    #[test]
    fn test_session_manager_logout() {
        let storage = Arc::new(MemoryStorage::new());
        let manager = SessionManager::new(storage);

        let session = AuthSession {
            instance: "meet.example.com".to_string(),
            session_token: "token123".to_string(),
            expires_at_ms: u64::MAX,
            user_name: None,
            user_email: None,
        };

        manager.store_session(session).unwrap();
        assert!(manager.is_authenticated("meet.example.com"));

        manager.remove_session("meet.example.com").unwrap();
        assert!(!manager.is_authenticated("meet.example.com"));
    }
}
