//! REST API endpoints

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse},
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
        // Configuration
        .route("/config/reload", post(reload_config))
        // Metrics
        .route("/metrics", get(get_metrics))
        .route("/metrics/{plugin_id}", get(plugin_metrics))
        // Tasks
        .route("/tasks", get(list_all_tasks))
        .route("/tasks/execute", post(execute_task))
        // Notifications
        .route("/notifications/{id}/test", post(test_notification))
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
    let task_count = if let Some(ref sched) = *scheduler {
        sched.task_count().await
    } else {
        0
    };

    Json(json!({
        "status": "running",
        "plugins_loaded": plugin_count,
        "scheduler_running": scheduler_running,
        "scheduled_tasks": task_count,
        "servers": state.config.servers.len()
    }))
}

/// Reload configuration from database
#[instrument(skip(state))]
async fn reload_config(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, Html<String>)> {
    info!("Configuration reload requested");

    state.reload_config().await.map_err(|e| {
        error!(error = %e, "Configuration reload failed");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Html(format!(
                r#"<div class="alert alert-error">❌ Configuration reload failed: {}</div>"#,
                e
            )),
        )
    })?;

    let plugins = state.plugins.read().await;
    let plugin_count = plugins.plugin_ids().len();

    let scheduler = state.scheduler.read().await;
    let task_count = if let Some(ref sched) = *scheduler {
        sched.task_count().await
    } else {
        0
    };

    info!("Configuration reloaded successfully");

    Ok(Html(format!(
        r#"<div class="alert alert-success">
            ✅ Configuration reloaded successfully!<br>
            <small class="text-secondary">
                Plugins loaded: {} | Scheduled tasks: {}
            </small>
        </div>"#,
        plugin_count, task_count
    )))
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

/// Test notification endpoint - sends a test message to verify backend configuration
#[instrument(skip(state))]
async fn test_notification(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, Html<String>)> {
    use svrctlrs_core::{
        GotifyBackend, NotificationBackend as CoreBackend, NotificationMessage, NtfyBackend,
    };
    use svrctlrs_database::queries;

    info!("Testing notification backend {}", id);

    // Load notification backend from database
    let db = state.db().await;
    let backend = queries::notifications::get_notification_backend(db.pool(), id)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to load notification backend");
            (
                StatusCode::NOT_FOUND,
                Html(format!(
                    r#"<div class="alert alert-error">❌ Failed to load notification backend: {}</div>"#,
                    e
                )),
            )
        })?;

    if !backend.enabled {
        return Err((
            StatusCode::BAD_REQUEST,
            Html(format!(
                r#"<div class="alert alert-error">❌ Backend "{}" is disabled</div>"#,
                backend.name
            )),
        ));
    }

    // Create test message
    let test_message = NotificationMessage {
        title: format!("🧪 Test from {}", backend.name),
        body: format!(
            "This is a test notification from SvrCtlRS.\n\nBackend: {}\nType: {}\n\nIf you received this, your notification backend is configured correctly! ✅",
            backend.name, backend.backend_type
        ),
        priority: 3,
        actions: vec![],
    };

    // Create HTTP client
    let client = reqwest::Client::new();

    // Create appropriate backend and send test
    let result = match backend.backend_type.as_str() {
        "gotify" => {
            let config = backend.get_config();
            let url = config["url"].as_str().unwrap_or_default();
            let token = config["token"].as_str().unwrap_or_default();

            let gotify = GotifyBackend::with_url_and_key(client, url, token)
                .map_err(|e| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Html(format!(
                            r#"<div class="alert alert-error">❌ Failed to create Gotify backend: {}</div>"#,
                            e
                        )),
                    )
                })?;
            gotify.send(&test_message).await
        }
        "ntfy" => {
            let config = backend.get_config();
            let url = config["url"].as_str().unwrap_or("https://ntfy.sh");
            let topic = config["topic"].as_str().unwrap_or_default();

            let mut ntfy = NtfyBackend::with_url_and_topic(client, url, topic)
                .map_err(|e| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Html(format!(
                            r#"<div class="alert alert-error">❌ Failed to create ntfy backend: {}</div>"#,
                            e
                        )),
                    )
                })?;

            // Add authentication if configured
            if let Some(token) = config.get("token").and_then(|t| t.as_str()) {
                if !token.trim().is_empty() {
                    ntfy = ntfy.with_token(token.trim());
                }
            } else if let (Some(username), Some(password)) = (
                config.get("username").and_then(|u| u.as_str()),
                config.get("password").and_then(|p| p.as_str()),
            ) {
                if !username.trim().is_empty() && !password.trim().is_empty() {
                    ntfy = ntfy.with_basic_auth(username.trim(), password.trim());
                }
            }

            // Register the topic for the test service
            ntfy.register_service("test", topic);

            // Send using service context
            ntfy.send_for_service("test", &test_message).await
        }
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                Html(format!(
                    r#"<div class="alert alert-error">❌ Unknown backend type: {}</div>"#,
                    backend.backend_type
                )),
            ))
        }
    };

    match result {
        Ok(()) => {
            info!("Test notification sent successfully to {}", backend.name);
            Ok(Html(format!(
                r#"<div class="alert alert-success">✅ Test notification sent successfully to "{}"!</div>"#,
                backend.name
            )))
        }
        Err(e) => {
            error!(error = %e, "Failed to send test notification");
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(format!(
                    r#"<div class="alert alert-error">❌ Failed to send test notification: {}</div>"#,
                    e
                )),
            ))
        }
    }
}
