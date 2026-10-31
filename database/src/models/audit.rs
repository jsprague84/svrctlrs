//! Audit log entries

use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value as JsonValue;
use sqlx::FromRow;

/// A stored audit entry. `username` is the account's current name when it still exists,
/// else the name recorded at the time (login failures carry only a name).
#[derive(Debug, Clone, Serialize, FromRow)]
pub struct AuditEntry {
    pub id: i64,
    pub at: DateTime<Utc>,
    pub user_id: Option<i64>,
    pub username: Option<String>,
    pub client_ip: Option<String>,
    pub action: String,
    pub target_type: Option<String>,
    pub target_id: Option<i64>,
    pub target_name: Option<String>,
    /// JSON object string
    pub details: Option<String>,
    /// The API token the request came in with, if any (its name as it was then)
    pub token_id: Option<i64>,
    pub token_name: Option<String>,
}

/// An entry to record. Built with the chained setters; nothing here may be a secret.
#[derive(Debug, Clone, Default)]
pub struct NewAuditEntry {
    pub user_id: Option<i64>,
    pub username: Option<String>,
    pub client_ip: Option<String>,
    pub action: String,
    pub target_type: Option<String>,
    pub target_id: Option<i64>,
    pub target_name: Option<String>,
    pub details: Option<JsonValue>,
    pub token_id: Option<i64>,
    pub token_name: Option<String>,
}

impl NewAuditEntry {
    pub fn new(action: &str) -> Self {
        Self {
            action: action.to_string(),
            ..Self::default()
        }
    }

    pub fn user(mut self, user_id: i64) -> Self {
        self.user_id = Some(user_id);
        self
    }

    pub fn username(mut self, username: &str) -> Self {
        self.username = Some(username.to_string());
        self
    }

    pub fn client_ip(mut self, ip: Option<String>) -> Self {
        self.client_ip = ip;
        self
    }

    pub fn target(mut self, target_type: &str, id: i64, name: &str) -> Self {
        self.target_type = Some(target_type.to_string());
        self.target_id = Some(id);
        self.target_name = Some(name.to_string());
        self
    }

    /// A target known by name only (a setting, by its key)
    pub fn target_named(mut self, target_type: &str, name: &str) -> Self {
        self.target_type = Some(target_type.to_string());
        self.target_name = Some(name.to_string());
        self
    }

    pub fn details(mut self, details: JsonValue) -> Self {
        self.details = Some(details);
        self
    }

    /// The API token behind the request
    pub fn token(mut self, id: i64, name: &str) -> Self {
        self.token_id = Some(id);
        self.token_name = Some(name.to_string());
        self
    }
}
