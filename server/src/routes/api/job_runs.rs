//! Job Runs API endpoints
//!
//! Provides read access to job run history and cancellation support.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;
use svrctlrs_database::queries::job_runs;
use tracing::{error, info, instrument};

use super::{ApiError, PaginationMeta, PaginationParams};
use crate::state::AppState;

/// Create job runs router
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list_job_runs))
        .route("/{id}", get(get_job_run))
        .route("/{id}/cancel", post(cancel_job_run))
}

/// Filter parameters for job runs
#[derive(Debug, Deserialize)]
struct JobRunFilters {
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    server_id: Option<i64>,
    #[serde(default)]
    job_template_id: Option<i64>,
    #[serde(flatten)]
    pagination: PaginationParams,
}

/// List job runs with pagination and filtering
#[instrument(skip(state))]
async fn list_job_runs(
    State(state): State<AppState>,
    Query(filters): Query<JobRunFilters>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    let per_page = filters.pagination.per_page.clamp(10, 100);
    let offset = (filters.pagination.page.max(1) - 1) * per_page;

    // Get total count for pagination
    let total = job_runs::count_job_runs(&state.pool).await.unwrap_or(0) as usize;

    // Get job runs with names for better API response
    let runs = job_runs::list_job_runs_with_names(&state.pool, per_page as i64, offset as i64)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to list job runs");
            ApiError::internal_error(format!("Failed to list job runs: {}", e))
        })?;

    // Apply filters (TODO: move to database query for better performance)
    let runs: Vec<_> = runs
        .into_iter()
        .filter(|r| {
            if let Some(ref status) = filters.status {
                if &r.status != status {
                    return false;
                }
            }
            if let Some(server_id) = filters.server_id {
                if r.server_id != Some(server_id) {
                    return false;
                }
            }
            if let Some(template_id) = filters.job_template_id {
                if r.job_template_id != template_id {
                    return false;
                }
            }
            true
        })
        .collect();

    let pagination = PaginationMeta::new(filters.pagination.page, per_page, total);

    Ok(Json(json!({
        "job_runs": runs,
        "pagination": pagination
    })))
}

/// Get job run by ID with full output
#[instrument(skip(state))]
async fn get_job_run(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    let run = job_runs::get_job_run(&state.pool, id).await.map_err(|e| {
        error!(error = %e, id = id, "Failed to get job run");
        ApiError::not_found("Job run")
    })?;

    // Get step results if this is a composite job
    let step_results = job_runs::get_step_execution_results(&state.pool, id)
        .await
        .unwrap_or_default();

    Ok(Json(json!({
        "job_run": run,
        "step_results": step_results
    })))
}

/// Cancel a running job
#[instrument(skip(state))]
async fn cancel_job_run(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    info!(id = id, "Cancelling job run");

    // Get job run to verify it exists and is running
    let run = job_runs::get_job_run(&state.pool, id).await.map_err(|e| {
        error!(error = %e, id = id, "Failed to get job run");
        ApiError::not_found("Job run")
    })?;

    if run.status_str != "running" && run.status_str != "pending" {
        return Err(ApiError::bad_request(format!(
            "Cannot cancel job run with status: {}",
            run.status_str
        )));
    }

    // Update status to cancelled
    job_runs::update_job_run_status(&state.pool, id, "cancelled", None)
        .await
        .map_err(|e| {
            error!(error = %e, id = id, "Failed to cancel job run");
            ApiError::internal_error(format!("Failed to cancel job run: {}", e))
        })?;

    // Broadcast status change
    state.broadcast_job_run_update(crate::state::JobRunUpdate::StatusChanged {
        job_run_id: id,
        status: "cancelled".to_string(),
    });

    info!(id = id, "Job run cancelled");

    Ok(Json(json!({
        "id": id,
        "message": "Job run cancelled successfully"
    })))
}
