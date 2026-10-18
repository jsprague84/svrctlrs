//! Terminal Profile model

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::FromRow;

/// Terminal profile database model
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct TerminalProfile {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub layout: String,
    /// JSON array of `{server_id}` objects, one per pane; opaque to the server
    pub pane_configs: Option<String>,
    pub is_default: bool,
    pub user_id: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Create a new terminal profile
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTerminalProfile {
    pub name: String,
    pub description: Option<String>,
    pub layout: String,
    pub pane_configs: Option<JsonValue>,
    pub is_default: bool,
}

impl Default for CreateTerminalProfile {
    fn default() -> Self {
        Self {
            name: String::new(),
            description: None,
            layout: "single".to_string(),
            pane_configs: None,
            is_default: false,
        }
    }
}

/// Update an existing terminal profile
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateTerminalProfile {
    pub name: Option<String>,
    pub description: Option<String>,
    pub layout: Option<String>,
    pub pane_configs: Option<JsonValue>,
    pub is_default: Option<bool>,
}
