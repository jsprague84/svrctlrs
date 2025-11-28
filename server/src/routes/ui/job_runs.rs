use askama::Template;
use axum::{
    extract::{Path, Query, State},
    response::Html,
};
use chrono::Utc;
use serde::Deserialize;
use svrctlrs_database::{
    models::{JobRun, ServerJobResult},
    queries::{job_runs as queries, server_job_results, servers as server_queries},
};
use tracing::{error, info, instrument, warn};

use crate::{
    routes::ui::AppError,
    state::AppState,
    templates::{
        JobRunDetailTemplate, JobRunListTemplate, JobRunsTemplate,
        ServerJobResultsTemplate, ServerJobResultDetailTemplate,
    },
};

// ============================================================================
// Page Routes
// ============================================================================

/// Display the job runs page (recent runs)
#[instrument(skip(state))]
pub async fn job_runs_page(
    State(state): State<AppState>,
    Query(params): Query<PaginationParams>,
) -> Result<Html<String>, AppError> {
    info!("Rendering job runs page");

    let page = params.page.unwrap_or(1);
    let per_page = params.per_page.unwrap_or(50);
    let offset = (page - 1) * per_page;

    let job_runs = queries::list_job_runs_paginated(&state.pool, per_page, offset)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to fetch job runs");
            AppError::DatabaseError(e.to_string())
        })?;

    let total_count = queries::count_job_runs(&state.pool)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to count job runs");
            AppError::DatabaseError(e.to_string())
        })?;

    let total_pages = (total_count as f64 / per_page as f64).ceil() as i64;

    let template = JobRunsTemplate {
        user: None, // TODO: Add authentication
        job_runs,
        current_page: page,
        total_pages,
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

#[derive(Debug, Deserialize)]
pub struct PaginationParams {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

/// Get the job runs list (HTMX, with pagination)
#[instrument(skip(state))]
pub async fn get_job_runs_list(
    State(state): State<AppState>,
    Query(params): Query<PaginationParams>,
) -> Result<Html<String>, AppError> {
    info!("Fetching job runs list");

    let page = params.page.unwrap_or(1);
    let per_page = params.per_page.unwrap_or(50);
    let offset = (page - 1) * per_page;

    let job_runs = queries::list_job_runs_paginated(&state.pool, per_page, offset)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to fetch job runs");
            AppError::DatabaseError(e.to_string())
        })?;

    let total_count = queries::count_job_runs(&state.pool)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to count job runs");
            AppError::DatabaseError(e.to_string())
        })?;

    let total_pages = (total_count as f64 / per_page as f64).ceil() as i64;

    let template = JobRunListTemplate {
        job_runs,
        current_page: page,
        total_pages,
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

/// Get detailed view of a single job run
#[instrument(skip(state))]
pub async fn get_job_run_detail(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Html<String>, AppError> {
    info!(job_run_id = id, "Fetching job run detail");

    let job_run = queries::get_job_run_by_id(&state.pool, id)
        .await
        .map_err(|e| {
            error!(job_run_id = id, error = %e, "Failed to fetch job run");
            AppError::DatabaseError(e.to_string())
        })?
        .ok_or_else(|| {
            warn!(job_run_id = id, "Job run not found");
            AppError::NotFound(format!("Job run {} not found", id))
        })?;

    // Get server results for this job run
    let server_results = server_job_results::list_results_for_job_run(&state.pool, id)
        .await
        .map_err(|e| {
            error!(job_run_id = id, error = %e, "Failed to fetch server results");
            AppError::DatabaseError(e.to_string())
        })?;

    // Get server names
    let servers = server_queries::list_servers(&state.pool)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to fetch servers");
            AppError::DatabaseError(e.to_string())
        })?;

    let template = JobRunDetailTemplate {
        user: None, // TODO: Add authentication
        job_run,
        server_results,
        servers,
    };

    let html = template.render().map_err(|e| {
        error!(error = %e, "Failed to render job run detail template");
        AppError::TemplateError(e.to_string())
    })?;

    Ok(Html(html))
}

