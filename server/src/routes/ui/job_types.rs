use askama::Template;
use axum::{
    extract::{Path, State},
    response::Html,
    Form,
};
use serde::Deserialize;
use svrctlrs_database::{
    models::{JobType, JobTypeCommandTemplate},
    queries::{job_types as queries, command_templates as template_queries},
};
use tracing::{error, info, instrument, warn};

use crate::{
    routes::ui::AppError,
    state::AppState,
    templates::{
        JobTypeFormTemplate, JobTypeListTemplate, JobTypesTemplate,
        CommandTemplateFormTemplate, CommandTemplateListTemplate,
    },
};

// ============================================================================
// Page Routes
// ============================================================================

/// Display the job types management page
#[instrument(skip(state))]
pub async fn job_types_page(State(state): State<AppState>) -> Result<Html<String>, AppError> {
    info!("Rendering job types page");

    let job_types = queries::list_job_types(&state.pool)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to fetch job types");
            AppError::DatabaseError(e.to_string())
        })?;

    let template = JobTypesTemplate {
        user: None, // TODO: Add authentication
        job_types,
    };

    let html = template.render().map_err(|e| {
        error!(error = %e, "Failed to render job types template");
        AppError::TemplateError(e.to_string())
    })?;

    Ok(Html(html))
}

// ============================================================================
// HTMX List Routes
// ============================================================================

/// Get the job types list (HTMX)
#[instrument(skip(state))]
pub async fn get_job_types_list(
    State(state): State<AppState>,
) -> Result<Html<String>, AppError> {
    info!("Fetching job types list");

    let job_types = queries::list_job_types(&state.pool)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to fetch job types");
            AppError::DatabaseError(e.to_string())
        })?;

    let template = JobTypeListTemplate { job_types };

    let html = template.render().map_err(|e| {
        error!(error = %e, "Failed to render job type list template");
        AppError::TemplateError(e.to_string())
    })?;

    Ok(Html(html))
}

// ============================================================================
// Form Routes
// ============================================================================

/// Show the new job type form (HTMX)
#[instrument(skip(_state))]
pub async fn new_job_type_form(State(_state): State<AppState>) -> Result<Html<String>, AppError> {
    info!("Rendering new job type form");

    let template = JobTypeFormTemplate {
        job_type: None,
        error: None,
    };

    let html = template.render().map_err(|e| {
        error!(error = %e, "Failed to render job type form template");
        AppError::TemplateError(e.to_string())
    })?;

    Ok(Html(html))
}

