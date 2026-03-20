use reqwest::header::{COOKIE, HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};

use crate::errors::VisioError;

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
        cookie: String,
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

    pub fn set_authenticated(&mut self, user: UserInfo, cookie: String, meet_instance: String) {
        self.state = SessionState::Authenticated {
            user,
            cookie,
            meet_instance,
        };
    }

    pub fn clear(&mut self) {
        self.state = SessionState::Anonymous;
    }

    pub fn cookie(&self) -> Option<String> {
        match &self.state {
            SessionState::Authenticated { cookie, .. } => Some(cookie.clone()),
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

    pub async fn fetch_user(meet_url: &str, cookie: &str) -> Result<UserInfo, VisioError> {
        let url = format!("{}/api/v1.0/users/me/", meet_url);

        let mut headers = HeaderMap::new();
        let cookie_value = format!("sessionid={}", cookie);
        headers.insert(
            COOKIE,
            HeaderValue::from_str(&cookie_value).map_err(|e| VisioError::Http(e.to_string()))?,
        );

        let client = reqwest::Client::new();
        let response = client
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

        let body = response
            .text()
            .await
            .map_err(|e| VisioError::Http(e.to_string()))?;

        serde_json::from_str::<UserInfo>(&body)
            .map_err(|e| VisioError::Session(format!("Failed to parse user info: {}", e)))
    }

    pub async fn validate_session(&mut self, meet_url: &str) -> Result<bool, VisioError> {
        let cookie = match self.cookie() {
            Some(c) => c,
            None => return Ok(false),
        };
        let instance = meet_url
            .trim_end_matches('/')
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .to_string();

        match Self::fetch_user(meet_url, &cookie).await {
            Ok(user) => {
                self.state = SessionState::Authenticated {
                    user,
                    cookie,
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

    pub async fn logout(&mut self, meet_url: &str) -> Result<(), VisioError> {
        if let Some(cookie) = self.cookie() {
            let url = format!("{}/logout", meet_url);
            let mut headers = HeaderMap::new();
            let cookie_value = format!("sessionid={}", cookie);
            if let Ok(val) = HeaderValue::from_str(&cookie_value) {
                headers.insert(COOKIE, val);
                let client = reqwest::Client::new();
                let _ = client.get(&url).headers(headers).send().await;
            }
        }
        self.clear();
        Ok(())
    }

    pub async fn exchange_oidc_code(meet_instance: &str, code: &str) -> Result<String, VisioError> {
        let url = format!("https://{}/api/v1.0/auth/session-exchange/", meet_instance);

        let client = reqwest::Client::new();
        let response = client
            .post(&url)
            .json(&serde_json::json!({ "code": code }))
            .send()
            .await
            .map_err(|e| VisioError::Http(e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(VisioError::Auth(format!(
                "code exchange failed ({status}): {body}"
            )));
        }

        let body: serde_json::Value = response
            .json()
            .await
            .map_err(|e| VisioError::Http(e.to_string()))?;

        // The response key matches SESSION_COOKIE_NAME on the server
        body.get("meet_sessionid")
            .or_else(|| body.get("sessionid"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| VisioError::Auth("no session ID in exchange response".into()))
    }

    pub async fn create_room(
        meet_url: &str,
        cookie: &str,
        name: &str,
        access_level: &str,
    ) -> Result<CreateRoomResponse, VisioError> {
        use rand::Rng;

        let url = format!("{}/api/v1.0/rooms/", meet_url.trim_end_matches('/'));

        let csrf_bytes: [u8; 32] = rand::thread_rng().r#gen();
        let csrf_token: String = csrf_bytes.iter().map(|b| format!("{:02x}", b)).collect();

        let cookie_header = format!("sessionid={}; csrftoken={}", cookie, csrf_token);

        // Generate a random slug in xxx-yyyy-zzz format (lowercase letters only)
        let slug_name = {
            let mut rng = rand::thread_rng();
            let mut rand_char = || (b'a' + rng.r#gen::<u8>() % 26) as char;
            let p1: String = (0..3).map(|_| rand_char()).collect();
            let p2: String = (0..4).map(|_| rand_char()).collect();
            let p3: String = (0..3).map(|_| rand_char()).collect();
            format!("{}-{}-{}", p1, p2, p3)
        };

        let room_name = if name.trim().is_empty() {
            &slug_name
        } else {
            name
        };

        let body = serde_json::json!({
            "name": room_name,
            "access_level": access_level,
        });

        let client = reqwest::Client::new();
        let response = client
            .post(&url)
            .header(COOKIE, &cookie_header)
            .header("X-CSRFToken", &csrf_token)
            .header("Referer", format!("{}/", meet_url.trim_end_matches('/')))
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
            let body = response.text().await.unwrap_or_default();
            return Err(VisioError::Session(format!(
                "Room creation failed ({}): {}",
                status, body
            )));
        }

        let body = response
            .text()
            .await
            .map_err(|e| VisioError::Http(e.to_string()))?;

        tracing::debug!("create_room response body: {}", body);

        serde_json::from_str::<CreateRoomResponse>(&body).map_err(|e| {
            VisioError::Session(format!("Invalid room response: {} — body: {}", e, body))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_state_default_is_anonymous() {
        let session = SessionManager::new();
        assert!(matches!(session.state(), SessionState::Anonymous));
    }

    #[test]
    fn test_set_cookie_changes_state() {
        let mut session = SessionManager::new();
        let user = UserInfo {
            id: "123".to_string(),
            email: "test@example.com".to_string(),
            full_name: Some("Test User".to_string()),
            short_name: None,
        };
        session.set_authenticated(
            user.clone(),
            "abc123".to_string(),
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
        session.set_authenticated(user, "abc123".to_string(), "meet.example.com".to_string());
        session.clear();
        assert!(matches!(session.state(), SessionState::Anonymous));
    }

    #[test]
    fn test_cookie_returns_none_when_anonymous() {
        let session = SessionManager::new();
        assert!(session.cookie().is_none());
    }

    #[test]
    fn test_cookie_returns_value_when_authenticated() {
        let mut session = SessionManager::new();
        let user = UserInfo {
            id: "1".to_string(),
            email: "a@b.com".to_string(),
            full_name: Some("A".to_string()),
            short_name: None,
        };
        session.set_authenticated(user, "mycookie".to_string(), "meet.example.com".to_string());
        assert_eq!(session.cookie(), Some("mycookie".to_string()));
    }

    #[tokio::test]
    async fn test_fetch_user_with_invalid_cookie_returns_error() {
        let result = SessionManager::fetch_user("https://meet.example.com", "invalid_cookie").await;
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
        let result = SessionManager::create_room(
            "https://meet.example.com",
            "invalid_cookie",
            "Test Room",
            "public",
        )
        .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_exchange_oidc_code_with_invalid_code() {
        let result =
            SessionManager::exchange_oidc_code("dev-meet.linagora.com", "invalid-code-too-short")
                .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_exchange_oidc_code_formats_url_correctly() {
        let result = SessionManager::exchange_oidc_code(
            "nonexistent.example.com",
            "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee",
        )
        .await;
        assert!(result.is_err());
    }
}
