//! Types shared by every part of the API.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Every error: `{ "success": false, "error": { "code", "message", "details"? } }`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ErrorBody {
    /// Always false
    pub success: bool,
    pub error: ErrorDetails,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ErrorDetails {
    /// `BAD_REQUEST`, `UNAUTHORIZED`, `FORBIDDEN`, `NOT_FOUND`, `CONFLICT`, `HOST_KEY_MISMATCH`,
    /// `HOST_KEY_UNCONFIRMED`, `TOKEN_INVALID`, `TOKEN_EXPIRED`, `TOKEN_REVOKED`,
    /// `TOKEN_IN_URL`, `TOKEN_SCOPE`, `INTERNAL_ERROR`
    pub code: String,
    pub message: String,
    /// Extras a client can act on (for example the fingerprint of an unconfirmed host key)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

/// `{ "message": "…" }`
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Message {
    pub message: String,
}

impl Message {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

/// `{ "id": 3, "message": "…" }`: what a create, update or delete of a numbered row answers.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct IdMessage {
    pub id: i64,
    pub message: String,
}

impl IdMessage {
    pub fn new(id: i64, message: impl Into<String>) -> Self {
        Self {
            id,
            message: message.into(),
        }
    }
}
