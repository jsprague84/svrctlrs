//! Webhook endpoints for remote triggering

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::post,
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, error, info, instrument, warn};

use crate::state::AppState;

/// Create webhook router
pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/trigger/{plugin_id}/{task_id}", post(trigger_task))
        .route("/docker/health", post(trigger_docker_health))
        .route("/docker/cleanup", post(trigger_docker_cleanup))
        .route("/docker/analysis", post(trigger_docker_analysis))
        .route("/updates/check", post(trigger_updates_check))
        .route("/updates/apply", post(trigger_updates_apply))
        .route("/updates/cleanup", post(trigger_os_cleanup))
}

/// Webhook trigger request body
#[derive(Debug, Deserialize)]
struct TriggerRequest {
    #[serde(default)]
    token: Option<String>,
    #[serde(default)]
    payload: Option<serde_json::Value>,
}

/// Verify webhook token
fn verify_token(headers: &HeaderMap, request_token: &Option<String>) -> bool {
    let expected_token = std::env::var("WEBHOOK_SECRET").ok();

    if expected_token.is_none() {
        // No token configured, allow all requests (development mode)
        warn!("WEBHOOK_SECRET not configured - accepting all webhook requests");
        return true;
    }

    let expected = expected_token.as_ref().unwrap();

    // Check Authorization header first
    if let Some(auth_header) = headers.get("Authorization") {
        if let Ok(value) = auth_header.to_str() {
            if let Some(token) = value.strip_prefix("Bearer ") {
                if token == expected {
                    return true;
                }
            }
        }
    }

    // Check token in request body
    if let Some(token) = request_token {
        if token == expected {
            return true;
        }
    }

    false
}

/// Generic task trigger endpoint
#[instrument(skip(state, headers))]
async fn trigger_task(
    State(state): State<Arc<AppState>>,
    Path((plugin_id, task_id)): Path<(String, String)>,
    headers: HeaderMap,
    Json(req): Json<TriggerRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    info!(plugin_id = %plugin_id, task_id = %task_id, "Webhook trigger received");

    // Verify token
    if !verify_token(&headers, &req.token) {
        warn!("Unauthorized webhook request");
        return Err((StatusCode::UNAUTHORIZED, "Invalid or missing token".to_string()));
    }

    let registry = state.plugins.read().await;

    let plugin = registry
        .get(&plugin_id)
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("Plugin {} not found", plugin_id)))?;

    // Create plugin context
    let context = svrctlrs_core::PluginContext {
        servers: state.config.servers.clone(),
        config: HashMap::new(),
        notification_manager: state.notification_manager().await,
    };

    // Execute the task
    let result = plugin
        .execute(&task_id, &context)
        .await
        .map_err(|e| {
            error!(error = %e, "Webhook task execution failed");
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Task execution failed: {}", e))
        })?;

    info!(success = result.success, "Webhook task execution completed");

    Ok(Json(json!({
        "success": result.success,
        "message": result.message,
        "executed_at": chrono::Utc::now().to_rfc3339()
    })))
}

/// Trigger Docker health check
#[instrument(skip(state, headers))]
async fn trigger_docker_health(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<TriggerRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    debug!("Docker health check webhook triggered");

    if !verify_token(&headers, &req.token) {
        return Err((StatusCode::UNAUTHORIZED, "Invalid or missing token".to_string()));
    }

    trigger_specific_task(state, "docker", "docker_health").await
}

/// Trigger Docker cleanup
#[instrument(skip(state, headers))]
async fn trigger_docker_cleanup(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<TriggerRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    debug!("Docker cleanup webhook triggered");

    if !verify_token(&headers, &req.token) {
        return Err((StatusCode::UNAUTHORIZED, "Invalid or missing token".to_string()));
    }

    trigger_specific_task(state, "docker", "docker_cleanup").await
}

/// Trigger Docker analysis
#[instrument(skip(state, headers))]
async fn trigger_docker_analysis(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<TriggerRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    debug!("Docker analysis webhook triggered");

    if !verify_token(&headers, &req.token) {
        return Err((StatusCode::UNAUTHORIZED, "Invalid or missing token".to_string()));
    }

    trigger_specific_task(state, "docker", "docker_analysis").await
}

/// Trigger updates check
#[instrument(skip(state, headers))]
async fn trigger_updates_check(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<TriggerRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    debug!("Updates check webhook triggered");

    if !verify_token(&headers, &req.token) {
        return Err((StatusCode::UNAUTHORIZED, "Invalid or missing token".to_string()));
    }

    trigger_specific_task(state, "updates", "updates_check").await
}

/// Trigger updates apply
#[instrument(skip(state, headers))]
async fn trigger_updates_apply(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<TriggerRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    debug!("Updates apply webhook triggered");

    if !verify_token(&headers, &req.token) {
        return Err((StatusCode::UNAUTHORIZED, "Invalid or missing token".to_string()));
    }

    trigger_specific_task(state, "updates", "updates_apply").await
}

/// Trigger OS cleanup
#[instrument(skip(state, headers))]
async fn trigger_os_cleanup(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<TriggerRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    debug!("OS cleanup webhook triggered");

    if !verify_token(&headers, &req.token) {
        return Err((StatusCode::UNAUTHORIZED, "Invalid or missing token".to_string()));
    }

    trigger_specific_task(state, "updates", "os_cleanup").await
}

/// Helper function to trigger a specific task
async fn trigger_specific_task(
    state: Arc<AppState>,
    plugin_id: &str,
    task_id: &str,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let registry = state.plugins.read().await;

    let plugin = registry
        .get(plugin_id)
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("Plugin {} not found", plugin_id)))?;

    // Create plugin context
    let context = svrctlrs_core::PluginContext {
        servers: state.config.servers.clone(),
        config: HashMap::new(),
        notification_manager: state.notification_manager().await,
    };

    // Execute the task
    let result = plugin
        .execute(task_id, &context)
        .await
        .map_err(|e| {
            error!(error = %e, "Webhook task execution failed");
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Task execution failed: {}", e))
        })?;

    info!(success = result.success, "Webhook task execution completed");

    Ok(Json(json!({
        "success": result.success,
        "message": result.message,
        "executed_at": chrono::Utc::now().to_rfc3339()
    })))
}
