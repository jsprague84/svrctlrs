//! Live shells of the calling user.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// `GET /sessions`
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SessionList {
    pub sessions: Vec<SessionSummary>,
    /// How long a shell with nobody attached is kept
    pub detach_ttl_secs: u64,
}

/// A shell the server holds for the caller, attached or not.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SessionSummary {
    pub id: String,
    pub server_id: i64,
    pub server_name: String,
    pub created_at: DateTime<Utc>,
    /// Sockets showing it now
    pub attachments: usize,
    /// The newest output sequence number
    pub latest_seq: u64,
    /// Its output is being recorded
    pub recorded: bool,
}

/// `DELETE /sessions/{id}`
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SessionClosed {
    pub id: String,
    pub message: String,
}
