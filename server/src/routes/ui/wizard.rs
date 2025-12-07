//! Job Wizard UI routes - Basic Mode for quick job creation
//!
//! Provides a step-by-step wizard for creating jobs from the catalog:
//! 1. Select Job - Browse/search the job catalog
//! 2. Configure - Fill in job parameters
//! 3. Select Server - Choose target server(s)
//! 4. Schedule - Run now or set schedule
//! 5. Notify - Configure notifications (optional)

use askama::Template;
use axum::{
    extract::{Path, Query, State},
    response::Html,
    routing::{get, post},
    Form, Router,
};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use svrctlrs_core::executor::JobExecutor;
use svrctlrs_database::queries;
use tracing::{error, info, warn};

use crate::{
    routes::ui::AppError,
    state::{AppState, JobRunUpdate},
    templates::{
        JobCatalogCategoryDisplay, JobCatalogItemDisplay, ServerDisplay, User,
        WizardNotificationChannelDisplay, WizardSelectedServerDisplay,
    },
};

/// Create router for job wizard routes
pub fn routes() -> Router<AppState> {
    Router::new()
        // Main wizard page
        .route("/wizard", get(wizard_page))
        // HTMX partials for wizard steps
        .route("/wizard/catalog", get(catalog_grid))
        .route("/wizard/catalog/{category}", get(catalog_category))
        .route("/wizard/configure/{id}", get(configure_step))
        .route("/wizard/servers/{id}", get(servers_step))
        .route("/wizard/schedule", get(schedule_step))
        .route("/wizard/notify", get(notify_step))
        .route("/wizard/review", post(review_step))
        // Actions
        .route("/wizard/create", post(create_job))
        .route("/wizard/quick-run/{catalog_id}", post(quick_run_job))
        .route("/wizard/toggle-favorite/{id}", post(toggle_favorite))
}

// ============================================================================
// Template Structs
// ============================================================================

#[derive(Template)]
#[template(path = "pages/wizard.html")]
struct WizardPageTemplate {
    user: Option<User>,
    categories: Vec<JobCatalogCategoryDisplay>,
    favorites: Vec<JobCatalogItemDisplay>,
}

#[derive(Template)]
#[template(path = "components/wizard/catalog_grid.html")]
#[allow(dead_code)]
struct CatalogGridTemplate {
    items: Vec<JobCatalogItemDisplay>,
    favorites: Vec<i64>,
}

#[derive(Template)]
#[template(path = "components/wizard/configure_step.html")]
struct ConfigureStepTemplate {
    catalog_item: JobCatalogItemDisplay,
}

#[derive(Template)]
#[template(path = "components/wizard/servers_step.html")]
struct ServersStepTemplate {
    catalog_item: JobCatalogItemDisplay,
    compatible_servers: Vec<ServerDisplay>,
    incompatible_servers: Vec<ServerDisplay>,
}

#[derive(Template)]
#[template(path = "components/wizard/schedule_step.html")]
struct ScheduleStepTemplate {
    channels: Vec<WizardNotificationChannelDisplay>,
}

#[derive(Template)]
#[template(path = "components/wizard/notify_step.html")]
struct NotifyStepTemplate {
    channels: Vec<WizardNotificationChannelDisplay>,
}

#[derive(Template)]
#[template(path = "components/wizard/review_step.html")]
struct ReviewStepTemplate {
    catalog_item: JobCatalogItemDisplay,
    servers: Vec<WizardSelectedServerDisplay>,
    parameters_json: String,
    schedule_type: String,
    cron_expression: Option<String>,
    notify_success: bool,
    notify_failure: bool,
    channel_ids: Vec<i64>,
}

#[derive(Template)]
#[template(path = "components/wizard/success.html")]
struct WizardSuccessTemplate {
    job_name: String,
    schedule_id: Option<i64>,
    job_run_id: Option<i64>,
    ran_immediately: bool,
}

// ============================================================================
// Route Handlers
// ============================================================================

