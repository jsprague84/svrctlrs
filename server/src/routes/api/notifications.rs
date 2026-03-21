//! Notification API endpoints

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::post,
    Json, Router,
};
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

/// Send a test notification to verify configuration
#[instrument(skip(state))]
async fn test_notification(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    info!("Sending test notification");

    let config = settings::get_notification_config(&state.pool)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to load notification config");
            ApiError::internal_error(format!("Failed to load config: {}", e))
        })?;

    if !config.enabled || config.url.is_empty() {
        return Err(ApiError::bad_request(
            "Notifications not configured. Set notification.enabled, notification.url, and notification.token in settings.",
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
