//! Testing where notifications go.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// `POST /notifications/test`: values from the form, unsaved; absent ones are the saved
/// settings, and an empty token is the saved token.
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
pub struct NotificationTest {
    /// `gotify` or `ntfy`
    pub provider: Option<String>,
    pub url: Option<String>,
    pub token: Option<String>,
    pub topic: Option<String>,
}

/// `POST /notifications/test-approval`: where approval requests would go, unsaved; an empty
/// secret is the saved one.
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
pub struct ApprovalChannelTest {
    /// `push`, `webhook`, `matrix` or `none`
    pub channel: Option<String>,
    pub webhook_url: Option<String>,
    pub matrix_homeserver: Option<String>,
    pub matrix_room: Option<String>,
    pub matrix_token: Option<String>,
}
