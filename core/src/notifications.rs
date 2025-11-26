//! Notification system with Gotify and ntfy.sh backends

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use tracing::{debug, warn};

use crate::{Error, Result};

/// Notification message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationMessage {
    /// Message title
    pub title: String,
    /// Message body
    pub body: String,
    /// Priority (1-5, default 3)
    #[serde(default = "default_priority")]
    pub priority: u8,
    /// Optional action buttons
    #[serde(default)]
    pub actions: Vec<NotificationAction>,
}

fn default_priority() -> u8 {
    3
}

/// Notification action button
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationAction {
    pub label: String,
    pub url: String,
    pub method: Option<String>,
}

impl NotificationAction {
    /// Create a view URL action button
    pub fn view(label: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            url: url.into(),
            method: None,
        }
    }

    /// Create an HTTP POST action button
    pub fn http_post(label: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            url: url.into(),
            method: Some("POST".to_string()),
        }
    }
}

/// Notification backend trait
#[async_trait]
pub trait NotificationBackend: Send + Sync {
    /// Send a notification
    async fn send(&self, message: &NotificationMessage) -> Result<()>;

    /// Backend name
    fn name(&self) -> &str;
}

// ============================================================================
// Gotify Backend
// ============================================================================

/// Gotify notification backend
#[derive(Debug, Clone)]
pub struct GotifyBackend {
    client: Client,
    base_url: String,
    /// Service-specific keys: service_name -> api_key
    keys: HashMap<String, String>,
    /// Global fallback key
    fallback_key: Option<String>,
    debug: bool,
}

impl GotifyBackend {
    /// Create a new Gotify backend from environment variables
    pub fn new(client: Client) -> Result<Self> {
        let base_url =
            env::var("GOTIFY_URL").unwrap_or_else(|_| "http://localhost:8080/message".to_string());

        let debug = env::var("GOTIFY_DEBUG")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

        // Load fallback key
        let fallback_key = Self::load_key_from_env("GOTIFY_KEY");

        Ok(Self {
            client,
            base_url,
            keys: HashMap::new(),
            fallback_key,
            debug,
        })
    }

    /// Create a new Gotify backend with explicit URL and key
    pub fn with_url_and_key(client: Client, url: impl Into<String>, key: impl Into<String>) -> Result<Self> {
        Ok(Self {
            client,
            base_url: url.into(),
            keys: HashMap::new(),
            fallback_key: Some(key.into()),
            debug: false,
        })
    }

    /// Register a service-specific key
    pub fn register_service(&mut self, service: impl Into<String>, key: impl Into<String>) {
        self.keys.insert(service.into(), key.into());
    }

    /// Load service-specific keys from environment
    pub fn load_service_keys(&mut self, services: &[&str]) {
        for service in services {
            let var_name = format!("{}_GOTIFY_KEY", service.to_uppercase());
            if let Some(key) = Self::load_key_from_env(&var_name) {
                self.register_service(*service, key);
            }
        }
    }

    /// Load key from environment variable or file
    fn load_key_from_env(var_name: &str) -> Option<String> {
        // Try direct env var first
        if let Ok(key) = env::var(var_name) {
            let key = key.trim();
            if !key.is_empty() {
                return Some(key.to_string());
            }
        }

        // Try file-based key (GOTIFY_KEY_FILE)
        if var_name == "GOTIFY_KEY" {
            if let Ok(path) = env::var("GOTIFY_KEY_FILE") {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    let key = content.trim();
                    if !key.is_empty() {
                        return Some(key.to_string());
                    }
                }
            }
        }

