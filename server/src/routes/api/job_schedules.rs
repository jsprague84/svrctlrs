//! Job Schedules API endpoints
//!
//! Provides CRUD operations for managing job schedules.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;
use svrctlrs_database::{queries::job_schedules, CreateJobSchedule, UpdateJobSchedule};
use tracing::{error, info, instrument};

use super::ApiError;
use crate::state::AppState;

/// Create job schedules router
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list_job_schedules).post(create_job_schedule))
        .route(
            "/{id}",
            get(get_job_schedule)
                .put(update_job_schedule)
                .delete(delete_job_schedule),
        )
        .route("/{id}/toggle", post(toggle_job_schedule))
}

/// List all job schedules
#[instrument(skip(state))]
async fn list_job_schedules(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    let schedules = job_schedules::list_job_schedules(&state.pool)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to list job schedules");
            ApiError::internal_error(format!("Failed to list job schedules: {}", e))
        })?;

    Ok(Json(json!({ "job_schedules": schedules })))
}

/// Get job schedule by ID
#[instrument(skip(state))]
async fn get_job_schedule(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    let schedule = job_schedules::get_job_schedule(&state.pool, id)
        .await
        .map_err(|e| {
            error!(error = %e, id = id, "Failed to get job schedule");
            ApiError::not_found("Job schedule")
        })?;

    Ok(Json(json!({ "job_schedule": schedule })))
}

/// Create job schedule input
#[derive(Debug, Deserialize)]
struct CreateJobScheduleInput {
    name: String,
    job_template_id: i64,
    server_id: i64,
    schedule: String, // Cron expression
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    timeout_seconds: Option<i32>,
    #[serde(default)]
    retry_count: Option<i32>,
    #[serde(default)]
    notify_on_success: Option<bool>,
    #[serde(default)]
    notify_on_failure: Option<bool>,
    #[serde(default)]
    notification_policy_id: Option<i64>,
    #[serde(default = "default_enabled")]
    enabled: bool,
}

fn default_enabled() -> bool {
    true
}

/// Validate cron expression (basic field count validation)
fn validate_cron(schedule: &str) -> Result<(), String> {
    let fields: Vec<&str> = schedule.split_whitespace().collect();
    if fields.len() != 5 && fields.len() != 6 {
        return Err(format!(
            "Invalid cron expression: expected 5 or 6 fields, got {}",
            fields.len()
        ));
    }
    Ok(())
}

/// Create a new job schedule
#[instrument(skip(state, input))]
async fn create_job_schedule(
    State(state): State<AppState>,
    Json(input): Json<CreateJobScheduleInput>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    info!(name = %input.name, schedule = %input.schedule, "Creating job schedule");

    // Validate cron expression
    if let Err(msg) = validate_cron(&input.schedule) {
        return Err(ApiError::bad_request(msg));
    }

    let create = CreateJobSchedule {
        name: input.name,
        job_template_id: input.job_template_id,
        server_id: input.server_id,
        schedule: input.schedule,
        description: input.description,
        timeout_seconds: input.timeout_seconds,
        retry_count: input.retry_count,
        notify_on_success: input.notify_on_success,
        notify_on_failure: input.notify_on_failure,
        notification_policy_id: input.notification_policy_id,
        metadata: None,
        enabled: input.enabled,
    };

    let id = job_schedules::create_job_schedule(&state.pool, &create)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to create job schedule");
            ApiError::internal_error(format!("Failed to create job schedule: {}", e))
        })?;

    info!(id = id, "Job schedule created successfully");

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "id": id,
            "message": "Job schedule created successfully"
        })),
    ))
}

/// Update job schedule input
#[derive(Debug, Deserialize)]
struct UpdateJobScheduleInput {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    schedule: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    timeout_seconds: Option<i32>,
    #[serde(default)]
    retry_count: Option<i32>,
    #[serde(default)]
    notify_on_success: Option<bool>,
    #[serde(default)]
    notify_on_failure: Option<bool>,
    #[serde(default)]
    notification_policy_id: Option<i64>,
    #[serde(default)]
    enabled: Option<bool>,
}

/// Update an existing job schedule
#[instrument(skip(state, input))]
async fn update_job_schedule(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(input): Json<UpdateJobScheduleInput>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    info!(id = id, "Updating job schedule");

    // Verify exists
    job_schedules::get_job_schedule(&state.pool, id)
        .await
        .map_err(|_| ApiError::not_found("Job schedule"))?;

    // Validate cron expression if provided
    if let Some(ref schedule) = input.schedule {
        if let Err(msg) = validate_cron(schedule) {
            return Err(ApiError::bad_request(msg));
        }
    }

    let update = UpdateJobSchedule {
        name: input.name,
        schedule: input.schedule,
        description: input.description,
        timeout_seconds: input.timeout_seconds,
        retry_count: input.retry_count,
        notify_on_success: input.notify_on_success,
        notify_on_failure: input.notify_on_failure,
        notification_policy_id: input.notification_policy_id,
        next_run_at: None,
        metadata: None,
        enabled: input.enabled,
    };

    job_schedules::update_job_schedule(&state.pool, id, &update)
        .await
        .map_err(|e| {
            error!(error = %e, id = id, "Failed to update job schedule");
            ApiError::internal_error(format!("Failed to update job schedule: {}", e))
        })?;

    info!(id = id, "Job schedule updated successfully");

    Ok(Json(json!({
        "id": id,
        "message": "Job schedule updated successfully"
    })))
}

/// Delete a job schedule
#[instrument(skip(state))]
async fn delete_job_schedule(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    info!(id = id, "Deleting job schedule");

    job_schedules::delete_job_schedule(&state.pool, id)
        .await
        .map_err(|e| {
            error!(error = %e, id = id, "Failed to delete job schedule");
            ApiError::internal_error(format!("Failed to delete job schedule: {}", e))
        })?;

    info!(id = id, "Job schedule deleted successfully");

    Ok(Json(json!({
        "id": id,
        "message": "Job schedule deleted successfully"
    })))
}

/// Toggle job schedule enabled state
#[instrument(skip(state))]
async fn toggle_job_schedule(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    info!(id = id, "Toggling job schedule");

    // Get current state
    let schedule = job_schedules::get_job_schedule(&state.pool, id)
        .await
        .map_err(|_| ApiError::not_found("Job schedule"))?;

    let new_enabled = !schedule.enabled;

    // Manually construct UpdateJobSchedule with only enabled field set
    let update = UpdateJobSchedule {
        name: None,
        description: None,
        schedule: None,
        enabled: Some(new_enabled),
        timeout_seconds: None,
        retry_count: None,
        notify_on_success: None,
        notify_on_failure: None,
        notification_policy_id: None,
        next_run_at: None,
        metadata: None,
    };

    job_schedules::update_job_schedule(&state.pool, id, &update)
        .await
        .map_err(|e| {
            error!(error = %e, id = id, "Failed to toggle job schedule");
            ApiError::internal_error(format!("Failed to toggle job schedule: {}", e))
        })?;

    info!(id = id, enabled = new_enabled, "Job schedule toggled");

    Ok(Json(json!({
        "id": id,
        "enabled": new_enabled,
        "message": format!("Job schedule {}", if new_enabled { "enabled" } else { "disabled" })
    })))
}
