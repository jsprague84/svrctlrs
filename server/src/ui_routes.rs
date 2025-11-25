//! UI routes for HTMX frontend

use askama::Template;
use axum::{
    extract::{Path, State},
    response::{Html, IntoResponse, Redirect},
    routing::{get, post, put},
    Form, Router,
};
use serde::Deserialize;
use tower_http::services::ServeDir;
use svrctlrs_database::{models::server as db_server, queries};

use crate::{state::AppState, templates::*};

/// Create UI router with all page and component routes
pub fn ui_routes() -> Router<AppState> {
    Router::new()
        // Pages
        .route("/", get(dashboard_page))
        .route("/servers", get(servers_page))
        .route("/tasks", get(tasks_page))
        .route("/plugins", get(plugins_page))
        .route("/settings", get(settings_page))
        
        // Server CRUD
        .route("/servers/new", get(server_form_new))
        .route("/servers", post(server_create))
        .route("/servers/test", post(server_test_connection))
        .route("/servers/{id}/edit", get(server_form_edit))
        .route("/servers/{id}", put(server_update).delete(server_delete))
        
        // Task list (for auto-refresh) and manual execution
        .route("/tasks/list", get(task_list))
        .route("/tasks/{id}/run", post(task_run_now))
        
        // Plugin toggle and configuration
        .route("/plugins/{id}/toggle", post(plugin_toggle))
        .route("/plugins/{id}/config", get(plugin_config_form).put(plugin_config_save))
        
        // Notification settings
        .route("/settings/notifications", get(notifications_page))
        .route("/settings/notifications/new", get(notification_form_new))
        .route("/settings/notifications", post(notification_create))
        .route("/settings/notifications/{id}/edit", get(notification_form_edit))
        .route("/settings/notifications/{id}", put(notification_update).delete(notification_delete))
        
        // Auth
        .route("/auth/login", get(login_page).post(login))
        .route("/auth/logout", post(logout))
        
        // Static files
        .nest_service(
            "/static",
            ServeDir::new(
                std::env::var("STATIC_DIR")
                    .unwrap_or_else(|_| "server/static".to_string())
            )
        )
        
        // 404 handler
        .fallback(not_found)
}

// ============================================================================
// Helper: Get user from session (placeholder for now)
// ============================================================================

async fn get_user_from_session() -> Option<User> {
    // TODO: Implement session management with tower-sessions
    // For now, return None (no auth)
    None
}

// ============================================================================
// Helper: Convert database server model to UI model
// ============================================================================

fn db_server_to_ui(db: db_server::Server) -> Server {
    Server {
        id: db.id,
        name: db.name,
        host: db.host.unwrap_or_default(),
        port: Some(db.port),
        username: Some(db.username),
        description: db.description,
        enabled: db.enabled,
    }
}

// ============================================================================
// Dashboard
// ============================================================================

async fn dashboard_page(State(state): State<AppState>) -> Result<Html<String>, AppError> {
    let user = get_user_from_session().await;
    
    // Get stats
    let plugins = state.plugins.read().await;
    let enabled_plugins = plugins.plugins().len();
    
    // Get server count from database
    let db = state.db().await;
    let servers = queries::servers::list_servers(db.pool()).await?;
    let total_servers = servers.len();
    
    let stats = DashboardStats {
        total_servers,
        active_tasks: 0,  // TODO: Track active tasks
        enabled_plugins,
        total_tasks: 0,   // TODO: Track total tasks
    };
    
    let template = DashboardTemplate { user, stats };
    Ok(Html(template.render()?))
}

// ============================================================================
// Servers
// ============================================================================

async fn servers_page(State(state): State<AppState>) -> Result<Html<String>, AppError> {
    let user = get_user_from_session().await;
    
    // Load servers from database
    let db = state.db().await;
    let db_servers = queries::servers::list_servers(db.pool()).await?;
    let servers = db_servers.into_iter().map(db_server_to_ui).collect();
    
    let template = ServersTemplate { user, servers };
    Ok(Html(template.render()?))
}

