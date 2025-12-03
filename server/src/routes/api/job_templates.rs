//! Job Templates API endpoints
//!
//! Provides CRUD operations for managing job templates and executing jobs.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;
use svrctlrs_database::{queries::job_templates, CreateJobTemplate, UpdateJobTemplate};
use tracing::{error, info, instrument};

use super::ApiError;
use crate::state::AppState;

/// Create job templates router
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list_job_templates).post(create_job_template))
        .route(
            "/{id}",
            get(get_job_template)
                .put(update_job_template)
                .delete(delete_job_template),
        )
        .route("/{id}/run", post(run_job_template))
}

/// List all job templates
#[instrument(skip(state))]
async fn list_job_templates(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    let templates = job_templates::list_job_templates(&state.pool)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to list job templates");
            ApiError::internal_error(format!("Failed to list job templates: {}", e))
        })?;

    Ok(Json(json!({ "job_templates": templates })))
}

/// Get job template by ID with steps (for composite templates)
#[instrument(skip(state))]
async fn get_job_template(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    let template = job_templates::get_job_template(&state.pool, id)
        .await
        .map_err(|e| {
            error!(error = %e, id = id, "Failed to get job template");
            ApiError::not_found("Job template")
        })?;

    // Get steps if composite
    let steps = if template.is_composite {
        job_templates::get_job_template_steps(&state.pool, id)
            .await
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    Ok(Json(json!({
        "job_template": template,
        "steps": steps
    })))
}

/// Create job template input
#[derive(Debug, Deserialize)]
struct CreateJobTemplateInput {
    name: String,
    display_name: String,
    job_type_id: i64,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    command_template_id: Option<i64>,
    #[serde(default)]
    is_composite: bool,
    #[serde(default)]
    variables: Option<std::collections::HashMap<String, String>>,
    #[serde(default = "default_timeout")]
    timeout_seconds: i32,
    #[serde(default)]
    retry_count: i32,
    #[serde(default = "default_retry_delay")]
    retry_delay_seconds: i32,
    #[serde(default)]
    notify_on_success: bool,
    #[serde(default = "default_notify_on_failure")]
    notify_on_failure: bool,
    #[serde(default)]
    notification_policy_id: Option<i64>,
}

fn default_timeout() -> i32 {
    300
}

fn default_retry_delay() -> i32 {
    60
}

fn default_notify_on_failure() -> bool {
    true
}

/// Create a new job template
#[instrument(skip(state, input))]
async fn create_job_template(
    State(state): State<AppState>,
    Json(input): Json<CreateJobTemplateInput>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    info!(name = %input.name, "Creating job template");

    let create = CreateJobTemplate {
        name: input.name,
        display_name: input.display_name,
        job_type_id: input.job_type_id,
        description: input.description,
        command_template_id: input.command_template_id,
        is_composite: input.is_composite,
        variables: input.variables,
        timeout_seconds: input.timeout_seconds,
        retry_count: input.retry_count,
        retry_delay_seconds: input.retry_delay_seconds,
        notify_on_success: input.notify_on_success,
        notify_on_failure: input.notify_on_failure,
        notification_policy_id: input.notification_policy_id,
        metadata: None,
    };

    let id = job_templates::create_job_template(&state.pool, &create)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to create job template");
            ApiError::internal_error(format!("Failed to create job template: {}", e))
        })?;

    info!(id = id, "Job template created successfully");

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "id": id,
            "message": "Job template created successfully"
        })),
    ))
}

/// Update job template input
#[derive(Debug, Deserialize)]
struct UpdateJobTemplateInput {
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    command_template_id: Option<i64>,
    #[serde(default)]
    variables: Option<std::collections::HashMap<String, String>>,
    #[serde(default)]
    timeout_seconds: Option<i32>,
    #[serde(default)]
    retry_count: Option<i32>,
    #[serde(default)]
    retry_delay_seconds: Option<i32>,
    #[serde(default)]
    notify_on_success: Option<bool>,
    #[serde(default)]
    notify_on_failure: Option<bool>,
    #[serde(default)]
    notification_policy_id: Option<i64>,
}