/// Get server-specific results for a job run (HTMX)
#[instrument(skip(state))]
pub async fn get_job_run_results(
    State(state): State<AppState>,
    Path(job_run_id): Path<i64>,
) -> Result<Html<String>, AppError> {
    info!(job_run_id, "Fetching server results for job run");

    let server_results = server_job_results::list_results_for_job_run(&state.pool, job_run_id)
        .await
        .map_err(|e| {
            error!(job_run_id, error = %e, "Failed to fetch server results");
            AppError::DatabaseError(e.to_string())
        })?;

    // Get server names
    let servers = server_queries::list_servers(&state.pool)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to fetch servers");
            AppError::DatabaseError(e.to_string())
        })?;

    let template = ServerJobResultsTemplate {
        job_run_id,
        server_results,
        servers,
    };

    let html = template.render().map_err(|e| {
        error!(error = %e, "Failed to render server job results template");
        AppError::TemplateError(e.to_string())
    })?;

    Ok(Html(html))
}

/// Get single server result with detailed logs (HTMX)
#[instrument(skip(state))]
pub async fn get_server_result_detail(
    State(state): State<AppState>,
    Path((job_run_id, server_id)): Path<(i64, i64)>,
) -> Result<Html<String>, AppError> {
    info!(job_run_id, server_id, "Fetching server result detail");

    // Get the server result
    let server_result = server_job_results::get_result_by_job_run_and_server(
        &state.pool,
        job_run_id,
        server_id,
    )
    .await
    .map_err(|e| {
        error!(job_run_id, server_id, error = %e, "Failed to fetch server result");
        AppError::DatabaseError(e.to_string())
    })?
    .ok_or_else(|| {
        warn!(job_run_id, server_id, "Server result not found");
        AppError::NotFound(format!(
            "Server result for job run {} and server {} not found",
            job_run_id, server_id
        ))
    })?;

    // Get server details
    let server = server_queries::get_server_by_id(&state.pool, server_id)
        .await
        .map_err(|e| {
            error!(server_id, error = %e, "Failed to fetch server");
            AppError::DatabaseError(e.to_string())
        })?
        .ok_or_else(|| {
            warn!(server_id, "Server not found");
            AppError::NotFound(format!("Server {} not found", server_id))
        })?;

    let template = ServerJobResultDetailTemplate {
        job_run_id,
        server,
        server_result,
    };

    let html = template.render().map_err(|e| {
        error!(error = %e, "Failed to render server result detail template");
        AppError::TemplateError(e.to_string())
    })?;

    Ok(Html(html))
}

// ============================================================================
// Action Routes
// ============================================================================

