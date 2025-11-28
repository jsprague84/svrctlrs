use askama::Template;
use axum::{
    extract::{Path, State},
    response::Html,
    Form,
};
use serde::Deserialize;
use svrctlrs_database::{
    models::{JobTemplate, JobTemplateStep},
    queries::{job_templates as queries, job_template_steps as step_queries, job_types},
};
use tracing::{error, info, instrument, warn};

use crate::{
    routes::ui::AppError,
    state::AppState,
    templates::{
        JobTemplateFormTemplate, JobTemplateListTemplate, JobTemplatesTemplate,
        JobTemplateStepFormTemplate, JobTemplateStepListTemplate,
    },
};

// ============================================================================
// Page Routes
// ============================================================================

/// Display the job templates management page
#[instrument(skip(state))]
pub async fn job_templates_page(
    State(state): State<AppState>,
) -> Result<Html<String>, AppError> {
    info!("Rendering job templates page");

    let job_templates = queries::list_job_templates(&state.pool)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to fetch job templates");
            AppError::DatabaseError(e.to_string())
        })?;

    let job_types = job_types::list_job_types(&state.pool)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to fetch job types");
            AppError::DatabaseError(e.to_string())
        })?;

    let template = JobTemplatesTemplate {
        user: None, // TODO: Add authentication
        job_templates,
        job_types,
    };

    let html = template.render().map_err(|e| {
        error!(error = %e, "Failed to render job templates template");
        AppError::TemplateError(e.to_string())
    })?;

    Ok(Html(html))
}

// ============================================================================
// HTMX List Routes
// ============================================================================

/// Get the job templates list (HTMX)
#[instrument(skip(state))]
pub async fn get_job_templates_list(
    State(state): State<AppState>,
) -> Result<Html<String>, AppError> {
    info!("Fetching job templates list");

    let job_templates = queries::list_job_templates(&state.pool)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to fetch job templates");
            AppError::DatabaseError(e.to_string())
        })?;

    let template = JobTemplateListTemplate { job_templates };

    let html = template.render().map_err(|e| {
        error!(error = %e, "Failed to render job template list template");
        AppError::TemplateError(e.to_string())
    })?;

    Ok(Html(html))
}

// ============================================================================
// Form Routes
// ============================================================================

/// Show the new job template form (HTMX)
#[instrument(skip(state))]
pub async fn new_job_template_form(
    State(state): State<AppState>,
) -> Result<Html<String>, AppError> {
    info!("Rendering new job template form");

    let job_types = job_types::list_job_types(&state.pool)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to fetch job types");
            AppError::DatabaseError(e.to_string())
        })?;

    let template = JobTemplateFormTemplate {
        job_template: None,
        job_types,
        error: None,
    };

    let html = template.render().map_err(|e| {
        error!(error = %e, "Failed to render job template form template");
        AppError::TemplateError(e.to_string())
    })?;

    Ok(Html(html))
}

/// Show the edit job template form (HTMX)
#[instrument(skip(state))]
pub async fn edit_job_template_form(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Html<String>, AppError> {
    info!(job_template_id = id, "Rendering edit job template form");

    let job_template = queries::get_job_template(&state.pool, id)
        .await
        .map_err(|e| {
            warn!(job_template_id = id, error = %e, "Job template not found");
            AppError::NotFound(format!("Job template {} not found", id))
        })?;

    let job_types = job_types::list_job_types(&state.pool)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to fetch job types");
            AppError::DatabaseError(e.to_string())
        })?;

    let template = JobTemplateFormTemplate {
        job_template: Some(job_template),
        job_types,
        error: None,
    };

    let html = template.render().map_err(|e| {
        error!(error = %e, "Failed to render job template form template");
        AppError::TemplateError(e.to_string())
    })?;

    Ok(Html(html))
}

