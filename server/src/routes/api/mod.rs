//! REST API v1 endpoints
//!
//! This module provides a comprehensive REST API for programmatic access
//! to SvrCtlRS functionality. All endpoints return JSON responses.
//!
//! ## API Structure
//!
//! ```text
//! /api/v1/
//! ├── health                      GET     Health check
//! ├── status                      GET     Server status
//! ├── metrics                     GET     System metrics
//! ├── config/reload               POST    Reload configuration
//! │
//! ├── servers/                    Full CRUD + test connection
//! ├── credentials/                Full CRUD
//! ├── tags/                       Full CRUD
//! ├── job-types/                  Full CRUD + command templates
//! ├── job-templates/              Full CRUD + execute
//! ├── job-schedules/              Full CRUD + toggle
//! ├── job-runs/                   List + get + cancel
//! ├── notifications/
//! │   ├── channels/               Full CRUD + test
//! │   └── policies/               Full CRUD
//! └── settings/                   List + get + update
//! ```

pub mod credentials;
pub mod job_runs;
pub mod job_schedules;
pub mod job_templates;
pub mod job_types;
pub mod notifications;
pub mod servers;
pub mod settings;
pub mod tags;

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{debug, error, info, instrument};

use crate::state::AppState;

/// Standard API error response
#[derive(Debug, Serialize)]
pub struct ApiError {
    pub success: bool,
    pub error: ApiErrorDetails,
}

#[derive(Debug, Serialize)]
pub struct ApiErrorDetails {
    pub code: String,
    pub message: String,
}

impl ApiError {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            success: false,
            error: ApiErrorDetails {
                code: code.to_string(),
                message: message.into(),
            },
        }
    }

    pub fn not_found(resource: &str) -> (StatusCode, Json<Self>) {
        (
            StatusCode::NOT_FOUND,
            Json(Self::new("NOT_FOUND", format!("{} not found", resource))),
        )
    }

    pub fn bad_request(message: impl Into<String>) -> (StatusCode, Json<Self>) {
        (
            StatusCode::BAD_REQUEST,
            Json(Self::new("BAD_REQUEST", message)),
        )
    }

    pub fn internal_error(message: impl Into<String>) -> (StatusCode, Json<Self>) {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(Self::new("INTERNAL_ERROR", message)),
        )
    }

    pub fn conflict(message: impl Into<String>) -> (StatusCode, Json<Self>) {
        (StatusCode::CONFLICT, Json(Self::new("CONFLICT", message)))
    }
}

/// Pagination parameters for list endpoints
#[derive(Debug, Deserialize)]
pub struct PaginationParams {
    #[serde(default = "default_page")]
    pub page: usize,
    #[serde(default = "default_per_page")]
    pub per_page: usize,
}

fn default_page() -> usize {
    1
}

fn default_per_page() -> usize {
    50
}

/// Pagination metadata for list responses
#[derive(Debug, Serialize)]
pub struct PaginationMeta {
    pub page: usize,
    pub per_page: usize,
    pub total: usize,
    pub total_pages: usize,
}

impl PaginationMeta {
    pub fn new(page: usize, per_page: usize, total: usize) -> Self {
        Self {
            page,
            per_page,
            total,
            total_pages: total.div_ceil(per_page),
        }
    }
}

/// Create the complete v1 API router
pub fn routes() -> Router<AppState> {
    Router::new()
        // System endpoints
        .route("/health", get(health_check))
        .route("/status", get(server_status))
        .route("/metrics", get(get_metrics))
        .route("/config/reload", post(reload_config))
        // Resource endpoints (nested routers)
        .nest("/servers", servers::routes())
        .nest("/credentials", credentials::routes())
        .nest("/tags", tags::routes())
        .nest("/job-types", job_types::routes())
        .nest("/job-templates", job_templates::routes())
        .nest("/job-schedules", job_schedules::routes())
        .nest("/job-runs", job_runs::routes())
        .nest("/notifications", notifications::routes())
        .nest("/settings", settings::routes())
}

