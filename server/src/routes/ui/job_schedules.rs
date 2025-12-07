use askama::Template;
use axum::{
    extract::{Path, State},
    response::Html,
    routing::{get, post, put},
    Router,
};
use axum_extra::extract::Form;
use cron::Schedule;
use serde::Deserialize;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use svrctlrs_core::executor::JobExecutor;
use svrctlrs_database::{
    models::{CreateJobSchedule, UpdateJobSchedule},
    queries::{job_schedules as queries, job_templates, servers as server_queries},
};
use tracing::{error, info, instrument, warn};

use crate::{
    routes::ui::AppError,
    state::{AppState, JobRunUpdate},
    templates::{
        GroupedSchedulesTemplate, JobScheduleFormTemplate, JobScheduleListTemplate,
        JobSchedulesTemplate, ServerScheduleGroup,
    },
};

/// Create router with all job schedule routes
pub fn routes() -> Router<AppState> {
    Router::new()
        // Main page
        .route(
            "/job-schedules",
            get(job_schedules_page).post(create_job_schedule),
        )
        // List endpoint
        .route("/job-schedules/list", get(get_job_schedules_list))
        .route("/job-schedules/grouped", get(get_grouped_schedules))
        // Form endpoints
        .route("/job-schedules/new", get(new_job_schedule_form))
        .route("/job-schedules/{id}/edit", get(edit_job_schedule_form))
        // CRUD endpoints
        .route(
            "/job-schedules/{id}",
            put(update_job_schedule).delete(delete_job_schedule),
        )
        // Action endpoints
        .route("/job-schedules/{id}/run-now", post(run_job_schedule))
        .route("/job-schedules/{id}/toggle", post(toggle_job_schedule))
}

// ============================================================================
// Page Routes
// ============================================================================

/// Display the job schedules management page
#[instrument(skip(state))]
pub async fn job_schedules_page(State(state): State<AppState>) -> Result<Html<String>, AppError> {
    info!("Rendering job schedules page");

    let schedules = queries::list_job_schedules_with_names(&state.pool)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to fetch job schedules");
            AppError::DatabaseError(e.to_string())
        })?;

    let job_templates = job_templates::list_job_templates_with_counts(&state.pool)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to fetch job templates");
            AppError::DatabaseError(e.to_string())
        })?;

    let servers = server_queries::list_servers_with_details(&state.pool)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to fetch servers");
            AppError::DatabaseError(e.to_string())
        })?;

    // Group schedules by server
    let schedule_groups = group_schedules_by_server(&schedules, &servers);

    let template = JobSchedulesTemplate {
        user: None, // TODO: Add authentication
        schedules: schedules.into_iter().map(Into::into).collect(),
        schedule_groups,
        job_templates: job_templates.into_iter().map(Into::into).collect(),
        servers: servers.into_iter().map(Into::into).collect(),
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

    let schedules = queries::list_job_schedules_with_names(&state.pool)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to fetch job schedules");
            AppError::DatabaseError(e.to_string())
        })?;

    let servers = server_queries::list_servers_with_details(&state.pool)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to fetch servers");
            AppError::DatabaseError(e.to_string())
        })?;

    // Group schedules by server
    let schedule_groups = group_schedules_by_server(&schedules, &servers);

    let template = JobScheduleListTemplate {
        schedules: schedules.into_iter().map(Into::into).collect(),
        schedule_groups,
    };

    let html = template.render().map_err(|e| {
        error!(error = %e, "Failed to render job schedule list template");
        AppError::TemplateError(e.to_string())
    })?;

    Ok(Html(html))
}