        None
    }

    /// Get key for a service (with fallback)
    fn get_key(&self, service: &str) -> Option<&str> {
        self.keys
            .get(service)
            .map(|s| s.as_str())
            .or(self.fallback_key.as_deref())
    }

    /// Mask token for debug output
    fn mask_token(token: &str) -> String {
        const MIN_LEN: usize = 8;
        const PREFIX_LEN: usize = 3;
        const SUFFIX_LEN: usize = 3;

        if token.len() > MIN_LEN {
            format!(
                "{}***{}",
                &token[..PREFIX_LEN],
                &token[token.len() - SUFFIX_LEN..]
            )
        } else {
            "***".to_string()
        }
    }

    /// Send notification for a specific service
    pub async fn send_for_service(
        &self,
        service: &str,
        message: &NotificationMessage,
    ) -> Result<()> {
        let key = match self.get_key(service) {
            Some(k) => k,
            None => {
                debug!(service = %service, "Gotify key not configured; skipping notification");
                return Ok(());
            }
        };

        if self.debug {
            debug!(
                service = %service,
                url = %self.base_url,
                key = %Self::mask_token(key),
                title_bytes = message.title.len(),
                body_bytes = message.body.len(),
                "Sending Gotify notification"
            );
        }

        // Gotify requires /message endpoint
        let url = format!("{}/message", self.base_url.trim_end_matches('/'));
        
        let response = self
            .client
            .post(&url)
            .header("X-Gotify-Key", key)
            .json(&serde_json::json!({
                "title": message.title,
                "message": message.body,
                "priority": message.priority
            }))
            .send()
            .await
            .map_err(|e| Error::HttpError(format!("Gotify request failed: {}", e)))?;

        response
            .error_for_status()
            .map_err(|e| Error::NotificationError(format!("Gotify error: {}", e)))?;

        Ok(())
    }
}

#[async_trait]
impl NotificationBackend for GotifyBackend {
    async fn send(&self, message: &NotificationMessage) -> Result<()> {
        // Use fallback key if available
        self.send_for_service("default", message).await
    }

    fn name(&self) -> &str {
        "gotify"
    }
}

// ============================================================================
// ntfy Backend
// ============================================================================

/// ntfy.sh notification backend
#[derive(Debug, Clone)]
pub struct NtfyBackend {
    client: Client,
    base_url: String,
    /// Service-specific topics: service_name -> topic
    topics: HashMap<String, String>,
    /// Optional auth token
    auth_token: Option<String>,
    debug: bool,
}

impl NtfyBackend {
    /// Create a new ntfy backend from environment variables
    pub fn new(client: Client) -> Result<Self> {
        let base_url = env::var("NTFY_URL").unwrap_or_else(|_| "https://ntfy.sh".to_string());

        let auth_token = env::var("NTFY_AUTH").ok().filter(|s| !s.trim().is_empty());

        let debug = env::var("NTFY_DEBUG")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

        Ok(Self {
            client,
            base_url,
            topics: HashMap::new(),
            auth_token,
            debug,
        })
    }

    /// Create a new ntfy backend with explicit URL and default topic
    pub fn with_url_and_topic(client: Client, url: impl Into<String>, topic: impl Into<String>) -> Result<Self> {
        let mut topics = HashMap::new();
        topics.insert("default".to_string(), topic.into());
        
        Ok(Self {
            client,
            base_url: url.into(),
            topics,
            auth_token: None,
            debug: false,
        })
    }

    /// Register a service-specific topic
    pub fn register_service(&mut self, service: impl Into<String>, topic: impl Into<String>) {
        self.topics.insert(service.into(), topic.into());
    }

    /// Load service-specific topics from environment
    pub fn load_service_topics(&mut self, services: &[&str]) {
        for service in services {
            let var_name = format!("{}_NTFY_TOPIC", service.to_uppercase());
            if let Ok(topic) = env::var(&var_name) {
                let topic = topic.trim();
                if !topic.is_empty() {
                    self.register_service(*service, topic);
                }
            }
        }
    }

    /// Get topic for a service
    fn get_topic(&self, service: &str) -> Option<&str> {
        self.topics.get(service).map(|s| s.as_str())
    }

