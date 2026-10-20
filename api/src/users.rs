//! Accounts and the caller's own preferences.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// An account. Password hashes never leave the server.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UserAccount {
    pub id: i64,
    pub username: String,
    /// The caller's own account
    pub is_me: bool,
    /// The most recent successful login in the audit log
    pub last_login_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UserList {
    pub users: Vec<UserAccount>,
}

/// `POST /users`
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct NewUser {
    /// 1–64 characters
    pub username: String,
    /// At least 12 characters
    pub password: String,
}

/// `PUT /users/me/password`
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PasswordChange {
    pub current_password: String,
    /// At least 12 characters
    pub new_password: String,
}

/// One of the caller's preference documents (`terminal`); `value` is null until saved.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Preference {
    pub key: String,
    /// A JSON object the UI owns
    pub value: Option<serde_json::Value>,
    pub updated_at: Option<DateTime<Utc>>,
}

/// `PUT /users/me/preferences/{key}`: a JSON object of at most 16 KiB
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PreferenceValue {
    pub value: serde_json::Value,
}
