use askama::Template;
use axum::{
    extract::{Path, State},
    response::Html,
    routing::{get, put},
    Form, Router,
};
use serde::Deserialize;
use svrctlrs_database::{
    models::{CreateJobTemplate, CreateJobTemplateStep, UpdateJobTemplate, UpdateJobTemplateStep},
    queries::job_templates as queries,
    queries::job_types,
};
use tracing::{error, info, instrument, warn};

use crate::{
    routes::ui::AppError,
    state::AppState,
    templates::{
        JobTemplateFormTemplate, JobTemplateListTemplate, JobTemplateStepFormTemplate,
        JobTemplateStepsTemplate, JobTemplatesTemplate,
    },
};

/// Create router with all job template routes
pub fn routes() -> Router<AppState> {
    Router::new()
        // Main page
        .route(
            "/job-templates",
            get(job_templates_page).post(create_job_template),
        )
        // List endpoint
        .route("/job-templates/list", get(get_job_templates_list))
        // Form endpoints
        .route("/job-templates/new", get(new_job_template_form))
        .route("/job-templates/{id}/edit", get(edit_job_template_form))
        // CRUD endpoints
        .route(
            "/job-templates/{id}",
            put(update_job_template).delete(delete_job_template),
        )
        // Template steps
        .route(
            "/job-templates/{template_id}/steps",
            get(get_template_steps).post(create_template_step),
        )
        .route(
            "/job-templates/{template_id}/steps/new",
            get(new_template_step_form),
        )
        .route(
            "/job-templates/{template_id}/steps/{step_id}/edit",
            get(edit_template_step_form),
        )
        .route(
            "/job-templates/{template_id}/steps/{step_id}",
            put(update_template_step).delete(delete_template_step),
        )
}

// ============================================================================
// Page Routes
// ============================================================================

