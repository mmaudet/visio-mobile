use serde::Deserialize;

use crate::auth::AuthService;
use crate::errors::VisioError;
use crate::http::{MAX_JSON_BODY, bounded_text, shared_http_client};

/// Status returned by the lobby API.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LobbyStatus {
    Waiting,
    Accepted,
    Denied,
    #[serde(other)]
    Unknown,
}

/// A participant currently waiting in the lobby.
#[derive(Debug, Clone, Deserialize)]
pub struct WaitingParticipant {
    pub id: String,
    pub username: String,
}

/// Wrapper for the waiting-participants API response.
#[derive(Debug, Deserialize)]
struct WaitingParticipantsResponse {
    participants: Vec<WaitingParticipant>,
}

/// Result of polling the lobby for entry status.
#[derive(Debug, Clone)]
pub enum LobbyPollResult {
    Waiting,
    Accepted { livekit_url: String, token: String },
    Denied,
}

#[derive(Debug, Clone, Deserialize)]
struct RequestEntryResponse {
    status: LobbyStatus,
    id: Option<String>,
    livekit: Option<RequestEntryLiveKit>,
}

#[derive(Debug, Clone, Deserialize)]
struct RequestEntryLiveKit {
    url: String,
    token: String,
}

/// Service for interacting with the waiting room (lobby) API.
pub struct LobbyService;

