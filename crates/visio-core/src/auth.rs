use crate::errors::VisioError;
use crate::session::{AuthSession, SessionManager};
use serde::Deserialize;

/// Response from the Meet API.
#[derive(Debug, Deserialize)]
struct MeetApiResponse {
    livekit: LiveKitCredentials,
}

#[derive(Debug, Deserialize)]
struct LiveKitCredentials {
    url: String,
    token: String,
}

/// Token and connection info returned by the Meet API.
#[derive(Debug, Clone)]
pub struct TokenInfo {
    /// WebSocket URL for LiveKit (wss://)
    pub livekit_url: String,
    /// JWT access token
    pub token: String,
}

/// Parameters parsed from the auth callback URL.
#[derive(Debug, Clone)]
pub struct AuthCallbackParams {
    /// The Meet instance
    pub instance: String,
    /// The session token/cookie
    pub token: String,
    /// The CSRF state parameter
    pub state: String,
    /// Session expiration timestamp (milliseconds)
    pub expires_at_ms: Option<u64>,
    /// User display name (if provided)
    pub user_name: Option<String>,
    /// User email (if provided)
    pub user_email: Option<String>,
}

/// Requests a LiveKit token from the Meet API.
pub struct AuthService;

impl AuthService {
    /// Generate a random meeting slug in the format xxx-yyyy-zzz.
    ///
    /// The slug consists of:
    /// - 3 lowercase letters
    /// - dash
    /// - 4 lowercase letters
    /// - dash
    /// - 3 lowercase letters
    ///
    /// # Returns
    /// A random slug like "abc-defg-hij"
    pub fn generate_random_slug() -> String {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        let mut gen_letters = |count: usize| -> String {
            (0..count)
                .map(|_| (b'a' + rng.gen_range(0..26)) as char)
                .collect()
        };

        format!("{}-{}-{}", gen_letters(3), gen_letters(4), gen_letters(3))
    }

    /// Call the Meet API to get a LiveKit token for the given room.
    ///
    /// `meet_url` should be a full URL like `https://meet.example.com/room-slug`
    /// or just `meet.example.com/room-slug`.
    pub async fn request_token(
        meet_url: &str,
        username: Option<&str>,
    ) -> Result<TokenInfo, VisioError> {
        let (instance, slug) = Self::parse_meet_url(meet_url)?;

        let mut api_url = format!("https://{}/api/v1.0/rooms/{}/", instance, slug);
        if let Some(name) = username {
            let encoded = urlencoding::encode(name);
            api_url.push_str(&format!("?username={encoded}"));
        }

        tracing::info!("requesting token from Meet API: {}", api_url);

        let resp = reqwest::get(&api_url)
            .await
            .map_err(|e| VisioError::Http(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(VisioError::Auth(format!(
                "Meet API returned status {}",
                resp.status()
            )));
        }

        let data: MeetApiResponse = resp
            .json()
            .await
            .map_err(|e| VisioError::Auth(format!("invalid Meet API response: {e}")))?;

        // Convert URL to WebSocket
        let livekit_url = data
            .livekit
            .url
            .replace("https://", "wss://")
            .replace("http://", "ws://");

        Ok(TokenInfo {
            livekit_url,
            token: data.livekit.token,
        })
    }

    /// Extract and validate the room slug from user input.
    /// Accepts full URL (`https://meet.example.com/abc-defg-hij`) or bare slug (`abc-defg-hij`).
    /// Slug format: 3 lowercase + dash + 4 lowercase + dash + 3 lowercase.
    pub fn extract_slug(input: &str) -> Result<String, VisioError> {
        let input = input.trim().trim_end_matches('/');
        let candidate = if input.contains('/') {
            input.rsplit('/').next().unwrap_or("")
        } else {
            input
        };
        let re = regex::Regex::new(r"^[a-z]{3}-[a-z]{4}-[a-z]{3}$").unwrap();
        if re.is_match(candidate) {
            Ok(candidate.to_string())
        } else {
            Err(VisioError::InvalidUrl(format!(
                "invalid room slug format: '{candidate}'"
            )))
        }
    }

    /// Validate a room URL by calling the Meet API.
    /// Returns Ok(TokenInfo) if the room exists, Err otherwise.
    pub async fn validate_room(
        meet_url: &str,
        username: Option<&str>,
    ) -> Result<TokenInfo, VisioError> {
        Self::request_token(meet_url, username).await
    }

    /// Parse a Meet URL into (instance, room_slug).
    fn parse_meet_url(url: &str) -> Result<(String, String), VisioError> {
        let url = url
            .trim()
            .trim_end_matches('/')
            .replace("https://", "")
            .replace("http://", "");

        let parts: Vec<&str> = url.splitn(2, '/').collect();
        if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
            return Err(VisioError::InvalidUrl(format!(
                "expected 'instance/room-slug', got '{url}'"
            )));
        }

        Ok((parts[0].to_string(), parts[1].to_string()))
    }

