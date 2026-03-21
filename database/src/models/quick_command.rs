//! Quick Command model

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// Quick command database model — reusable command snippets
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct QuickCommand {
    pub id: i64,
    pub user_id: Option<i64>,
    pub name: String,
    pub command: String,
    pub server_id: Option<i64>,
    pub category: String,
    pub sort_order: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Input for creating a new quick command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateQuickCommand {
    pub name: String,
    pub command: String,
    pub server_id: Option<i64>,
    pub category: Option<String>,
    pub sort_order: Option<i32>,
}

/// Input for updating an existing quick command
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateQuickCommand {
    pub name: Option<String>,
    pub command: Option<String>,
    pub server_id: Option<i64>,
    pub category: Option<String>,
    pub sort_order: Option<i32>,
}

impl UpdateQuickCommand {
    pub fn has_changes(&self) -> bool {
        self.name.is_some()
            || self.command.is_some()
            || self.server_id.is_some()
            || self.category.is_some()
            || self.sort_order.is_some()
    }
}