// ============================================================================
// CRUD Operations
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct CreateJobTemplateInput {
    pub name: String,
    pub description: Option<String>,
    pub job_type_id: i64,
    pub timeout_seconds: Option<i64>,
    pub retry_count: Option<i64>,
    pub retry_delay_seconds: Option<i64>,
    pub continue_on_failure: Option<String>, // "on" or absent
    pub parallel_execution: Option<String>,  // "on" or absent
}

/// Create a new job template
#[instrument(skip(state))]
pub async fn create_job_template(
    State(state): State<AppState>,
    Form(input): Form<CreateJobTemplateInput>,
) -> Result<Html<String>, AppError> {
    info!(name = %input.name, "Creating job template");

    // Validate input
    if input.name.trim().is_empty() {
        warn!("Job template name is empty");
        let job_types = job_types::list_job_types(&state.pool)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        let template = JobTemplateFormTemplate {
            job_template: None,
            job_types,
            error: Some("Name is required".to_string()),
        };
        let html = template.render().map_err(|e| {
            error!(error = %e, "Failed to render error template");
            AppError::TemplateError(e.to_string())
        })?;
        return Ok(Html(html));
    }

    // Verify job type exists
    let job_type = job_types::get_job_type(&state.pool, input.job_type_id)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to verify job type");
            AppError::DatabaseError(e.to_string())
        })?
        .ok_or_else(|| {
            warn!(job_type_id = input.job_type_id, "Job type not found");
            AppError::NotFound(format!("Job type {} not found", input.job_type_id))
        })?;

    // Create job template
    let job_template_id = queries::create_job_template(
        &state.pool,
        &input.name,
        input.description.as_deref(),
        input.job_type_id,
        input.timeout_seconds,
        input.retry_count,
        input.retry_delay_seconds,
        input.continue_on_failure.is_some(),
        input.parallel_execution.is_some(),
    )
    .await
    .map_err(|e| {
        error!(error = %e, "Failed to create job template");
        AppError::DatabaseError(e.to_string())
    })?;

    info!(
        job_template_id,
        name = %input.name,
        "Job template created successfully"
    );

    // Return updated list
    get_job_templates_list(State(state)).await
}

#[derive(Debug, Deserialize)]
pub struct UpdateJobTemplateInput {
    pub name: String,
    pub description: Option<String>,
    pub job_type_id: i64,
    pub timeout_seconds: Option<i64>,
    pub retry_count: Option<i64>,
    pub retry_delay_seconds: Option<i64>,
    pub continue_on_failure: Option<String>,
    pub parallel_execution: Option<String>,
}

/// Update an existing job template
#[instrument(skip(state))]
pub async fn update_job_template(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Form(input): Form<UpdateJobTemplateInput>,
) -> Result<Html<String>, AppError> {
    info!(job_template_id = id, name = %input.name, "Updating job template");

    // Validate input
    if input.name.trim().is_empty() {
        warn!("Job template name is empty");
        let job_template = queries::get_job_template(&state.pool, id)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        let job_types = job_types::list_job_types(&state.pool)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        let template = JobTemplateFormTemplate {
            job_template,
            job_types,
            error: Some("Name is required".to_string()),
        };
        let html = template.render().map_err(|e| {
            error!(error = %e, "Failed to render error template");
            AppError::TemplateError(e.to_string())
        })?;
        return Ok(Html(html));
    }

    // Verify job type exists
    job_types::get_job_type(&state.pool, input.job_type_id)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to verify job type");
            AppError::DatabaseError(e.to_string())
        })?
        .ok_or_else(|| {
            warn!(job_type_id = input.job_type_id, "Job type not found");
            AppError::NotFound(format!("Job type {} not found", input.job_type_id))
        })?;

    // Update job template
    queries::update_job_template(
        &state.pool,
        id,
        &input.name,
        input.description.as_deref(),
        input.job_type_id,
        input.timeout_seconds,
        input.retry_count,
        input.retry_delay_seconds,
        input.continue_on_failure.is_some(),
        input.parallel_execution.is_some(),
    )
    .await
    .map_err(|e| {
        error!(job_template_id = id, error = %e, "Failed to update job template");
        AppError::DatabaseError(e.to_string())
    })?;

    info!(job_template_id = id, "Job template updated successfully");

    // Return updated list
    get_job_templates_list(State(state)).await
}