    /// Generate the SSO login URL for a Meet instance.
    ///
    /// The URL opens in the system browser, which handles the OIDC flow
    /// and redirects back to the app via deep link.
    ///
    /// # Arguments
    /// * `instance` - The Meet server hostname (e.g., "meet.example.com")
    /// * `state` - CSRF state parameter (from SessionManager::start_auth)
    ///
    /// # Returns
    /// The full URL to open in the browser for authentication.
    pub fn get_login_url(instance: &str, state: &str) -> String {
        // Build the return URL that will come back to our app
        let return_to = format!(
            "visio://auth-callback?instance={}&state={}",
            urlencoding::encode(instance),
            urlencoding::encode(state)
        );

        // Build the authentication URL
        format!(
            "https://{}/api/v1.0/authenticate/?silent=false&returnTo={}",
            instance,
            urlencoding::encode(&return_to)
        )
    }

    /// Parse the auth callback URL received from the deep link.
    ///
    /// Expected URL format:
    /// `visio://auth-callback?instance=...&state=...&token=...&expires=...&name=...&email=...`
    ///
    /// # Arguments
    /// * `callback_url` - The full callback URL from the deep link
    ///
    /// # Returns
    /// Parsed callback parameters, or an error if parsing fails.
    pub fn parse_auth_callback(callback_url: &str) -> Result<AuthCallbackParams, VisioError> {
        let url = url::Url::parse(callback_url)
            .map_err(|e| VisioError::Oidc(format!("invalid callback URL: {e}")))?;

        // Verify scheme and host
        if url.scheme() != "visio" {
            return Err(VisioError::Oidc(format!(
                "unexpected URL scheme: {}",
                url.scheme()
            )));
        }
        if url.host_str() != Some("auth-callback") {
            return Err(VisioError::Oidc(format!(
                "unexpected URL host: {:?}",
                url.host_str()
            )));
        }

        // Parse query parameters
        let params: std::collections::HashMap<_, _> = url.query_pairs().collect();

        let instance = params
            .get("instance")
            .ok_or_else(|| VisioError::Oidc("missing instance parameter".to_string()))?
            .to_string();

        let state = params
            .get("state")
            .ok_or_else(|| VisioError::Oidc("missing state parameter".to_string()))?
            .to_string();

        let token = params
            .get("token")
            .ok_or_else(|| VisioError::Oidc("missing token parameter".to_string()))?
            .to_string();

        let expires_at_ms = params
            .get("expires")
            .and_then(|s| s.parse::<u64>().ok());

        let user_name = params.get("name").map(|s| s.to_string());
        let user_email = params.get("email").map(|s| s.to_string());

        Ok(AuthCallbackParams {
            instance,
            token,
            state,
            expires_at_ms,
            user_name,
            user_email,
        })
    }