/// Main wizard page - shows job catalog with categories and favorites
async fn wizard_page(State(state): State<AppState>) -> Result<Html<String>, AppError> {
    let db = state.db().await;

    // Get categories with counts
    let categories_with_counts =
        queries::job_catalog::list_catalog_categories_with_counts(db.pool()).await?;

    let categories: Vec<JobCatalogCategoryDisplay> = categories_with_counts
        .into_iter()
        .map(|cat| JobCatalogCategoryDisplay {
            id: cat.id,
            name: cat.name.clone(),
            display_name: cat.display_name,
            description: cat.description.unwrap_or_default(),
            icon: cat.icon,
            color: cat.color.unwrap_or_else(|| "#5E81AC".to_string()),
            sort_order: cat.sort_order,
            item_count: cat.item_count,
        })
        .collect();

    // Get favorite items
    let favorite_items = queries::job_catalog::list_catalog_favorites(db.pool()).await?;
    let favorites: Vec<JobCatalogItemDisplay> = favorite_items
        .into_iter()
        .map(|item| {
            let mut display: JobCatalogItemDisplay = item.into();
            display.is_favorite = true;
            display
        })
        .collect();

    let template = WizardPageTemplate {
        user: None,
        categories,
        favorites,
    };

    Ok(Html(template.render()?))
}

#[derive(Deserialize)]
struct CatalogQuery {
    search: Option<String>,
    difficulty: Option<String>,
    category: Option<String>,
}

/// Filter catalog items by search text and difficulty, and mark favorites
fn filter_catalog_items(
    items: Vec<svrctlrs_database::models::JobCatalogItem>,
    query: &CatalogQuery,
    favorite_ids: &[i64],
) -> Vec<JobCatalogItemDisplay> {
    items
        .into_iter()
        .filter(|item| {
            // Search filter (case-insensitive, min 2 chars)
            if let Some(ref search) = query.search {
                if search.len() >= 2 {
                    let search_lower = search.to_lowercase();
                    if !item.display_name.to_lowercase().contains(&search_lower)
                        && !item.description.to_lowercase().contains(&search_lower)
                    {
                        return false;
                    }
                }
            }
            // Difficulty filter
            if let Some(ref diff) = query.difficulty {
                if diff != "all" && item.difficulty != *diff {
                    return false;
                }
            }
            true
        })
        .map(|item| {
            let mut display: JobCatalogItemDisplay = item.into();
            display.is_favorite = favorite_ids.contains(&display.id);
            display
        })
        .collect()
}

/// Catalog grid partial - returns job cards for HTMX
async fn catalog_grid(
    State(state): State<AppState>,
    Query(query): Query<CatalogQuery>,
) -> Result<Html<String>, AppError> {
    let db = state.db().await;

    // Get all items or filter by category
    let items = if let Some(ref category) = query.category {
        queries::job_catalog::list_catalog_items_by_category(db.pool(), category).await?
    } else {
        queries::job_catalog::list_catalog_items(db.pool()).await?
    };

    // Get favorites for marking
    let favorite_ids: Vec<i64> = queries::job_catalog::list_catalog_favorites(db.pool())
        .await?
        .into_iter()
        .map(|f| f.id)
        .collect();

    let filtered = filter_catalog_items(items, &query, &favorite_ids);

    let template = CatalogGridTemplate {
        items: filtered,
        favorites: favorite_ids,
    };

    Ok(Html(template.render()?))
}

/// Category-specific catalog grid
async fn catalog_category(
    State(state): State<AppState>,
    Path(category): Path<String>,
    Query(query): Query<CatalogQuery>,
) -> Result<Html<String>, AppError> {
    let db = state.db().await;

    let items = queries::job_catalog::list_catalog_items_by_category(db.pool(), &category).await?;

    // Get favorites
    let favorite_ids: Vec<i64> = queries::job_catalog::list_catalog_favorites(db.pool())
        .await?
        .into_iter()
        .map(|f| f.id)
        .collect();

    let filtered = filter_catalog_items(items, &query, &favorite_ids);

    let template = CatalogGridTemplate {
        items: filtered,
        favorites: favorite_ids,
    };

    Ok(Html(template.render()?))
}

