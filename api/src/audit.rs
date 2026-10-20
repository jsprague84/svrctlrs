//! The audit log.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

/// `GET /audit` query
#[derive(Debug, Clone, Default, Serialize, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct AuditQuery {
    /// 1–500, default 100
    pub limit: Option<i64>,
    /// The `next_before` of the previous page
    pub before: Option<i64>,
    /// Comma-separated action prefixes, e.g. `login,session.`
    pub action: Option<String>,
    pub user_id: Option<i64>,
    /// Case-insensitive text in the action, target, details, user, address or token name
    pub q: Option<String>,
}

/// One page of the log, newest first.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AuditPage {
    pub entries: Vec<AuditEntry>,
    /// Pass as `before` for the next page; null on the last one
    pub next_before: Option<i64>,
}

/// One thing someone did. Entries never hold secrets and outlive deleted users.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AuditEntry {
    pub id: i64,
    pub at: DateTime<Utc>,
    pub user_id: Option<i64>,
    /// The account's current name while it exists, else the name at the time
    pub username: Option<String>,
    pub client_ip: Option<String>,
    /// `login`, `server.created`, `session.pty_opened`, `token.created`, …
    pub action: String,
    pub target_type: Option<String>,
    pub target_id: Option<i64>,
    pub target_name: Option<String>,
    /// A JSON object, as a string
    pub details: Option<String>,
    /// The API token the request came in with
    pub token_id: Option<i64>,
    /// That token's name at the time
    pub token_name: Option<String>,
}
