//! Notification API endpoints

use axum::{extract::State, response::IntoResponse, routing::post, Json};
use serde_json::json;
use svrctlrs_core::notifications::NotificationClient;
use svrctlrs_database::queries::settings;
use tracing::{info, instrument};

use super::ApiError;
use crate::state::AppState;
use utoipa_axum::router::OpenApiRouter;

/// Create notifications router
pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .route("/test", post(test_notification))
        .route("/test-approval", post(test_approval_channel))
}

/// Send a sample approval request through the approval channel, as saved or as the form has
/// it (an empty secret means the saved one).
#[instrument(skip(state, headers, body))]
async fn test_approval_channel(
    State(state): State<AppState>,
    crate::routes::ui::auth::CurrentUser(me): crate::routes::ui::auth::CurrentUser,
    headers: axum::http::HeaderMap,
    body: Option<Json<crate::approvals::ChannelOverride>>,
) -> Result<impl IntoResponse, ApiError> {
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
        Ok(channel) => Ok(Json(
            json!({ "message": format!("Test request sent via {channel}") }),
        )),
        Err(e) => Err(ApiError::bad_request(e.to_string())),
    }
}

/// Optional unsaved values to test with (`POST /notifications/test`)
#[derive(Debug, Default, serde::Deserialize)]
struct TestInput {
    provider: Option<String>,
    url: Option<String>,
    token: Option<String>,
    topic: Option<String>,
}

/// Send a test notification to verify configuration
#[instrument(skip(state))]
async fn test_notification(
    State(state): State<AppState>,
    body: Option<Json<TestInput>>,
) -> Result<impl IntoResponse, ApiError> {
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

    Ok(Json(json!({
        "message": "Test notification sent successfully"
    })))
}
