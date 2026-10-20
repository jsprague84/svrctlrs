//! Notification API endpoints

use axum::{extract::State, Json};
use svrctlrs_api::notifications::{ApprovalChannelTest, NotificationTest};
use svrctlrs_api::{ErrorBody, Message};
use svrctlrs_core::notifications::NotificationClient;
use svrctlrs_database::queries::settings;
use tracing::{info, instrument};

use super::ApiError;
use crate::state::AppState;
use utoipa_axum::{router::OpenApiRouter, routes};

/// Create notifications router
pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(test_notification))
        .routes(routes!(test_approval_channel))
}

/// Send a sample approval request through the approval channel, as saved or as the form has
/// it (an empty secret means the saved one).
#[utoipa::path(post, path = "/test-approval", tag = "notifications",
    request_body(content = Option<ApprovalChannelTest>),
    responses(
        (status = 200, body = Message),
        (status = 400, description = "Not configured, or the channel refused", body = ErrorBody),
    ))]
#[instrument(skip(state, headers, body))]
async fn test_approval_channel(
    State(state): State<AppState>,
    crate::routes::ui::auth::CurrentUser(me): crate::routes::ui::auth::CurrentUser,
    headers: axum::http::HeaderMap,
    body: Option<Json<ApprovalChannelTest>>,
) -> Result<Json<Message>, ApiError> {
    let now = chrono::Utc::now();
    let sample = svrctlrs_database::models::Approval {
        id: "test".into(),
        user_id: me,
        token_id: None,
        token_name: "test".into(),
        action: "server.delete".into(),
        method: "DELETE".into(),
        path: "/api/v1/servers/0".into(),
        target_type: "server".into(),
        target_id: 0,
        target_name: Some("example (a test; nothing will happen)".into()),
        client_ip: None,
        url: format!(
            "{}/approvals",
            crate::approvals::link_base(&state.pool, &headers).await
        ),
        status: "pending".into(),
        created_at: now,
        expires_at: now,
        decided_at: None,
        decided_by: None,
        result_status: None,
        result: None,
    };
    let over = body.map(|Json(o)| o);
    match crate::approvals::notify(&state.pool, &sample, over.as_ref()).await {
        Ok("none") => Err(ApiError::bad_request(
            "The approval channel is None: requests are only in the caller's answer",
        )),
        Ok(channel) => Ok(Json(Message::new(format!(
            "Test request sent via {channel}"
        )))),
        Err(e) => Err(ApiError::bad_request(e.to_string())),
    }
}

/// Send a test notification to verify configuration
#[utoipa::path(post, path = "/test", tag = "notifications",
    request_body(content = Option<NotificationTest>),
    responses(
        (status = 200, body = Message),
        (status = 400, description = "Not configured", body = ErrorBody),
    ))]
#[instrument(skip(state))]
async fn test_notification(
    State(state): State<AppState>,
    body: Option<Json<NotificationTest>>,
) -> Result<Json<Message>, ApiError> {
    info!("Sending test notification");

    let mut config = settings::get_notification_config(&state.pool).await?;

    // The form's current values, so a test does not need a save first. An empty token means
    // "the stored one" (the UI never sees the stored token).
    if let Some(Json(input)) = body {
        if let Some(p) = input.provider {
            config.provider = svrctlrs_core::notifications::NotificationProvider::parse(&p);
        }
        if let Some(u) = input.url {
            config.url = u;
        }
        if let Some(t) = input.token.filter(|t| !t.is_empty()) {
            config.token = t;
        }
        if let Some(t) = input.topic {
            config.topic = t;
        }
        config.enabled = true;
    }

    if !config.enabled || config.url.is_empty() {
        return Err(ApiError::bad_request(
            "Notifications not configured: set a provider URL (and token) first.",
        ));
    }

    let client = NotificationClient::new(config);
    client.send_test().await?;

    Ok(Json(Message::new("Test notification sent successfully")))
}
