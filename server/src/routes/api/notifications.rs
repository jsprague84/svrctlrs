//! Notification API endpoints

use axum::{extract::State, http::StatusCode, response::IntoResponse, routing::post, Json, Router};
use serde_json::json;
use svrctlrs_core::notifications::NotificationClient;
use svrctlrs_database::queries::settings;
use tracing::{error, info, instrument};

use super::ApiError;
use crate::state::AppState;

/// Create notifications router
pub fn routes() -> Router<AppState> {
    Router::new().route("/test", post(test_notification))
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
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    info!("Sending test notification");

    let mut config = settings::get_notification_config(&state.pool)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to load notification config");
            ApiError::internal_error(format!("Failed to load config: {}", e))
        })?;

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
    client.send_test().await.map_err(|e| {
        error!(error = %e, "Test notification failed");
        ApiError::internal_error(format!("Failed to send test notification: {}", e))
    })?;

    Ok(Json(json!({
        "message": "Test notification sent successfully"
    })))
}
