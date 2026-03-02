use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// SSH host key stored for a server (TOFU - Trust On First Use)
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ServerHostKey {
    pub id: i64,
    pub server_id: i64,
    pub key_type: String,
    pub public_key: String,
    pub first_seen_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
}