/// Health check endpoint
///
/// Returns basic health status of the service.
///
/// ## Response
/// ```json
/// {
///   "status": "ok",
///   "service": "svrctlrs",
///   "version": "0.1.0"
/// }
/// ```
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
///
/// Returns detailed status information about the server.
#[instrument(skip(state))]
async fn server_status(State(state): State<AppState>) -> impl IntoResponse {
    use svrctlrs_database::queries;

    let scheduler = state.scheduler.read().await;
    let scheduler_running = scheduler.is_some();
    let task_count = if let Some(ref sched) = *scheduler {
        sched.task_count().await
    } else {
        0
    };

    // Get actual counts from database
    let server_count = queries::servers::list_servers(&state.pool)
        .await
        .map(|s| s.len())
        .unwrap_or(0);

    let job_type_count = queries::job_types::list_job_types(&state.pool)
        .await
        .map(|t| t.len())
        .unwrap_or(0);

    let job_template_count = queries::job_templates::list_job_templates(&state.pool)
        .await
        .map(|t| t.len())
        .unwrap_or(0);

    let job_schedule_count = queries::job_schedules::list_job_schedules(&state.pool)
        .await
        .map(|s| s.len())
        .unwrap_or(0);

    Json(json!({
        "status": "running",
        "version": env!("CARGO_PKG_VERSION"),
        "scheduler": {
            "running": scheduler_running,
            "scheduled_tasks": task_count
        },
        "resources": {
            "servers": server_count,
            "job_types": job_type_count,
            "job_templates": job_template_count,
            "job_schedules": job_schedule_count
        }
    }))
}

/// Reload configuration from database
#[instrument(skip(state))]
async fn reload_config(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    info!("Configuration reload requested via API");

    state.reload_config().await.map_err(|e| {
        error!(error = %e, "Configuration reload failed");
        ApiError::internal_error(format!("Configuration reload failed: {}", e))
    })?;

    let scheduler = state.scheduler.read().await;
    let task_count = if let Some(ref sched) = *scheduler {
        sched.task_count().await
    } else {
        0
    };

    info!("Configuration reloaded successfully");

    Ok(Json(json!({
        "success": true,
        "message": "Configuration reloaded successfully",
        "scheduled_tasks": task_count
    })))
}

/// Get system metrics
#[instrument(skip(state))]
async fn get_metrics(State(state): State<AppState>) -> impl IntoResponse {
    use svrctlrs_database::queries;

    // Get job run statistics
    let total_runs = queries::job_runs::count_job_runs(&state.pool)
        .await
        .unwrap_or(0);

    let recent_runs = queries::job_runs::list_job_runs(&state.pool, 100, 0)
        .await
        .unwrap_or_default();

    let success_count = recent_runs
        .iter()
        .filter(|r| r.status_str == "success")
        .count();
    let failed_count = recent_runs
        .iter()
        .filter(|r| r.status_str == "failed")
        .count();
    let running_count = recent_runs
        .iter()
        .filter(|r| r.status_str == "running")
        .count();

    // Get server count
    let server_count = queries::servers::list_servers(&state.pool)
        .await
        .map(|s| s.len())
        .unwrap_or(0);

    // Get enabled schedules
    let schedules = queries::job_schedules::list_job_schedules(&state.pool)
        .await
        .unwrap_or_default();
    let enabled_schedules = schedules.iter().filter(|s| s.enabled).count();

    Json(json!({
        "metrics": {
            "servers": {
                "total": server_count
            },
            "job_runs": {
                "total": total_runs,
                "recent_success": success_count,
                "recent_failed": failed_count,
                "currently_running": running_count
            },
            "schedules": {
                "total": schedules.len(),
                "enabled": enabled_schedules
            }
        }
    }))
}
