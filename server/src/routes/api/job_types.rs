//! Job Types API endpoints
//!
//! Provides CRUD operations for managing job types and command templates.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;
use svrctlrs_database::{
    queries::job_types, CreateCommandTemplate, CreateJobType, UpdateCommandTemplate, UpdateJobType,
};
use tracing::{error, info, instrument};

use super::ApiError;
use crate::state::AppState;

/// Create job types router
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list_job_types).post(create_job_type))
        .route(
            "/{id}",
            get(get_job_type)
                .put(update_job_type)
                .delete(delete_job_type),
        )
        .route(
            "/{id}/command-templates",
            get(list_command_templates).post(create_command_template),
        )
        .route(
            "/command-templates/{template_id}",
            get(get_command_template)
                .put(update_command_template)
                .delete(delete_command_template),
        )
}

/// List all job types
#[instrument(skip(state))]
async fn list_job_types(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    let types = job_types::list_job_types(&state.pool).await.map_err(|e| {
        error!(error = %e, "Failed to list job types");
        ApiError::internal_error(format!("Failed to list job types: {}", e))
    })?;

    Ok(Json(json!({ "job_types": types })))
}

/// Get job type by ID with command templates
#[instrument(skip(state))]
async fn get_job_type(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    let job_type = job_types::get_job_type(&state.pool, id)
        .await
        .map_err(|e| {
            error!(error = %e, id = id, "Failed to get job type");
            ApiError::not_found("Job type")
        })?;

    let templates = job_types::get_command_templates(&state.pool, id)
        .await
        .unwrap_or_default();

    Ok(Json(json!({
        "job_type": job_type,
        "command_templates": templates
    })))
}

/// Create job type input
#[derive(Debug, Deserialize)]
struct CreateJobTypeInput {
    name: String,
    display_name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    icon: Option<String>,
    #[serde(default)]
    color: Option<String>,
    #[serde(default)]
    requires_capabilities: Option<Vec<String>>,
    #[serde(default = "default_enabled")]
    enabled: bool,
}

fn default_enabled() -> bool {
    true
}

/// Create a new job type
#[instrument(skip(state, input))]
async fn create_job_type(
    State(state): State<AppState>,
    Json(input): Json<CreateJobTypeInput>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    info!(name = %input.name, "Creating job type");

    let create = CreateJobType {
        name: input.name,
        display_name: input.display_name,
        description: input.description,
        icon: input.icon,
        color: input.color,
        requires_capabilities: input.requires_capabilities,
        metadata: None,
        enabled: input.enabled,
    };

    let id = job_types::create_job_type(&state.pool, &create)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to create job type");
            ApiError::internal_error(format!("Failed to create job type: {}", e))
        })?;

    info!(id = id, "Job type created successfully");

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "id": id,
            "message": "Job type created successfully"
        })),
    ))
}

/// Update job type input
#[derive(Debug, Deserialize)]
struct UpdateJobTypeInput {
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    icon: Option<String>,
    #[serde(default)]
    color: Option<String>,
    #[serde(default)]
    requires_capabilities: Option<Vec<String>>,
    #[serde(default)]
    enabled: Option<bool>,
}

/// Update an existing job type
#[instrument(skip(state, input))]
async fn update_job_type(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(input): Json<UpdateJobTypeInput>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    info!(id = id, "Updating job type");

    // Verify exists
    job_types::get_job_type(&state.pool, id)
        .await
        .map_err(|_| ApiError::not_found("Job type"))?;

    let update = UpdateJobType {
        display_name: input.display_name,
        description: input.description,
        icon: input.icon,
        color: input.color,
        requires_capabilities: input.requires_capabilities,
        metadata: None,
        enabled: input.enabled,
    };

    job_types::update_job_type(&state.pool, id, &update)
        .await
        .map_err(|e| {
            error!(error = %e, id = id, "Failed to update job type");
            ApiError::internal_error(format!("Failed to update job type: {}", e))
        })?;

    info!(id = id, "Job type updated successfully");

    Ok(Json(json!({
        "id": id,
        "message": "Job type updated successfully"
    })))
}

/// Delete a job type
#[instrument(skip(state))]
async fn delete_job_type(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    info!(id = id, "Deleting job type");

    job_types::delete_job_type(&state.pool, id)
        .await
        .map_err(|e| {
            error!(error = %e, id = id, "Failed to delete job type");
            if e.to_string().contains("in use") {
                ApiError::conflict(
                    "Cannot delete job type: it is in use by one or more job templates",
                )
            } else {
                ApiError::internal_error(format!("Failed to delete job type: {}", e))
            }
        })?;

    info!(id = id, "Job type deleted successfully");

    Ok(Json(json!({
        "id": id,
        "message": "Job type deleted successfully"
    })))
}

// === Command Templates ===

/// List command templates for a job type
#[instrument(skip(state))]
async fn list_command_templates(
    State(state): State<AppState>,
    Path(job_type_id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    // Verify job type exists
    job_types::get_job_type(&state.pool, job_type_id)
        .await
        .map_err(|_| ApiError::not_found("Job type"))?;

    let templates = job_types::get_command_templates(&state.pool, job_type_id)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to list command templates");
            ApiError::internal_error(format!("Failed to list command templates: {}", e))
        })?;

    Ok(Json(json!({ "command_templates": templates })))
}

