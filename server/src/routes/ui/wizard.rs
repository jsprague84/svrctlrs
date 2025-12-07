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
use svrctlrs_database::queries;

use crate::{
    routes::ui::AppError,
    state::AppState,
    templates::{JobCatalogCategoryDisplay, JobCatalogItemDisplay, ServerDisplay, User},
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
    channels: Vec<NotificationChannelDisplay>,
}

#[derive(Template)]
#[template(path = "components/wizard/notify_step.html")]
struct NotifyStepTemplate {
    channels: Vec<NotificationChannelDisplay>,
}

#[derive(Debug, Clone)]
struct NotificationChannelDisplay {
    pub id: i64,
    pub name: String,
    pub channel_type: String,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct SelectedServerDisplay {
    pub id: i64,
    pub name: String,
}

#[derive(Template)]
#[template(path = "components/wizard/review_step.html")]
struct ReviewStepTemplate {
    catalog_item: JobCatalogItemDisplay,
    servers: Vec<SelectedServerDisplay>,
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

    // Apply filters
    let filtered: Vec<JobCatalogItemDisplay> = items
        .into_iter()
        .filter(|item| {
            // Search filter
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
        .collect();

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

    // Apply search/difficulty filters
    let filtered: Vec<JobCatalogItemDisplay> = items
        .into_iter()
        .filter(|item| {
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
        .collect();

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

    // Load notification channels
    let channels = queries::notifications::list_notification_channels(db.pool()).await?;
    let channel_displays: Vec<NotificationChannelDisplay> = channels
        .into_iter()
        .filter(|c| c.enabled)
        .map(|c| NotificationChannelDisplay {
            id: c.id,
            name: c.name,
            channel_type: c.channel_type_str,
        })
        .collect();

    let template = ScheduleStepTemplate {
        channels: channel_displays,
    };
    Ok(Html(template.render()?))
}

/// Notify step - configure notifications
async fn notify_step(State(state): State<AppState>) -> Result<Html<String>, AppError> {
    let db = state.db().await;

    let channels = queries::notifications::list_notification_channels(db.pool()).await?;

    let channel_displays: Vec<NotificationChannelDisplay> = channels
        .into_iter()
        .filter(|c| c.enabled)
        .map(|c| NotificationChannelDisplay {
            id: c.id,
            name: c.name,
            channel_type: c.channel_type_str,
        })
        .collect();

    let template = NotifyStepTemplate {
        channels: channel_displays,
    };

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
    let servers: Vec<SelectedServerDisplay> = server_ids
        .iter()
        .map(|id| {
            let name = server_names
                .get(&id.to_string())
                .cloned()
                .unwrap_or_else(|| format!("Server {}", id));
            SelectedServerDisplay { id: *id, name }
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
                schedule: "0 0 31 2 *".to_string(), // Feb 31st = never (cron that never runs)
                enabled: false, // Disabled so it won't be picked up by scheduler
                timeout_seconds: None,
                retry_count: None,
                notify_on_success: Some(input.notify_success.is_some()),
                notify_on_failure: Some(input.notify_failure.is_some()),
                notification_policy_id: None,
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
        }

        // TODO: Trigger actual execution via executor
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
                notify_on_success: Some(input.notify_success.is_some()),
                notify_on_failure: Some(input.notify_failure.is_some()),
                notification_policy_id: None,
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
        command_template_id: None,
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

    // Create job run using individual parameters
    let run_metadata = serde_json::json!({
        "triggered_by": "quick_run",
        "catalog_item_id": catalog_id
    });

    let job_run_id = queries::job_runs::create_job_run(
        db.pool(),
        0, // No schedule
        job_template_id,
        input.server_id,
        0,     // retry_attempt
        false, // is_retry
        Some(serde_json::to_string(&run_metadata).unwrap_or_default()),
    )
    .await?;

    // TODO: Trigger execution

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