/// Configure step - shows parameter form for selected catalog item
async fn configure_step(
    State(state): State<AppState>,
    Path(catalog_id): Path<i64>,
) -> Result<Html<String>, AppError> {
    let db = state.db().await;

    let item = queries::job_catalog::get_catalog_item(db.pool(), catalog_id).await?;
    let display: JobCatalogItemDisplay = item.into();

    let template = ConfigureStepTemplate {
        catalog_item: display,
    };

    Ok(Html(template.render()?))
}

/// Servers step - shows compatible/incompatible servers
async fn servers_step(
    State(state): State<AppState>,
    Path(catalog_id): Path<i64>,
) -> Result<Html<String>, AppError> {
    let db = state.db().await;

    let item = queries::job_catalog::get_catalog_item(db.pool(), catalog_id).await?;
    let required_caps = item.get_required_capabilities();

    let all_servers = queries::servers::list_servers_with_details(db.pool()).await?;

    // Partition servers by compatibility
    let (compatible, incompatible): (Vec<_>, Vec<_>) =
        all_servers.into_iter().partition(|server| {
            if required_caps.is_empty() {
                return true; // No requirements = compatible with all
            }
            required_caps.iter().all(|cap| {
                server
                    .capabilities
                    .iter()
                    .any(|sc| sc.capability == *cap && sc.available)
            })
        });

    let template = ServersStepTemplate {
        catalog_item: item.into(),
        compatible_servers: compatible.into_iter().map(Into::into).collect(),
        incompatible_servers: incompatible.into_iter().map(Into::into).collect(),
    };

    Ok(Html(template.render()?))
}

/// Schedule step - choose run now or scheduled
async fn schedule_step(State(state): State<AppState>) -> Result<Html<String>, AppError> {
    let db = state.db().await;

    // Load enabled notification channels
    let channels: Vec<WizardNotificationChannelDisplay> =
        queries::notifications::list_notification_channels(db.pool())
            .await?
            .into_iter()
            .filter(|c| c.enabled)
            .map(Into::into)
            .collect();

    let template = ScheduleStepTemplate { channels };
    Ok(Html(template.render()?))
}

/// Notify step - configure notifications
async fn notify_step(State(state): State<AppState>) -> Result<Html<String>, AppError> {
    let db = state.db().await;

    // Load enabled notification channels
    let channels: Vec<WizardNotificationChannelDisplay> =
        queries::notifications::list_notification_channels(db.pool())
            .await?
            .into_iter()
            .filter(|c| c.enabled)
            .map(Into::into)
            .collect();

    let template = NotifyStepTemplate { channels };
    Ok(Html(template.render()?))
}

#[derive(Deserialize, Debug)]
struct ReviewInput {
    catalog_item_id: i64,
    server_ids: Option<String>,   // JSON array string
    server_names: Option<String>, // JSON object string
    parameters: Option<String>,   // JSON string
    schedule_type: String,        // "now" or "scheduled"
    cron_expression: Option<String>,
    notify_success: Option<String>,
    notify_failure: Option<String>,
    channel_ids: Option<String>, // JSON array string
}

