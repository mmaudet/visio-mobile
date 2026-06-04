use serde::{Deserialize, Serialize};

use crate::auth::AuthService;
use crate::errors::VisioError;
use crate::http::{MAX_JSON_BODY, bounded_text, shared_http_client};

/// A user returned by the Meet user search API.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UserSearchResult {
    pub id: String,
    pub email: String,
    pub full_name: Option<String>,
    pub short_name: Option<String>,
}

/// A resource access entry (user linked to a room with a role).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RoomAccess {
    pub id: String,
    pub user: UserSearchResult,
    pub resource: String,
    pub role: String,
}

/// Paginated response wrapper from Meet API.
#[derive(Debug, Deserialize)]
struct PaginatedResponse<T> {
    results: Vec<T>,
}

/// Room detail response — only the fields we need.
#[derive(Debug, Deserialize)]
struct RoomDetailResponse {
    #[serde(default)]
    accesses: Vec<RoomAccess>,
}

/// Service for managing room access (restricted rooms).
pub struct AccessService;

impl AccessService {
    /// Search users by email (trigram similarity).
    pub async fn search_users(
        meet_url: &str,
        access_token: &str,
        query: &str,
    ) -> Result<Vec<UserSearchResult>, VisioError> {
        let (instance, _slug) = AuthService::parse_meet_url(meet_url)?;

        let api_url = format!(
            "{}://{}/api/v1.0/users/?q={}",
            crate::tokens::scheme_for(&instance),
            instance,
            urlencoding::encode(query)
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
                "user search returned status {}",
                resp.status()
            )));
        }

        let body = bounded_text(resp, MAX_JSON_BODY).await?;

        let page: PaginatedResponse<UserSearchResult> = serde_json::from_str(&body)
            .map_err(|e| VisioError::Auth(format!("invalid user search response: {e}")))?;

        Ok(page.results)
    }

    /// List accesses for a room by fetching the room detail.
    pub async fn list_accesses(
        meet_url: &str,
        access_token: &str,
        room_id: &str,
    ) -> Result<Vec<RoomAccess>, VisioError> {
        let (instance, _slug) = AuthService::parse_meet_url(meet_url)?;

        let api_url = format!(
            "{}://{}/api/v1.0/rooms/{}/",
            crate::tokens::scheme_for(&instance),
            instance,
            room_id
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
                "room detail returned status {}",
                resp.status()
            )));
        }

        let body = bounded_text(resp, MAX_JSON_BODY).await?;

        let room: RoomDetailResponse = serde_json::from_str(&body)
            .map_err(|e| VisioError::Auth(format!("invalid room detail response: {e}")))?;

        Ok(room.accesses)
    }

    /// Add a user as member of a room.
    pub async fn add_access(
        meet_url: &str,
        access_token: &str,
        user_id: &str,
        room_id: &str,
    ) -> Result<RoomAccess, VisioError> {
        let (instance, _slug) = AuthService::parse_meet_url(meet_url)?;

        let api_url = format!(
            "{}://{}/api/v1.0/resource-accesses/",
            crate::tokens::scheme_for(&instance),
            instance
        );

        let body = serde_json::json!({
            "user": user_id,
            "resource": room_id,
            "role": "member",
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

        if resp.status() == reqwest::StatusCode::BAD_REQUEST {
            return Err(VisioError::Session("Already invited".to_string()));
        }

        if !resp.status().is_success() {
            let status = resp.status();
            let body = bounded_text(resp, MAX_JSON_BODY).await.unwrap_or_default();
            return Err(VisioError::Auth(format!(
                "add-access returned status {}: {}",
                status, body
            )));
        }

        let body = bounded_text(resp, MAX_JSON_BODY).await?;

        serde_json::from_str(&body)
            .map_err(|e| VisioError::Auth(format!("invalid add-access response: {e}")))
    }

    /// Remove an access (revoke membership).
    pub async fn remove_access(
        meet_url: &str,
        access_token: &str,
        access_id: &str,
    ) -> Result<(), VisioError> {
        let (instance, _slug) = AuthService::parse_meet_url(meet_url)?;

        let api_url = format!(
            "{}://{}/api/v1.0/resource-accesses/{}/",
            crate::tokens::scheme_for(&instance),
            instance,
            access_id
        );

        let resp = shared_http_client()
            .delete(&api_url)
            .header(
                reqwest::header::AUTHORIZATION,
                format!("Bearer {}", access_token),
            )
            .send()
            .await
            .map_err(|e| VisioError::Http(e.to_string()))?;

        if !resp.status().is_success() && resp.status() != reqwest::StatusCode::NO_CONTENT {
            let status = resp.status();
            let body = bounded_text(resp, MAX_JSON_BODY).await.unwrap_or_default();
            return Err(VisioError::Auth(format!(
                "remove-access returned status {}: {}",
                status, body
            )));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_user_search_result() {
        let json = r#"{"id": "abc-123", "email": "alice@example.com", "full_name": "Alice Doe", "short_name": "Alice"}"#;
        let user: UserSearchResult = serde_json::from_str(json).unwrap();
        assert_eq!(user.id, "abc-123");
        assert_eq!(user.email, "alice@example.com");
        assert_eq!(user.full_name, Some("Alice Doe".to_string()));
    }

    #[test]
    fn parse_user_search_result_minimal() {
        let json = r#"{"id": "abc-123", "email": "alice@example.com"}"#;
        let user: UserSearchResult = serde_json::from_str(json).unwrap();
        assert!(user.full_name.is_none());
        assert!(user.short_name.is_none());
    }

    #[test]
    fn parse_room_access() {
        let json = r#"{
            "id": "ra-1",
            "user": {"id": "u-1", "email": "bob@example.com", "full_name": "Bob", "short_name": null},
            "resource": "room-123",
            "role": "member"
        }"#;
        let access: RoomAccess = serde_json::from_str(json).unwrap();
        assert_eq!(access.id, "ra-1");
        assert_eq!(access.user.email, "bob@example.com");
        assert_eq!(access.role, "member");
    }

    #[test]
    fn parse_room_access_with_extra_user_fields() {
        // The rooms endpoint returns user objects with extra fields (timezone, language)
        // that we ignore via serde.
        let json = r#"{
            "id": "ra-1",
            "user": {"id": "u-1", "email": "bob@example.com", "full_name": null, "short_name": null, "timezone": "Europe/Paris", "language": "fr-fr"},
            "resource": "room-123",
            "role": "owner"
        }"#;
        let access: RoomAccess = serde_json::from_str(json).unwrap();
        assert_eq!(access.user.id, "u-1");
        assert_eq!(access.role, "owner");
    }

    #[test]
    fn parse_room_detail_with_accesses() {
        let json = r#"{
            "id": "room-1",
            "name": "Test Room",
            "slug": "test-room",
            "access_level": "restricted",
            "accesses": [
                {"id": "ra-1", "user": {"id": "u-1", "email": "a@b.com", "full_name": null, "short_name": null}, "resource": "room-1", "role": "owner"},
                {"id": "ra-2", "user": {"id": "u-2", "email": "c@d.com", "full_name": "C D", "short_name": null}, "resource": "room-1", "role": "member"}
            ]
        }"#;
        let room: RoomDetailResponse = serde_json::from_str(json).unwrap();
        assert_eq!(room.accesses.len(), 2);
        assert_eq!(room.accesses[0].role, "owner");
        assert_eq!(room.accesses[1].user.email, "c@d.com");
    }

    #[test]
    fn parse_room_detail_without_accesses() {
        // Non-admin users won't see the accesses field
        let json = r#"{"id": "room-1", "name": "Test", "slug": "test"}"#;
        let room: RoomDetailResponse = serde_json::from_str(json).unwrap();
        assert!(room.accesses.is_empty());
    }

    #[test]
    fn parse_paginated_users() {
        let json = r#"{"count": 2, "next": null, "previous": null, "results": [
            {"id": "u-1", "email": "a@b.com", "full_name": "A", "short_name": null},
            {"id": "u-2", "email": "c@d.com", "full_name": "C", "short_name": null}
        ]}"#;
        let page: PaginatedResponse<UserSearchResult> = serde_json::from_str(json).unwrap();
        assert_eq!(page.results.len(), 2);
    }

    #[tokio::test]
    async fn search_users_invalid_url_returns_error() {
        assert!(
            AccessService::search_users("invalid", "cookie", "query")
                .await
                .is_err()
        );
    }
}
