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
    /// `session_cookie` is an optional `sessionid` cookie for authenticated instances.
    pub async fn request_token(
        meet_url: &str,
        username: Option<&str>,
        session_cookie: Option<&str>,
    ) -> Result<TokenInfo, VisioError> {
        let (instance, slug) = Self::parse_meet_url(meet_url)?;

        let mut api_url = format!("https://{}/api/v1.0/rooms/{}/", instance, slug);
        if let Some(name) = username {
            let encoded = urlencoding::encode(name);
            api_url.push_str(&format!("?username={encoded}"));
        }

        tracing::info!("requesting token from Meet API: {}", api_url);

        // Disable auto-redirect so we can detect auth redirects (302 → /authenticate/)
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| VisioError::Http(e.to_string()))?;

        let mut request = client.get(&api_url);
        if let Some(cookie) = session_cookie {
            request = request.header(reqwest::header::COOKIE, format!("sessionid={}", cookie));
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

        tracing::info!("Meet API response body: {}", body);

        let data: MeetApiResponse = serde_json::from_str(&body).map_err(|e| {
            VisioError::Auth(format!("invalid Meet API response: {e} — body: {body}"))
        })?;

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

        // Strip query parameters before extracting the slug
        let stripped = Self::strip_query_params(input);
        let input = stripped.trim().trim_end_matches('/');
        let candidate = if input.contains('/') {
            input.rsplit('/').next().unwrap_or("")
        } else {
            input
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
        session_cookie: Option<&str>,
    ) -> Result<TokenInfo, VisioError> {
        Self::request_token(meet_url, username, session_cookie).await
    }

    /// Extract the Meet instance hostname from a room URL.
    pub fn parse_instance(meet_url: &str) -> Result<String, VisioError> {
        let (instance, _) = Self::parse_meet_url(meet_url)?;
        Ok(instance)
    }

    /// Parse a Meet URL into (instance, room_slug).
    /// Query parameters (e.g. `?name=...`) are stripped from the slug.
    pub fn parse_meet_url(url: &str) -> Result<(String, String), VisioError> {
        let url = Self::strip_query_params(url);
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

    /// Extract the `name` query parameter from a URL, if present.
    /// E.g. `https://meet.example.com/abc-defg-hij?name=Team+Standup` → Some("Team Standup")
    pub fn extract_room_name(url: &str) -> Option<String> {
        let query_start = url.find('?')?;
        let query = &url[query_start + 1..];
        for pair in query.split('&') {
            let mut kv = pair.splitn(2, '=');
            let key = kv.next()?;
            let value = kv.next().unwrap_or("");
            if key == "name" {
                let decoded = urlencoding::decode(value).unwrap_or_else(|_| value.into());
                let decoded = decoded.replace('+', " ");
                if decoded.is_empty() {
                    return None;
                }
                return Some(decoded);
            }
        }
        None
    }

    /// Strip all query parameters from a URL.
    /// E.g. `https://meet.example.com/abc-defg-hij?name=Foo` → `https://meet.example.com/abc-defg-hij`
    pub fn strip_query_params(url: &str) -> String {
        match url.find('?') {
            Some(pos) => url[..pos].to_string(),
            None => url.to_string(),
        }
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
    fn extract_slug_strips_query_params() {
        let slug =
            AuthService::extract_slug("https://meet.example.com/abc-defg-hij?name=Team+Standup")
                .unwrap();
        assert_eq!(slug, "abc-defg-hij");
    }

    #[test]
    fn parse_meet_url_strips_query_params() {
        let (instance, slug) =
            AuthService::parse_meet_url("https://meet.example.com/abc-defg-hij?name=My+Room")
                .unwrap();
        assert_eq!(instance, "meet.example.com");
        assert_eq!(slug, "abc-defg-hij");
    }

    #[test]
    fn extract_room_name_basic() {
        let name = AuthService::extract_room_name(
            "https://meet.example.com/abc-defg-hij?name=Team+Standup",
        );
        assert_eq!(name, Some("Team Standup".to_string()));
    }

    #[test]
    fn extract_room_name_encoded() {
        let name = AuthService::extract_room_name(
            "https://meet.example.com/abc-defg-hij?name=My%20Room%21",
        );
        assert_eq!(name, Some("My Room!".to_string()));
    }

    #[test]
    fn extract_room_name_missing() {
        let name = AuthService::extract_room_name("https://meet.example.com/abc-defg-hij");
        assert_eq!(name, None);
    }

    #[test]
    fn extract_room_name_empty() {
        let name = AuthService::extract_room_name("https://meet.example.com/abc-defg-hij?name=");
        assert_eq!(name, None);
    }

    #[test]
    fn strip_query_params_basic() {
        assert_eq!(
            AuthService::strip_query_params("https://meet.example.com/room?name=Foo&bar=1"),
            "https://meet.example.com/room"
        );
    }

    #[test]
    fn strip_query_params_no_params() {
        assert_eq!(
            AuthService::strip_query_params("https://meet.example.com/room"),
            "https://meet.example.com/room"
        );
    }
}