/// Review step - show summary before creation
async fn review_step(
    State(state): State<AppState>,
    Form(input): Form<ReviewInput>,
) -> Result<Html<String>, AppError> {
    let db = state.db().await;

    let item = queries::job_catalog::get_catalog_item(db.pool(), input.catalog_item_id).await?;

    // Parse server_ids and server_names from JSON strings
    let server_ids: Vec<i64> = input
        .server_ids
        .as_ref()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_default();

    let server_names: HashMap<String, String> = input
        .server_names
        .as_ref()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_default();

    // Build servers list for display
    let servers: Vec<WizardSelectedServerDisplay> = server_ids
        .iter()
        .map(|id| {
            let name = server_names
                .get(&id.to_string())
                .cloned()
                .unwrap_or_else(|| format!("Server {}", id));
            WizardSelectedServerDisplay { id: *id, name }
        })
        .collect();

    // Parse channel_ids from JSON string
    let channel_ids: Vec<i64> = input
        .channel_ids
        .as_ref()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_default();

    let template = ReviewStepTemplate {
        catalog_item: item.into(),
        servers,
        parameters_json: input.parameters.unwrap_or_else(|| "{}".to_string()),
        schedule_type: input.schedule_type,
        cron_expression: input.cron_expression,
        notify_success: input.notify_success.is_some(),
        notify_failure: input.notify_failure.is_some(),
        channel_ids,
    };

    Ok(Html(template.render()?))
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
struct CreateJobInput {
    catalog_item_id: i64,
    server_ids: Option<String>, // JSON array string
    parameters: Option<String>,
    schedule_type: String,
    cron_expression: Option<String>,
    notify_success: Option<String>,
    notify_failure: Option<String>,
    channel_ids: Option<String>, // JSON array string - reserved for future use
    job_name: Option<String>,
}

/// Helper to get or create a wizard job type
async fn get_wizard_job_type_id(
    db: &svrctlrs_database::Database,
) -> Result<i64, crate::routes::ui::AppError> {
    // Try to find existing "wizard" job type
    match queries::job_types::get_job_types_by_name(db.pool(), "wizard").await {
        Ok(job_type) => Ok(job_type.id),
        Err(_) => {
            // Create a new wizard job type
            let create_input = svrctlrs_database::models::CreateJobType {
                name: "wizard".to_string(),
                display_name: "Wizard Jobs".to_string(),
                description: Some("Jobs created via the wizard interface".to_string()),
                icon: Some("wand".to_string()),
                color: Some("#5E81AC".to_string()),
                requires_capabilities: None,
                metadata: None,
                enabled: true,
            };
            Ok(queries::job_types::create_job_type(db.pool(), &create_input).await?)
        }
    }
}

/// Helper to get or create a command template from a catalog item
async fn get_or_create_command_template(
    db: &svrctlrs_database::Database,
    item: &svrctlrs_database::models::JobCatalogItem,
    job_type_id: i64,
) -> Result<i64, crate::routes::ui::AppError> {
    // Try to find an existing command template for this catalog item
    let template_name = format!("wizard_catalog_{}", item.id);

    // Check if command template already exists
    if let Ok(templates) = queries::job_types::get_command_templates(db.pool(), job_type_id).await {
        if let Some(existing) = templates.iter().find(|t| t.name == template_name) {
            return Ok(existing.id);
        }
    }

    // Get required capabilities as Option<Vec<String>>
    let required_caps = item.get_required_capabilities();
    let required_capabilities = if required_caps.is_empty() {
        None
    } else {
        Some(required_caps)
    };

    // Get parameters as Option<JsonValue>
    let params = item.get_parameters();
    let parameter_schema = if params.is_empty() {
        None
    } else {
        Some(serde_json::to_value(&params).unwrap_or_default())
    };

    // Create a new command template from the catalog item
    let create_input = svrctlrs_database::models::CreateCommandTemplate {
        job_type_id,
        name: template_name,
        display_name: item.display_name.clone(),
        description: Some(item.description.clone()),
        command: item.command.clone(),
        required_capabilities,
        os_filter: None,
        timeout_seconds: item.default_timeout as i32,
        working_directory: None,
        environment: None,
        output_format: None,
        parse_output: false,
        output_parser: None,
        notify_on_success: false,
        notify_on_failure: true,
        parameter_schema,
        metadata: None,
    };

    Ok(queries::job_types::create_command_template(db.pool(), &create_input).await?)
}

/// Create a notification policy for wizard jobs if channels are selected
async fn create_wizard_notification_policy(
    db: &svrctlrs_database::Database,
    policy_name: &str,
    on_success: bool,
    on_failure: bool,
    channel_ids: &[i64],
) -> Result<Option<i64>, crate::routes::ui::AppError> {
    // If no channels selected or notifications disabled, return None
    if channel_ids.is_empty() || (!on_success && !on_failure) {
        return Ok(None);
    }

    info!(
        channels = ?channel_ids,
        on_success,
        on_failure,
        "Creating notification policy for wizard job"
    );

    // Create the notification policy
    let policy_input = svrctlrs_database::models::CreateNotificationPolicy {
        name: policy_name.to_string(),
        description: Some("Auto-created by job wizard".to_string()),
        on_success,
        on_failure,
        on_timeout: on_failure, // Also notify on timeout if failure notifications enabled
        job_type_filter: None,
        server_filter: None,
        tag_filter: None,
        min_severity: 0,
        max_per_hour: None,
        title_template: None,
        body_template: None,
        enabled: true,
        metadata: Some(serde_json::json!({"source": "wizard"})),
    };

    let policy_id = queries::notifications::create_notification_policy(db.pool(), &policy_input)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to create notification policy");
            AppError::DatabaseError(e.to_string())
        })?;

    // Link each channel to the policy
    for channel_id in channel_ids {
        if let Err(e) =
            queries::notifications::add_policy_channel(db.pool(), policy_id, *channel_id, None)
                .await
        {
            warn!(
                channel_id,
                policy_id,
                error = %e,
                "Failed to link channel to policy (channel may not exist)"
            );
        }
    }

    info!(policy_id, "Created notification policy for wizard job");
    Ok(Some(policy_id))
}

