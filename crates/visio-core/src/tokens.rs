//! OAuth2 token pair management with PKCE exchange and refresh.
//!
//! Companion to [`crate::pkce`]. After the SSO browser closes and the deep
//! link delivers an authorization code, callers exchange `(code, verifier)`
//! at the Meet `oauth/token/` endpoint to obtain a JWT access + refresh pair.
//! Subsequent refresh requests are deduplicated so concurrent 401s only
//! produce one network call.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, RwLock};

use crate::errors::VisioError;
#[cfg(feature = "oidc")]
use crate::http::{MAX_JSON_BODY, bounded_text, shared_http_client};

/// Pick the scheme for a Meet host. Returns `http` only for parsable loopback
/// or RFC1918 IP literals, plus a tiny allowlist of dev hostnames; everything
/// else (including any *.example.com lookalike, public IPs, or untrusted
/// input) defaults to `https`.
///
/// Background: we initially detected "local" hosts via string-prefix matches
/// such as `starts_with("10.")` — which accepts `10.attacker.com` as well as
/// `10.0.0.5` and would silently downgrade a refresh-token POST to cleartext.
/// To prevent that, we now parse the host part as an [`IpAddr`] and apply the
/// well-defined RFC1918 / loopback predicates. Hostnames that aren't IPs only
/// match an explicit allowlist; anything else stays on TLS.
pub fn scheme_for(host: &str) -> &'static str {
    use std::net::IpAddr;

    // Strip an optional `:port` while preserving IPv6 literals. URL syntax
    // wraps IPv6 in brackets (`[::1]:8080`); a bare `::1` (test/helper input
    // form) must not be sliced at the last colon or it becomes ":" + "1".
    let host_part = if let Some(rest) = host.strip_prefix('[') {
        rest.split_once(']').map(|(h, _)| h).unwrap_or(rest)
    } else if host.matches(':').count() <= 1 {
        host.rsplit_once(':').map(|(h, _)| h).unwrap_or(host)
    } else {
        host
    };
    let lower = host_part.to_ascii_lowercase();

    // Explicit dev hostnames we trust to be loopback aliases.
    const DEV_HOSTNAMES: &[&str] = &[
        "localhost",
        "127.0.0.1.nip.io", // LiveKit dev URL Meet returns in compose
    ];
    if DEV_HOSTNAMES.contains(&lower.as_str()) {
        return "http";
    }

    // Otherwise, only IP literals can opt in to cleartext, and only if they
    // are loopback or RFC1918 private ranges.
    match lower.parse::<IpAddr>() {
        Ok(IpAddr::V4(ip)) => {
            if ip.is_loopback() || ip.is_private() {
                "http"
            } else {
                "https"
            }
        }
        Ok(IpAddr::V6(ip)) => {
            if ip.is_loopback() {
                "http"
            } else {
                "https"
            }
        }
        Err(_) => "https",
    }
}

/// Access + refresh JWT pair as returned by SimpleJWT.
///
/// `Debug` is intentionally redacted (`<access redacted> / <refresh redacted>`)
/// so accidental `tracing::error!("{:?}", state)` or `panic!("{:?}", pair)`
/// in the call chain cannot leak meeting credentials to logcat / Console.
#[derive(Clone, Serialize, Deserialize)]
pub struct TokenPair {
    pub access: String,
    pub refresh: String,
}

impl std::fmt::Debug for TokenPair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TokenPair")
            .field("access", &"<redacted>")
            .field("refresh", &"<redacted>")
            .finish()
    }
}

/// In-memory token store with refresh deduplication.
///
/// `Arc<RwLock<...>>` lets read-only access (extracting current access token
/// for an outgoing request) coexist with refresh writes. A separate `Mutex`
/// gates the refresh task so concurrent expirations collapse to a single
/// network call. The lock is only consulted by `refresh_tokens` which is
/// gated on the `oidc` feature, so we silence the dead-code warning on
/// builds without it.
#[derive(Default)]
pub struct TokenStore {
    pair: RwLock<Option<TokenPair>>,
    #[cfg_attr(not(feature = "oidc"), allow(dead_code))]
    refresh_lock: Mutex<()>,
}