/// Cancel a running job (if supported)
#[instrument(skip(state))]
pub async fn cancel_job_run(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Html<String>, AppError> {
    info!(job_run_id = id, "Attempting to cancel job run");

    // Get job run
    let job_run = queries::get_job_run_by_id(&state.pool, id)
        .await
        .map_err(|e| {
            error!(job_run_id = id, error = %e, "Failed to fetch job run");
            AppError::DatabaseError(e.to_string())
        })?
        .ok_or_else(|| {
            warn!(job_run_id = id, "Job run not found");
            AppError::NotFound(format!("Job run {} not found", id))
        })?;

    // Check if job is in a cancellable state
    if job_run.status != "running" && job_run.status != "pending" {
        warn!(
            job_run_id = id,
            status = %job_run.status,
            "Job run is not in a cancellable state"
        );
        return Ok(Html(format!(
            r#"<div class="alert alert-warning">
                Job run is in '{}' state and cannot be cancelled.
            </div>"#,
            job_run.status
        )));
    }

    // Update job run status to cancelled
    queries::update_job_run_status(&state.pool, id, "cancelled")
        .await
        .map_err(|e| {
            error!(job_run_id = id, error = %e, "Failed to cancel job run");
            AppError::DatabaseError(e.to_string())
        })?;

    // Set end time
    queries::set_job_run_end_time(&state.pool, id, Utc::now())
        .await
        .map_err(|e| {
            error!(job_run_id = id, error = %e, "Failed to set end time");
            AppError::DatabaseError(e.to_string())
        })?;

    info!(job_run_id = id, "Job run cancelled successfully");

    // TODO: Actually signal the running job to stop
    // This would require integration with the job execution engine

    // Return success message
    Ok(Html(format!(
        r#"<div class="alert alert-success">
            Job run cancelled successfully. <a href="/runs/{}">View details</a>
        </div>"#,
        id
    )))
}

// ============================================================================
// Filter Routes
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct FilterParams {
    pub status: Option<String>,
    pub job_template_id: Option<i64>,
    pub trigger_type: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

/// Get filtered job runs list (HTMX)
#[instrument(skip(state))]
pub async fn get_filtered_job_runs(
    State(state): State<AppState>,
    Query(params): Query<FilterParams>,
) -> Result<Html<String>, AppError> {
    info!("Fetching filtered job runs");

    let page = params.page.unwrap_or(1);
    let per_page = params.per_page.unwrap_or(50);
    let offset = (page - 1) * per_page;

    // Build filter query
    // This is a simplified version - you'd want to add proper filtering to the database layer
    let mut job_runs = queries::list_job_runs_paginated(&state.pool, per_page, offset)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to fetch job runs");
            AppError::DatabaseError(e.to_string())
        })?;

    // Apply filters in memory (should be done in database for performance)
    if let Some(status) = &params.status {
        job_runs.retain(|jr| &jr.status == status);
    }

    if let Some(job_template_id) = params.job_template_id {
        job_runs.retain(|jr| jr.job_template_id == job_template_id);
    }

    if let Some(trigger_type) = &params.trigger_type {
        job_runs.retain(|jr| &jr.trigger_type == trigger_type);
    }

    // Date filtering would require parsing the date strings
    // and comparing with job_run.started_at

    let total_count = job_runs.len() as i64;
    let total_pages = (total_count as f64 / per_page as f64).ceil() as i64;

    let template = JobRunListTemplate {
        job_runs,
        current_page: page,
        total_pages,
        per_page,
    };

    let html = template.render().map_err(|e| {
        error!(error = %e, "Failed to render filtered job run list");
        AppError::TemplateError(e.to_string())
    })?;

    Ok(Html(html))
}

// ============================================================================
// Statistics Routes
// ============================================================================

/// Get job run statistics (HTMX)
#[instrument(skip(state))]
pub async fn get_job_run_stats(
    State(state): State<AppState>,
) -> Result<Html<String>, AppError> {
    info!("Fetching job run statistics");

    // Get recent job runs (last 24 hours, last 7 days, etc.)
    let all_runs = queries::list_job_runs_paginated(&state.pool, 1000, 0)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to fetch job runs for statistics");
            AppError::DatabaseError(e.to_string())
        })?;

    let now = Utc::now();
    let day_ago = now - chrono::Duration::hours(24);
    let week_ago = now - chrono::Duration::days(7);

    let last_24h: Vec<_> = all_runs
        .iter()
        .filter(|jr| jr.started_at >= day_ago)
        .collect();

    let last_7d: Vec<_> = all_runs
        .iter()
        .filter(|jr| jr.started_at >= week_ago)
        .collect();

    let stats_html = format!(
        r#"<div class="stats-grid">
            <div class="stat-card">
                <h3>Last 24 Hours</h3>
                <p class="stat-value">{}</p>
                <p class="stat-label">Total Runs</p>
            </div>
            <div class="stat-card">
                <h3>Last 7 Days</h3>
                <p class="stat-value">{}</p>
                <p class="stat-label">Total Runs</p>
            </div>
            <div class="stat-card">
                <h3>Success Rate (24h)</h3>
                <p class="stat-value">{}%</p>
                <p class="stat-label">Successful Jobs</p>
            </div>
            <div class="stat-card">
                <h3>All Time</h3>
                <p class="stat-value">{}</p>
                <p class="stat-label">Total Runs</p>
            </div>
        </div>"#,
        last_24h.len(),
        last_7d.len(),
        calculate_success_rate(&last_24h),
        all_runs.len()
    );

    Ok(Html(stats_html))
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Calculate success rate from job runs
fn calculate_success_rate(job_runs: &[&JobRun]) -> i64 {
    if job_runs.is_empty() {
        return 0;
    }

    let successful = job_runs
        .iter()
        .filter(|jr| jr.status == "completed" || jr.status == "success")
        .count();

    ((successful as f64 / job_runs.len() as f64) * 100.0) as i64
}