/// Create job from wizard input
async fn create_job(
    State(state): State<AppState>,
    Form(input): Form<CreateJobInput>,
) -> Result<Html<String>, AppError> {
    let db = state.db().await;

    // Get catalog item
    let item = queries::job_catalog::get_catalog_item(db.pool(), input.catalog_item_id).await?;

    // Parse parameters as HashMap<String, String>
    let parameters: HashMap<String, String> = input
        .parameters
        .as_ref()
        .and_then(|p| serde_json::from_str(p).ok())
        .unwrap_or_default();

    // Parse server_ids from JSON string
    let server_ids: Vec<i64> = input
        .server_ids
        .as_ref()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_default();

    // Parse channel_ids from JSON string
    let channel_ids: Vec<i64> = input
        .channel_ids
        .as_ref()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_default();

    // Use first server ID for naming, or 0 if none
    let first_server_id = server_ids.first().copied().unwrap_or(0);
    let server_count = server_ids.len();

    // Generate job name
    let job_name = input.job_name.unwrap_or_else(|| {
        if server_count == 1 {
            format!("{} - Server {}", item.display_name.clone(), first_server_id)
        } else {
            format!("{} - {} servers", item.display_name.clone(), server_count)
        }
    });

    // Get or create wizard job type
    let job_type_id = get_wizard_job_type_id(&db).await?;

    // Get or create command template from catalog item
    let command_template_id = get_or_create_command_template(&db, &item, job_type_id).await?;

    // Store catalog item command and info in metadata
    let metadata = serde_json::json!({
        "catalog_item_id": input.catalog_item_id,
        "catalog_item_name": item.name,
        "command": item.command,
        "source": "wizard",
        "target_servers": server_ids
    });

    // Create job template
    let job_template_input = svrctlrs_database::models::CreateJobTemplate {
        name: format!(
            "wizard_{}_{}_{}",
            item.name,
            first_server_id,
            chrono::Utc::now().timestamp()
        ),
        display_name: job_name.clone(),
        description: Some(format!("Created from wizard: {}", item.description)),
        job_type_id,
        is_composite: false,
        command_template_id: Some(command_template_id),
        variables: if parameters.is_empty() {
            None
        } else {
            Some(parameters.clone())
        },
        timeout_seconds: item.default_timeout as i32,
        retry_count: item.default_retry_count as i32,
        retry_delay_seconds: 60,
        notify_on_success: input.notify_success.is_some(),
        notify_on_failure: input.notify_failure.is_some(),
        notification_policy_id: None,
        metadata: Some(metadata),
    };

    let job_template_id =
        queries::job_templates::create_job_template(db.pool(), &job_template_input).await?;

    // Create notification policy if channels are selected and notifications are enabled
    let notify_success = input.notify_success.is_some();
    let notify_failure = input.notify_failure.is_some();
    let notification_policy_id = create_wizard_notification_policy(
        &db,
        &format!("Wizard: {}", job_name),
        notify_success,
        notify_failure,
        &channel_ids,
    )
    .await?;

    let mut schedule_id = None;
    let mut job_run_id = None;

    if input.schedule_type == "now" {
        // Run immediately - create a disabled one-time schedule, then create job runs
        // This satisfies the foreign key constraint while keeping the schedule from repeating
        let run_metadata = serde_json::json!({
            "triggered_by": "wizard",
            "catalog_item_id": input.catalog_item_id,
            "total_servers": server_count,
            "run_type": "immediate"
        });

        // Collect all job run IDs for execution
        let mut job_run_ids_to_execute: Vec<i64> = Vec::new();

        // Create job runs for all servers (we'll return the first one for the success message)
        for (idx, server_id) in server_ids.iter().enumerate() {
            let schedule_name = if server_count == 1 {
                format!("{} (One-time)", job_name.clone())
            } else {
                format!("{} - Server {} (One-time)", job_name.clone(), server_id)
            };

            // Create a disabled one-time schedule (satisfies FK, won't re-run)
            let schedule_input = svrctlrs_database::models::CreateJobSchedule {
                name: schedule_name,
                description: Some(format!("One-time run from wizard: {}", item.description)),
                job_template_id,
                server_id: *server_id,
                schedule: "0 0 0 31 2 *".to_string(), // 6-field cron: sec min hour day month dow (Feb 31st = never)
                enabled: false, // Disabled so it won't be picked up by scheduler
                timeout_seconds: None,
                retry_count: None,
                notify_on_success: Some(notify_success),
                notify_on_failure: Some(notify_failure),
                notification_policy_id,
                metadata: Some(serde_json::json!({
                    "one_time": true,
                    "triggered_by": "wizard"
                })),
            };

            let sched_id =
                queries::job_schedules::create_job_schedule(db.pool(), &schedule_input).await?;

            // Store the first schedule ID for the success message
            if idx == 0 {
                schedule_id = Some(sched_id);
            }

            // Now create the job run with a valid schedule reference
            let run_id = queries::job_runs::create_job_run(
                db.pool(),
                sched_id,
                job_template_id,
                *server_id,
                0,     // retry_attempt
                false, // is_retry
                Some(serde_json::to_string(&run_metadata).unwrap_or_default()),
            )
            .await?;

            // Store the first job run ID for the success message
            if idx == 0 {
                job_run_id = Some(run_id);
            }

            // Collect for execution
            job_run_ids_to_execute.push(run_id);
        }

        // Trigger actual execution via executor in background tasks
        info!(
            job_run_count = job_run_ids_to_execute.len(),
            "Spawning immediate job execution tasks"
        );

        let executor = Arc::new(JobExecutor::new(
            state.pool.clone(),
            state.config.ssh_key_path.clone(),
            10, // max_concurrent_jobs
        ));

        for run_id in job_run_ids_to_execute {
            info!(
                job_run_id = run_id,
                "Queuing job run for immediate execution"
            );

            let executor = executor.clone();
            let job_run_tx = state.job_run_tx.clone();
            let notification_service = state.notification_service.clone();

            // Broadcast that job run was created
            if let Err(e) = job_run_tx.send(JobRunUpdate::Created { job_run_id: run_id }) {
                warn!(job_run_id = run_id, error = %e, "Failed to broadcast job run creation");
            }

            // Spawn background task to execute the job
            tokio::spawn(async move {
                info!(job_run_id = run_id, "Starting wizard job execution");

                match executor.execute_job_run(run_id).await {
                    Ok(()) => {
                        info!(job_run_id = run_id, "Wizard job completed successfully");
                        let _ = job_run_tx.send(JobRunUpdate::StatusChanged {
                            job_run_id: run_id,
                            status: "success".to_string(),
                        });
                    }
                    Err(e) => {
                        error!(job_run_id = run_id, error = %e, "Wizard job execution failed");
                        let _ = job_run_tx.send(JobRunUpdate::StatusChanged {
                            job_run_id: run_id,
                            status: "failed".to_string(),
                        });
                    }
                }

                // Send notification after job completion
                if let Err(e) = notification_service.notify_job_run(run_id).await {
                    warn!(
                        job_run_id = run_id,
                        error = %e,
                        "Failed to send job completion notification"
                    );
                }
            });
        }
    } else {
        // Create schedule for each server
        for (idx, server_id) in server_ids.iter().enumerate() {
            let schedule_name = if server_count == 1 {
                job_name.clone()
            } else {
                format!("{} (Server {})", job_name.clone(), server_id)
            };

            let schedule_input = svrctlrs_database::models::CreateJobSchedule {
                name: schedule_name,
                description: Some(format!("Created from wizard: {}", item.description)),
                job_template_id,
                server_id: *server_id,
                schedule: input
                    .cron_expression
                    .clone()
                    .unwrap_or_else(|| "0 * * * *".to_string()),
                enabled: true,
                timeout_seconds: None,
                retry_count: None,
                notify_on_success: Some(notify_success),
                notify_on_failure: Some(notify_failure),
                notification_policy_id,
                metadata: None,
            };

            let sched_id =
                queries::job_schedules::create_job_schedule(db.pool(), &schedule_input).await?;

            // Store the first schedule ID for the success message
            if idx == 0 {
                schedule_id = Some(sched_id);
            }
        }
    }

    let template = WizardSuccessTemplate {
        job_name,
        schedule_id,
        job_run_id,
        ran_immediately: input.schedule_type == "now",
    };

    Ok(Html(template.render()?))
}

