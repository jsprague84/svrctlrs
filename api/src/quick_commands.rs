//! Quick commands: snippets typed into a terminal (per user).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct QuickCommand {
    pub id: i64,
    pub user_id: Option<i64>,
    pub name: String,
    /// May hold `{{host}}`, `{{user}}`, `{{port}}`, `{{name}}` and `{{word}}` variables
    pub command: String,
    /// Only offered on this server, when set
    pub server_id: Option<i64>,
    pub category: String,
    pub sort_order: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct QuickCommandList {
    pub commands: Vec<QuickCommand>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct QuickCommandOne {
    pub command: QuickCommand,
}

/// `GET /quick-commands` query
#[derive(Debug, Clone, Default, Serialize, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct QuickCommandQuery {
    /// Only the commands for this server (and those for every server)
    pub server_id: Option<i64>,
}

/// `POST /quick-commands`
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct NewQuickCommand {
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub server_id: Option<i64>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub sort_order: Option<i32>,
}

/// `PUT /quick-commands/{id}`: only the fields given change
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
pub struct QuickCommandChanges {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub server_id: Option<i64>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub sort_order: Option<i32>,
}
