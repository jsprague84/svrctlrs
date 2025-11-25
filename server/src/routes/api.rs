//! REST API endpoints

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;
use tracing::{debug, error, info, instrument};

use crate::state::AppState;

/// Create API router
pub fn routes() -> Router<AppState> {
    Router::new()
        // Health and status
        .route("/health", get(health_check))
        .route("/status", get(server_status))
        // Metrics
        .route("/metrics", get(get_metrics))
        .route("/metrics/{plugin_id}", get(plugin_metrics))
        // Tasks
        .route("/tasks", get(list_all_tasks))
        .route("/tasks/execute", post(execute_task))
}

/// Health check endpoint
#[instrument]
async fn health_check() -> impl IntoResponse {
    debug!("Health check requested");
    Json(json!({
        "status": "ok",
        "service": "svrctlrs",
        "version": env!("CARGO_PKG_VERSION")
    }))
}

/// Server status endpoint
#[instrument(skip(state))]
async fn server_status(State(state): State<AppState>) -> impl IntoResponse {
    let plugins = state.plugins.read().await;
    let plugin_count = plugins.plugin_ids().len();

    let scheduler = state.scheduler.read().await;
    let scheduler_running = scheduler.is_some();

    Json(json!({
        "status": "running",
        "plugins_loaded": plugin_count,
        "scheduler_running": scheduler_running,
        "servers": state.config.servers.len()
    }))
}


/// Get system metrics
#[instrument(skip(state))]
async fn get_metrics(State(state): State<AppState>) -> impl IntoResponse {
    // TODO: Fetch actual metrics from database
    Json(json!({
        "metrics": {
            "plugins_loaded": state.plugins.read().await.plugin_ids().len(),
            "servers_configured": state.config.servers.len()
        }
    }))
}

/// Get plugin-specific metrics
#[instrument(skip(state))]
async fn plugin_metrics(
    State(state): State<AppState>,
    Path(plugin_id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let registry = state.plugins.read().await;

    // Verify plugin exists
    registry.get(&plugin_id).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            format!("Plugin {} not found", plugin_id),
        )
    })?;

    // TODO: Fetch actual metrics from database
    Ok(Json(json!({
        "plugin_id": plugin_id,
        "metrics": {}
    })))
}

/// List all scheduled tasks
#[instrument(skip(state))]
async fn list_all_tasks(State(state): State<AppState>) -> impl IntoResponse {
    let registry = state.plugins.read().await;
    let mut all_tasks = Vec::new();

    for plugin_id in registry.plugin_ids() {
        if let Some(plugin) = registry.get(&plugin_id) {
            let tasks = plugin.scheduled_tasks();
            for task in tasks {
                all_tasks.push(json!({
                    "plugin_id": plugin_id,
                    "task_id": task.id,
                    "description": task.description,
                    "schedule": task.schedule,
                    "enabled": task.enabled
                }));
            }
        }
    }

    Json(json!({
        "tasks": all_tasks
    }))
}

/// Request to execute a task
#[derive(Debug, Deserialize)]
struct ExecuteTaskRequest {
    plugin_id: String,
    task_id: String,
}

/// Execute a task manually
#[instrument(skip(state))]
async fn execute_task(
    State(state): State<AppState>,
    Json(req): Json<ExecuteTaskRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    info!(plugin_id = %req.plugin_id, task_id = %req.task_id, "Manual task execution requested");

    let registry = state.plugins.read().await;

    let plugin = registry.get(&req.plugin_id).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            format!("Plugin {} not found", req.plugin_id),
        )
    })?;

    // Create plugin context
    let context = svrctlrs_core::PluginContext {
        servers: state.config.servers.clone(),
        config: HashMap::new(),
        notification_manager: state.notification_manager().await,
    };

    // Execute the task
    let result = plugin.execute(&req.task_id, &context).await.map_err(|e| {
        error!(error = %e, "Task execution failed");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Task execution failed: {}", e),
        )
    })?;

    info!(success = result.success, "Task execution completed");

    Ok(Json(json!({
        "success": result.success,
        "message": result.message,
        "data": result.data,
        "metrics": result.metrics
    })))
}
