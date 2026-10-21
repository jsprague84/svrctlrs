use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::FromRow;

/// One backup attempt (`backup_runs`): a copy written, or the error that stopped it.
#[derive(Debug, Clone, Serialize, FromRow)]
pub struct BackupRun {
    pub id: i64,
    /// `nightly`, `manual` or `svrctl`
    pub trigger: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
    pub file_name: Option<String>,
    pub size_bytes: Option<i64>,
    pub removed: i64,
    pub error: Option<String>,
}

/// What a finished attempt records.
#[derive(Debug, Clone)]
pub struct NewBackupRun<'a> {
    pub trigger: &'a str,
    pub started_at: DateTime<Utc>,
    pub file_name: Option<&'a str>,
    pub size_bytes: Option<i64>,
    pub removed: i64,
    pub error: Option<&'a str>,
}
