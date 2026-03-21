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