/// Get command template by ID
#[instrument(skip(state))]
async fn get_command_template(
    State(state): State<AppState>,
    Path(template_id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    let template = job_types::get_command_template(&state.pool, template_id)
        .await
        .map_err(|e| {
            error!(error = %e, template_id = template_id, "Failed to get command template");
            ApiError::not_found("Command template")
        })?;

    Ok(Json(json!({ "command_template": template })))
}

/// Create command template input
#[derive(Debug, Deserialize)]
struct CreateCommandTemplateInput {
    // job_type_id comes from the path parameter, not request body
    name: String,
    display_name: String,
    command: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    required_capabilities: Option<Vec<String>>,
    #[serde(default = "default_timeout")]
    timeout_seconds: i32,
    #[serde(default)]
    working_directory: Option<String>,
    #[serde(default)]
    notify_on_success: bool,
    #[serde(default = "default_notify_failure")]
    notify_on_failure: bool,
    #[serde(default)]
    parameter_schema: Option<serde_json::Value>,
}

fn default_timeout() -> i32 {
    300
}

fn default_notify_failure() -> bool {
    true
}

/// Create a new command template
#[instrument(skip(state, input))]
async fn create_command_template(
    State(state): State<AppState>,
    Path(job_type_id): Path<i64>,
    Json(input): Json<CreateCommandTemplateInput>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    info!(name = %input.name, job_type_id = job_type_id, "Creating command template");

    // Verify job type exists
    job_types::get_job_type(&state.pool, job_type_id)
        .await
        .map_err(|_| ApiError::not_found("Job type"))?;

    let create = CreateCommandTemplate {
        job_type_id,
        name: input.name,
        display_name: input.display_name,
        description: input.description,
        command: input.command,
        required_capabilities: input.required_capabilities,
        os_filter: None,
        timeout_seconds: input.timeout_seconds,
        working_directory: input.working_directory,
        environment: None,
        output_format: None,
        parse_output: false,
        output_parser: None,
        notify_on_success: input.notify_on_success,
        notify_on_failure: input.notify_on_failure,
        parameter_schema: input.parameter_schema,
        metadata: None,
    };

    let id = job_types::create_command_template(&state.pool, &create)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to create command template");
            ApiError::internal_error(format!("Failed to create command template: {}", e))
        })?;

    info!(id = id, "Command template created successfully");

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "id": id,
            "message": "Command template created successfully"
        })),
    ))
}

/// Update command template input
#[derive(Debug, Deserialize)]
struct UpdateCommandTemplateInput {
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    command: Option<String>,
    #[serde(default)]
    required_capabilities: Option<Vec<String>>,
    #[serde(default)]
    timeout_seconds: Option<i32>,
    #[serde(default)]
    working_directory: Option<String>,
    #[serde(default)]
    notify_on_success: Option<bool>,
    #[serde(default)]
    notify_on_failure: Option<bool>,
    #[serde(default)]
    parameter_schema: Option<serde_json::Value>,
}

/// Update an existing command template
#[instrument(skip(state, input))]
async fn update_command_template(
    State(state): State<AppState>,
    Path(template_id): Path<i64>,
    Json(input): Json<UpdateCommandTemplateInput>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    info!(template_id = template_id, "Updating command template");

    // Verify exists
    job_types::get_command_template(&state.pool, template_id)
        .await
        .map_err(|_| ApiError::not_found("Command template"))?;

    let update = UpdateCommandTemplate {
        display_name: input.display_name,
        description: input.description,
        command: input.command,
        required_capabilities: input.required_capabilities,
        os_filter: None,
        timeout_seconds: input.timeout_seconds,
        working_directory: input.working_directory,
        environment: None,
        output_format: None,
        parse_output: None,
        output_parser: None,
        notify_on_success: input.notify_on_success,
        notify_on_failure: input.notify_on_failure,
        parameter_schema: input.parameter_schema,
        metadata: None,
    };

    job_types::update_command_template(&state.pool, template_id, &update)
        .await
        .map_err(|e| {
            error!(error = %e, template_id = template_id, "Failed to update command template");
            ApiError::internal_error(format!("Failed to update command template: {}", e))
        })?;

    info!(
        template_id = template_id,
        "Command template updated successfully"
    );

    Ok(Json(json!({
        "id": template_id,
        "message": "Command template updated successfully"
    })))
}

/// Delete a command template
#[instrument(skip(state))]
async fn delete_command_template(
    State(state): State<AppState>,
    Path(template_id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    info!(template_id = template_id, "Deleting command template");

    job_types::delete_command_template(&state.pool, template_id)
        .await
        .map_err(|e| {
            error!(error = %e, template_id = template_id, "Failed to delete command template");
            if e.to_string().contains("in use") {
                ApiError::conflict(
                    "Cannot delete command template: it is in use by one or more job templates",
                )
            } else {
                ApiError::internal_error(format!("Failed to delete command template: {}", e))
            }
        })?;

    info!(
        template_id = template_id,
        "Command template deleted successfully"
    );

    Ok(Json(json!({
        "id": template_id,
        "message": "Command template deleted successfully"
    })))
}
