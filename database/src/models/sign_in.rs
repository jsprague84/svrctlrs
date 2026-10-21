use chrono::{DateTime, Utc};
use sqlx::FromRow;

/// A user's TOTP enrolment (`user_totp`). `secret` is the base32 secret, decrypted; it never
/// leaves the server after the enrolment response that shows it.
#[derive(Clone, FromRow)]
pub struct UserTotp {
    pub user_id: i64,
    pub secret: String,
    /// None while the enrolment waits for its first code
    pub confirmed_at: Option<DateTime<Utc>>,
    /// The time step last accepted; a code for it or an earlier one is refused
    pub last_step: i64,
    pub created_at: DateTime<Utc>,
}

impl UserTotp {
    pub fn is_enabled(&self) -> bool {
        self.confirmed_at.is_some()
    }
}

impl std::fmt::Debug for UserTotp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UserTotp")
            .field("user_id", &self.user_id)
            .field("secret", &"[REDACTED]")
            .field("confirmed_at", &self.confirmed_at)
            .field("last_step", &self.last_step)
            .finish()
    }
}

/// An identity at an OpenID Connect provider, linked to an account (`user_identities`).
#[derive(Debug, Clone, FromRow)]
pub struct UserIdentity {
    pub id: i64,
    pub user_id: i64,
    pub issuer: String,
    pub subject: String,
    pub email: Option<String>,
    pub display_name: Option<String>,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
}

/// What a link stores
#[derive(Debug, Clone)]
pub struct NewUserIdentity {
    pub user_id: i64,
    pub issuer: String,
    pub subject: String,
    pub email: Option<String>,
    pub display_name: Option<String>,
}
