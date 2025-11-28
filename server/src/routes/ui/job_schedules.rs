use askama::Template;
use axum::{
    extract::{Path, State},
    response::Html,
    Form,
};
use chrono::Utc;
use serde::Deserialize;
use svrctlrs_database::{
    models::{JobSchedule, JobRun},
    queries::{
        job_schedules as queries, job_templates, job_runs, servers as server_queries,
    },
};
use tracing::{error, info, instrument, warn};

use crate::{
    routes::ui::AppError,
    state::AppState,
    templates::{
        JobScheduleFormTemplate, JobScheduleListTemplate, JobSchedulesTemplate,
        GroupedSchedulesTemplate,
    },
};

// ============================================================================
// Page Routes
// ============================================================================

/// Display the job schedules management page (replaces /tasks)
#[instrument(skip(state))]
pub async fn job_schedules_page(
    State(state): State<AppState>,
) -> Result<Html<String>, AppError> {
    info!("Rendering job schedules page");

    let schedules = queries::list_job_schedules(&state.pool)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to fetch job schedules");
            AppError::DatabaseError(e.to_string())
        })?;

    let job_templates = job_templates::list_job_templates(&state.pool)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to fetch job templates");
            AppError::DatabaseError(e.to_string())
        })?;

    let servers = server_queries::list_servers(&state.pool)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to fetch servers");
            AppError::DatabaseError(e.to_string())
        })?;

    let template = JobSchedulesTemplate {
        user: None, // TODO: Add authentication
        schedules,
        job_templates,
        servers,
    };

    let html = template.render().map_err(|e| {
        error!(error = %e, "Failed to render job schedules template");
        AppError::TemplateError(e.to_string())
    })?;

    Ok(Html(html))
}

// ============================================================================
// HTMX List Routes
// ============================================================================

/// Get the job schedules list (HTMX, grouped by server)
#[instrument(skip(state))]
pub async fn get_job_schedules_list(
    State(state): State<AppState>,
) -> Result<Html<String>, AppError> {
    info!("Fetching job schedules list");

    let schedules = queries::list_job_schedules(&state.pool)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to fetch job schedules");
            AppError::DatabaseError(e.to_string())
        })?;

    let servers = server_queries::list_servers(&state.pool)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to fetch servers");
            AppError::DatabaseError(e.to_string())
        })?;

    // Group schedules by server
    let mut grouped_schedules = std::collections::HashMap::new();
    for schedule in schedules {
        let server_name = if let Some(server_id) = schedule.server_id {
            servers
                .iter()
                .find(|s| s.id == server_id)
                .map(|s| s.name.clone())
                .unwrap_or_else(|| format!("Server {}", server_id))
        } else {
            "All Servers".to_string()
        };

        grouped_schedules
            .entry(server_name)
            .or_insert_with(Vec::new)
            .push(schedule);
    }

    let template = GroupedSchedulesTemplate {
        grouped_schedules,
    };

    let html = template.render().map_err(|e| {
        error!(error = %e, "Failed to render job schedule list template");
        AppError::TemplateError(e.to_string())
    })?;

    Ok(Html(html))
}

// ============================================================================
// Form Routes
// ============================================================================

/// Show the new job schedule form (HTMX)
#[instrument(skip(state))]
pub async fn new_job_schedule_form(
    State(state): State<AppState>,
) -> Result<Html<String>, AppError> {
    info!("Rendering new job schedule form");

    let job_templates = job_templates::list_job_templates(&state.pool)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to fetch job templates");
            AppError::DatabaseError(e.to_string())
        })?;

    let servers = server_queries::list_servers(&state.pool)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to fetch servers");
            AppError::DatabaseError(e.to_string())
        })?;

    let template = JobScheduleFormTemplate {
        job_schedule: None,
        job_templates,
        servers,
        error: None,
    };

    let html = template.render().map_err(|e| {
        error!(error = %e, "Failed to render job schedule form template");
        AppError::TemplateError(e.to_string())
    })?;

    Ok(Html(html))
}

/// Show the edit job schedule form (HTMX)
#[instrument(skip(state))]
pub async fn edit_job_schedule_form(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Html<String>, AppError> {
    info!(job_schedule_id = id, "Rendering edit job schedule form");

    let job_schedule = queries::get_job_schedule(&state.pool, id)
        .await
        .map_err(|e| {
            error!(job_schedule_id = id, error = %e, "Failed to fetch job schedule");
            AppError::DatabaseError(e.to_string())
        })?
        .ok_or_else(|| {
            warn!(job_schedule_id = id, "Job schedule not found");
            AppError::NotFound(format!("Job schedule {} not found", id))
        })?;

    let job_templates = job_templates::list_job_templates(&state.pool)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to fetch job templates");
            AppError::DatabaseError(e.to_string())
        })?;

    let servers = server_queries::list_servers(&state.pool)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to fetch servers");
            AppError::DatabaseError(e.to_string())
        })?;

    let template = JobScheduleFormTemplate {
        job_schedule: Some(job_schedule),
        job_templates,
        servers,
        error: None,
    };

    let html = template.render().map_err(|e| {
        error!(error = %e, "Failed to render job schedule form template");
        AppError::TemplateError(e.to_string())
    })?;

    Ok(Html(html))
}