async fn server_form_new() -> Result<Html<String>, AppError> {
    let template = ServerFormTemplate {
        server: None,
        error: None,
    };
    Ok(Html(template.render()?))
}

async fn server_form_edit(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Html<String>, AppError> {
    // Load server from database
    let db = state.db().await;
    let db_server = queries::servers::get_server(db.pool(), id).await;
    
    let (server, error) = match db_server {
        Ok(s) => (Some(db_server_to_ui(s)), None),
        Err(e) => {
            tracing::warn!("Failed to load server {}: {}", id, e);
            (None, Some(format!("Server with ID {} not found", id)))
        }
    };
    
    let template = ServerFormTemplate { server, error };
    Ok(Html(template.render()?))
}

async fn server_create(
    State(state): State<AppState>,
    Form(input): Form<CreateServerInput>,
) -> Result<Html<String>, AppError> {
    // Validate
    if input.name.is_empty() || input.host.is_empty() {
        let template = ServerFormTemplate {
            server: None,
            error: Some("Name and host are required".to_string()),
        };
        return Ok(Html(template.render()?));
    }
    
    // Save to database
    tracing::info!("Creating server: {} @ {}", input.name, input.host);
    let db = state.db().await;
    
    let create_server = db_server::CreateServer {
        name: input.name.clone(),
        host: input.host.clone(),
        port: input.port.unwrap_or(22),
        username: input.username.unwrap_or_else(|| "root".to_string()),
        ssh_key_path: None,
        description: input.description,
        tags: None,
    };
    
    // Try to create, handle duplicate name error
    match queries::servers::create_server(db.pool(), &create_server).await {
        Ok(_) => {
            // Success - return updated list with success message
            let db_servers = queries::servers::list_servers(db.pool()).await?;
            let servers = db_servers.into_iter().map(db_server_to_ui).collect();
            let template = ServerListTemplate { servers };
            let list_html = template.render()?;
            
            // Prepend success message
            Ok(Html(format!(
                r#"<div class="alert alert-success">✓ Server '{}' created successfully!</div>{}"#,
                input.name, list_html
            )))
        }
        Err(e) => {
            // Check if it's a duplicate name error
            let error_msg = e.to_string();
            if error_msg.contains("UNIQUE constraint") && error_msg.contains("servers.name") {
                Ok(Html(format!(
                    r#"<div class="alert alert-error">✗ A server with the name '{}' already exists. Please use a different name.</div>"#,
                    input.name
                )))
            } else {
                // Other database error
                Err(e.into())
            }
        }
    }
}

async fn server_update(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Form(input): Form<UpdateServerInput>,
) -> Result<Html<String>, AppError> {
    // Update in database
    tracing::info!("Updating server {}: {:?}", id, input);
    let db = state.db().await;
    
    // Get the server name for the success message
    let server_name = if let Some(ref name) = input.name {
        name.clone()
    } else {
        // If name wasn't changed, get it from database
        queries::servers::get_server(db.pool(), id)
            .await?
            .name
    };
    
    let update_server = db_server::UpdateServer {
        name: input.name,
        host: input.host,
        port: input.port,
        username: input.username,
        ssh_key_path: None,
        description: input.description,
        tags: None,
        enabled: input.enabled,
        connection_timeout: None,
        retry_attempts: None,
    };
    
    // Try to update, handle duplicate name error
    match queries::servers::update_server(db.pool(), id, &update_server).await {
        Ok(_) => {
            // Success - return updated list with success message
            let db_servers = queries::servers::list_servers(db.pool()).await?;
            let servers = db_servers.into_iter().map(db_server_to_ui).collect();
            let template = ServerListTemplate { servers };
            let list_html = template.render()?;
            
            // Prepend success message
            Ok(Html(format!(
                r#"<div class="alert alert-success">✓ Server '{}' updated successfully!</div>{}"#,
                server_name, list_html
            )))
        }
        Err(e) => {
            // Check if it's a duplicate name error
            let error_msg = e.to_string();
            if error_msg.contains("UNIQUE constraint") && error_msg.contains("servers.name") {
                Ok(Html(format!(
                    r#"<div class="alert alert-error">✗ A server with that name already exists. Please use a different name.</div>"#
                )))
            } else {
                // Other database error
                Err(e.into())
            }
        }
    }
}

async fn server_delete(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, AppError> {
    // Get server name before deleting
    let db = state.db().await;
    let server_name = queries::servers::get_server(db.pool(), id)
        .await
        .map(|s| s.name)
        .unwrap_or_else(|_| format!("Server {}", id));
    
    // Delete from database
    tracing::info!("Deleting server {}", id);
    queries::servers::delete_server(db.pool(), id).await?;
    
    // Return success message
    Ok(Html(format!(
        r#"<div class="alert alert-success">✓ Server '{}' deleted successfully!</div>"#,
        server_name
    )))
}

#[derive(Debug, Deserialize)]
struct TestConnectionInput {
    host: String,
    port: Option<i32>,
    username: Option<String>,
}

async fn server_test_connection(
    State(_state): State<AppState>,
    Form(input): Form<TestConnectionInput>,
) -> Result<Html<String>, AppError> {
    let port = input.port.unwrap_or(22);
    let username = input.username.unwrap_or_else(|| "root".to_string());
    
    tracing::info!("Testing SSH connection to {}@{}:{}", username, input.host, port);
    
    // Create SSH config
    let ssh_config = crate::ssh::SshConfig {
        host: input.host.clone(),
        port: port as u16,
        username: username.clone(),
        key_path: None, // Will use default SSH keys
        timeout: std::time::Duration::from_secs(10),
    };
    
    // Test the connection
    match crate::ssh::test_connection(&ssh_config).await {
        Ok(message) => {
            tracing::info!("SSH connection test successful: {}", message);
            Ok(Html(format!(
                r#"<div class="alert alert-success">✓ Successfully connected to {}@{}:{}<br><small>{}</small></div>"#,
                username, input.host, port, message
            )))
        }
        Err(e) => {
            tracing::error!("SSH connection test failed: {}", e);
            Ok(Html(format!(
                r#"<div class="alert alert-error">✗ Failed to connect to {}@{}:{}<br><small>{}</small></div>"#,
                username, input.host, port, e
            )))
        }
    }
}

// ============================================================================
// Tasks
// ============================================================================

async fn tasks_page(State(state): State<AppState>) -> Result<Html<String>, AppError> {
    let user = get_user_from_session().await;
    let tasks = get_tasks(&state).await;
    
    let template = TasksTemplate { user, tasks };
    Ok(Html(template.render()?))
}

async fn task_list(State(state): State<AppState>) -> Result<Html<String>, AppError> {
    let tasks = get_tasks(&state).await;
    let template = TaskListTemplate { tasks };
    Ok(Html(template.render()?))
}

async fn get_tasks(state: &AppState) -> Vec<Task> {
    // Load tasks from database
    let db = state.db().await;
    let db_tasks = queries::tasks::list_tasks(db.pool()).await.unwrap_or_default();
    
    db_tasks.into_iter().map(|t| Task {
        id: t.id,
        name: t.name,
        description: t.description,
        plugin_id: t.plugin_id,
        schedule: t.schedule,
        last_run_at: t.last_run_at.map(|dt| dt.to_rfc3339()),
        next_run_at: t.next_run_at.map(|dt| dt.to_rfc3339()),
    }).collect()
}

async fn task_run_now(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Html<String>, AppError> {
    tracing::info!("Running task {} manually", id);
    
    // Execute task using the executor
    match crate::executor::execute_task(&state, id).await {
        Ok(result) => {
            if result.success {
                // Escape HTML in output to prevent XSS
                let escaped_output = result.output
                    .replace('&', "&amp;")
                    .replace('<', "&lt;")
                    .replace('>', "&gt;")
                    .replace('"', "&quot;");
                
                Ok(Html(format!(
                    r#"<div class="alert alert-success">✓ Task executed successfully in {}ms<br><pre>{}</pre></div>"#,
                    result.duration_ms,
                    escaped_output
                )))
            } else {
                Ok(Html(format!(
                    r#"<div class="alert alert-error">✗ Task execution failed: {}</div>"#,
                    result.error.unwrap_or_else(|| "Unknown error".to_string())
                )))
            }
        }
        Err(e) => {
            tracing::error!("Failed to execute task {}: {}", id, e);
            Ok(Html(format!(
                r#"<div class="alert alert-error">✗ Failed to execute task: {}</div>"#,
                e
            )))
        }
    }
}

// ============================================================================
// Plugins
// ============================================================================

async fn plugins_page(State(state): State<AppState>) -> Result<Html<String>, AppError> {
    let user = get_user_from_session().await;
    
    // Load plugins from database
    let db = state.db().await;
    let db_plugins = queries::plugins::list_plugins(db.pool()).await?;
    let plugins = db_plugins.into_iter().map(db_plugin_to_ui).collect();
    
    let template = PluginsTemplate { user, plugins };
    Ok(Html(template.render()?))
}

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

fn db_plugin_to_ui(db: svrctlrs_database::models::plugin::Plugin) -> Plugin {
    Plugin {
        id: db.id,
        name: db.name,
        description: db.description.unwrap_or_default(),
        version: "1.0.0".to_string(), // TODO: Get from plugin metadata
        enabled: db.enabled,
    }
}

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
        config_schedule: config.get("schedule").and_then(|v| v.as_str()).unwrap_or("0 */5 * * * *").to_string(),
        config_api_key: config.get("api_key").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        config_zip: config.get("zip").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        config_location: config.get("location").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        config_units: config.get("units").and_then(|v| v.as_str()).unwrap_or("imperial").to_string(),
        config_min_down: config.get("min_down").and_then(|v| v.as_i64()).map(|v| v.to_string()).unwrap_or_else(|| "100".to_string()),
        config_min_up: config.get("min_up").and_then(|v| v.as_i64()).map(|v| v.to_string()).unwrap_or_else(|| "20".to_string()),
        error: None,
    };
    
    Ok(Html(template.render()?))
}

async fn plugin_config_save(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(input): Form<PluginConfigInput>,
) -> Result<Html<String>, AppError> {
    tracing::info!("Saving plugin config: {} {:?}", id, input);
    
    // Extract schedule first to avoid move issues
    let schedule = input.schedule.clone().unwrap_or_else(|| "0 */5 * * * *".to_string());
    
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
    let existing_task = existing_tasks.iter().find(|t| t.plugin_id == id);
    
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
        // Create new task
        let create_task = svrctlrs_database::models::task::CreateTask {
            name: format!("{} Task", id),
            description: Some(format!("Scheduled task for {} plugin", id)),
            plugin_id: id.clone(),
            server_id: None, // Run on all servers
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

// ============================================================================
// Settings
// ============================================================================

async fn settings_page() -> Result<Html<String>, AppError> {
    let user = get_user_from_session().await;
    let template = SettingsTemplate { user };
    Ok(Html(template.render()?))
}

// ============================================================================
// Notifications
// ============================================================================

async fn notifications_page(State(state): State<AppState>) -> Result<Html<String>, AppError> {
    let user = get_user_from_session().await;
    
    // Load notification backends from database
    let db = state.db().await;
    let db_notifications = queries::notifications::list_notification_backends(db.pool()).await?;
    let notifications = db_notifications.into_iter().map(db_notification_to_ui).collect();
    
    let template = NotificationsTemplate { user, notifications };
    Ok(Html(template.render()?))
}

async fn notification_form_new() -> Result<Html<String>, AppError> {
    tracing::info!("notification_form_new called - loading add backend form");
    let template = NotificationFormTemplate {
        notification: None,
        config_url: String::new(),
        config_token: String::new(),
        config_topic: String::new(),
        error: None,
    };
    Ok(Html(template.render()?))
}

async fn notification_form_edit(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Html<String>, AppError> {
    // Load notification backend from database
    let db = state.db().await;
    let db_notification = queries::notifications::get_notification_backend(db.pool(), id).await;
    
    let (notification, error) = match db_notification {
        Ok(n) => {
            let config = n.get_config();
            let template_notification = Some(db_notification_to_ui(n));
            let config_url = config.get("url").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let config_token = config.get("token").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let config_topic = config.get("topic").and_then(|v| v.as_str()).unwrap_or("").to_string();
            
            let template = NotificationFormTemplate {
                notification: template_notification,
                config_url,
                config_token,
                config_topic,
                error: None,
            };
            return Ok(Html(template.render()?));
        }
        Err(e) => {
            tracing::warn!("Failed to load notification backend {}: {}", id, e);
            (None, Some(format!("Notification backend with ID {} not found", id)))
        }
    };
    
    let template = NotificationFormTemplate {
        notification,
        config_url: String::new(),
        config_token: String::new(),
        config_topic: String::new(),
        error,
    };
    Ok(Html(template.render()?))
}

async fn notification_create(
    State(state): State<AppState>,
    Form(input): Form<CreateNotificationInput>,
) -> Result<Html<String>, AppError> {
    tracing::info!("notification_create called with: name={}, type={}, url={:?}, token={:?}, topic={:?}", 
        input.name, input.backend_type, input.url, input.token, input.topic);
    
    // Validate
    if input.name.is_empty() || input.backend_type.is_empty() {
        let template = NotificationFormTemplate {
            notification: None,
            config_url: String::new(),
            config_token: String::new(),
            config_topic: String::new(),
            error: Some("Name and backend type are required".to_string()),
        };
        return Ok(Html(template.render()?));
    }
    
    // Build config JSON based on backend type
    let config_json = if input.backend_type == "gotify" {
        serde_json::json!({
            "url": input.url.unwrap_or_default(),
            "token": input.token.unwrap_or_default(),
        })
    } else {
        serde_json::json!({
            "url": input.url.unwrap_or_default(),
            "topic": input.topic.unwrap_or_default(),
            "token": input.token.unwrap_or_default(),
        })
    };
    
    // Save to database
    tracing::info!("Creating notification backend: {} ({})", input.name, input.backend_type);
    let db = state.db().await;
    
    let create_backend = svrctlrs_database::models::notification::CreateNotificationBackend {
        backend_type: input.backend_type.clone(),
        name: input.name.clone(),
        config: config_json,
        priority: input.priority.unwrap_or(5),
    };
    
    match queries::notifications::create_notification_backend(db.pool(), &create_backend).await {
        Ok(_) => {
            // Success - return updated list with success message
            let db_notifications = queries::notifications::list_notification_backends(db.pool()).await?;
            let notifications = db_notifications.into_iter().map(db_notification_to_ui).collect();
            let template = NotificationListTemplate { notifications };
            let list_html = template.render()?;
            
            Ok(Html(format!(
                r#"<div class="alert alert-success">✓ Notification backend '{}' ({}) created successfully!</div>{}"#,
                input.name, input.backend_type, list_html
            )))
        }
        Err(e) => {
            // Check if it's a duplicate name error
            let error_msg = e.to_string();
            if error_msg.contains("UNIQUE constraint") {
                Ok(Html(format!(
                    r#"<div class="alert alert-error">✗ A notification backend with the name '{}' already exists. Please use a different name.</div>"#,
                    input.name
                )))
            } else {
                Err(e.into())
            }
        }
    }
}

async fn notification_update(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Form(input): Form<UpdateNotificationInput>,
) -> Result<Html<String>, AppError> {
    tracing::info!("Updating notification backend {}: {:?}", id, input);
    
    // Get existing backend to determine type
    let db = state.db().await;
    let existing = queries::notifications::get_notification_backend(db.pool(), id).await?;
    
    // Build config JSON based on backend type
    let config_json = if existing.backend_type == "gotify" {
        serde_json::json!({
            "url": input.url.unwrap_or_default(),
            "token": input.token.unwrap_or_default(),
        })
    } else {
        serde_json::json!({
            "url": input.url.unwrap_or_default(),
            "topic": input.topic.unwrap_or_default(),
            "token": input.token.unwrap_or_default(),
        })
    };
    
    // Get backend name for success message
    let backend_name = if let Some(ref name) = input.name {
        name.clone()
    } else {
        existing.name.clone()
    };
    
    // Update in database
    let update_backend = svrctlrs_database::models::notification::UpdateNotificationBackend {
        name: input.name,
        enabled: input.enabled.map(|s| s == "on"),
        config: Some(config_json),
        priority: input.priority,
    };
    
    match queries::notifications::update_notification_backend(db.pool(), id, &update_backend).await {
        Ok(_) => {
            // Success - return updated list with success message
            let db_notifications = queries::notifications::list_notification_backends(db.pool()).await?;
            let notifications = db_notifications.into_iter().map(db_notification_to_ui).collect();
            let template = NotificationListTemplate { notifications };
            let list_html = template.render()?;
            
            Ok(Html(format!(
                r#"<div class="alert alert-success">✓ Notification backend '{}' updated successfully!</div>{}"#,
                backend_name, list_html
            )))
        }
        Err(e) => {
            let error_msg = e.to_string();
            if error_msg.contains("UNIQUE constraint") {
                Ok(Html(format!(
                    r#"<div class="alert alert-error">✗ A notification backend with that name already exists. Please use a different name.</div>"#
                )))
            } else {
                Err(e.into())
            }
        }
    }
}

async fn notification_delete(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, AppError> {
    // Get backend name before deleting
    let db = state.db().await;
    let backend_name = queries::notifications::get_notification_backend(db.pool(), id)
        .await
        .map(|b| b.name)
        .unwrap_or_else(|_| format!("Backend {}", id));
    
    tracing::info!("Deleting notification backend {}", id);
    queries::notifications::delete_notification_backend(db.pool(), id).await?;
    
    // Return success message
    Ok(Html(format!(
        r#"<div class="alert alert-success">✓ Notification backend '{}' deleted successfully!</div>"#,
        backend_name
    )))
}

fn db_notification_to_ui(db: svrctlrs_database::models::notification::NotificationBackend) -> NotificationBackend {
    NotificationBackend {
        id: db.id,
        backend_type: db.backend_type,
        name: db.name,
        enabled: db.enabled,
        priority: db.priority,
    }
}

// ============================================================================
// Auth
// ============================================================================

async fn login_page() -> Result<Html<String>, AppError> {
    let template = LoginTemplate { error: None };
    Ok(Html(template.render()?))
}

async fn login(
    State(_state): State<AppState>,
    Form(creds): Form<LoginForm>,
) -> Result<impl IntoResponse, AppError> {
    // TODO: Implement authentication
    tracing::info!("Login attempt: {}", creds.username);
    
    // For now, just redirect to dashboard
    Ok(Redirect::to("/"))
}

async fn logout() -> Result<impl IntoResponse, AppError> {
    // TODO: Clear session
    Ok(Redirect::to("/auth/login"))
}

// ============================================================================
// Error Handling
// ============================================================================

async fn not_found() -> Result<Html<String>, AppError> {
    let user = get_user_from_session().await;
    let template = NotFoundTemplate { user };
    Ok(Html(template.render()?))
}

// Custom error type
pub struct AppError(anyhow::Error);

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        tracing::error!("Application error: {:?}", self.0);
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("Internal server error: {}", self.0),
        )
            .into_response()
    }
}

impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}