    /// Handle the auth callback and store the session.
    ///
    /// # Arguments
    /// * `callback_url` - The full callback URL from the deep link
    /// * `session_manager` - The session manager to validate state and store the session
    ///
    /// # Returns
    /// The authenticated session, or an error if validation fails.
    pub fn handle_auth_callback(
        callback_url: &str,
        session_manager: &SessionManager,
    ) -> Result<AuthSession, VisioError> {
        let params = Self::parse_auth_callback(callback_url)?;

        // Validate the state parameter (CSRF protection)
        let expected_instance = session_manager.validate_state(&params.state)?;
        if expected_instance != params.instance {
            return Err(VisioError::Oidc(format!(
                "instance mismatch: expected '{}', got '{}'",
                expected_instance, params.instance
            )));
        }

        // Default expiration: 24 hours from now
        let default_expiry = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
            + 24 * 60 * 60 * 1000;

        let session = AuthSession {
            instance: params.instance,
            session_token: params.token,
            expires_at_ms: params.expires_at_ms.unwrap_or(default_expiry),
            user_name: params.user_name,
            user_email: params.user_email,
        };

        // Store the session
        session_manager.store_session(session.clone())?;

        tracing::info!(
            "OIDC authentication successful for instance: {}",
            session.instance
        );

        Ok(session)
    }

    /// Create an HTTP client with session cookies for authenticated requests.
    ///
    /// # Arguments
    /// * `session` - The authenticated session to use
    ///
    /// # Returns
    /// A reqwest client configured with the session cookie.
    pub fn create_authenticated_client(session: &AuthSession) -> Result<reqwest::Client, VisioError> {
        use reqwest::header::{HeaderMap, HeaderValue, COOKIE};

        let mut headers = HeaderMap::new();
        let cookie_value = format!("session={}", session.session_token);
        headers.insert(
            COOKIE,
            HeaderValue::from_str(&cookie_value)
                .map_err(|e| VisioError::Auth(format!("invalid cookie value: {e}")))?,
        );

        reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .map_err(|e| VisioError::Http(format!("failed to create client: {e}")))
    }

