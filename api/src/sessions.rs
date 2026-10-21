//! Live shells of the calling user.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// `GET /sessions`
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SessionList {
    pub sessions: Vec<SessionSummary>,
    /// How long the caller's shells are kept with nobody attached
    pub detach_ttl_secs: u64,
    /// The longest this server allows (`SESSION_DETACH_TTL_MAX_SECS`)
    pub detach_ttl_max_secs: u64,
    /// The lengths the caller can choose from (`PUT /sessions/keep`), within the maximum
    pub detach_ttl_choices: Vec<u64>,
    /// The caller chose `detach_ttl_secs`; false means the server's default
    pub detach_ttl_chosen: bool,
}

/// `PUT /sessions/keep`: how long the caller's shells are kept with nobody attached.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SessionKeepUpdate {
    /// One of `detach_ttl_choices`, or null for the server's default
    pub detach_ttl_secs: Option<u64>,
}

/// The caller's keep time after a change.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SessionKeep {
    pub detach_ttl_secs: u64,
    pub detach_ttl_chosen: bool,
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
    /// It runs in tmux on the server and outlives a restart of SvrCtlRS
    pub persistent: bool,
    /// Kept in tmux but not connected since SvrCtlRS restarted; attaching reconnects it
    pub waiting: bool,
}

/// `DELETE /sessions/{id}`
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SessionClosed {
    pub id: String,
    pub message: String,
}
