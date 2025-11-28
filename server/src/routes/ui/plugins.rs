//! Plugin management routes

use askama::Template;
use axum::{
    extract::{Path, State},
    response::Html,
    routing::{get, post},
    Form, Router,
};
use svrctlrs_database::queries;

use super::{get_user_from_session, AppError};
use crate::{state::AppState, templates::*};

/// Create features router (formerly plugins)
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/features", get(plugins_page))
        .route("/features/{id}/toggle", post(plugin_toggle))
        .route(
            "/features/{id}/config",
            get(plugin_config_form).put(plugin_config_save),
        )
}

/// Helper to convert DB plugin to UI model
fn db_plugin_to_ui(db: svrctlrs_database::models::plugin::Plugin) -> Plugin {
    Plugin {
        id: db.id,
        name: db.name,
        description: db.description.unwrap_or_default(),
        version: "1.0.0".to_string(), // TODO: Get from plugin metadata
        enabled: db.enabled,
    }
}

/// Plugins page handler
async fn plugins_page(State(state): State<AppState>) -> Result<Html<String>, AppError> {
    let user = get_user_from_session().await;

    // Load plugins from database
    let db = state.db().await;
    let db_plugins = queries::plugins::list_plugins(db.pool()).await?;
    let plugins = db_plugins.into_iter().map(db_plugin_to_ui).collect();

    let template = PluginsTemplate { user, plugins };
    Ok(Html(template.render()?))
}

