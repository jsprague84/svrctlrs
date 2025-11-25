use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::FromRow;

/// Notification backend model
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct NotificationBackend {
    pub id: i64,
    #[sqlx(rename = "type")]
    pub backend_type: String,  // 'gotify', 'ntfy'
    pub name: String,
    pub enabled: bool,
    pub config: String,  // JSON string
    pub priority: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Create notification backend input
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateNotificationBackend {
    #[serde(rename = "type")]
    pub backend_type: String,
    pub name: String,
    pub config: JsonValue,
    #[serde(default = "default_priority")]
    pub priority: i32,
}

/// Update notification backend input
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateNotificationBackend {
    pub name: Option<String>,
    pub enabled: Option<bool>,
    pub config: Option<JsonValue>,
    pub priority: Option<i32>,
}

fn default_priority() -> i32 {
    5
}

impl NotificationBackend {
    /// Get config as JSON value
    pub fn get_config(&self) -> JsonValue {
        serde_json::from_str(&self.config)
            .unwrap_or(JsonValue::Object(serde_json::Map::new()))
    }
}

