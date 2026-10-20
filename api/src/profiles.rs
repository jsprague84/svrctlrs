//! Terminal profiles: saved layouts with a server per pane (per user).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Profile {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    /// `single`, `split-h`, `split-v` or `quad`
    pub layout: String,
    /// A JSON array of `{server_id}` objects, one per pane, as a string
    pub pane_configs: Option<String>,
    pub is_default: bool,
    pub user_id: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ProfileList {
    pub profiles: Vec<Profile>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ProfileOne {
    pub profile: Profile,
}

/// `POST /profiles`
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct NewProfile {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    /// Default `single`
    #[serde(default = "single")]
    pub layout: String,
    /// JSON array of `{server_id}` objects, one per pane
    #[serde(default)]
    pub pane_configs: Option<serde_json::Value>,
    #[serde(default)]
    pub is_default: bool,
}

fn single() -> String {
    "single".to_string()
}

/// `PUT /profiles/{id}`: only the fields given change
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
pub struct ProfileChanges {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub layout: Option<String>,
    #[serde(default)]
    pub pane_configs: Option<serde_json::Value>,
    #[serde(default)]
    pub is_default: Option<bool>,
}
