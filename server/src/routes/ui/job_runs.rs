use askama::Template;
use axum::{
    extract::{Path, Query, State},
    response::Html,
    routing::{get, post},
    Router,
};
use serde::Deserialize;
use svrctlrs_database::queries::{job_runs as queries, servers as server_queries};
use tracing::{error, info, instrument, warn};

use crate::{
    routes::ui::AppError,
    state::AppState,
    templates::{
        JobRunDetailTemplate, JobRunListTemplate, JobRunsTemplate, ServerJobResultsTemplate,
    },
};

/// Create router with all job run routes
pub fn routes() -> Router<AppState> {
    Router::new()
        // Main page
        .route("/job-runs", get(job_runs_page))
        // List and filter endpoints
        .route("/job-runs/list", get(get_job_runs_list))
        // Detail endpoints
        .route("/job-runs/{id}", get(get_job_run_detail))
        .route("/job-runs/{id}/results", get(get_job_run_results))
        // Action endpoints
        .route("/job-runs/{id}/cancel", post(cancel_job_run))
}

// ============================================================================
// Page Routes
// ============================================================================

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

/// Display the job runs page (recent runs)
#[instrument(skip(state))]
pub async fn job_runs_page(
    State(state): State<AppState>,
    Query(params): Query<PaginationParams>,
) -> Result<Html<String>, AppError> {
    info!(page = params.page, "Rendering job runs page");

    let per_page = params.per_page.min(100); // Cap at 100
    let offset = ((params.page.max(1) - 1) * per_page) as i64;

    // Get total count for pagination
    let total_count = queries::count_job_runs(&state.pool).await.map_err(|e| {
        error!(error = %e, "Failed to count job runs");
        AppError::DatabaseError(e.to_string())
    })? as usize;

    let total_pages = total_count.div_ceil(per_page);

    // Get job runs with names (JOIN query)
    let job_runs = queries::list_job_runs_with_names(&state.pool, per_page as i64, offset)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to fetch job runs");
            AppError::DatabaseError(e.to_string())
        })?;

    let template = JobRunsTemplate {
        user: None, // TODO: Add authentication
        job_runs: job_runs.into_iter().map(Into::into).collect(),
        current_page: params.page.max(1),
        total_pages: total_pages.max(1),
        per_page,
    };

    let html = template.render().map_err(|e| {
        error!(error = %e, "Failed to render job runs template");
        AppError::TemplateError(e.to_string())
    })?;

    Ok(Html(html))
}

// ============================================================================
// HTMX List Routes
// ============================================================================

/// Get the job runs list (HTMX)
#[instrument(skip(state))]
pub async fn get_job_runs_list(
    State(state): State<AppState>,
    Query(params): Query<PaginationParams>,
) -> Result<Html<String>, AppError> {
    info!(page = params.page, "Fetching job runs list");

    let per_page = params.per_page.min(100);
    let offset = ((params.page.max(1) - 1) * per_page) as i64;

    // Get total count for pagination
    let total_count = queries::count_job_runs(&state.pool).await.map_err(|e| {
        error!(error = %e, "Failed to count job runs");
        AppError::DatabaseError(e.to_string())
    })? as usize;

    let total_pages = total_count.div_ceil(per_page);

    // Get job runs with names
    let job_runs = queries::list_job_runs_with_names(&state.pool, per_page as i64, offset)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to fetch job runs");
            AppError::DatabaseError(e.to_string())
        })?;

    let template = JobRunListTemplate {
        job_runs: job_runs.into_iter().map(Into::into).collect(),
        current_page: params.page.max(1),
        total_pages: total_pages.max(1),
        per_page,
    };

    let html = template.render().map_err(|e| {
        error!(error = %e, "Failed to render job run list template");
        AppError::TemplateError(e.to_string())
    })?;

    Ok(Html(html))
}

// ============================================================================
// Detail Routes
// ============================================================================