/// Get grouped schedules (HTMX)
#[instrument(skip(state))]
pub async fn get_grouped_schedules(
    State(state): State<AppState>,
) -> Result<Html<String>, AppError> {
    info!("Fetching grouped schedules");

    let schedules = queries::list_job_schedules_with_names(&state.pool)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to fetch job schedules");
            AppError::DatabaseError(e.to_string())
        })?;

    let servers = server_queries::list_servers_with_details(&state.pool)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to fetch servers");
            AppError::DatabaseError(e.to_string())
        })?;

    // Group schedules by server
    let groups = group_schedules_by_server(&schedules, &servers);

    let template = GroupedSchedulesTemplate {
        grouped_schedules: groups.clone(),
        groups, // Alias
    };

    let html = template.render().map_err(|e| {
        error!(error = %e, "Failed to render grouped schedules template");
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

    let job_templates = job_templates::list_job_templates_with_counts(&state.pool)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to fetch job templates");
            AppError::DatabaseError(e.to_string())
        })?;

    let servers = server_queries::list_servers_with_details(&state.pool)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to fetch servers");
            AppError::DatabaseError(e.to_string())
        })?;

    let template = JobScheduleFormTemplate {
        schedule: None,
        job_templates: job_templates.into_iter().map(Into::into).collect(),
        servers: servers.into_iter().map(Into::into).collect(),
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

    let job_schedule = queries::get_job_schedule_with_names(&state.pool, id)
        .await
        .map_err(|e| {
            warn!(job_schedule_id = id, error = %e, "Job schedule not found");
            AppError::NotFound(format!("Job schedule {} not found", id))
        })?;

    let job_templates = job_templates::list_job_templates_with_counts(&state.pool)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to fetch job templates");
            AppError::DatabaseError(e.to_string())
        })?;

    let servers = server_queries::list_servers_with_details(&state.pool)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to fetch servers");
            AppError::DatabaseError(e.to_string())
        })?;

    let template = JobScheduleFormTemplate {
        schedule: Some(job_schedule.into()),
        job_templates: job_templates.into_iter().map(Into::into).collect(),
        servers: servers.into_iter().map(Into::into).collect(),
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
    pub server_id: i64,
    pub schedule: String, // Cron expression
    pub timeout_seconds: Option<i32>,
    pub retry_count: Option<i32>,
    pub enabled: Option<String>,           // "on" or absent
    pub notify_on_success: Option<String>, // "on" or absent
    pub notify_on_failure: Option<String>, // "on" or absent
}

/// Create a new job schedule
#[instrument(skip(state))]
pub async fn create_job_schedule(
    State(state): State<AppState>,
    Form(input): Form<CreateJobScheduleInput>,
) -> Result<Html<String>, AppError> {
    info!(name = %input.name, "Creating new job schedule");

    // Validate input
    if input.name.trim().is_empty() {
        warn!("Job schedule name is empty");
        return Err(AppError::ValidationError("Name is required".to_string()));
    }

    // Validate cron expression
    if Schedule::from_str(&input.schedule).is_err() {
        warn!(schedule = %input.schedule, "Invalid cron expression");
        return Err(AppError::ValidationError(format!(
            "Invalid cron expression: {}",
            input.schedule
        )));
    }

    // Verify job template exists
    job_templates::get_job_template(&state.pool, input.job_template_id)
        .await
        .map_err(|e| {
            warn!(job_template_id = input.job_template_id, error = %e, "Job template not found");
            AppError::NotFound(format!("Job template {} not found", input.job_template_id))
        })?;

    // Create job schedule
    // For checkbox overrides: if checked, set Some(true), otherwise None to use template default
    let create_input = CreateJobSchedule {
        name: input.name.clone(),
        description: input.description,
        job_template_id: input.job_template_id,
        server_id: input.server_id,
        schedule: input.schedule,
        enabled: input.enabled.is_some(),
        timeout_seconds: input.timeout_seconds,
        retry_count: input.retry_count,
        notify_on_success: input.notify_on_success.map(|_| true),
        notify_on_failure: input.notify_on_failure.map(|_| true),
        notification_policy_id: None,
        metadata: None,
    };

    queries::create_job_schedule(&state.pool, &create_input)
        .await
        .map_err(|e| {
            error!(name = %input.name, error = %e, "Failed to create job schedule");
            AppError::DatabaseError(e.to_string())
        })?;

    info!(name = %input.name, "Job schedule created successfully");

    // Return updated list
    get_job_schedules_list(State(state)).await
}

