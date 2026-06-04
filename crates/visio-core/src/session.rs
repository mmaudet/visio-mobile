#[cfg(feature = "oidc")]
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};

use crate::errors::VisioError;
#[cfg(feature = "oidc")]
use crate::http::{MAX_JSON_BODY, bounded_text, shared_http_client};
use crate::tokens::TokenPair;

/// Returns whether OIDC authentication support is compiled in.
pub fn is_oidc_enabled() -> bool {
    cfg!(feature = "oidc")
}

/// Returns whether OIDC is both compiled in AND enabled at runtime via feature flags.
pub fn is_oidc_enabled_runtime(features: &crate::features::FeatureService) -> bool {
    cfg!(feature = "oidc") && features.is_enabled("oidc_auth")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub id: String,
    pub email: String,
    #[serde(default)]
    pub full_name: Option<String>,
    #[serde(default)]
    pub short_name: Option<String>,
}

impl UserInfo {
    /// Best available display name: full_name, short_name, or email prefix.
    pub fn display_name(&self) -> String {
        self.full_name
            .as_deref()
            .or(self.short_name.as_deref())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| self.email.split('@').next().unwrap_or(&self.email))
            .to_string()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateRoomLiveKit {
    pub url: String,
    pub room: String,
    pub token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateRoomResponse {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub access_level: String,
    #[serde(default)]
    pub livekit: Option<CreateRoomLiveKit>,
}

#[derive(Debug, Clone)]
pub enum SessionState {
    Anonymous,
    Authenticated {
        user: UserInfo,
        tokens: TokenPair,
        meet_instance: String,
    },
}

pub struct SessionManager {
    state: SessionState,
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            state: SessionState::Anonymous,
        }
    }

    pub fn state(&self) -> &SessionState {
        &self.state
    }

    pub fn set_authenticated(&mut self, user: UserInfo, tokens: TokenPair, meet_instance: String) {
        self.state = SessionState::Authenticated {
            user,
            tokens,
            meet_instance,
        };
    }

    /// Replace just the token pair (used after a refresh).
    pub fn update_tokens(&mut self, new_tokens: TokenPair) {
        if let SessionState::Authenticated { tokens, .. } = &mut self.state {
            *tokens = new_tokens;
        }
    }

    pub fn clear(&mut self) {
        self.state = SessionState::Anonymous;
    }

    pub fn access_token(&self) -> Option<String> {
        match &self.state {
            SessionState::Authenticated { tokens, .. } => Some(tokens.access.clone()),
            SessionState::Anonymous => None,
        }
    }

    pub fn refresh_token(&self) -> Option<String> {
        match &self.state {
            SessionState::Authenticated { tokens, .. } => Some(tokens.refresh.clone()),
            SessionState::Anonymous => None,
        }
    }

    pub fn tokens(&self) -> Option<TokenPair> {
        match &self.state {
            SessionState::Authenticated { tokens, .. } => Some(tokens.clone()),
            SessionState::Anonymous => None,
        }
    }

    pub fn user(&self) -> Option<&UserInfo> {
        match &self.state {
            SessionState::Authenticated { user, .. } => Some(user),
            SessionState::Anonymous => None,
        }
    }

    pub fn meet_instance(&self) -> Option<&str> {
        match &self.state {
            SessionState::Authenticated { meet_instance, .. } => Some(meet_instance),
            SessionState::Anonymous => None,
        }
    }

