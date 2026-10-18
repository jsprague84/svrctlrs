use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// An SSH host key seen for a server. Trusted once the user has confirmed its fingerprint
/// (`trusted_at`); until then it is the key the server presented on first contact, recorded so
/// the fingerprint can be shown and confirmed.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ServerHostKey {
    pub id: i64,
    pub server_id: i64,
    pub key_type: String,
    pub public_key: String,
    pub first_seen_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
    pub trusted_at: Option<DateTime<Utc>>,
}

impl ServerHostKey {
    pub fn is_trusted(&self) -> bool {
        self.trusted_at.is_some()
    }
}