#[derive(Debug, Deserialize)]
pub struct UpdateJobScheduleInput {
    pub name: Option<String>,
    pub description: Option<String>,
    pub schedule: Option<String>, // Cron expression
    pub enabled: Option<String>,
    pub timeout_seconds: Option<i32>,
    pub retry_count: Option<i32>,
    pub notify_on_success: Option<String>, // "on" or absent
    pub notify_on_failure: Option<String>, // "on" or absent
}

/// Update an existing job schedule
#[instrument(skip(state))]
pub async fn update_job_schedule(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Form(input): Form<UpdateJobScheduleInput>,
) -> Result<Html<String>, AppError> {
    info!(job_schedule_id = id, "Updating job schedule");

    // Validate cron expression if provided
    if let Some(ref schedule) = input.schedule {
        if Schedule::from_str(schedule).is_err() {
            warn!(schedule = %schedule, "Invalid cron expression");
            return Err(AppError::ValidationError(format!(
                "Invalid cron expression: {}",
                schedule
            )));
        }
    }

    // Update job schedule
    // For checkbox overrides: if checked, set Some(true), otherwise None to use template default
    let update_input = UpdateJobSchedule {
        name: input.name,
        description: input.description,
        schedule: input.schedule,
        enabled: Some(input.enabled.is_some()),
        timeout_seconds: input.timeout_seconds,
        retry_count: input.retry_count,
        notify_on_success: input.notify_on_success.map(|_| true),
        notify_on_failure: input.notify_on_failure.map(|_| true),
        notification_policy_id: None,
        next_run_at: None,
        metadata: None,
    };

    queries::update_job_schedule(&state.pool, id, &update_input)
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
    let _job_schedule = queries::get_job_schedule(&state.pool, id)
        .await
        .map_err(|e| {
            warn!(job_schedule_id = id, error = %e, "Job schedule not found");
            AppError::NotFound(format!("Job schedule {} not found", id))
        })?;

    // Delete job schedule
    queries::delete_job_schedule(&state.pool, id)
        .await
        .map_err(|e| {
            error!(job_schedule_id = id, error = %e, "Failed to delete job schedule");
            AppError::DatabaseError(e.to_string())
        })?;

    info!(job_schedule_id = id, "Job schedule deleted successfully");

    // Return empty response (HTMX will remove the element)
    Ok(Html(String::new()))
}

// ============================================================================
// Action Endpoints
// ============================================================================

