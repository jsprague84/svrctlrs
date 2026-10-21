//! Backups of the database (ROADMAP 24).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// `GET /backups`: where copies go, when the next one runs, what happened last, and the
/// copies that exist.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct BackupStatus {
    /// The folder on the server (`BACKUP_DIR`)
    pub dir: String,
    /// Copies kept (`backup.keep`); 0 means nightly backups are off
    pub keep: i64,
    /// Hour of the day, UTC, the nightly copy runs (`backup.hour`)
    pub hour: i64,
    /// When the next nightly copy is due; null when they are off
    pub next_run: Option<DateTime<Utc>>,
    /// Where the encryption key comes from: `environment` or `file`. Backups never hold it.
    pub key_source: String,
    /// A copy is being written right now
    pub running: bool,
    /// The latest attempt, whatever its outcome
    pub last_run: Option<BackupRun>,
    /// The latest attempt that wrote a copy
    pub last_success: Option<BackupRun>,
    /// The copies in the folder, newest first
    pub files: Vec<BackupFile>,
}

/// One attempt at a copy.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct BackupRun {
    pub id: i64,
    /// `nightly`, `manual` or `svrctl`
    pub trigger: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
    pub file_name: Option<String>,
    pub size_bytes: Option<i64>,
    /// Older copies retention removed after this one
    pub removed: i64,
    /// Why it failed; null when it wrote a copy
    pub error: Option<String>,
}

/// A copy in the backup folder.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct BackupFile {
    pub name: String,
    pub size_bytes: u64,
    pub created_at: DateTime<Utc>,
}

/// `POST /backups`: the copy just written.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct BackupCreated {
    pub file: BackupFile,
    /// Older copies removed to stay within `keep`
    pub removed: usize,
}
