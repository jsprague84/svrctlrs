//! UI routes for HTMX frontend

use askama::Template;
use axum::{
    extract::{Path, State},
    response::{Html, IntoResponse, Redirect},
    routing::{delete, get, post, put},
    Form, Router,
};
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
        .route("/servers/{id}/edit", get(server_form_edit))
        .route("/servers/{id}", put(server_update).delete(server_delete))
        
        // Task list (for auto-refresh)
        .route("/tasks/list", get(task_list))
        
        // Plugin toggle
        .route("/plugins/{id}/toggle", post(plugin_toggle))
        
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
        name: input.name,
        host: input.host,
        port: input.port.unwrap_or(22),
        username: input.username.unwrap_or_else(|| "root".to_string()),
        ssh_key_path: None,
        description: input.description,
        tags: None,
    };
    
    queries::servers::create_server(db.pool(), &create_server).await?;
    
    // Return updated server list
    let db_servers = queries::servers::list_servers(db.pool()).await?;
    let servers = db_servers.into_iter().map(db_server_to_ui).collect();
    let template = ServerListTemplate { servers };
    Ok(Html(template.render()?))
}

async fn server_update(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Form(input): Form<UpdateServerInput>,
) -> Result<Html<String>, AppError> {
    // Update in database
    tracing::info!("Updating server {}: {:?}", id, input);
    let db = state.db().await;
    
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
    
    queries::servers::update_server(db.pool(), id, &update_server).await?;
    
    // Return updated server list
    let db_servers = queries::servers::list_servers(db.pool()).await?;
    let servers = db_servers.into_iter().map(db_server_to_ui).collect();
    let template = ServerListTemplate { servers };
    Ok(Html(template.render()?))
}

async fn server_delete(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, AppError> {
    // Delete from database
    tracing::info!("Deleting server {}", id);
    let db = state.db().await;
    
    queries::servers::delete_server(db.pool(), id).await?;
    
    // Return empty response (HTMX will remove the element)
    Ok(Html(""))
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

async fn get_tasks(_state: &AppState) -> Vec<Task> {
    // TODO: Implement task tracking
    // For now, return empty list
    vec![]
}

// ============================================================================
// Plugins
// ============================================================================

async fn plugins_page(State(state): State<AppState>) -> Result<Html<String>, AppError> {
    let user = get_user_from_session().await;
    let plugins = get_plugins(&state).await;
    
    let template = PluginsTemplate { user, plugins };
    Ok(Html(template.render()?))
}

async fn plugin_toggle(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Html<String>, AppError> {
    // TODO: Implement plugin enable/disable
    tracing::info!("Toggling plugin: {}", id);
    
    let plugins = get_plugins(&state).await;
    let template = PluginListTemplate { plugins };
    Ok(Html(template.render()?))
}

async fn get_plugins(state: &AppState) -> Vec<Plugin> {
    let plugins = state.plugins.read().await;
    let plugin_list = plugins.plugins();
    
    plugin_list
        .into_iter()
        .map(|plugin| {
            let meta = plugin.metadata();
            Plugin {
                id: meta.name.clone(),
                name: meta.name,
                description: meta.description,
                version: meta.version,
                enabled: true, // TODO: Track enabled state
            }
        })
        .collect()
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