// ============================================================================
// CRUD Operations
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct CreateJobScheduleInput {
    pub name: String,
    pub description: Option<String>,
    pub job_template_id: i64,
    pub server_id: Option<i64>, // None means all servers
    pub cron_expression: String,
    pub enabled: Option<String>, // "on" or absent
    pub max_retries: Option<i64>,
    pub retry_delay_seconds: Option<i64>,
}

/// Create a new job schedule
#[instrument(skip(state))]
pub async fn create_job_schedule(
    State(state): State<AppState>,
    Form(input): Form<CreateJobScheduleInput>,
) -> Result<Html<String>, AppError> {
    info!(name = %input.name, "Creating job schedule");

    // Validate input
    if input.name.trim().is_empty() {
        warn!("Job schedule name is empty");
        let job_templates = job_templates::list_job_templates(&state.pool)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        let servers = server_queries::list_servers(&state.pool)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        let template = JobScheduleFormTemplate {
            job_schedule: None,
            job_templates,
            servers,
            error: Some("Name is required".to_string()),
        };
        let html = template.render().map_err(|e| {
            error!(error = %e, "Failed to render error template");
            AppError::TemplateError(e.to_string())
        })?;
        return Ok(Html(html));
    }

    // Validate cron expression
    if let Err(e) = cron::Schedule::from_str(&input.cron_expression) {
        warn!(cron_expression = %input.cron_expression, error = %e, "Invalid cron expression");
        let job_templates = job_templates::list_job_templates(&state.pool)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        let servers = server_queries::list_servers(&state.pool)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        let template = JobScheduleFormTemplate {
            job_schedule: None,
            job_templates,
            servers,
            error: Some(format!("Invalid cron expression: {}", e)),
        };
        let html = template.render().map_err(|e| {
            error!(error = %e, "Failed to render error template");
            AppError::TemplateError(e.to_string())
        })?;
        return Ok(Html(html));
    }

    // Verify job template exists
    job_templates::get_job_template(&state.pool, input.job_template_id)
        .await
        .map_err(|e| {
            warn!(job_template_id = input.job_template_id, error = %e, "Job template not found");
            AppError::NotFound(format!("Job template {} not found", input.job_template_id))
        })?;

    // Verify server exists if specified
    if let Some(server_id) = input.server_id {
        server_queries::get_server_by_id(&state.pool, server_id)
            .await
            .map_err(|e| {
                warn!(server_id, error = %e, "Server not found");
                AppError::NotFound(format!("Server {} not found", server_id))
            })?;
    }

    // Create job schedule
    let job_schedule_id = queries::create_job_schedule(
        &state.pool,
        &input.name,
        input.description.as_deref(),
        input.job_template_id,
        input.server_id,
        &input.cron_expression,
        input.enabled.is_some(),
        input.max_retries,
        input.retry_delay_seconds,
    )
    .await
    .map_err(|e| {
        error!(error = %e, "Failed to create job schedule");
        AppError::DatabaseError(e.to_string())
    })?;

    info!(
        job_schedule_id,
        name = %input.name,
        "Job schedule created successfully"
    );

    // Return updated list
    get_job_schedules_list(State(state)).await
}

#[derive(Debug, Deserialize)]
pub struct UpdateJobScheduleInput {
    pub name: String,
    pub description: Option<String>,
    pub job_template_id: i64,
    pub server_id: Option<i64>,
    pub cron_expression: String,
    pub enabled: Option<String>,
    pub max_retries: Option<i64>,
    pub retry_delay_seconds: Option<i64>,
}