/// Display the job templates management page
#[instrument(skip(state))]
pub async fn job_templates_page(State(state): State<AppState>) -> Result<Html<String>, AppError> {
    info!("Rendering job templates page");

    let job_templates = queries::list_job_templates(&state.pool)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to fetch job templates");
            AppError::DatabaseError(e.to_string())
        })?;

    let job_types = job_types::list_job_types(&state.pool).await.map_err(|e| {
        error!(error = %e, "Failed to fetch job types");
        AppError::DatabaseError(e.to_string())
    })?;

    let template = JobTemplatesTemplate {
        user: None, // TODO: Add authentication
        job_templates: job_templates.into_iter().map(Into::into).collect(),
        job_types: job_types.into_iter().map(Into::into).collect(),
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

    let template = JobTemplateListTemplate {
        job_templates: job_templates.into_iter().map(Into::into).collect(),
    };

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

    let job_types = job_types::list_job_types(&state.pool).await.map_err(|e| {
        error!(error = %e, "Failed to fetch job types");
        AppError::DatabaseError(e.to_string())
    })?;

    // Get all command templates (not filtered by job type)
    let command_templates_list: Vec<svrctlrs_database::CommandTemplate> =
        svrctlrs_database::sqlx::query_as("SELECT * FROM command_templates ORDER BY name")
            .fetch_all(&state.pool)
            .await
            .map_err(|e| {
                error!(error = %e, "Failed to fetch command templates");
                AppError::DatabaseError(e.to_string())
            })?;

    let template = JobTemplateFormTemplate {
        job_template: None,
        job_types: job_types.into_iter().map(Into::into).collect(),
        command_templates: command_templates_list.into_iter().map(Into::into).collect(),
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

    let job_types = job_types::list_job_types(&state.pool).await.map_err(|e| {
        error!(error = %e, "Failed to fetch job types");
        AppError::DatabaseError(e.to_string())
    })?;

    let command_templates_list = svrctlrs_database::sqlx::query_as::<
        _,
        svrctlrs_database::CommandTemplate,
    >("SELECT * FROM command_templates ORDER BY name")
    .fetch_all(&state.pool)
    .await
    .map_err(|e| {
        error!(error = %e, "Failed to fetch command templates");
        AppError::DatabaseError(e.to_string())
    })?;

    let template = JobTemplateFormTemplate {
        job_template: Some(job_template.into()),
        job_types: job_types.into_iter().map(Into::into).collect(),
        command_templates: command_templates_list.into_iter().map(Into::into).collect(),
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
    pub display_name: String,
    pub description: Option<String>,
    pub job_type_id: i64,
    pub is_composite: Option<String>, // "on" or absent
    pub command_template_id: Option<i64>,
    pub timeout_seconds: Option<i32>,
    pub retry_count: Option<i32>,
    pub retry_delay_seconds: Option<i32>,
    pub notify_on_success: Option<String>,
    pub notify_on_failure: Option<String>,
}

/// Create a new job template
#[instrument(skip(state))]
pub async fn create_job_template(
    State(state): State<AppState>,
    Form(input): Form<CreateJobTemplateInput>,
) -> Result<Html<String>, AppError> {
    info!(name = %input.name, "Creating new job template");

    // Validate input
    if input.name.trim().is_empty() || input.display_name.trim().is_empty() {
        warn!("Job template name or display name is empty");
        return Err(AppError::ValidationError(
            "Name and display name are required".to_string(),
        ));
    }

    // Verify job type exists
    job_types::get_job_type(&state.pool, input.job_type_id)
        .await
        .map_err(|e| {
            warn!(job_type_id = input.job_type_id, error = %e, "Job type not found");
            AppError::NotFound(format!("Job type {} not found", input.job_type_id))
        })?;

    // Create job template
    let create_input = CreateJobTemplate {
        name: input.name.clone(),
        display_name: input.display_name,
        description: input.description,
        job_type_id: input.job_type_id,
        is_composite: input.is_composite.is_some(),
        command_template_id: input.command_template_id,
        variables: None,
        timeout_seconds: input.timeout_seconds.unwrap_or(300),
        retry_count: input.retry_count.unwrap_or(0),
        retry_delay_seconds: input.retry_delay_seconds.unwrap_or(30),
        notify_on_success: input.notify_on_success.is_some(),
        notify_on_failure: input.notify_on_failure.is_some(),
        notification_policy_id: None,
        metadata: None,
    };

    queries::create_job_template(&state.pool, &create_input)
        .await
        .map_err(|e| {
            error!(name = %input.name, error = %e, "Failed to create job template");
            AppError::DatabaseError(e.to_string())
        })?;

    info!(
        name = %input.name,
        "Job template created successfully"
    );

    // Return updated list
    get_job_templates_list(State(state)).await
}

#[derive(Debug, Deserialize)]
pub struct UpdateJobTemplateInput {
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub command_template_id: Option<i64>,
    pub timeout_seconds: Option<i32>,
    pub retry_count: Option<i32>,
    pub retry_delay_seconds: Option<i32>,
    pub notify_on_success: Option<String>,
    pub notify_on_failure: Option<String>,
}

/// Update an existing job template
#[instrument(skip(state))]
pub async fn update_job_template(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Form(input): Form<UpdateJobTemplateInput>,
) -> Result<Html<String>, AppError> {
    info!(job_template_id = id, "Updating job template");

    // Validate input
    if let Some(ref display_name) = input.display_name {
        if display_name.trim().is_empty() {
            warn!("Job template display name is empty");
            return Err(AppError::ValidationError(
                "Display name cannot be empty".to_string(),
            ));
        }
    }

    // Update job template
    let update_input = UpdateJobTemplate {
        display_name: input.display_name,
        description: input.description,
        command_template_id: input.command_template_id,
        variables: None,
        timeout_seconds: input.timeout_seconds,
        retry_count: input.retry_count,
        retry_delay_seconds: input.retry_delay_seconds,
        notify_on_success: Some(input.notify_on_success.is_some()),
        notify_on_failure: Some(input.notify_on_failure.is_some()),
        notification_policy_id: None,
        metadata: None,
    };

    queries::update_job_template(&state.pool, id, &update_input)
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
    let _job_template = queries::get_job_template(&state.pool, id)
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

    info!(job_template_id = id, "Job template deleted successfully");

    // Return empty response (HTMX will remove the element)
    Ok(Html(String::new()))
}

// ============================================================================
// Job Template Steps
// ============================================================================

/// Get template steps list (HTMX)
#[instrument(skip(state))]
pub async fn get_template_steps(
    State(state): State<AppState>,
    Path(template_id): Path<i64>,
) -> Result<Html<String>, AppError> {
    info!(template_id, "Fetching template steps");

    let steps = queries::get_job_template_steps(&state.pool, template_id)
        .await
        .map_err(|e| {
            error!(template_id, error = %e, "Failed to fetch template steps");
            AppError::DatabaseError(e.to_string())
        })?;

    let template = JobTemplateStepsTemplate {
        job_template_id: template_id,
        steps: steps.into_iter().map(Into::into).collect(),
    };

    let html = template.render().map_err(|e| {
        error!(error = %e, "Failed to render template steps");
        AppError::TemplateError(e.to_string())
    })?;

    Ok(Html(html))
}

/// Show new template step form (HTMX)
#[instrument(skip(state))]
pub async fn new_template_step_form(
    State(state): State<AppState>,
    Path(template_id): Path<i64>,
) -> Result<Html<String>, AppError> {
    info!(template_id, "Rendering new template step form");

    let job_types = job_types::list_job_types(&state.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

    // Get command templates
    let command_templates = svrctlrs_database::sqlx::query_as::<
        _,
        svrctlrs_database::CommandTemplate,
    >("SELECT * FROM command_templates ORDER BY name")
    .fetch_all(&state.pool)
    .await
    .map_err(|e| AppError::DatabaseError(e.to_string()))?;

    // Get next order index
    let steps = queries::get_job_template_steps(&state.pool, template_id)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

    let next_order = steps.len() as i32 + 1;

    let template = JobTemplateStepFormTemplate {
        job_template_id: template_id,
        step: None,
        command_templates: command_templates.into_iter().map(Into::into).collect(),
        job_types: job_types.into_iter().map(Into::into).collect(),
        next_order_index: next_order,
        error: None,
    };

    let html = template.render().map_err(|e| {
        error!(error = %e, "Failed to render template step form");
        AppError::TemplateError(e.to_string())
    })?;

    Ok(Html(html))
}

/// Show edit template step form (HTMX)
#[instrument(skip(state))]
pub async fn edit_template_step_form(
    State(state): State<AppState>,
    Path((template_id, step_id)): Path<(i64, i64)>,
) -> Result<Html<String>, AppError> {
    info!(template_id, step_id, "Rendering edit template step form");

    let step = queries::get_job_template_step(&state.pool, step_id)
        .await
        .map_err(|e| {
            warn!(step_id, error = %e, "Template step not found");
            AppError::NotFound(format!("Template step {} not found", step_id))
        })?;

    let job_types = job_types::list_job_types(&state.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

    let command_templates = svrctlrs_database::sqlx::query_as::<
        _,
        svrctlrs_database::CommandTemplate,
    >("SELECT * FROM command_templates ORDER BY name")
    .fetch_all(&state.pool)
    .await
    .map_err(|e| AppError::DatabaseError(e.to_string()))?;

    let template = JobTemplateStepFormTemplate {
        job_template_id: template_id,
        step: Some(step.into()),
        command_templates: command_templates.into_iter().map(Into::into).collect(),
        job_types: job_types.into_iter().map(Into::into).collect(),
        next_order_index: 0, // Not used for edit
        error: None,
    };

    let html = template.render().map_err(|e| {
        error!(error = %e, "Failed to render template step form");
        AppError::TemplateError(e.to_string())
    })?;

    Ok(Html(html))
}

#[derive(Debug, Deserialize)]
pub struct CreateTemplateStepInput {
    pub name: String,
    pub command_template_id: i64,
    pub step_order: i32,
    pub continue_on_failure: Option<String>,
    pub timeout_seconds: Option<i32>,
}

/// Create a new template step
#[instrument(skip(state))]
pub async fn create_template_step(
    State(state): State<AppState>,
    Path(template_id): Path<i64>,
    Form(input): Form<CreateTemplateStepInput>,
) -> Result<Html<String>, AppError> {
    info!(template_id, name = %input.name, "Creating template step");

    if input.name.trim().is_empty() {
        return Err(AppError::ValidationError(
            "Step name is required".to_string(),
        ));
    }

    let create_input = CreateJobTemplateStep {
        job_template_id: template_id,
        step_order: input.step_order,
        name: input.name,
        command_template_id: input.command_template_id,
        variables: None,
        continue_on_failure: input.continue_on_failure.is_some(),
        timeout_seconds: input.timeout_seconds,
        metadata: None,
    };

    queries::create_job_template_step(&state.pool, &create_input)
        .await
        .map_err(|e| {
            error!(template_id, error = %e, "Failed to create template step");
            AppError::DatabaseError(e.to_string())
        })?;

    info!(template_id, "Template step created successfully");

    // Return updated steps list
    get_template_steps(State(state), Path(template_id)).await
}

#[derive(Debug, Deserialize)]
pub struct UpdateTemplateStepInput {
    pub name: Option<String>,
    pub command_template_id: Option<i64>,
    pub step_order: Option<i32>,
    pub continue_on_failure: Option<String>,
    pub timeout_seconds: Option<i32>,
}

/// Update a template step
#[instrument(skip(state))]
pub async fn update_template_step(
    State(state): State<AppState>,
    Path((template_id, step_id)): Path<(i64, i64)>,
    Form(input): Form<UpdateTemplateStepInput>,
) -> Result<Html<String>, AppError> {
    info!(template_id, step_id, "Updating template step");

    let update_input = UpdateJobTemplateStep {
        step_order: input.step_order,
        name: input.name,
        command_template_id: input.command_template_id,
        variables: None,
        continue_on_failure: Some(input.continue_on_failure.is_some()),
        timeout_seconds: input.timeout_seconds,
        metadata: None,
    };

    queries::update_job_template_step(&state.pool, step_id, &update_input)
        .await
        .map_err(|e| {
            error!(step_id, error = %e, "Failed to update template step");
            AppError::DatabaseError(e.to_string())
        })?;

    info!(step_id, "Template step updated successfully");

    // Return updated steps list
    get_template_steps(State(state), Path(template_id)).await
}

/// Delete a template step
#[instrument(skip(state))]
pub async fn delete_template_step(
    State(state): State<AppState>,
    Path((template_id, step_id)): Path<(i64, i64)>,
) -> Result<Html<String>, AppError> {
    info!(template_id, step_id, "Deleting template step");

    queries::delete_job_template_step(&state.pool, step_id)
        .await
        .map_err(|e| {
            error!(step_id, error = %e, "Failed to delete template step");
            AppError::DatabaseError(e.to_string())
        })?;

    info!(step_id, "Template step deleted successfully");

    // Return empty response (HTMX will remove the element)
    Ok(Html(String::new()))
}