/// Toggle plugin handler
async fn plugin_toggle(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Html<String>, AppError> {
    tracing::info!("Toggling plugin: {}", id);

    // Toggle plugin in database
    let db = state.db().await;
    queries::plugins::toggle_plugin(db.pool(), &id).await?;

    // Return updated plugin list
    let db_plugins = queries::plugins::list_plugins(db.pool()).await?;
    let plugins = db_plugins.into_iter().map(db_plugin_to_ui).collect();
    let template = PluginListTemplate { plugins };
    Ok(Html(template.render()?))
}

/// Plugin configuration form handler
async fn plugin_config_form(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Html<String>, AppError> {
    // Load plugin from database
    let db = state.db().await;
    let db_plugin = queries::plugins::get_plugin(db.pool(), &id).await?;

    // Parse config JSON
    let config = db_plugin.get_config();

    let template = PluginConfigFormTemplate {
        plugin: db_plugin_to_ui(db_plugin),
        config_schedule: config
            .get("schedule")
            .and_then(|v| v.as_str())
            .unwrap_or("0 */5 * * * *")
            .to_string(),
        // Weather plugin
        config_api_key: config
            .get("api_key")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        config_zip: config
            .get("zip")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        config_location: config
            .get("location")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        config_units: config
            .get("units")
            .and_then(|v| v.as_str())
            .unwrap_or("imperial")
            .to_string(),
        // Speedtest plugin
        config_min_down: config
            .get("min_down")
            .and_then(|v| v.as_i64())
            .map(|v| v.to_string())
            .unwrap_or_else(|| "100".to_string()),
        config_min_up: config
            .get("min_up")
            .and_then(|v| v.as_i64())
            .map(|v| v.to_string())
            .unwrap_or_else(|| "20".to_string()),
        // Docker plugin
        config_send_summary: config
            .get("send_summary")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        config_cpu_warn_pct: config
            .get("cpu_warn_pct")
            .and_then(|v| v.as_f64())
            .map(|v| v.to_string())
            .unwrap_or_else(|| "80.0".to_string()),
        config_mem_warn_pct: config
            .get("mem_warn_pct")
            .and_then(|v| v.as_f64())
            .map(|v| v.to_string())
            .unwrap_or_else(|| "80.0".to_string()),
        // Updates plugin
        config_updates_send_summary: config
            .get("send_summary")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        // Health plugin
        config_health_send_summary: config
            .get("send_summary")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        config_health_cpu_warn_pct: config
            .get("cpu_warn_pct")
            .and_then(|v| v.as_f64())
            .map(|v| v.to_string())
            .unwrap_or_else(|| "80.0".to_string()),
        config_health_mem_warn_pct: config
            .get("mem_warn_pct")
            .and_then(|v| v.as_f64())
            .map(|v| v.to_string())
            .unwrap_or_else(|| "80.0".to_string()),
        config_health_disk_warn_pct: config
            .get("disk_warn_pct")
            .and_then(|v| v.as_f64())
            .map(|v| v.to_string())
            .unwrap_or_else(|| "85.0".to_string()),
        error: None,
    };

    Ok(Html(template.render()?))
}

/// Plugin configuration save handler
async fn plugin_config_save(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(input): Form<PluginConfigInput>,
) -> Result<Html<String>, AppError> {
    tracing::info!("Saving plugin config: {} {:?}", id, input);

    // Extract schedule first to avoid move issues
    let schedule = input
        .schedule
        .clone()
        .unwrap_or_else(|| "0 */5 * * * *".to_string());

    // Validate cron expression before saving
    use cron::Schedule;
    use std::str::FromStr;
    if let Err(e) = Schedule::from_str(&schedule) {
        tracing::error!("Invalid cron expression '{}': {}", schedule, e);
        return Ok(Html(format!(
            r#"<div class="alert alert-error">Invalid cron expression '{}': {}. Please use format: SEC MIN HOUR DAY MONTH DAYOFWEEK (e.g., "0 */5 * * * *")</div>"#,
            schedule, e
        )));
    }

    // Build config JSON based on plugin type
    let config_json = if id == "weather" {
        serde_json::json!({
            "schedule": schedule,
            "api_key": input.api_key.unwrap_or_default(),
            "zip": input.zip.unwrap_or_default(),
            "location": input.location.unwrap_or_default(),
            "units": input.units.unwrap_or_else(|| "imperial".to_string()),
        })
    } else if id == "speedtest" {
        serde_json::json!({
            "schedule": schedule,
            "min_down": input.min_down.and_then(|s| s.parse::<i64>().ok()).unwrap_or(100),
            "min_up": input.min_up.and_then(|s| s.parse::<i64>().ok()).unwrap_or(20),
        })
    } else if id == "docker" {
        serde_json::json!({
            "schedule": schedule,
            "send_summary": input.send_summary.is_some(),
            "cpu_warn_pct": input.cpu_warn_pct.and_then(|s| s.parse::<f64>().ok()).unwrap_or(80.0),
            "mem_warn_pct": input.mem_warn_pct.and_then(|s| s.parse::<f64>().ok()).unwrap_or(80.0),
        })
    } else if id == "updates" {
        serde_json::json!({
            "schedule": schedule,
            "send_summary": input.updates_send_summary.is_some(),
        })
    } else if id == "health" {
        serde_json::json!({
            "schedule": schedule,
            "send_summary": input.health_send_summary.is_some(),
            "cpu_warn_pct": input.health_cpu_warn_pct.and_then(|s| s.parse::<f64>().ok()).unwrap_or(80.0),
            "mem_warn_pct": input.health_mem_warn_pct.and_then(|s| s.parse::<f64>().ok()).unwrap_or(80.0),
            "disk_warn_pct": input.health_disk_warn_pct.and_then(|s| s.parse::<f64>().ok()).unwrap_or(85.0),
        })
    } else {
        serde_json::json!({
            "schedule": schedule,
        })
    };

    // Update plugin in database
    let db = state.db().await;
    let update = svrctlrs_database::models::plugin::UpdatePlugin {
        enabled: None,
        config: Some(config_json.clone()),
    };
    queries::plugins::update_plugin(db.pool(), &id, &update).await?;

    // Create or update scheduled task for this plugin (schedule already extracted above)

    // Check if task already exists for this plugin
    let existing_tasks = queries::tasks::list_tasks(db.pool()).await?;
    let existing_task = existing_tasks.iter().find(|t| t.feature_id == id);

    if let Some(task) = existing_task {
        // Update existing task
        let update_task = svrctlrs_database::models::task::UpdateTask {
            name: None,
            description: None,
            schedule: Some(schedule.clone()),
            enabled: Some(true),
            command: None,
            args: None,
            timeout: None,
        };
        queries::tasks::update_task(db.pool(), task.id, &update_task).await?;
    } else {
        // Create new task for local execution (server_id = NULL)
        // Local plugin tasks run on the SvrCtlRS host without SSH
        let create_task = svrctlrs_database::models::task::CreateTask {
            name: format!("{} Task", id),
            description: Some(format!("Scheduled task for {} plugin", id)),
            feature_id: id.clone(),
            server_id: None,   // NULL = local execution
            server_name: None, // No server name for local tasks
            schedule: schedule.clone(),
            command: "execute".to_string(),
            args: Some(config_json),
            timeout: 300,
        };
        queries::tasks::create_task(db.pool(), &create_task).await?;
    }

    // Return success message
    Ok(Html("<div class=\"alert alert-success\">Configuration saved successfully! Task created/updated.</div>".to_string()))
}
