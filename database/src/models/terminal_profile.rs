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
    pub pane_configs: Option<String>,   // JSON string
    pub quick_commands: Option<String>, // JSON string
    pub is_default: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl TerminalProfile {
    /// Parse pane_configs JSON into structured data
    pub fn get_pane_configs(&self) -> Vec<PaneConfig> {
        self.pane_configs
            .as_ref()
            .and_then(|json| serde_json::from_str(json).ok())
            .unwrap_or_default()
    }

    /// Parse quick_commands JSON into a list
    pub fn get_quick_commands(&self) -> Vec<String> {
        self.quick_commands
            .as_ref()
            .and_then(|json| serde_json::from_str(json).ok())
            .unwrap_or_default()
    }
}

/// Pane configuration for a terminal profile
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaneConfig {
    pub server_id: Option<i64>,
}

/// Create a new terminal profile
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTerminalProfile {
    pub name: String,
    pub description: Option<String>,
    pub layout: String,
    pub pane_configs: Option<JsonValue>,
    pub quick_commands: Option<JsonValue>,
    pub is_default: bool,
}

impl Default for CreateTerminalProfile {
    fn default() -> Self {
        Self {
            name: String::new(),
            description: None,
            layout: "2h".to_string(),
            pane_configs: None,
            quick_commands: None,
            is_default: false,
        }
    }
}

/// Update an existing terminal profile
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateTerminalProfile {
    pub name: Option<String>,
    pub description: Option<String>,
    pub layout: Option<String>,
    pub pane_configs: Option<JsonValue>,
    pub quick_commands: Option<JsonValue>,
    pub is_default: Option<bool>,
}
