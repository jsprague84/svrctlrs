//! Signed-in devices (`devices`): one browser or desktop-app sign-in each

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::FromRow;

/// A sign-in as stored. The session it belongs to is named only by `session_hash`.
#[derive(Debug, Clone, Serialize, FromRow)]
pub struct Device {
    pub id: i64,
    pub user_id: i64,
    /// SHA-256 (hex) of the session id; never sent to a client
    pub session_hash: String,
    /// `password`, `password+totp`, `password+recovery_code`, `oidc`, or `app` (the desktop
    /// app; everything else is a browser)
    pub method: String,
    pub user_agent: Option<String>,
    pub first_ip: Option<String>,
    pub last_ip: Option<String>,
    pub created_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
}

/// A sign-in to record
#[derive(Debug, Clone)]
pub struct NewDevice {
    pub user_id: i64,
    pub session_hash: String,
    pub method: String,
    pub user_agent: Option<String>,
    pub ip: Option<String>,
}
