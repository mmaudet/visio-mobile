use crate::errors::VisioError;
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

/// Requests a LiveKit token from the Meet API.
pub struct AuthService;

impl AuthService {
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
        let (instance, slug) =
            AuthService::parse_meet_url("meet.example.com/room-123").unwrap();
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
}
