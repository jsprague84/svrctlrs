//! Approval requests: a token's held delete, waiting for its owner.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Approval {
    /// Random; it is in the link
    pub id: String,
    /// `pending`, `expired`, `denied`, `approved` (running), `done` or `failed`
    pub status: String,
    /// `server.delete`, `credential.delete` or `server.host_keys_forget`
    pub action: String,
    /// "Delete server web-01"
    pub summary: String,
    pub method: String,
    pub path: String,
    pub target_type: String,
    pub target_id: i64,
    pub target_name: Option<String>,
    pub token_id: Option<i64>,
    pub token_name: String,
    pub client_ip: Option<String>,
    /// Where the owner approves or denies it, signed in
    pub url: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub decided_at: Option<DateTime<Utc>>,
    /// What the approved call answered
    pub result_status: Option<i64>,
    pub result: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ApprovalList {
    pub approvals: Vec<Approval>,
    /// How many are waiting now
    pub waiting: i64,
}

/// The 202 a token set to ask first gets for a gated delete.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct HeldForApproval {
    pub approval: Approval,
    pub message: String,
    /// `GET` it to wait for the answer
    pub wait_url: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct WaitQuery {
    /// Seconds to wait for a decision before answering (at most 25)
    pub wait: Option<u64>,
}
