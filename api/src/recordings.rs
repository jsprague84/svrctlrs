//! The caller's session recordings.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

/// One recorded shell (asciicast v2, encrypted at rest; `GET /recordings/{id}/cast` streams it).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Recording {
    pub id: i64,
    pub user_id: i64,
    /// Null once the server is deleted; `server_name` keeps what it was called
    pub server_id: Option<i64>,
    pub server_name: String,
    /// The live session this recorded
    pub session_id: String,
    /// Terminal size when the recording started; resizes are events in the file
    pub cols: i64,
    pub rows: i64,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub duration_ms: i64,
    /// Plaintext asciicast bytes written so far (the size of a download)
    pub size_bytes: i64,
    /// `recording`, `complete`, `truncated`, `interrupted` or `failed`
    pub status: String,
    /// Why it ended: the shell's exit, the size limit, the error
    pub end_reason: Option<String>,
}

/// `GET /recordings`
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RecordingList {
    pub recordings: Vec<Recording>,
    /// Recordings older than this are deleted; 0 keeps them
    pub retention_days: i64,
    /// A recording stops at this size and the shell goes on unrecorded
    pub max_bytes: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct RecordingQuery {
    pub server_id: Option<i64>,
    /// Default 100
    pub limit: Option<i64>,
    /// The smallest id of the previous page
    pub before: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RecordingOne {
    pub recording: Recording,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct CastQuery {
    /// Serve as a file download (and audit it) instead of for the player
    #[serde(default)]
    pub download: bool,
}
