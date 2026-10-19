//! Session recordings: one row per recorded shell, owned by the user whose shell it was.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::FromRow;

/// Where a recording stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordingStatus {
    /// The shell is still running and being written
    Recording,
    /// The shell ended and everything it printed was written
    Complete,
    /// Writing stopped at the size limit; the shell went on unrecorded
    Truncated,
    /// The server stopped while the shell was running
    Interrupted,
    /// Writing failed (disk full, permissions); what was written before is kept
    Failed,
}

impl RecordingStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Recording => "recording",
            Self::Complete => "complete",
            Self::Truncated => "truncated",
            Self::Interrupted => "interrupted",
            Self::Failed => "failed",
        }
    }
}

/// A stored recording. `file_name` is internal (relative to the user's recordings
/// directory) and never serialized.
#[derive(Debug, Clone, Serialize, FromRow)]
pub struct Recording {
    pub id: i64,
    pub user_id: i64,
    /// `None` once the server is deleted; `server_name` keeps what it was called
    pub server_id: Option<i64>,
    pub server_name: String,
    /// The live session this recorded
    pub session_id: String,
    #[serde(skip)]
    pub file_name: String,
    /// Terminal size when the recording started; resizes are events in the file
    pub cols: i64,
    pub rows: i64,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub duration_ms: i64,
    /// Plaintext asciicast bytes written so far (the size of a download)
    pub size_bytes: i64,
    /// One of [`RecordingStatus::as_str`]
    pub status: String,
    /// Why it ended: the shell's exit, the size limit, the error
    pub end_reason: Option<String>,
}

/// A recording about to start.
#[derive(Debug, Clone)]
pub struct NewRecording {
    pub user_id: i64,
    pub server_id: i64,
    pub server_name: String,
    pub session_id: String,
    pub file_name: String,
    pub cols: i64,
    pub rows: i64,
}
