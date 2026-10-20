//! Approvals (`approvals`): a held destructive call from an API token, waiting for its owner

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::FromRow;

/// A stored approval request.
#[derive(Debug, Clone, Serialize, FromRow)]
pub struct Approval {
    pub id: String,
    pub user_id: i64,
    pub token_id: Option<i64>,
    pub token_name: String,
    pub action: String,
    pub method: String,
    pub path: String,
    pub target_type: String,
    pub target_id: i64,
    pub target_name: Option<String>,
    pub client_ip: Option<String>,
    pub url: String,
    /// `pending`, `approved` or `denied`; see [`Approval::state_at`] for what to show
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub decided_at: Option<DateTime<Utc>>,
    pub decided_by: Option<i64>,
    pub result_status: Option<i64>,
    /// JSON text of the approved call's response body
    pub result: Option<String>,
}

impl Approval {
    /// `pending`, `expired`, `denied`, `approved` (running), `done` or `failed`
    pub fn state_at(&self, now: DateTime<Utc>) -> &'static str {
        match self.status.as_str() {
            "pending" if self.expires_at <= now => "expired",
            "pending" => "pending",
            "denied" => "denied",
            _ => match self.result_status {
                None => "approved",
                Some(s) if (200..300).contains(&s) => "done",
                Some(_) => "failed",
            },
        }
    }
}

/// An approval request to store.
#[derive(Debug, Clone)]
pub struct NewApproval {
    pub id: String,
    pub user_id: i64,
    pub token_id: i64,
    pub token_name: String,
    pub action: String,
    pub method: String,
    pub path: String,
    pub target_type: String,
    pub target_id: i64,
    pub target_name: Option<String>,
    pub client_ip: Option<String>,
    pub url: String,
    pub ttl_minutes: u32,
}