/// Update an existing job template
#[instrument(skip(state, input))]
async fn update_job_template(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(input): Json<UpdateJobTemplateInput>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    info!(id = id, "Updating job template");

    // Verify exists
    job_templates::get_job_template(&state.pool, id)
        .await
        .map_err(|_| ApiError::not_found("Job template"))?;

    let update = UpdateJobTemplate {
        display_name: input.display_name,
        description: input.description,
        command_template_id: input.command_template_id,
        variables: input.variables,
        timeout_seconds: input.timeout_seconds,
        retry_count: input.retry_count,
        retry_delay_seconds: input.retry_delay_seconds,
        notify_on_success: input.notify_on_success,
        notify_on_failure: input.notify_on_failure,
        notification_policy_id: input.notification_policy_id,
        metadata: None,
    };

    job_templates::update_job_template(&state.pool, id, &update)
        .await
        .map_err(|e| {
            error!(error = %e, id = id, "Failed to update job template");
            ApiError::internal_error(format!("Failed to update job template: {}", e))
        })?;

    info!(id = id, "Job template updated successfully");

    Ok(Json(json!({
        "id": id,
        "message": "Job template updated successfully"
    })))
}

/// Delete a job template
#[instrument(skip(state))]
async fn delete_job_template(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    info!(id = id, "Deleting job template");

    job_templates::delete_job_template(&state.pool, id)
        .await
        .map_err(|e| {
            error!(error = %e, id = id, "Failed to delete job template");
            if e.to_string().contains("in use") {
                ApiError::conflict(
                    "Cannot delete job template: it has associated schedules or job runs",
                )
            } else {
                ApiError::internal_error(format!("Failed to delete job template: {}", e))
            }
        })?;

    info!(id = id, "Job template deleted successfully");

    Ok(Json(json!({
        "id": id,
        "message": "Job template deleted successfully"
    })))
}

/// Run job template input
#[derive(Debug, Deserialize)]
struct RunJobTemplateInput {
    server_id: i64,
    #[serde(default)]
    variables: Option<serde_json::Value>,
}

/// Execute a job template immediately
#[instrument(skip(state, input))]
async fn run_job_template(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(input): Json<RunJobTemplateInput>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    info!(id = id, server_id = input.server_id, "Running job template");

    use svrctlrs_database::queries::{job_runs, job_templates as jt_queries, servers};

    // Verify template exists
    let template = jt_queries::get_job_template(&state.pool, id)
        .await
        .map_err(|_| ApiError::not_found("Job template"))?;

    // Verify server exists
    let server = servers::get_server(&state.pool, input.server_id)
        .await
        .map_err(|_| ApiError::not_found("Server"))?;

    // Create job run record (using 0 for schedule_id when manually triggered)
    // The create_job_run function takes individual parameters
    let metadata = input
        .variables
        .as_ref()
        .map(|v| serde_json::json!({"triggered_by": "api", "variables": v}).to_string());

    let job_run_id = job_runs::create_job_run(
        &state.pool,
        0, // No schedule, manually triggered
        id,
        input.server_id,
        0,     // retry_attempt
        false, // is_retry
        metadata,
    )
    .await
    .map_err(|e| {
        error!(error = %e, "Failed to create job run");
        ApiError::internal_error(format!("Failed to create job run: {}", e))
    })?;

    // Broadcast job run creation
    state.broadcast_job_run_update(crate::state::JobRunUpdate::Created { job_run_id });

    info!(
        job_run_id = job_run_id,
        template_name = %template.name,
        server_name = %server.name,
        "Job run initiated via API"
    );

    Ok((
        StatusCode::ACCEPTED,
        Json(json!({
            "job_run_id": job_run_id,
            "message": "Job execution initiated",
            "template_name": template.name,
            "server_name": server.name
        })),
    ))
}