/// Update an existing job schedule
#[instrument(skip(state))]
pub async fn update_job_schedule(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Form(input): Form<UpdateJobScheduleInput>,
) -> Result<Html<String>, AppError> {
    info!(job_schedule_id = id, name = %input.name, "Updating job schedule");

    // Validate input
    if input.name.trim().is_empty() {
        warn!("Job schedule name is empty");
        let job_schedule = queries::get_job_schedule(&state.pool, id)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        let job_templates = job_templates::list_job_templates(&state.pool)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        let servers = server_queries::list_servers(&state.pool)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        let template = JobScheduleFormTemplate {
            job_schedule,
            job_templates,
            servers,
            error: Some("Name is required".to_string()),
        };
        let html = template.render().map_err(|e| {
            error!(error = %e, "Failed to render error template");
            AppError::TemplateError(e.to_string())
        })?;
        return Ok(Html(html));
    }

    // Validate cron expression
    if let Err(e) = cron::Schedule::from_str(&input.cron_expression) {
        warn!(cron_expression = %input.cron_expression, error = %e, "Invalid cron expression");
        let job_schedule = queries::get_job_schedule(&state.pool, id)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        let job_templates = job_templates::list_job_templates(&state.pool)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        let servers = server_queries::list_servers(&state.pool)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        let template = JobScheduleFormTemplate {
            job_schedule,
            job_templates,
            servers,
            error: Some(format!("Invalid cron expression: {}", e)),
        };
        let html = template.render().map_err(|e| {
            error!(error = %e, "Failed to render error template");
            AppError::TemplateError(e.to_string())
        })?;
        return Ok(Html(html));
    }

    // Verify job template exists
    job_templates::get_job_template(&state.pool, input.job_template_id)
        .await
        .map_err(|e| {
            warn!(job_template_id = input.job_template_id, error = %e, "Job template not found");
            AppError::NotFound(format!("Job template {} not found", input.job_template_id))
        })?;

    // Verify server exists if specified
    if let Some(server_id) = input.server_id {
        server_queries::get_server_by_id(&state.pool, server_id)
            .await
            .map_err(|e| {
                warn!(server_id, error = %e, "Server not found");
                AppError::NotFound(format!("Server {} not found", server_id))
            })?;
    }

    // Update job schedule
    queries::update_job_schedule(
        &state.pool,
        id,
        &input.name,
        input.description.as_deref(),
        input.job_template_id,
        input.server_id,
        &input.cron_expression,
        input.enabled.is_some(),
        input.max_retries,
        input.retry_delay_seconds,
    )
    .await
    .map_err(|e| {
        error!(job_schedule_id = id, error = %e, "Failed to update job schedule");
        AppError::DatabaseError(e.to_string())
    })?;

    info!(job_schedule_id = id, "Job schedule updated successfully");

    // Return updated list
    get_job_schedules_list(State(state)).await
}

/// Delete a job schedule
#[instrument(skip(state))]
pub async fn delete_job_schedule(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Html<String>, AppError> {
    info!(job_schedule_id = id, "Deleting job schedule");

    // Check if job schedule exists
    let job_schedule = queries::get_job_schedule(&state.pool, id)
        .await
        .map_err(|e| {
            error!(job_schedule_id = id, error = %e, "Failed to fetch job schedule");
            AppError::DatabaseError(e.to_string())
        })?
        .ok_or_else(|| {
            warn!(job_schedule_id = id, "Job schedule not found");
            AppError::NotFound(format!("Job schedule {} not found", id))
        })?;

    // Delete job schedule
    queries::delete_job_schedule(&state.pool, id)
        .await
        .map_err(|e| {
            error!(job_schedule_id = id, error = %e, "Failed to delete job schedule");
            AppError::DatabaseError(e.to_string())
        })?;

    info!(
        job_schedule_id = id,
        name = %job_schedule.name,
        "Job schedule deleted successfully"
    );

    // Return updated list
    get_job_schedules_list(State(state)).await
}

// ============================================================================
// Action Routes
// ============================================================================

/// Manually trigger a job schedule
#[instrument(skip(state))]
pub async fn run_job_schedule(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Html<String>, AppError> {
    info!(job_schedule_id = id, "Manually triggering job schedule");

    // Get job schedule
    let schedule = queries::get_job_schedule(&state.pool, id)
        .await
        .map_err(|e| {
            warn!(job_schedule_id = id, error = %e, "Job schedule not found");
            AppError::NotFound(format!("Job schedule {} not found", id))
        })?;

    // Create a job run
    let job_run_id = job_runs::create_job_run(
        &state.pool,
        schedule.job_template_id,
        Some(id),
        "manual",
    )
    .await
    .map_err(|e| {
        error!(job_schedule_id = id, error = %e, "Failed to create job run");
        AppError::DatabaseError(e.to_string())
    })?;

    info!(
        job_schedule_id = id,
        job_run_id,
        "Job schedule triggered manually, job run created"
    );

    // TODO: Actually execute the job asynchronously
    // For now, just return success message
    Ok(Html(format!(
        r#"<div class="alert alert-success">
            Job scheduled for execution. <a href="/runs/{}">View job run</a>
        </div>"#,
        job_run_id
    )))
}

/// Toggle a job schedule enabled/disabled
#[instrument(skip(state))]
pub async fn toggle_job_schedule(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Html<String>, AppError> {
    info!(job_schedule_id = id, "Toggling job schedule");

    // Get current schedule
    let schedule = queries::get_job_schedule(&state.pool, id)
        .await
        .map_err(|e| {
            warn!(job_schedule_id = id, error = %e, "Job schedule not found");
            AppError::NotFound(format!("Job schedule {} not found", id))
        })?;

    // Toggle enabled state
    let new_enabled = !schedule.enabled;

    queries::update_job_schedule(
        &state.pool,
        id,
        &schedule.name,
        schedule.description.as_deref(),
        schedule.job_template_id,
        schedule.server_id,
        &schedule.cron_expression,
        new_enabled,
        schedule.max_retries,
        schedule.retry_delay_seconds,
    )
    .await
    .map_err(|e| {
        error!(job_schedule_id = id, error = %e, "Failed to toggle job schedule");
        AppError::DatabaseError(e.to_string())
    })?;

    info!(
        job_schedule_id = id,
        enabled = new_enabled,
        "Job schedule toggled successfully"
    );

    // Return updated list
    get_job_schedules_list(State(state)).await
}
