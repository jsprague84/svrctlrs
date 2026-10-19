use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::FromRow;

/// One of a user's own preferences (`user_settings`): a JSON document under a key.
#[derive(Debug, Clone, Serialize, FromRow)]
pub struct UserSetting {
    pub user_id: i64,
    pub key: String,
    /// JSON text
    pub value: String,
    pub updated_at: DateTime<Utc>,
}
