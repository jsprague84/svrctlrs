//! Notification system

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::Result;

/// Notification message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationMessage {
    /// Message title
    pub title: String,
    /// Message body
    pub body: String,
    /// Priority (1-5, default 3)
    pub priority: u8,
    /// Optional action buttons
    pub actions: Vec<NotificationAction>,
}

/// Notification action button
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationAction {
    pub label: String,
    pub url: String,
    pub method: Option<String>,
}

/// Notification backend trait
#[async_trait]
pub trait NotificationBackend: Send + Sync {
    /// Send a notification
    async fn send(&self, message: &NotificationMessage) -> Result<()>;

    /// Backend name
    fn name(&self) -> &str;
}
