//! API tokens (`api_tokens`): named, hashed credentials that act as their owner

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::FromRow;

/// A stored token. The secret and its hash never leave the database layer; `prefix` is the
/// first characters of the secret, enough to recognise it in a list.
#[derive(Debug, Clone, Serialize, FromRow)]
pub struct ApiToken {
    pub id: i64,
    pub user_id: i64,
    pub name: String,
    pub prefix: String,
    /// `full_admin`, `servers_keys` or `read_only` (server/src/api_tokens.rs)
    pub preset: String,
    pub created_at: DateTime<Utc>,
    /// None: never expires
    pub expires_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub last_used_ip: Option<String>,
    pub revoked_at: Option<DateTime<Utc>>,
}

impl ApiToken {
    pub fn is_expired_at(&self, now: DateTime<Utc>) -> bool {
        self.expires_at.is_some_and(|at| at <= now)
    }

    /// `active`, `expired` or `revoked` (revoked wins when both apply)
    pub fn status_at(&self, now: DateTime<Utc>) -> &'static str {
        if self.revoked_at.is_some() {
            "revoked"
        } else if self.is_expired_at(now) {
            "expired"
        } else {
            "active"
        }
    }
}

/// A token to store: the caller generated the secret and passes only its hash and prefix.
#[derive(Debug, Clone)]
pub struct NewApiToken {
    pub user_id: i64,
    pub name: String,
    pub prefix: String,
    pub token_hash: String,
    pub preset: String,
    /// Lifetime from now; None: never expires
    pub expires_in_days: Option<u32>,
}