    #[cfg(feature = "oidc")]
    pub async fn fetch_user(meet_url: &str, access_token: &str) -> Result<UserInfo, VisioError> {
        let url = format!("{}/api/v1.0/users/me/", meet_url);

        let mut headers = HeaderMap::new();
        let bearer = format!("Bearer {}", access_token);
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&bearer).map_err(|e| VisioError::Http(e.to_string()))?,
        );

        let response = shared_http_client()
            .get(&url)
            .headers(headers)
            .send()
            .await
            .map_err(|e| VisioError::Http(e.to_string()))?;

        let status = response.status();
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(VisioError::Session(
                "Session expired or invalid".to_string(),
            ));
        }

        let body = bounded_text(response, MAX_JSON_BODY).await?;

        serde_json::from_str::<UserInfo>(&body)
            .map_err(|e| VisioError::Session(format!("Failed to parse user info: {}", e)))
    }

    #[cfg(not(feature = "oidc"))]
    pub async fn fetch_user(_meet_url: &str, _access_token: &str) -> Result<UserInfo, VisioError> {
        Err(VisioError::Session("OIDC support is not enabled".into()))
    }

    #[cfg(feature = "oidc")]
    pub async fn validate_session(&mut self, meet_url: &str) -> Result<bool, VisioError> {
        let tokens = match self.tokens() {
            Some(t) => t,
            None => return Ok(false),
        };
        let instance = meet_url
            .trim_end_matches('/')
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .to_string();

        match Self::fetch_user(meet_url, &tokens.access).await {
            Ok(user) => {
                self.state = SessionState::Authenticated {
                    user,
                    tokens,
                    meet_instance: instance,
                };
                Ok(true)
            }
            Err(_) => {
                self.clear();
                Ok(false)
            }
        }
    }

    #[cfg(not(feature = "oidc"))]
    pub async fn validate_session(&mut self, _meet_url: &str) -> Result<bool, VisioError> {
        Err(VisioError::Auth("OIDC support is not enabled".into()))
    }

    /// Clear the local session. With JWT tokens we don't strictly need to call
    /// the server — the refresh token will simply expire. We blacklist it via
    /// the refresh-rotation flow on the server side. For now, drop local state.
    pub async fn logout(&mut self, _meet_url: &str) -> Result<(), VisioError> {
        self.clear();
        Ok(())
    }

    #[cfg(feature = "oidc")]
    pub async fn create_room(
        meet_url: &str,
        access_token: &str,
        access_level: &str,
    ) -> Result<CreateRoomResponse, VisioError> {
        use rand::Rng;

        let url = format!("{}/api/v1.0/rooms/", meet_url.trim_end_matches('/'));

        // Random slug in xxx-yyyy-zzz format (lowercase letters only).
        let slug_name = {
            let mut rng = rand::thread_rng();
            let mut rand_char = || (b'a' + rng.r#gen::<u8>() % 26) as char;
            let p1: String = (0..3).map(|_| rand_char()).collect();
            let p2: String = (0..4).map(|_| rand_char()).collect();
            let p3: String = (0..3).map(|_| rand_char()).collect();
            format!("{}-{}-{}", p1, p2, p3)
        };

        let body = serde_json::json!({
            "name": slug_name,
            "access_level": access_level,
        });

        let response = shared_http_client()
            .post(&url)
            .header(AUTHORIZATION, format!("Bearer {}", access_token))
            .json(&body)
            .send()
            .await
            .map_err(|e| VisioError::Http(e.to_string()))?;

        let status = response.status();
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(VisioError::Session(
                "Authentication required to create a room".to_string(),
            ));
        }

        if !status.is_success() {
            let body = bounded_text(response, MAX_JSON_BODY)
                .await
                .unwrap_or_default();
            return Err(VisioError::Session(format!(
                "Room creation failed ({}): {}",
                status, body
            )));
        }

        let body = bounded_text(response, MAX_JSON_BODY).await?;

        // Body contains a LiveKit JWT — never log it. Status + length only.
        tracing::debug!("create_room status={} body_len={}", status, body.len());

        serde_json::from_str::<CreateRoomResponse>(&body)
            .map_err(|e| VisioError::Session(format!("Invalid room response: {}", e)))
    }

    #[cfg(not(feature = "oidc"))]
    pub async fn create_room(
        _meet_url: &str,
        _access_token: &str,
        _access_level: &str,
    ) -> Result<CreateRoomResponse, VisioError> {
        Err(VisioError::Session("OIDC support is not enabled".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_tokens() -> TokenPair {
        TokenPair {
            access: "access-jwt".to_string(),
            refresh: "refresh-jwt".to_string(),
        }
    }

    #[test]
    fn test_session_state_default_is_anonymous() {
        let session = SessionManager::new();
        assert!(matches!(session.state(), SessionState::Anonymous));
    }

    #[test]
    fn test_set_authenticated_changes_state() {
        let mut session = SessionManager::new();
        let user = UserInfo {
            id: "123".to_string(),
            email: "test@example.com".to_string(),
            full_name: Some("Test User".to_string()),
            short_name: None,
        };
        session.set_authenticated(
            user.clone(),
            sample_tokens(),
            "meet.example.com".to_string(),
        );
        match session.state() {
            SessionState::Authenticated { user: u, .. } => {
                assert_eq!(u.display_name(), "Test User");
            }
            _ => panic!("Expected Authenticated state"),
        }
    }

    #[test]
    fn test_clear_session_returns_to_anonymous() {
        let mut session = SessionManager::new();
        let user = UserInfo {
            id: "123".to_string(),
            email: "test@example.com".to_string(),
            full_name: Some("Test".to_string()),
            short_name: None,
        };
        session.set_authenticated(user, sample_tokens(), "meet.example.com".to_string());
        session.clear();
        assert!(matches!(session.state(), SessionState::Anonymous));
    }

    #[test]
    fn test_access_token_returns_none_when_anonymous() {
        let session = SessionManager::new();
        assert!(session.access_token().is_none());
        assert!(session.refresh_token().is_none());
    }

    #[test]
    fn test_tokens_returned_when_authenticated() {
        let mut session = SessionManager::new();
        let user = UserInfo {
            id: "1".to_string(),
            email: "a@b.com".to_string(),
            full_name: Some("A".to_string()),
            short_name: None,
        };
        session.set_authenticated(user, sample_tokens(), "meet.example.com".to_string());
        assert_eq!(session.access_token().as_deref(), Some("access-jwt"));
        assert_eq!(session.refresh_token().as_deref(), Some("refresh-jwt"));
    }

    #[test]
    fn test_update_tokens_after_refresh() {
        let mut session = SessionManager::new();
        let user = UserInfo {
            id: "1".to_string(),
            email: "a@b.com".to_string(),
            full_name: None,
            short_name: None,
        };
        session.set_authenticated(user, sample_tokens(), "meet.example.com".to_string());
        session.update_tokens(TokenPair {
            access: "new-access".into(),
            refresh: "new-refresh".into(),
        });
        assert_eq!(session.access_token().as_deref(), Some("new-access"));
        assert_eq!(session.refresh_token().as_deref(), Some("new-refresh"));
    }

    #[tokio::test]
    async fn test_fetch_user_with_invalid_token_returns_error() {
        let result = SessionManager::fetch_user("https://meet.example.com", "invalid-token").await;
        assert!(result.is_err());
    }

    #[test]
    fn test_create_room_response_deserialization() {
        let json = r#"{
            "id": "cc9950db-cf78-4bf0-84b2-4d906148c849",
            "name": "Test Room",
            "slug": "test-room",
            "access_level": "public",
            "livekit": {
                "url": "https://livekit.example.com",
                "room": "cc9950db-cf78-4bf0-84b2-4d906148c849",
                "token": "eyJhbGciOiJIUzI1NiJ9.test"
            }
        }"#;
        let resp: CreateRoomResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.slug, "test-room");
        assert_eq!(resp.name, "Test Room");
        assert_eq!(resp.access_level, "public");
        let lk = resp.livekit.unwrap();
        assert_eq!(lk.url, "https://livekit.example.com");
        assert!(!lk.token.is_empty());
    }

    #[tokio::test]
    async fn test_create_room_without_auth_returns_error() {
        let result =
            SessionManager::create_room("https://meet.example.com", "invalid-token", "public")
                .await;
        assert!(result.is_err());
    }
}