/// Delete a job template
#[instrument(skip(state))]
pub async fn delete_job_template(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Html<String>, AppError> {
    info!(job_template_id = id, "Deleting job template");

    // Check if job template exists
    let job_template = queries::get_job_template(&state.pool, id)
        .await
        .map_err(|e| {
            warn!(job_template_id = id, error = %e, "Job template not found");
            AppError::NotFound(format!("Job template {} not found", id))
        })?;

    // Delete job template
    queries::delete_job_template(&state.pool, id)
        .await
        .map_err(|e| {
            error!(job_template_id = id, error = %e, "Failed to delete job template");
            AppError::DatabaseError(e.to_string())
        })?;

    info!(
        job_template_id = id,
        name = %job_template.name,
        "Job template deleted successfully"
    );

    // Return updated list
    get_job_templates_list(State(state)).await
}

// ============================================================================
// Job Template Steps Routes
// ============================================================================

/// Get steps for a job template (HTMX)
#[instrument(skip(state))]
pub async fn get_template_steps(
    State(state): State<AppState>,
    Path(template_id): Path<i64>,
) -> Result<Html<String>, AppError> {
    info!(template_id, "Fetching template steps");

    let steps = step_queries::list_template_steps(&state.pool, template_id)
        .await
        .map_err(|e| {
            error!(template_id, error = %e, "Failed to fetch template steps");
            AppError::DatabaseError(e.to_string())
        })?;

    let template = JobTemplateStepListTemplate {
        job_template_id: template_id,
        steps,
    };

    let html = template.render().map_err(|e| {
        error!(error = %e, "Failed to render template step list");
        AppError::TemplateError(e.to_string())
    })?;

    Ok(Html(html))
}

#[derive(Debug, Deserialize)]
pub struct CreateTemplateStepInput {
    pub step_order: i64,
    pub step_name: String,
    pub step_command: String,
    pub step_description: Option<String>,
    pub timeout_seconds: Option<i64>,
    pub continue_on_failure: Option<String>, // "on" or absent
}

/// Add a step to a job template
#[instrument(skip(state))]
pub async fn create_template_step(
    State(state): State<AppState>,
    Path(template_id): Path<i64>,
    Form(input): Form<CreateTemplateStepInput>,
) -> Result<Html<String>, AppError> {
    info!(template_id, step_name = %input.step_name, "Creating template step");

    // Validate input
    if input.step_name.trim().is_empty() || input.step_command.trim().is_empty() {
        warn!("Step name or command is empty");
        return Err(AppError::ValidationError(
            "Name and command are required".to_string(),
        ));
    }

    // Verify template exists
    queries::get_job_template(&state.pool, template_id)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to verify job template");
            AppError::DatabaseError(e.to_string())
        })?
        .ok_or_else(|| {
            warn!(template_id, "Job template not found");
            AppError::NotFound(format!("Job template {} not found", template_id))
        })?;

    // Create step
    step_queries::create_template_step(
        &state.pool,
        template_id,
        input.step_order,
        &input.step_name,
        &input.step_command,
        input.step_description.as_deref(),
        input.timeout_seconds,
        input.continue_on_failure.is_some(),
    )
    .await
    .map_err(|e| {
        error!(template_id, error = %e, "Failed to create template step");
        AppError::DatabaseError(e.to_string())
    })?;

    info!(template_id, "Template step created successfully");

    // Return updated list
    get_template_steps(State(state), Path(template_id)).await
}

