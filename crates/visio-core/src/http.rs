//! Shared HTTP client and bounded-body helpers.
//!
//! Every outgoing request from `visio-core` (and the FFI's inline refresh
//! path) routes through [`shared_http_client`] so we get:
//!
//! - `redirect::Policy::none()` — never follow a redirect. reqwest only strips
//!   the `Authorization` header on *cross-host* redirects, so a 302 from
//!   `meet.example.com` to `http://meet.example.com/` (same host, plaintext)
//!   or to a different port would leak the Bearer otherwise.
//! - `timeout(30s)` — a hung server must not stall FFI threads forever.
//! - `https_only(true)` in release builds — refuses to even attempt cleartext.
//!   Debug builds keep HTTP available for `localhost` / emulator targets.
//!
//! [`bounded_text`] caps response bodies before feeding them to
//! `serde_json::from_str`. A malicious or compromised backend returning a
//! multi-gigabyte JSON document would otherwise OOM-kill the mobile process.
//! 10 MiB is several orders of magnitude above every endpoint we hit.
//!
//! Audit findings: H2 (default `reqwest::Client::new()` follows redirects /
//! has no timeout) and M4 (unbounded `resp.text().await`) — see
//! `docs/security/2026-06-04-pkce-audit-rust.md`.

use std::sync::OnceLock;
use std::time::Duration;

use crate::errors::VisioError;

/// 10 MiB. Larger than any JSON body any Meet endpoint we touch ever returns.
pub const MAX_JSON_BODY: usize = 10 * 1024 * 1024;

/// Build the process-wide HTTP client used by every `visio-core` and
/// `visio-ffi` request. Constructed once on first call; reused across
/// modules so TLS handshakes are pooled.
fn build_client() -> reqwest::Client {
    let builder = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(30));

    // Force TLS in release builds. Debug builds still need HTTP for
    // `localhost` / `10.0.2.2` / emulator hosts (see `tokens::scheme_for`).
    #[cfg(not(debug_assertions))]
    let builder = builder.https_only(true);

    builder.build().expect("failed to build shared HTTP client")
}

/// Returns the shared HTTP client. First call constructs it; subsequent
/// calls return the cached instance.
pub fn shared_http_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(build_client)
}

/// Read the response body as UTF-8 text, refusing to allocate more than
/// `max_bytes`. Mirrors `Response::text()` but bails as soon as the running
/// total exceeds the cap, so a malicious server cannot OOM the device.
///
/// Uses `Response::chunk()` (always available, no `stream` feature needed).
pub async fn bounded_text(
    mut resp: reqwest::Response,
    max_bytes: usize,
) -> Result<String, VisioError> {
    // If the server advertises an oversized body up front, refuse immediately.
    if let Some(len) = resp.content_length()
        && len as usize > max_bytes
    {
        return Err(VisioError::Http(format!(
            "response body too large: {len} > {max_bytes}"
        )));
    }

    let mut buf: Vec<u8> = Vec::new();
    while let Some(chunk) = resp
        .chunk()
        .await
        .map_err(|e| VisioError::Http(e.to_string()))?
    {
        if buf.len() + chunk.len() > max_bytes {
            return Err(VisioError::Http(format!(
                "response body exceeded {max_bytes} bytes"
            )));
        }
        buf.extend_from_slice(&chunk);
    }

    String::from_utf8(buf).map_err(|e| VisioError::Http(format!("invalid UTF-8 body: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_is_reused() {
        let a = shared_http_client() as *const _;
        let b = shared_http_client() as *const _;
        assert_eq!(a, b, "shared_http_client must return the same instance");
    }
}