/// Get job run detail (HTMX)
#[instrument(skip(state))]
pub async fn get_job_run_detail(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Html<String>, AppError> {
    info!(job_run_id = id, "Fetching job run detail");

    // Get job run with names
    let job_runs = queries::list_job_runs_with_names(&state.pool, 1, 0)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to fetch job runs");
            AppError::DatabaseError(e.to_string())
        })?;

    let job_run = job_runs.into_iter().find(|jr| jr.id == id).ok_or_else(|| {
        warn!(job_run_id = id, "Job run not found");
        AppError::NotFound(format!("Job run {} not found", id))
    })?;

    // Get server results
    let server_results = queries::get_server_job_results(&state.pool, id)
        .await
        .map_err(|e| {
            error!(job_run_id = id, error = %e, "Failed to fetch server results");
            AppError::DatabaseError(e.to_string())
        })?;

    // Get servers for display
    let servers = server_queries::list_servers(&state.pool)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to fetch servers");
            AppError::DatabaseError(e.to_string())
        })?;

    let server_results_display: Vec<_> = server_results.into_iter().map(Into::into).collect();

    let template = JobRunDetailTemplate {
        user: None,
        job_run: job_run.into(),
        server_results: server_results_display.clone(),
        results: server_results_display, // Alias
        servers: servers.into_iter().map(Into::into).collect(),
    };

    let html = template.render().map_err(|e| {
        error!(error = %e, "Failed to render job run detail template");
        AppError::TemplateError(e.to_string())
    })?;

    Ok(Html(html))
}

/// Get job run results (server results list, HTMX)
#[instrument(skip(state))]
pub async fn get_job_run_results(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Html<String>, AppError> {
    info!(job_run_id = id, "Fetching job run results");

    // Get server results
    let server_results = queries::get_server_job_results(&state.pool, id)
        .await
        .map_err(|e| {
            error!(job_run_id = id, error = %e, "Failed to fetch server results");
            AppError::DatabaseError(e.to_string())
        })?;

    // Get servers for display
    let servers = server_queries::list_servers(&state.pool)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to fetch servers");
            AppError::DatabaseError(e.to_string())
        })?;

    let server_results_display: Vec<_> = server_results.into_iter().map(Into::into).collect();

    let template = ServerJobResultsTemplate {
        job_run_id: id,
        server_results: server_results_display.clone(),
        results: server_results_display, // Alias
        servers: servers.into_iter().map(Into::into).collect(),
    };

    let html = template.render().map_err(|e| {
        error!(error = %e, "Failed to render server results template");
        AppError::TemplateError(e.to_string())
    })?;

    Ok(Html(html))
}

// ============================================================================
// Action Endpoints
// ============================================================================

/// Cancel a running job
#[instrument(skip(state))]
pub async fn cancel_job_run(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Html<String>, AppError> {
    info!(job_run_id = id, "Cancelling job run");

    // Get the job run
    let job_run = queries::get_job_run(&state.pool, id).await.map_err(|e| {
        warn!(job_run_id = id, error = %e, "Job run not found");
        AppError::NotFound(format!("Job run {} not found", id))
    })?;

    // Check if job is still running
    if job_run.status_str != "running" {
        warn!(job_run_id = id, status = %job_run.status_str, "Job run is not running");
        return Err(AppError::ValidationError(format!(
            "Job run {} is not running (status: {})",
            id, job_run.status_str
        )));
    }

    // Update status to cancelled
    queries::update_job_run_status(&state.pool, id, "cancelled", None)
        .await
        .map_err(|e| {
            error!(job_run_id = id, error = %e, "Failed to cancel job run");
            AppError::DatabaseError(e.to_string())
        })?;

    // Finish the job run with cancelled status
    queries::finish_job_run(
        &state.pool,
        id,
        "cancelled",
        None,
        None,
        Some("Job run cancelled by user".to_string()),
    )
    .await
    .map_err(|e| {
        error!(job_run_id = id, error = %e, "Failed to finish job run");
        AppError::DatabaseError(e.to_string())
    })?;

    info!(job_run_id = id, "Job run cancelled successfully");

    // Return updated job run detail
    get_job_run_detail(State(state), Path(id)).await
}