impl TokenStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn set(&self, pair: TokenPair) {
        *self.pair.write().await = Some(pair);
    }

    pub async fn clear(&self) {
        *self.pair.write().await = None;
    }

    pub async fn access(&self) -> Option<String> {
        self.pair.read().await.as_ref().map(|p| p.access.clone())
    }

    pub async fn refresh_token(&self) -> Option<String> {
        self.pair.read().await.as_ref().map(|p| p.refresh.clone())
    }

    pub async fn snapshot(&self) -> Option<TokenPair> {
        self.pair.read().await.clone()
    }
}

/// Exchange a PKCE authorization code for a fresh token pair.
///
/// `meet_instance` is the bare host (e.g. `meet.numerique.gouv.fr`).
#[cfg(feature = "oidc")]
pub async fn exchange_pkce_code(
    meet_instance: &str,
    code: &str,
    code_verifier: &str,
) -> Result<TokenPair, VisioError> {
    let url = format!(
        "{}://{}/api/v1.0/oauth/token/",
        scheme_for(meet_instance),
        meet_instance
    );
    let body = serde_json::json!({
        "code": code,
        "code_verifier": code_verifier,
    });

    let resp = shared_http_client()
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| VisioError::Http(e.to_string()))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let txt = bounded_text(resp, MAX_JSON_BODY).await.unwrap_or_default();
        return Err(VisioError::Auth(format!(
            "PKCE token exchange failed ({status}): {txt}"
        )));
    }

    let body = bounded_text(resp, MAX_JSON_BODY).await?;
    serde_json::from_str::<TokenPair>(&body)
        .map_err(|e| VisioError::Auth(format!("invalid token response: {e}")))
}

#[cfg(not(feature = "oidc"))]
pub async fn exchange_pkce_code(
    _meet_instance: &str,
    _code: &str,
    _code_verifier: &str,
) -> Result<TokenPair, VisioError> {
    Err(VisioError::Auth("OIDC support is not enabled".into()))
}

/// Refresh a token pair via SimpleJWT's `/oauth/token/refresh/` endpoint.
///
/// Concurrent refreshes are serialized: only the first caller hits the
/// network, the others wait and observe the new pair via the shared store.
#[cfg(feature = "oidc")]
pub async fn refresh_tokens(
    meet_instance: &str,
    store: Arc<TokenStore>,
) -> Result<TokenPair, VisioError> {
    let _guard = store.refresh_lock.lock().await;

    // Re-read inside the lock: a previous caller may have refreshed already.
    let current = store
        .snapshot()
        .await
        .ok_or_else(|| VisioError::Auth("no refresh token available".into()))?;

    let url = format!(
        "{}://{}/api/v1.0/oauth/token/refresh/",
        scheme_for(meet_instance),
        meet_instance
    );
    let resp = shared_http_client()
        .post(&url)
        .json(&serde_json::json!({ "refresh": current.refresh }))
        .send()
        .await
        .map_err(|e| VisioError::Http(e.to_string()))?;

    let status = resp.status();
    if !status.is_success() {
        let txt = bounded_text(resp, MAX_JSON_BODY).await.unwrap_or_default();
        // On invalid/expired refresh, clear the store so callers re-login.
        store.clear().await;
        return Err(VisioError::Auth(format!(
            "token refresh failed ({status}): {txt}"
        )));
    }

    let body = bounded_text(resp, MAX_JSON_BODY).await?;
    let new_pair: TokenPair = serde_json::from_str(&body)
        .map_err(|e| VisioError::Auth(format!("invalid refresh response: {e}")))?;
    store.set(new_pair.clone()).await;
    Ok(new_pair)
}

