//! Runtime feature flags via Unleash proxy.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Duration;

pub struct FeatureService {
    proxy_url: RwLock<Option<String>>,
    flags: Arc<RwLock<HashMap<String, bool>>>,
    defaults: HashMap<String, bool>,
}

impl FeatureService {
    pub fn new() -> Self {
        let defaults = HashMap::from([
            ("background_blur".into(), true),
            ("screen_share".into(), true),
            ("pip".into(), true),
            ("chat_encryption".into(), false),
            ("speaker_mode".into(), true),
            ("smart_subscriptions".into(), true),
            ("oidc_auth".into(), false),
        ]);
        Self {
            proxy_url: RwLock::new(None),
            flags: Arc::new(RwLock::new(HashMap::new())),
            defaults,
        }
    }

    pub fn is_enabled(&self, name: &str) -> bool {
        if let Some(&val) = self.flags.read().unwrap().get(name) {
            return val;
        }
        self.defaults.get(name).copied().unwrap_or(false)
    }

    pub fn set_proxy_url(&self, url: Option<String>) {
        *self.proxy_url.write().unwrap() = url;
    }

    pub async fn refresh(&self) {
        let url = self.proxy_url.read().unwrap().clone();
        let Some(base_url) = url else { return };
        let client = match reqwest::Client::builder()
            .timeout(Duration::from_secs(3))
            .build()
        {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("feature flags: client build failed: {e}");
                return;
            }
        };
        let resp = match client
            .get(format!("{base_url}/api/client/features"))
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("feature flags: fetch failed: {e}");
                return;
            }
        };
        let body: serde_json::Value = match resp.json().await {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("feature flags: parse failed: {e}");
                return;
            }
        };
        if let Some(toggles) = body.get("toggles").and_then(|t| t.as_array()) {
            let mut flags = self.flags.write().unwrap();
            for toggle in toggles {
                if let (Some(name), Some(enabled)) = (
                    toggle.get("name").and_then(|n| n.as_str()),
                    toggle.get("enabled").and_then(|e| e.as_bool()),
                ) {
                    flags.insert(name.to_string(), enabled);
                }
            }
        }
    }

    pub async fn start_periodic_refresh(self: Arc<Self>) {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            self.refresh().await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_defaults_returned_without_proxy() {
        let svc = FeatureService::new();
        assert!(svc.is_enabled("background_blur"));
        assert!(!svc.is_enabled("chat_encryption"));
    }

    #[test]
    fn test_unknown_feature_returns_false() {
        let svc = FeatureService::new();
        assert!(!svc.is_enabled("nonexistent_feature"));
    }

    #[test]
    fn test_runtime_override() {
        let svc = FeatureService::new();
        assert!(!svc.is_enabled("chat_encryption"));
        svc.flags
            .write()
            .unwrap()
            .insert("chat_encryption".into(), true);
        assert!(svc.is_enabled("chat_encryption"));
    }

    #[tokio::test]
    async fn test_refresh_with_mock_server() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/api/client/features")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{
                    "toggles": [
                        {"name": "chat_encryption", "enabled": true},
                        {"name": "pip", "enabled": false}
                    ]
                }"#,
            )
            .create_async()
            .await;

        let svc = FeatureService::new();
        svc.set_proxy_url(Some(server.url()));
        svc.refresh().await;

        assert!(svc.is_enabled("chat_encryption"));
        assert!(!svc.is_enabled("pip"));
        // Unchanged default
        assert!(svc.is_enabled("background_blur"));

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_refresh_survives_unreachable() {
        let svc = FeatureService::new();
        svc.set_proxy_url(Some("http://127.0.0.1:1".to_string()));
        // Should not panic
        svc.refresh().await;
        // Defaults still work
        assert!(svc.is_enabled("background_blur"));
    }
}
