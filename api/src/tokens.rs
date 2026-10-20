//! API tokens (signed-in users only: no token can manage tokens).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::double_option;

/// A token as listed: never its secret or hash.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ApiToken {
    pub id: i64,
    pub name: String,
    /// `svr_` and the first six characters of the secret
    pub prefix: String,
    /// `full_admin`, `servers_keys` or `read_only`
    pub preset: String,
    /// `active`, `expired` or `revoked`
    pub status: String,
    pub created_at: DateTime<Utc>,
    /// Null: never expires
    pub expires_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub last_used_ip: Option<String>,
    pub revoked_at: Option<DateTime<Utc>>,
    /// Deleting a server or credential, or forgetting host keys, waits for the owner's yes
    pub require_approval: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TokenList {
    pub tokens: Vec<ApiToken>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TokenOne {
    pub token: ApiToken,
}

/// `POST /tokens`
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct NewToken {
    /// 1–64 characters, unique among your tokens
    pub name: String,
    /// `full_admin`, `servers_keys` or `read_only`
    pub preset: String,
    /// Required: 1–3650 days, or null for a token that never expires
    #[serde(default, deserialize_with = "double_option")]
    #[schema(value_type = Option<u32>)]
    pub expires_in_days: Option<Option<u32>>,
    /// Default true
    #[serde(default)]
    pub require_approval: Option<bool>,
}

/// What `POST /tokens` answers: the only time the secret is shown.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreatedToken {
    pub token: ApiToken,
    /// `svr_` + 40 letters and digits; send it as `Authorization: Bearer …`
    pub secret: String,
}

/// `PUT /tokens/{id}`
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TokenChanges {
    pub require_approval: bool,
}