#[cfg(not(feature = "oidc"))]
pub async fn refresh_tokens(
    _meet_instance: &str,
    _store: Arc<TokenStore>,
) -> Result<TokenPair, VisioError> {
    Err(VisioError::Auth("OIDC support is not enabled".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn empty_store_returns_none() {
        let s = TokenStore::new();
        assert!(s.access().await.is_none());
        assert!(s.refresh_token().await.is_none());
    }

    #[tokio::test]
    async fn set_then_get_roundtrips() {
        let s = TokenStore::new();
        s.set(TokenPair {
            access: "a".into(),
            refresh: "r".into(),
        })
        .await;
        assert_eq!(s.access().await.as_deref(), Some("a"));
        assert_eq!(s.refresh_token().await.as_deref(), Some("r"));
    }

    #[tokio::test]
    async fn clear_resets() {
        let s = TokenStore::new();
        s.set(TokenPair {
            access: "a".into(),
            refresh: "r".into(),
        })
        .await;
        s.clear().await;
        assert!(s.access().await.is_none());
    }

    // ── scheme_for tests ─────────────────────────────────────────────

    #[test]
    fn scheme_for_localhost_is_http() {
        assert_eq!(scheme_for("localhost"), "http");
        assert_eq!(scheme_for("LOCALHOST"), "http");
        assert_eq!(scheme_for("localhost:8071"), "http");
    }

    #[test]
    fn scheme_for_loopback_ipv4_is_http() {
        assert_eq!(scheme_for("127.0.0.1"), "http");
        assert_eq!(scheme_for("127.0.0.1:8071"), "http");
        assert_eq!(scheme_for("127.1.2.3"), "http"); // entire 127/8 is loopback
    }

    #[test]
    fn scheme_for_loopback_ipv6_is_http() {
        assert_eq!(scheme_for("::1"), "http");
    }

    #[test]
    fn scheme_for_rfc1918_is_http() {
        assert_eq!(scheme_for("10.0.2.2"), "http"); // Android emulator host
        assert_eq!(scheme_for("10.0.3.2"), "http"); // Genymotion host
        assert_eq!(scheme_for("10.0.0.5"), "http");
        assert_eq!(scheme_for("192.168.1.5"), "http");
        assert_eq!(scheme_for("172.16.0.1"), "http");
        assert_eq!(scheme_for("172.31.255.255"), "http");
    }

    #[test]
    fn scheme_for_livekit_dev_alias_is_http() {
        assert_eq!(scheme_for("127.0.0.1.nip.io"), "http");
        assert_eq!(scheme_for("127.0.0.1.nip.io:7880"), "http");
    }

    #[test]
    fn scheme_for_public_hostnames_is_https() {
        assert_eq!(scheme_for("meet.numerique.gouv.fr"), "https");
        assert_eq!(scheme_for("meet.linagora.com"), "https");
        assert_eq!(scheme_for("example.com"), "https");
    }

    #[test]
    fn scheme_for_public_ipv4_is_https() {
        assert_eq!(scheme_for("8.8.8.8"), "https");
        assert_eq!(scheme_for("172.32.0.1"), "https"); // just outside RFC1918
        assert_eq!(scheme_for("172.15.255.255"), "https"); // just below RFC1918
        assert_eq!(scheme_for("11.0.0.1"), "https"); // just outside 10/8
    }

    /// Regression: the previous prefix-based check would happily downgrade
    /// `10.attacker.com` (or any `192.168.…example.com` lookalike) to HTTP,
    /// leaking the Bearer / refresh token in cleartext to whoever owns that
    /// domain. The current implementation refuses to treat non-IP hosts as
    /// private — they must round-trip through `IpAddr::from_str`.
    #[test]
    fn scheme_for_rejects_rfc1918_lookalike_hostnames() {
        assert_eq!(scheme_for("10.attacker.com"), "https");
        assert_eq!(scheme_for("10.0.0.5.attacker.tld"), "https");
        assert_eq!(scheme_for("192.168.1.evil.example"), "https");
        assert_eq!(scheme_for("172.16.evil.net"), "https");
    }
}
