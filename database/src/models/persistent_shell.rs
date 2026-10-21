use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::FromRow;

/// A shell running in tmux on a server with `persistent_shell` set (`persistent_shells`):
/// what a restarted SvrCtlRS needs to attach to it again.
#[derive(Debug, Clone, Serialize, FromRow)]
pub struct PersistentShell {
    /// The SvrCtlRS session id
    pub id: String,
    pub user_id: i64,
    pub server_id: i64,
    /// The tmux session's name on the server
    pub tmux_name: String,
    pub created_at: DateTime<Utc>,
}