    /// Request a token with authentication (if session exists for the instance).
    ///
    /// Falls back to unauthenticated request if no session is available.
    pub async fn request_token_with_auth(
        meet_url: &str,
        username: Option<&str>,
        session_manager: Option<&SessionManager>,
    ) -> Result<TokenInfo, VisioError> {
        let (instance, slug) = Self::parse_meet_url(meet_url)?;

        // Check if we have a valid session for this instance
        let session = session_manager.and_then(|sm| sm.get_session(&instance).ok().flatten());

        let mut api_url = format!("https://{}/api/v1.0/rooms/{}/", instance, slug);
        if let Some(name) = username {
            let encoded = urlencoding::encode(name);
            api_url.push_str(&format!("?username={encoded}"));
        }

        tracing::info!("requesting token from Meet API: {}", api_url);

        let resp = if let Some(session) = session {
            // Use authenticated client
            let client = Self::create_authenticated_client(&session)?;
            client
                .get(&api_url)
                .send()
                .await
                .map_err(|e| VisioError::Http(e.to_string()))?
        } else {
            // Use unauthenticated request
            reqwest::get(&api_url)
                .await
                .map_err(|e| VisioError::Http(e.to_string()))?
        };

        if !resp.status().is_success() {
            // Check if this is an auth error
            if resp.status().as_u16() == 401 || resp.status().as_u16() == 403 {
                return Err(VisioError::SessionExpired(format!(
                    "authentication required or session expired for {}",
                    instance
                )));
            }
            return Err(VisioError::Auth(format!(
                "Meet API returned status {}",
                resp.status()
            )));
        }

        let data: MeetApiResponse = resp
            .json()
            .await
            .map_err(|e| VisioError::Auth(format!("invalid Meet API response: {e}")))?;

        // Convert URL to WebSocket
        let livekit_url = data
            .livekit
            .url
            .replace("https://", "wss://")
            .replace("http://", "ws://");

        Ok(TokenInfo {
            livekit_url,
            token: data.livekit.token,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secure_storage::MemoryStorage;
    use std::sync::Arc;

    #[test]
    fn parse_meet_url_with_https() {
        let (instance, slug) =
            AuthService::parse_meet_url("https://meet.example.com/my-room").unwrap();
        assert_eq!(instance, "meet.example.com");
        assert_eq!(slug, "my-room");
    }

    #[test]
    fn parse_meet_url_without_scheme() {
        let (instance, slug) = AuthService::parse_meet_url("meet.example.com/room-123").unwrap();
        assert_eq!(instance, "meet.example.com");
        assert_eq!(slug, "room-123");
    }

    #[test]
    fn parse_meet_url_with_trailing_slash() {
        let (instance, slug) =
            AuthService::parse_meet_url("https://meet.example.com/my-room/").unwrap();
        assert_eq!(instance, "meet.example.com");
        assert_eq!(slug, "my-room");
    }

    #[test]
    fn parse_meet_url_invalid() {
        assert!(AuthService::parse_meet_url("invalid").is_err());
        assert!(AuthService::parse_meet_url("").is_err());
    }

    #[test]
    fn extract_slug_from_full_url() {
        let slug = AuthService::extract_slug("https://meet.linagora.com/dpd-jffv-trg").unwrap();
        assert_eq!(slug, "dpd-jffv-trg");
    }

    #[test]
    fn extract_slug_from_bare_slug() {
        let slug = AuthService::extract_slug("dpd-jffv-trg").unwrap();
        assert_eq!(slug, "dpd-jffv-trg");
    }

    #[test]
    fn extract_slug_invalid_format() {
        assert!(AuthService::extract_slug("hello").is_err());
        assert!(AuthService::extract_slug("").is_err());
        assert!(AuthService::extract_slug("abc-defg-hi").is_err());
        assert!(AuthService::extract_slug("ABC-DEFG-HIJ").is_err());
    }

    #[test]
    fn extract_slug_from_url_with_trailing_slash() {
        let slug = AuthService::extract_slug("https://meet.example.com/abc-defg-hij/").unwrap();
        assert_eq!(slug, "abc-defg-hij");
    }

    #[test]
    fn get_login_url_format() {
        let url = AuthService::get_login_url("meet.example.com", "state123");
        assert!(url.starts_with("https://meet.example.com/api/v1.0/authenticate/"));
        assert!(url.contains("silent=false"));
        assert!(url.contains("returnTo="));
        assert!(url.contains("visio%3A%2F%2Fauth-callback"));
        assert!(url.contains("instance%3Dmeet.example.com"));
        assert!(url.contains("state%3Dstate123"));
    }

    #[test]
    fn parse_auth_callback_valid() {
        let callback = "visio://auth-callback?instance=meet.example.com&state=abc123&token=sessiontoken&expires=9999999999999&name=John%20Doe&email=john%40example.com";
        let params = AuthService::parse_auth_callback(callback).unwrap();
        assert_eq!(params.instance, "meet.example.com");
        assert_eq!(params.state, "abc123");
        assert_eq!(params.token, "sessiontoken");
        assert_eq!(params.expires_at_ms, Some(9999999999999));
        assert_eq!(params.user_name, Some("John Doe".to_string()));
        assert_eq!(params.user_email, Some("john@example.com".to_string()));
    }

    #[test]
    fn parse_auth_callback_minimal() {
        let callback = "visio://auth-callback?instance=meet.example.com&state=abc&token=tok";
        let params = AuthService::parse_auth_callback(callback).unwrap();
        assert_eq!(params.instance, "meet.example.com");
        assert_eq!(params.state, "abc");
        assert_eq!(params.token, "tok");
        assert!(params.expires_at_ms.is_none());
        assert!(params.user_name.is_none());
        assert!(params.user_email.is_none());
    }

    #[test]
    fn parse_auth_callback_missing_params() {
        // Missing token
        let callback = "visio://auth-callback?instance=meet.example.com&state=abc";
        assert!(AuthService::parse_auth_callback(callback).is_err());

        // Missing state
        let callback = "visio://auth-callback?instance=meet.example.com&token=tok";
        assert!(AuthService::parse_auth_callback(callback).is_err());

        // Missing instance
        let callback = "visio://auth-callback?state=abc&token=tok";
        assert!(AuthService::parse_auth_callback(callback).is_err());
    }

    #[test]
    fn parse_auth_callback_wrong_scheme() {
        let callback = "https://auth-callback?instance=meet.example.com&state=abc&token=tok";
        assert!(AuthService::parse_auth_callback(callback).is_err());
    }

    #[test]
    fn handle_auth_callback_success() {
        let storage = Arc::new(MemoryStorage::new());
        let session_manager = SessionManager::new(storage);

        // Start auth flow
        let state = session_manager.start_auth("meet.example.com").unwrap();

        // Build callback URL
        let callback = format!(
            "visio://auth-callback?instance=meet.example.com&state={}&token=sessiontoken&expires=9999999999999",
            state
        );

        // Handle callback
        let session = AuthService::handle_auth_callback(&callback, &session_manager).unwrap();
        assert_eq!(session.instance, "meet.example.com");
        assert_eq!(session.session_token, "sessiontoken");

        // Verify session is stored
        assert!(session_manager.is_authenticated("meet.example.com"));
    }

    #[test]
    fn handle_auth_callback_invalid_state() {
        let storage = Arc::new(MemoryStorage::new());
        let session_manager = SessionManager::new(storage);

        // Start auth flow
        let _state = session_manager.start_auth("meet.example.com").unwrap();

        // Try to use a different state
        let callback = "visio://auth-callback?instance=meet.example.com&state=wrong_state&token=tok";
        assert!(AuthService::handle_auth_callback(callback, &session_manager).is_err());
    }

    #[test]
    fn handle_auth_callback_instance_mismatch() {
        let storage = Arc::new(MemoryStorage::new());
        let session_manager = SessionManager::new(storage);

        // Start auth flow for one instance
        let state = session_manager.start_auth("meet.example.com").unwrap();

        // Callback comes from a different instance
        let callback = format!(
            "visio://auth-callback?instance=other.example.com&state={}&token=tok",
            state
        );
        assert!(AuthService::handle_auth_callback(&callback, &session_manager).is_err());
    }

    #[test]
    fn generate_random_slug_format() {
        for _ in 0..10 {
            let slug = AuthService::generate_random_slug();
            // Verify format: xxx-yyyy-zzz
            let parts: Vec<&str> = slug.split('-').collect();
            assert_eq!(parts.len(), 3, "slug should have 3 parts: {}", slug);
            assert_eq!(parts[0].len(), 3, "first part should be 3 chars");
            assert_eq!(parts[1].len(), 4, "second part should be 4 chars");
            assert_eq!(parts[2].len(), 3, "third part should be 3 chars");
            // Verify all lowercase
            assert!(
                slug.chars().all(|c| c.is_ascii_lowercase() || c == '-'),
                "slug should be lowercase letters and dashes: {}",
                slug
            );
            // Verify it passes extract_slug validation
            assert!(
                AuthService::extract_slug(&slug).is_ok(),
                "generated slug should be valid: {}",
                slug
            );
        }
    }

    #[test]
    fn generate_random_slug_uniqueness() {
        let slugs: std::collections::HashSet<String> = (0..100)
            .map(|_| AuthService::generate_random_slug())
            .collect();
        // With 26^10 possibilities, collisions are extremely unlikely
        assert_eq!(slugs.len(), 100, "100 slugs should all be unique");
    }
}
