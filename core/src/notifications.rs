//! Notification client for rstify/Gotify/ntfy-compatible services

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

/// Notification provider type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum NotificationProvider {
    Gotify,
    Ntfy,
}

impl NotificationProvider {
    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "ntfy" => Self::Ntfy,
            _ => Self::Gotify,
        }
    }
}

/// Notification configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationConfig {
    pub enabled: bool,
    pub provider: NotificationProvider,
    pub url: String,
    pub token: String,
    pub topic: String,
}

impl Default for NotificationConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: NotificationProvider::Gotify,
            url: String::new(),
            token: String::new(),
            topic: "svrctlrs".to_string(),
        }
    }
}

/// Rstify/Gotify/ntfy notification client
#[derive(Debug, Clone)]
pub struct NotificationClient {
    config: NotificationConfig,
    http: reqwest::Client,
}

impl NotificationClient {
    /// Create a new notification client from config
    pub fn new(config: NotificationConfig) -> Self {
        Self {
            config,
            http: reqwest::Client::new(),
        }
    }

    /// Check if notifications are enabled and configured
    pub fn is_enabled(&self) -> bool {
        self.config.enabled && !self.config.url.is_empty()
    }

    /// Send a notification
    pub async fn send(&self, title: &str, message: &str, priority: u8) -> Result<()> {
        if !self.is_enabled() {
            debug!("Notifications disabled, skipping");
            return Ok(());
        }

        match self.config.provider {
            NotificationProvider::Gotify => self.send_gotify(title, message, priority).await,
            NotificationProvider::Ntfy => self.send_ntfy(title, message, priority).await,
        }
    }

    /// Send via Gotify-compatible API (rstify default)
    async fn send_gotify(&self, title: &str, message: &str, priority: u8) -> Result<()> {
        let url = format!("{}/message", self.config.url.trim_end_matches('/'));

        info!(title, provider = "gotify", "Sending notification");

        self.http
            .post(&url)
            .header("X-Gotify-Key", &self.config.token)
            .json(&serde_json::json!({
                "title": title,
                "message": message,
                "priority": priority
            }))
            .send()
            .await
            .context("Failed to send Gotify notification")?
            .error_for_status()
            .context("Gotify API returned error")?;

        debug!("Notification sent successfully");
        Ok(())
    }

    /// Send via ntfy-compatible API
    async fn send_ntfy(&self, title: &str, message: &str, priority: u8) -> Result<()> {
        let url = format!(
            "{}/{}",
            self.config.url.trim_end_matches('/'),
            self.config.topic
        );

        let ntfy_priority = match priority {
            1..=3 => "low",
            4..=6 => "default",
            7..=8 => "high",
            _ => "urgent",
        };

        info!(title, provider = "ntfy", "Sending notification");

        self.http
            .post(&url)
            .header("Title", title)
            .header("Priority", ntfy_priority)
            .header("Authorization", format!("Bearer {}", self.config.token))
            .body(message.to_string())
            .send()
            .await
            .context("Failed to send ntfy notification")?
            .error_for_status()
            .context("ntfy API returned error")?;

        debug!("Notification sent successfully");
        Ok(())
    }

    /// Send a test notification to verify configuration
    pub async fn send_test(&self) -> Result<()> {
        self.send(
            "SvrCtlRS Test",
            "Notification configuration is working correctly.",
            5,
        )
        .await
    }
}

/// How long a webhook or Matrix post may take before it counts as failed.
const SEND_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

fn http_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(SEND_TIMEOUT)
        .build()
        .context("Failed to build HTTP client")
}

/// POST `payload` as JSON to a webhook (an n8n workflow that forwards it, say).
pub async fn post_webhook(url: &str, payload: &serde_json::Value) -> Result<()> {
    info!(provider = "webhook", "Sending notification");
    http_client()?
        .post(url)
        .json(payload)
        .send()
        .await
        .context("Failed to call the webhook")?
        .error_for_status()
        .context("The webhook returned an error")?;
    Ok(())
}

/// A Matrix room, posted to with an account's access token (client-server API).
#[derive(Debug, Clone)]
pub struct MatrixRoom {
    /// `https://matrix.example.org`
    pub homeserver: String,
    /// `!abc123:example.org`
    pub room_id: String,
    pub access_token: String,
}

impl MatrixRoom {
    /// The `PUT …/rooms/{room}/send/m.room.message/{txn}` URL, segments percent-encoded.
    fn send_url(&self, txn_id: &str) -> Result<reqwest::Url> {
        let mut url = reqwest::Url::parse(self.homeserver.trim_end_matches('/'))
            .context("The Matrix homeserver is not a URL")?;
        url.path_segments_mut()
            .map_err(|_| anyhow::anyhow!("The Matrix homeserver URL cannot take a path"))?
            .pop_if_empty()
            .extend([
                "_matrix",
                "client",
                "v3",
                "rooms",
                &self.room_id,
                "send",
                "m.room.message",
                txn_id,
            ]);
        Ok(url)
    }

    /// Post a message: `text` for clients without formatting, `html` for the rest. `txn_id`
    /// makes a retry of the same message idempotent.
    pub async fn send(&self, txn_id: &str, text: &str, html: &str) -> Result<()> {
        info!(provider = "matrix", "Sending notification");
        http_client()?
            .put(self.send_url(txn_id)?)
            .bearer_auth(&self.access_token)
            .json(&serde_json::json!({
                "msgtype": "m.text",
                "body": text,
                "format": "org.matrix.custom.html",
                "formatted_body": html,
            }))
            .send()
            .await
            .context("Failed to post to Matrix")?
            .error_for_status()
            .context("Matrix returned an error")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matrix_send_url_encodes_the_room_and_keeps_a_base_path() {
        let room = MatrixRoom {
            homeserver: "https://matrix.example.org/".into(),
            room_id: "!abc123:example.org".into(),
            access_token: "t".into(),
        };
        assert_eq!(
            room.send_url("svr-1").unwrap().as_str(),
            "https://matrix.example.org/_matrix/client/v3/rooms/!abc123:example.org/send/m.room.message/svr-1"
        );
        let behind_a_path = MatrixRoom {
            homeserver: "https://example.org/matrix".into(),
            ..room
        };
        assert!(behind_a_path
            .send_url("x")
            .unwrap()
            .as_str()
            .starts_with("https://example.org/matrix/_matrix/client/v3/rooms/"));
    }
}