impl LobbyService {
    /// Request entry to a room's lobby.
    ///
    /// Returns `(participant_id, lobby_cookie, LobbyPollResult)`.
    pub async fn request_entry(
        meet_url: &str,
        username: &str,
    ) -> Result<(String, String, LobbyPollResult), VisioError> {
        let (instance, slug) = AuthService::parse_meet_url(meet_url)?;

        let api_url = format!(
            "{}://{}/api/v1.0/rooms/{}/request-entry/",
            crate::tokens::scheme_for(&instance),
            instance,
            slug
        );

        let body = serde_json::json!({ "username": username });

        let resp = shared_http_client()
            .post(&api_url)
            .json(&body)
            .send()
            .await
            .map_err(|e| VisioError::Http(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(VisioError::Auth(format!(
                "request-entry returned status {}",
                resp.status()
            )));
        }

        // Extract lobby cookie from Set-Cookie header.
        // This is server-issued lobby state (unrelated to OAuth), keyed by the
        // anonymous waiting user; we forward it back when polling status.
        let lobby_cookie = resp
            .headers()
            .get_all(reqwest::header::SET_COOKIE)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .filter_map(|s| s.split(';').next())
            .next()
            .unwrap_or("")
            .to_string();

        let resp_body = bounded_text(resp, MAX_JSON_BODY).await?;

        let data: RequestEntryResponse = serde_json::from_str(&resp_body)
            .map_err(|e| VisioError::Auth(format!("invalid request-entry response: {e}")))?;

        let participant_id = data.id.clone().unwrap_or_default();
        let poll_result = Self::status_to_poll_result(&data);

        Ok((participant_id, lobby_cookie, poll_result))
    }

    /// Poll the lobby for entry status (re-sends lobby cookie).
    pub async fn poll_entry(
        meet_url: &str,
        username: &str,
        lobby_cookie: &str,
    ) -> Result<LobbyPollResult, VisioError> {
        let (instance, slug) = AuthService::parse_meet_url(meet_url)?;

        let api_url = format!(
            "{}://{}/api/v1.0/rooms/{}/request-entry/",
            crate::tokens::scheme_for(&instance),
            instance,
            slug
        );

        let body = serde_json::json!({ "username": username });

        // lobby_cookie is server-issued lobby state, sent back as a Cookie.
        let resp = shared_http_client()
            .post(&api_url)
            .header(reqwest::header::COOKIE, lobby_cookie)
            .json(&body)
            .send()
            .await
            .map_err(|e| VisioError::Http(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(VisioError::Auth(format!(
                "poll-entry returned status {}",
                resp.status()
            )));
        }

        let resp_body = bounded_text(resp, MAX_JSON_BODY).await?;

        let data: RequestEntryResponse = serde_json::from_str(&resp_body)
            .map_err(|e| VisioError::Auth(format!("invalid poll-entry response: {e}")))?;

        Ok(Self::status_to_poll_result(&data))
    }

    /// List participants currently waiting in the lobby (host only).
    pub async fn list_waiting(
        meet_url: &str,
        access_token: &str,
    ) -> Result<Vec<WaitingParticipant>, VisioError> {
        let (instance, slug) = AuthService::parse_meet_url(meet_url)?;

        let api_url = format!(
            "{}://{}/api/v1.0/rooms/{}/waiting-participants/",
            crate::tokens::scheme_for(&instance),
            instance,
            slug
        );

        let resp = shared_http_client()
            .get(&api_url)
            .header(
                reqwest::header::AUTHORIZATION,
                format!("Bearer {}", access_token),
            )
            .send()
            .await
            .map_err(|e| VisioError::Http(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(VisioError::Auth(format!(
                "list-waiting returned status {}",
                resp.status()
            )));
        }

        let body = bounded_text(resp, MAX_JSON_BODY).await?;

        let resp: WaitingParticipantsResponse = serde_json::from_str(&body)
            .map_err(|e| VisioError::Auth(format!("invalid waiting-participants response: {e}")))?;

        Ok(resp.participants)
    }

    /// Allow or deny a waiting participant (host only).
    pub async fn handle_entry(
        meet_url: &str,
        access_token: &str,
        participant_id: &str,
        allow: bool,
    ) -> Result<(), VisioError> {
        let (instance, slug) = AuthService::parse_meet_url(meet_url)?;

        let api_url = format!(
            "{}://{}/api/v1.0/rooms/{}/enter/",
            crate::tokens::scheme_for(&instance),
            instance,
            slug
        );

        let body = serde_json::json!({
            "participant_id": participant_id,
            "allow_entry": allow,
        });

        let resp = shared_http_client()
            .post(&api_url)
            .header(
                reqwest::header::AUTHORIZATION,
                format!("Bearer {}", access_token),
            )
            .json(&body)
            .send()
            .await
            .map_err(|e| VisioError::Http(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = bounded_text(resp, MAX_JSON_BODY).await.unwrap_or_default();
            return Err(VisioError::Auth(format!(
                "handle-entry returned status {}: {}",
                status, body
            )));
        }

        Ok(())
    }

    fn status_to_poll_result(data: &RequestEntryResponse) -> LobbyPollResult {
        match data.status {
            LobbyStatus::Accepted => {
                if let Some(ref lk) = data.livekit {
                    let livekit_url = lk
                        .url
                        .replace("https://", "wss://")
                        .replace("http://", "ws://");
                    LobbyPollResult::Accepted {
                        livekit_url,
                        token: lk.token.clone(),
                    }
                } else {
                    // Accepted but no LiveKit credentials — treat as waiting
                    LobbyPollResult::Waiting
                }
            }
            LobbyStatus::Denied => LobbyPollResult::Denied,
            _ => LobbyPollResult::Waiting,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_lobby_status_waiting() {
        let json = r#"{"status": "waiting", "id": "abc-123", "livekit": null}"#;
        let resp: RequestEntryResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.status, LobbyStatus::Waiting);
        assert_eq!(resp.id, Some("abc-123".to_string()));
        assert!(resp.livekit.is_none());
    }

    #[test]
    fn parse_lobby_status_accepted_with_livekit() {
        let json = r#"{"status": "accepted", "id": "abc-123", "livekit": {"url": "https://lk.example.com", "token": "eyJ..."}}"#;
        let resp: RequestEntryResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.status, LobbyStatus::Accepted);
        assert!(resp.livekit.is_some());
    }

    #[test]
    fn parse_lobby_status_denied() {
        let json = r#"{"status": "denied", "id": "abc-123", "livekit": null}"#;
        let resp: RequestEntryResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.status, LobbyStatus::Denied);
    }

    #[test]
    fn parse_lobby_status_unknown() {
        let json = r#"{"status": "expired", "id": "abc-123", "livekit": null}"#;
        let resp: RequestEntryResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.status, LobbyStatus::Unknown);
    }

    #[test]
    fn parse_waiting_participant() {
        let wp: WaitingParticipant =
            serde_json::from_str(r#"{"id": "p-123", "username": "Alice"}"#).unwrap();
        assert_eq!(wp.id, "p-123");
        assert_eq!(wp.username, "Alice");
    }

    #[test]
    fn parse_waiting_participants_response() {
        let resp: WaitingParticipantsResponse = serde_json::from_str(
            r#"{"participants": [{"id": "p-1", "username": "Alice"}, {"id": "p-2", "username": "Bob"}]}"#,
        )
        .unwrap();
        assert_eq!(resp.participants.len(), 2);
    }

    #[tokio::test]
    async fn request_entry_with_invalid_url_returns_error() {
        assert!(
            LobbyService::request_entry("invalid-url", "Alice")
                .await
                .is_err()
        );
    }
}
