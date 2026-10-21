//! Signing in and WebSocket tickets.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// `POST /auth/login`
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Login {
    pub username: String,
    pub password: String,
    /// A TOTP code or a recovery code, for an account with two-factor sign-in on
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub totp: Option<String>,
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
    /// `TOTP_REQUIRED` (send `totp` too) or `TOTP_INVALID`; absent for other refusals
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}

/// `POST /auth/app-token`: the desktop app trades the one-time code its loopback port received
/// from the browser, and the PKCE verifier only it holds, for a session token.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AppTokenRequest {
    pub code: String,
    pub code_verifier: String,
}

/// `POST /auth/ws-ticket`: one use, on a WebSocket upgrade only (`?ticket=`).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct WsTicket {
    pub ticket: String,
    pub expires_in_secs: u64,
}
