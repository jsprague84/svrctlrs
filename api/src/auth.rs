//! Signing in and WebSocket tickets.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// `POST /auth/login`
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Login {
    pub username: String,
    pub password: String,
}

/// A successful login: the session id doubles as a Bearer token for native apps.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct LoggedIn {
    pub success: bool,
    pub user_id: i64,
    pub session_id: String,
}

/// A refused login (this route predates the error envelope)
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct LoginRefused {
    pub error: String,
}

/// `POST /auth/ws-ticket`: one use, on a WebSocket upgrade only (`?ticket=`).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct WsTicket {
    pub ticket: String,
    pub expires_in_secs: u64,
}