    /// Send notification for a specific service
    pub async fn send_for_service(
        &self,
        service: &str,
        message: &NotificationMessage,
    ) -> Result<()> {
        let topic = match self.get_topic(service) {
            Some(t) => t,
            None => {
                warn!(
                    service = %service,
                    available_topics = ?self.topics.keys().collect::<Vec<_>>(),
                    "ntfy topic not configured for service; skipping notification"
                );
                return Ok(());
            }
        };

        tracing::info!(
            service = %service,
            topic = %topic,
            url = %format!("{}/{}", self.base_url, topic),
            title = %message.title,
            priority = message.priority,
            actions = message.actions.len(),
            "Sending ntfy notification"
        );

        // Build JSON body with markdown support
        let mut json_body = serde_json::json!({
            "topic": topic,
            "title": message.title,
            "message": message.body,
            "priority": message.priority,
            "markdown": true,
        });

        // Add actions if provided
        if !message.actions.is_empty() {
            json_body["actions"] =
                serde_json::to_value(&message.actions).map_err(Error::SerializationError)?;
        }

        if self.debug {
            debug!(
                payload = %serde_json::to_string_pretty(&json_body)
                    .unwrap_or_else(|_| "error serializing".to_string()),
                "ntfy JSON payload"
            );
        }

        // Post to base URL (topic is in JSON body)
        let url = self.base_url.trim_end_matches('/');
        let mut request = self.client.post(url).json(&json_body);

        // Add auth if configured
        if let Some(token) = &self.auth_token {
            request = request.header("Authorization", format!("Bearer {}", token));
        }

        let response = request
            .send()
            .await
            .map_err(|e| {
                tracing::error!(
                    service = %service,
                    topic = %topic,
                    url = %url,
                    error = %e,
                    "ntfy HTTP request failed"
                );
                Error::HttpError(format!("ntfy request failed: {}", e))
            })?;

        let status = response.status();
        if !status.is_success() {
            let error_body = response.text().await.unwrap_or_else(|_| "unknown".to_string());
            tracing::error!(
                service = %service,
                topic = %topic,
                status = %status,
                error_body = %error_body,
                "ntfy returned error status"
            );
            return Err(Error::NotificationError(format!(
                "ntfy error: {} - {}",
                status, error_body
            )));
        }

        tracing::info!(
            service = %service,
            topic = %topic,
            "ntfy notification sent successfully"
        );

        Ok(())
    }
}

#[async_trait]
impl NotificationBackend for NtfyBackend {
    async fn send(&self, _message: &NotificationMessage) -> Result<()> {
        // ntfy requires a topic, so we can't send without one
        warn!("ntfy backend called without service context; skipping");
        Ok(())
    }

    fn name(&self) -> &str {
        "ntfy"
    }
}

// ============================================================================
// Notification Manager
// ============================================================================

/// Manages multiple notification backends
#[derive(Debug, Clone)]
pub struct NotificationManager {
    gotify: Option<GotifyBackend>,
    ntfy: Option<NtfyBackend>,
}

impl NotificationManager {
    /// Create a new notification manager with auto-detected backends from environment
    pub fn new(client: Client, services: &[&str]) -> Result<Self> {
        let mut gotify = None;
        let mut ntfy = None;

        // Try to initialize Gotify
        if let Ok(mut backend) = GotifyBackend::new(client.clone()) {
            backend.load_service_keys(services);
            gotify = Some(backend);
        }

        // Try to initialize ntfy
        if let Ok(mut backend) = NtfyBackend::new(client.clone()) {
            backend.load_service_topics(services);
            ntfy = Some(backend);
        }

        Ok(Self { gotify, ntfy })
    }

    /// Create a new notification manager from pre-configured backends
    pub fn from_backends(gotify: Option<GotifyBackend>, ntfy: Option<NtfyBackend>) -> Self {
        Self { gotify, ntfy }
    }

    /// Send notification via all configured backends for a service
    pub async fn send_for_service(
        &self,
        service: &str,
        message: &NotificationMessage,
    ) -> Result<()> {
        let mut errors = Vec::new();

        // Send to Gotify
        if let Some(backend) = &self.gotify {
            if let Err(e) = backend.send_for_service(service, message).await {
                warn!(service = %service, backend = "gotify", error = %e, "Notification failed");
                errors.push(format!("Gotify: {}", e));
            }
        }

        // Send to ntfy
        if let Some(backend) = &self.ntfy {
            if let Err(e) = backend.send_for_service(service, message).await {
                warn!(service = %service, backend = "ntfy", error = %e, "Notification failed");
                errors.push(format!("ntfy: {}", e));
            }
        }

        if !errors.is_empty() {
            return Err(Error::NotificationError(errors.join("; ")));
        }

        Ok(())
    }

    /// Get Gotify backend reference
    pub fn gotify(&self) -> Option<&GotifyBackend> {
        self.gotify.as_ref()
    }

    /// Get ntfy backend reference
    pub fn ntfy(&self) -> Option<&NtfyBackend> {
        self.ntfy.as_ref()
    }
}
