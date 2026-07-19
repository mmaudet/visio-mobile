use crate::errors::VisioError;
use serde::Deserialize;

/// Response from the Meet API.
#[derive(Debug, Deserialize)]
struct MeetApiResponse {
    livekit: Option<LiveKitCredentials>,
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

/// Requests a LiveKit token from the Meet API.
pub struct AuthService;

impl AuthService {
    /// Call the Meet API to get a LiveKit token for the given room.
    ///
    /// `meet_url` should be a full URL like `https://meet.example.com/room-slug`
    /// or just `meet.example.com/room-slug`.
    ///
    /// `access_token` is an optional OAuth2 JWT for authenticated instances.
    pub async fn request_token(
        meet_url: &str,
        username: Option<&str>,
        access_token: Option<&str>,
    ) -> Result<TokenInfo, VisioError> {
        let (instance, slug) = Self::parse_meet_url(meet_url)?;

        let mut api_url = format!(
            "{}://{}/api/v1.0/rooms/{}/",
            crate::tokens::scheme_for(&instance),
            instance,
            slug
        );
        if let Some(name) = username {
            let encoded = urlencoding::encode(name);
            api_url.push_str(&format!("?username={encoded}"));
        }

        tracing::info!(
            "requesting token from Meet API: instance={}, slug={}",
            instance,
            slug
        );

        // Disable auto-redirect so we can detect auth redirects (302 → /authenticate/)
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| VisioError::Http(e.to_string()))?;

        let mut request = client.get(&api_url);
        if let Some(token) = access_token {
            request = request.header(reqwest::header::AUTHORIZATION, format!("Bearer {}", token));
        }

        let resp = request
            .send()
            .await
            .map_err(|e| VisioError::Http(e.to_string()))?;

        // A redirect means the server wants us to authenticate
        if resp.status().is_redirection() || resp.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(VisioError::AuthRequired);
        }

        if !resp.status().is_success() {
            return Err(VisioError::Auth(format!(
                "Meet API returned status {}",
                resp.status()
            )));
        }

        let body = resp
            .text()
            .await
            .map_err(|e| VisioError::Http(e.to_string()))?;

        tracing::debug!("Meet API response received ({} bytes)", body.len());

        let data: MeetApiResponse = serde_json::from_str(&body)
            .map_err(|e| VisioError::Auth(format!("invalid Meet API response: {e}")))?;

        let lk = data.livekit.ok_or(VisioError::WaitingForHost)?;

        // Convert URL to WebSocket
        let livekit_url = lk
            .url
            .replace("https://", "wss://")
            .replace("http://", "ws://");

        Ok(TokenInfo {
            livekit_url,
            token: lk.token,
        })
    }

    /// Extract and validate the room slug from user input.
    /// Accepts full URL (`https://meet.example.com/abc-defg-hij`) or bare slug (`abc-defg-hij`).
    /// Slug format: 3 lowercase + dash + 4 lowercase + dash + 3 lowercase.
    pub fn extract_slug(input: &str) -> Result<String, VisioError> {
        use std::sync::OnceLock;
        static SLUG_RE: OnceLock<regex::Regex> = OnceLock::new();

        let input = input.trim().trim_end_matches('/');
        let candidate = if input.contains('/') {
            input.rsplit('/').next().unwrap_or("")
        } else {
            input
        };
        let candidate = if candidate.contains('?') {
            candidate.split('?').next().unwrap_or("")
        } else {
            candidate
        };
        let re =
            SLUG_RE.get_or_init(|| regex::Regex::new(r"^[a-z]{3}-[a-z]{4}-[a-z]{3}$").unwrap());
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
        access_token: Option<&str>,
    ) -> Result<TokenInfo, VisioError> {
        Self::request_token(meet_url, username, access_token).await
    }

    /// Extract the Meet instance hostname from a room URL.
    pub fn parse_instance(meet_url: &str) -> Result<String, VisioError> {
        let (instance, _) = Self::parse_meet_url(meet_url)?;
        Ok(instance)
    }

    /// Parse a Meet URL into (instance, room_slug).
    pub fn parse_meet_url(url: &str) -> Result<(String, String), VisioError> {
        let url = crate::strip_room_display_name_param(url.trim().trim_end_matches('/'));
        let url = url.replace("https://", "").replace("http://", "");

        // Reject dangerous schemes that might have slipped through
        let lower = url.to_lowercase();
        if lower.starts_with("javascript:")
            || lower.starts_with("data:")
            || lower.starts_with("file:")
            || lower.starts_with("vbscript:")
        {
            return Err(VisioError::InvalidUrl(format!(
                "invalid scheme in URL: {}",
                url.split(':').next().unwrap_or("")
            )));
        }

        // Reject null bytes
        if url.contains('\0') {
            return Err(VisioError::InvalidUrl("null byte in URL".into()));
        }

        // Reject path traversal attempts in the slug part
        let parts: Vec<&str> = url.splitn(2, '/').collect();
        if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
            return Err(VisioError::InvalidUrl(format!(
                "expected 'instance/room-slug', got '{url}'"
            )));
        }

        // Check for path traversal in slug
        if parts[1].contains("..") {
            return Err(VisioError::InvalidUrl(
                "path traversal detected in room slug".into(),
            ));
        }

        Ok((parts[0].to_string(), parts[1].to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn parse_meet_url_strips_display_name_param() {
        let (instance, slug) =
            AuthService::parse_meet_url("https://meet.example.com/abc-defg-hij?visio=Comex")
                .unwrap();
        assert_eq!(instance, "meet.example.com");
        assert_eq!(slug, "abc-defg-hij");
    }

    #[test]
    fn extract_slug_with_query_param() {
        let slug =
            AuthService::extract_slug("https://meet.example.com/abc-defg-hij?visio=Test").unwrap();
        assert_eq!(slug, "abc-defg-hij");
    }
}