/// Quick run - execute a catalog job immediately on a specific server
async fn quick_run_job(
    State(state): State<AppState>,
    Path(catalog_id): Path<i64>,
    Form(input): Form<QuickRunInput>,
) -> Result<Html<String>, AppError> {
    let db = state.db().await;

    let item = queries::job_catalog::get_catalog_item(db.pool(), catalog_id).await?;

    // Get or create wizard job type
    let job_type_id = get_wizard_job_type_id(&db).await?;

    // Get or create command template from catalog item (required for simple jobs)
    let command_template_id = get_or_create_command_template(&db, &item, job_type_id).await?;

    // Parse parameters as HashMap<String, String>
    let parameters: Option<HashMap<String, String>> = input
        .parameters
        .as_ref()
        .and_then(|p| serde_json::from_str(p).ok());

    // Store catalog item command and info in metadata
    let metadata = serde_json::json!({
        "catalog_item_id": catalog_id,
        "catalog_item_name": item.name,
        "command": item.command,
        "source": "quick_run"
    });

    // Create minimal job template
    let job_template_input = svrctlrs_database::models::CreateJobTemplate {
        name: format!(
            "quickrun_{}_{}_{}",
            item.name,
            input.server_id,
            chrono::Utc::now().timestamp()
        ),
        display_name: format!("Quick Run: {}", item.display_name),
        description: Some(format!("Quick run from dashboard: {}", item.description)),
        job_type_id,
        is_composite: false,
        command_template_id: Some(command_template_id),
        variables: parameters,
        timeout_seconds: item.default_timeout as i32,
        retry_count: 0,
        retry_delay_seconds: 60,
        notify_on_success: false,
        notify_on_failure: false,
        notification_policy_id: None,
        metadata: Some(metadata),
    };

    let job_template_id =
        queries::job_templates::create_job_template(db.pool(), &job_template_input).await?;

    // Create a disabled one-time schedule (required for FK constraint on job_runs)
    let schedule_input = svrctlrs_database::models::CreateJobSchedule {
        name: format!("Quick Run: {} (One-time)", item.display_name),
        description: Some(format!("One-time quick run: {}", item.description)),
        job_template_id,
        server_id: input.server_id,
        schedule: "0 0 0 31 2 *".to_string(), // 6-field cron: Feb 31st = never
        enabled: false,                       // Disabled so it won't be picked up by scheduler
        timeout_seconds: None,
        retry_count: None,
        notify_on_success: Some(false),
        notify_on_failure: Some(false),
        notification_policy_id: None,
        metadata: Some(serde_json::json!({
            "one_time": true,
            "triggered_by": "quick_run",
            "catalog_item_id": catalog_id
        })),
    };

    let schedule_id =
        queries::job_schedules::create_job_schedule(db.pool(), &schedule_input).await?;

    // Create job run
    let run_metadata = serde_json::json!({
        "triggered_by": "quick_run",
        "catalog_item_id": catalog_id
    });

    let job_run_id = queries::job_runs::create_job_run(
        db.pool(),
        schedule_id,
        job_template_id,
        input.server_id,
        0,     // retry_attempt
        false, // is_retry
        Some(serde_json::to_string(&run_metadata).unwrap_or_default()),
    )
    .await?;

    // Trigger execution in background
    let executor = Arc::new(JobExecutor::new(
        state.pool.clone(),
        state.config.ssh_key_path.clone(),
        10, // max_concurrent_jobs
    ));
    let job_run_tx = state.job_run_tx.clone();

    // Broadcast that job run was created
    let _ = job_run_tx.send(JobRunUpdate::Created { job_run_id });

    // Spawn background task to execute the job
    tokio::spawn(async move {
        info!(job_run_id, "Starting quick run job execution");

        match executor.execute_job_run(job_run_id).await {
            Ok(()) => {
                info!(job_run_id, "Quick run job completed successfully");
                let _ = job_run_tx.send(JobRunUpdate::StatusChanged {
                    job_run_id,
                    status: "success".to_string(),
                });
            }
            Err(e) => {
                error!(job_run_id, error = %e, "Quick run job execution failed");
                let _ = job_run_tx.send(JobRunUpdate::StatusChanged {
                    job_run_id,
                    status: "failed".to_string(),
                });
            }
        }
    });

    let template = WizardSuccessTemplate {
        job_name: format!("Quick Run: {}", item.display_name),
        schedule_id: None,
        job_run_id: Some(job_run_id),
        ran_immediately: true,
    };

    Ok(Html(template.render()?))
}

#[derive(Deserialize)]
struct QuickRunInput {
    server_id: i64,
    parameters: Option<String>,
}

/// Toggle favorite status for a catalog item
async fn toggle_favorite(
    State(state): State<AppState>,
    Path(catalog_id): Path<i64>,
) -> Result<Html<String>, AppError> {
    let db = state.db().await;

    let is_now_favorite =
        queries::job_catalog::toggle_catalog_favorite(db.pool(), catalog_id).await?;

    // Return updated favorite button
    let html = if is_now_favorite {
        format!(
            r#"<button class="btn btn-icon btn-favorite active"
                       hx-post="/wizard/toggle-favorite/{}"
                       hx-swap="outerHTML"
                       title="Remove from favorites">
                    <i data-lucide="star-off"></i>
                </button>"#,
            catalog_id
        )
    } else {
        format!(
            r#"<button class="btn btn-icon btn-favorite"
                       hx-post="/wizard/toggle-favorite/{}"
                       hx-swap="outerHTML"
                       title="Add to favorites">
                    <i data-lucide="star"></i>
                </button>"#,
            catalog_id
        )
    };

    Ok(Html(html))
}
