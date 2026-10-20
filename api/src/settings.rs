//! Application settings (global, not per user).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use utoipa::ToSchema;

/// One setting. Secret ones (`notification.token`, `approval.matrix_token`,
/// `approval.webhook_url`) read back with an empty `value` and `is_set`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Setting {
    pub key: String,
    pub value: String,
    /// Write-only through the API
    pub secret: bool,
    /// Whether a value is stored (the only thing a secret setting tells)
    pub is_set: bool,
    /// `string`, `integer`, `boolean` or `json`
    pub value_type: String,
    pub description: Option<String>,
    pub updated_at: DateTime<Utc>,
}

/// `GET /settings`: settings grouped by the part of the key before the first dot
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SettingsList {
    pub settings: BTreeMap<String, Vec<Setting>>,
}

/// `PUT /settings`: several at once; a secret key with an empty value is left as it is
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SettingsUpdate {
    pub values: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SettingsUpdated {
    /// The keys written (secrets sent empty are not)
    pub updated: Vec<String>,
    pub message: String,
}

/// `POST /settings`
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct NewSetting {
    pub key: String,
    pub value: String,
    /// `string` (default), `integer`, `boolean` or `json`
    #[serde(default = "string_type")]
    pub value_type: String,
    pub description: Option<String>,
}

fn string_type() -> String {
    "string".to_string()
}

/// `PUT /settings/{key}`
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SettingValue {
    pub value: String,
}

/// What a create or update of one setting answers
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SettingSaved {
    pub key: String,
    pub message: String,
}