/// Show the edit job type form (HTMX)
#[instrument(skip(state))]
pub async fn edit_job_type_form(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Html<String>, AppError> {
    info!(job_type_id = id, "Rendering edit job type form");

    let job_type = queries::get_job_type(&state.pool, id)
        .await
        .map_err(|e| {
            warn!(job_type_id = id, error = %e, "Job type not found");
            AppError::NotFound(format!("Job type {} not found", id))
        })?;

    let template = JobTypeFormTemplate {
        job_type: Some(job_type),
        error: None,
    };

    let html = template.render().map_err(|e| {
        error!(error = %e, "Failed to render job type form template");
        AppError::TemplateError(e.to_string())
    })?;

    Ok(Html(html))
}

// ============================================================================
// CRUD Operations
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct CreateJobTypeInput {
    pub name: String,
    pub description: Option<String>,
    pub execution_type: String, // "simple", "composite", "script"
    pub default_timeout_seconds: Option<i64>,
    pub requires_confirmation: Option<String>, // "on" or absent
    pub icon: Option<String>,
    pub color: Option<String>,
}

/// Create a new job type
#[instrument(skip(state))]
pub async fn create_job_type(
    State(state): State<AppState>,
    Form(input): Form<CreateJobTypeInput>,
) -> Result<Html<String>, AppError> {
    info!(name = %input.name, "Creating job type");

    // Validate input
    if input.name.trim().is_empty() {
        warn!("Job type name is empty");
        let template = JobTypeFormTemplate {
            job_type: None,
            error: Some("Name is required".to_string()),
        };
        let html = template.render().map_err(|e| {
            error!(error = %e, "Failed to render error template");
            AppError::TemplateError(e.to_string())
        })?;
        return Ok(Html(html));
    }

    // Validate execution type
    if !["simple", "composite", "script"].contains(&input.execution_type.as_str()) {
        warn!(execution_type = %input.execution_type, "Invalid execution type");
        let template = JobTypeFormTemplate {
            job_type: None,
            error: Some("Invalid execution type".to_string()),
        };
        let html = template.render().map_err(|e| {
            error!(error = %e, "Failed to render error template");
            AppError::TemplateError(e.to_string())
        })?;
        return Ok(Html(html));
    }

    // Create job type
    let job_type_id = queries::create_job_type(
        &state.pool,
        &input.name,
        input.description.as_deref(),
        &input.execution_type,
        input.default_timeout_seconds,
        input.requires_confirmation.is_some(),
        input.icon.as_deref(),
        input.color.as_deref(),
    )
    .await
    .map_err(|e| {
        error!(error = %e, "Failed to create job type");
        AppError::DatabaseError(e.to_string())
    })?;

    info!(job_type_id, name = %input.name, "Job type created successfully");

    // Return updated list
    get_job_types_list(State(state)).await
}

#[derive(Debug, Deserialize)]
pub struct UpdateJobTypeInput {
    pub name: String,
    pub description: Option<String>,
    pub execution_type: String,
    pub default_timeout_seconds: Option<i64>,
    pub requires_confirmation: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
}

/// Update an existing job type
#[instrument(skip(state))]
pub async fn update_job_type(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Form(input): Form<UpdateJobTypeInput>,
) -> Result<Html<String>, AppError> {
    info!(job_type_id = id, name = %input.name, "Updating job type");

    // Validate input
    if input.name.trim().is_empty() {
        warn!("Job type name is empty");
        let job_type = queries::get_job_type(&state.pool, id)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        let template = JobTypeFormTemplate {
            job_type,
            error: Some("Name is required".to_string()),
        };
        let html = template.render().map_err(|e| {
            error!(error = %e, "Failed to render error template");
            AppError::TemplateError(e.to_string())
        })?;
        return Ok(Html(html));
    }

    // Validate execution type
    if !["simple", "composite", "script"].contains(&input.execution_type.as_str()) {
        warn!(execution_type = %input.execution_type, "Invalid execution type");
        let job_type = queries::get_job_type(&state.pool, id)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        let template = JobTypeFormTemplate {
            job_type,
            error: Some("Invalid execution type".to_string()),
        };
        let html = template.render().map_err(|e| {
            error!(error = %e, "Failed to render error template");
            AppError::TemplateError(e.to_string())
        })?;
        return Ok(Html(html));
    }

    // Update job type
    queries::update_job_type(
        &state.pool,
        id,
        &input.name,
        input.description.as_deref(),
        &input.execution_type,
        input.default_timeout_seconds,
        input.requires_confirmation.is_some(),
        input.icon.as_deref(),
        input.color.as_deref(),
    )
    .await
    .map_err(|e| {
        error!(job_type_id = id, error = %e, "Failed to update job type");
        AppError::DatabaseError(e.to_string())
    })?;

    info!(job_type_id = id, "Job type updated successfully");

    // Return updated list
    get_job_types_list(State(state)).await
}

/// Delete a job type
#[instrument(skip(state))]
pub async fn delete_job_type(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Html<String>, AppError> {
    info!(job_type_id = id, "Deleting job type");

    // Check if job type exists
    let job_type = queries::get_job_type(&state.pool, id)
        .await
        .map_err(|e| {
            warn!(job_type_id = id, error = %e, "Job type not found");
            AppError::NotFound(format!("Job type {} not found", id))
        })?;

    // Delete job type
    queries::delete_job_type(&state.pool, id)
        .await
        .map_err(|e| {
            error!(job_type_id = id, error = %e, "Failed to delete job type");
            AppError::DatabaseError(e.to_string())
        })?;

    info!(job_type_id = id, name = %job_type.name, "Job type deleted successfully");

    // Return updated list
    get_job_types_list(State(state)).await
}

// ============================================================================
// Command Template Routes
// ============================================================================

/// Get command templates for a job type (HTMX)
#[instrument(skip(state))]
pub async fn get_command_templates(
    State(state): State<AppState>,
    Path(job_type_id): Path<i64>,
) -> Result<Html<String>, AppError> {
    info!(job_type_id, "Fetching command templates");

    let templates = template_queries::list_command_templates_for_job_type(&state.pool, job_type_id)
        .await
        .map_err(|e| {
            error!(job_type_id, error = %e, "Failed to fetch command templates");
            AppError::DatabaseError(e.to_string())
        })?;

    let template = CommandTemplateListTemplate {
        job_type_id,
        templates,
    };

    let html = template.render().map_err(|e| {
        error!(error = %e, "Failed to render command template list");
        AppError::TemplateError(e.to_string())
    })?;

    Ok(Html(html))
}

#[derive(Debug, Deserialize)]
pub struct CreateCommandTemplateInput {
    pub name: String,
    pub command: String,
    pub description: Option<String>,
    pub order_index: Option<i64>,
}

/// Add a command template to a job type
#[instrument(skip(state))]
pub async fn create_command_template(
    State(state): State<AppState>,
    Path(job_type_id): Path<i64>,
    Form(input): Form<CreateCommandTemplateInput>,
) -> Result<Html<String>, AppError> {
    info!(job_type_id, name = %input.name, "Creating command template");

    // Validate input
    if input.name.trim().is_empty() || input.command.trim().is_empty() {
        warn!("Command template name or command is empty");
        return Err(AppError::ValidationError(
            "Name and command are required".to_string(),
        ));
    }

    // Create command template
    template_queries::create_command_template(
        &state.pool,
        job_type_id,
        &input.name,
        &input.command,
        input.description.as_deref(),
        input.order_index.unwrap_or(0),
    )
    .await
    .map_err(|e| {
        error!(job_type_id, error = %e, "Failed to create command template");
        AppError::DatabaseError(e.to_string())
    })?;

    info!(job_type_id, "Command template created successfully");

    // Return updated list
    get_command_templates(State(state), Path(job_type_id)).await
}

#[derive(Debug, Deserialize)]
pub struct UpdateCommandTemplateInput {
    pub name: String,
    pub command: String,
    pub description: Option<String>,
    pub order_index: Option<i64>,
}

/// Update a command template
#[instrument(skip(state))]
pub async fn update_command_template(
    State(state): State<AppState>,
    Path(template_id): Path<i64>,
    Form(input): Form<UpdateCommandTemplateInput>,
) -> Result<Html<String>, AppError> {
    info!(template_id, name = %input.name, "Updating command template");

    // Get template to find job_type_id
    let template = template_queries::get_command_template_by_id(&state.pool, template_id)
        .await
        .map_err(|e| {
            error!(template_id, error = %e, "Failed to fetch command template");
            AppError::DatabaseError(e.to_string())
        })?
        .ok_or_else(|| {
            warn!(template_id, "Command template not found");
            AppError::NotFound(format!("Command template {} not found", template_id))
        })?;

    // Validate input
    if input.name.trim().is_empty() || input.command.trim().is_empty() {
        warn!("Command template name or command is empty");
        return Err(AppError::ValidationError(
            "Name and command are required".to_string(),
        ));
    }

    // Update command template
    template_queries::update_command_template(
        &state.pool,
        template_id,
        &input.name,
        &input.command,
        input.description.as_deref(),
        input.order_index.unwrap_or(0),
    )
    .await
    .map_err(|e| {
        error!(template_id, error = %e, "Failed to update command template");
        AppError::DatabaseError(e.to_string())
    })?;

    info!(template_id, "Command template updated successfully");

    // Return updated list
    get_command_templates(State(state), Path(template.job_type_id)).await
}

/// Delete a command template
#[instrument(skip(state))]
pub async fn delete_command_template(
    State(state): State<AppState>,
    Path(template_id): Path<i64>,
) -> Result<Html<String>, AppError> {
    info!(template_id, "Deleting command template");

    // Get template to find job_type_id
    let template = template_queries::get_command_template_by_id(&state.pool, template_id)
        .await
        .map_err(|e| {
            error!(template_id, error = %e, "Failed to fetch command template");
            AppError::DatabaseError(e.to_string())
        })?
        .ok_or_else(|| {
            warn!(template_id, "Command template not found");
            AppError::NotFound(format!("Command template {} not found", template_id))
        })?;

    let job_type_id = template.job_type_id;

    // Delete command template
    template_queries::delete_command_template(&state.pool, template_id)
        .await
        .map_err(|e| {
            error!(template_id, error = %e, "Failed to delete command template");
            AppError::DatabaseError(e.to_string())
        })?;

    info!(template_id, "Command template deleted successfully");

    // Return updated list
    get_command_templates(State(state), Path(job_type_id)).await
}