/// Run a job schedule immediately (manual trigger)
#[instrument(skip(state))]
pub async fn run_job_schedule(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Html<String>, AppError> {
    info!(job_schedule_id = id, "Running job schedule manually");

    // Get the job schedule
    let job_schedule = queries::get_job_schedule_with_names(&state.pool, id)
        .await
        .map_err(|e| {
            warn!(job_schedule_id = id, error = %e, "Job schedule not found");
            AppError::NotFound(format!("Job schedule {} not found", id))
        })?;

    // Validate schedule is enabled
    if !job_schedule.enabled {
        return Ok(Html(format!(
            r#"<div class="alert alert-warning">⚠️ Job schedule '{}' is disabled. Enable it first to run.</div>"#,
            job_schedule.name
        )));
    }

    // Create a job run record
    use svrctlrs_database::queries::job_runs;

    let job_run_id = job_runs::create_job_run(
        &state.pool,
        job_schedule.id,
        job_schedule.job_template_id,
        job_schedule.server_id.unwrap_or(1), // Default to localhost (id=1) if not specified
        0,                                   // retry_attempt
        false,                               // is_retry
        Some(r#"{"triggered":"manual","user":"system"}"#.to_string()),
    )
    .await
    .map_err(|e| {
        error!(job_schedule_id = id, error = %e, "Failed to create job run");
        AppError::DatabaseError(e.to_string())
    })?;

    info!(
        job_schedule_id = id,
        job_run_id, "Job schedule triggered for manual execution"
    );

    // Trigger actual job execution in background
    let executor = Arc::new(JobExecutor::new(
        state.pool.clone(),
        state.config.ssh_key_path.clone(),
        10, // max_concurrent_jobs
    ));
    let job_run_tx = state.job_run_tx.clone();

    // Broadcast that job run was created
    if let Err(e) = job_run_tx.send(JobRunUpdate::Created { job_run_id }) {
        warn!(job_run_id, error = %e, "Failed to broadcast job run creation");
    }

    // Spawn background task to execute the job
    tokio::spawn(async move {
        info!(job_run_id, "Starting manual job execution");

        match executor.execute_job_run(job_run_id).await {
            Ok(()) => {
                info!(job_run_id, "Manual job completed successfully");
                let _ = job_run_tx.send(JobRunUpdate::StatusChanged {
                    job_run_id,
                    status: "success".to_string(),
                });
            }
            Err(e) => {
                error!(job_run_id, error = %e, "Manual job execution failed");
                let _ = job_run_tx.send(JobRunUpdate::StatusChanged {
                    job_run_id,
                    status: "failed".to_string(),
                });
            }
        }
    });

    // Return success message
    Ok(Html(format!(
        r#"<div class="alert alert-success alert-auto-dismiss">
            ✓ Job schedule '{}' triggered successfully<br>
            <small>Job Run ID: {} - <a href="/job-runs/{}" class="link">View Details</a></small>
        </div>"#,
        job_schedule.name, job_run_id, job_run_id
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
    let job_schedule = queries::get_job_schedule_with_names(&state.pool, id)
        .await
        .map_err(|e| {
            warn!(job_schedule_id = id, error = %e, "Job schedule not found");
            AppError::NotFound(format!("Job schedule {} not found", id))
        })?;

    // Toggle enabled state
    let update_input = UpdateJobSchedule {
        name: None,
        description: None,
        schedule: None,
        enabled: Some(!job_schedule.enabled),
        timeout_seconds: None,
        retry_count: None,
        notify_on_success: None,
        notify_on_failure: None,
        notification_policy_id: None,
        next_run_at: None,
        metadata: None,
    };

    queries::update_job_schedule(&state.pool, id, &update_input)
        .await
        .map_err(|e| {
            error!(job_schedule_id = id, error = %e, "Failed to toggle job schedule");
            AppError::DatabaseError(e.to_string())
        })?;

    info!(
        job_schedule_id = id,
        enabled = !job_schedule.enabled,
        "Job schedule toggled"
    );

    // Return updated schedule row
    get_job_schedules_list(State(state)).await
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Group schedules by server for display
fn group_schedules_by_server(
    schedules: &[svrctlrs_database::queries::JobScheduleWithNames],
    servers: &[svrctlrs_database::ServerWithDetails],
) -> Vec<ServerScheduleGroup> {
    let mut groups: HashMap<Option<i64>, ServerScheduleGroup> = HashMap::new();

    for schedule in schedules {
        let group = groups.entry(schedule.server_id).or_insert_with(|| {
            let server_name = if let Some(server_id) = schedule.server_id {
                servers
                    .iter()
                    .find(|s| s.server.id == server_id)
                    .map(|s| s.server.name.clone())
            } else {
                None
            };

            ServerScheduleGroup {
                server_id: schedule.server_id,
                server_name,
                schedules: Vec::new(),
            }
        });

        group.schedules.push(schedule.clone().into());
    }

    let mut result: Vec<ServerScheduleGroup> = groups.into_values().collect();
    result.sort_by(|a, b| {
        match (&a.server_name, &b.server_name) {
            (None, None) => std::cmp::Ordering::Equal,
            (None, Some(_)) => std::cmp::Ordering::Less, // Local first
            (Some(_), None) => std::cmp::Ordering::Greater,
            (Some(a_name), Some(b_name)) => a_name.cmp(b_name),
        }
    });

    result
}
