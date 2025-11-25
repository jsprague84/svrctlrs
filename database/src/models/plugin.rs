use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::FromRow;

/// Plugin model
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Plugin {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub enabled: bool,
    pub config: Option<String>,  // JSON string
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Update plugin input
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdatePlugin {
    pub enabled: Option<bool>,
    pub config: Option<JsonValue>,
}

impl Plugin {
    /// Get config as JSON value
    pub fn get_config(&self) -> JsonValue {
        self.config
            .as_ref()
            .and_then(|c| serde_json::from_str(c).ok())
            .unwrap_or(JsonValue::Object(serde_json::Map::new()))
    }
}