#[derive(Debug, Deserialize)]
pub struct UpdateTemplateStepInput {
    pub step_order: i64,
    pub step_name: String,
    pub step_command: String,
    pub step_description: Option<String>,
    pub timeout_seconds: Option<i64>,
    pub continue_on_failure: Option<String>,
}

/// Update a template step
#[instrument(skip(state))]
pub async fn update_template_step(
    State(state): State<AppState>,
    Path((template_id, step_id)): Path<(i64, i64)>,
    Form(input): Form<UpdateTemplateStepInput>,
) -> Result<Html<String>, AppError> {
    info!(template_id, step_id, step_name = %input.step_name, "Updating template step");

    // Validate input
    if input.step_name.trim().is_empty() || input.step_command.trim().is_empty() {
        warn!("Step name or command is empty");
        return Err(AppError::ValidationError(
            "Name and command are required".to_string(),
        ));
    }

    // Update step
    step_queries::update_template_step(
        &state.pool,
        step_id,
        input.step_order,
        &input.step_name,
        &input.step_command,
        input.step_description.as_deref(),
        input.timeout_seconds,
        input.continue_on_failure.is_some(),
    )
    .await
    .map_err(|e| {
        error!(step_id, error = %e, "Failed to update template step");
        AppError::DatabaseError(e.to_string())
    })?;

    info!(step_id, "Template step updated successfully");

    // Return updated list
    get_template_steps(State(state), Path(template_id)).await
}

/// Delete a template step
#[instrument(skip(state))]
pub async fn delete_template_step(
    State(state): State<AppState>,
    Path((template_id, step_id)): Path<(i64, i64)>,
) -> Result<Html<String>, AppError> {
    info!(template_id, step_id, "Deleting template step");

    // Delete step
    step_queries::delete_template_step(&state.pool, step_id)
        .await
        .map_err(|e| {
            error!(step_id, error = %e, "Failed to delete template step");
            AppError::DatabaseError(e.to_string())
        })?;

    info!(step_id, "Template step deleted successfully");

    // Return updated list
    get_template_steps(State(state), Path(template_id)).await
}

#[derive(Debug, Deserialize)]
pub struct ReorderStepsInput {
    pub step_ids: String, // Comma-separated list of step IDs in new order
}

/// Reorder template steps
#[instrument(skip(state))]
pub async fn reorder_template_steps(
    State(state): State<AppState>,
    Path(template_id): Path<i64>,
    Form(input): Form<ReorderStepsInput>,
) -> Result<Html<String>, AppError> {
    info!(template_id, "Reordering template steps");

    // Parse step IDs
    let step_ids: Vec<i64> = input
        .step_ids
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();

    if step_ids.is_empty() {
        warn!("No valid step IDs provided");
        return Err(AppError::ValidationError(
            "No valid step IDs provided".to_string(),
        ));
    }

    // Update each step's order
    for (index, step_id) in step_ids.iter().enumerate() {
        let order = (index + 1) as i64;

        // Get current step
        let step = step_queries::get_template_step_by_id(&state.pool, *step_id)
            .await
            .map_err(|e| {
                error!(step_id, error = %e, "Failed to fetch template step");
                AppError::DatabaseError(e.to_string())
            })?
            .ok_or_else(|| {
                warn!(step_id, "Template step not found");
                AppError::NotFound(format!("Template step {} not found", step_id))
            })?;

        // Update order
        step_queries::update_template_step(
            &state.pool,
            *step_id,
            order,
            &step.step_name,
            &step.step_command,
            step.step_description.as_deref(),
            step.timeout_seconds,
            step.continue_on_failure,
        )
        .await
        .map_err(|e| {
            error!(step_id, error = %e, "Failed to reorder template step");
            AppError::DatabaseError(e.to_string())
        })?;
    }

    info!(template_id, "Template steps reordered successfully");

    // Return updated list
    get_template_steps(State(state), Path(template_id)).await
}
