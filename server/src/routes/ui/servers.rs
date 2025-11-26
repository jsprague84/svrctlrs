//! Server management routes

use askama::Template;
use axum::{
    extract::{Path, State},
    response::{Html, IntoResponse},
    routing::{get, post, put},
    Form, Router,
};
use serde::Deserialize;
use svrctlrs_database::{models::server as db_server, queries};

use crate::{state::AppState, templates::*};
use super::{get_user_from_session, AppError};

/// Create servers router
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/servers", get(servers_page).post(server_create))
        .route("/servers/new", get(server_form_new))
        .route("/servers/test", post(server_test_connection))
        .route("/servers/{id}/edit", get(server_form_edit))
        .route("/servers/{id}", put(server_update).delete(server_delete))
}

/// Helper to convert DB server to UI model
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

/// Servers page handler
async fn servers_page(State(state): State<AppState>) -> Result<Html<String>, AppError> {
    let user = get_user_from_session().await;

    // Load servers from database
    let db = state.db().await;
    let db_servers = queries::servers::list_servers(db.pool()).await?;
    let servers = db_servers.into_iter().map(db_server_to_ui).collect();

    let template = ServersTemplate { user, servers };
    Ok(Html(template.render()?))
}

/// New server form
async fn server_form_new() -> Result<Html<String>, AppError> {
    let template = ServerFormTemplate {
        server: None,
        error: None,
    };
    Ok(Html(template.render()?))
}

/// Edit server form
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

/// Create server handler
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

/// Update server handler
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
        queries::servers::get_server(db.pool(), id).await?.name
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
                Ok(Html(
                    r#"<div class="alert alert-error">✗ A server with that name already exists. Please use a different name.</div>"#.to_string()
                ))
            } else {
                // Other database error
                Err(e.into())
            }
        }
    }
}

/// Delete server handler
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

/// Test SSH connection input
#[derive(Debug, Deserialize)]
struct TestConnectionInput {
    host: String,
    port: Option<i32>,
    username: Option<String>,
}

/// Test SSH connection handler
async fn server_test_connection(
    State(_state): State<AppState>,
    Form(input): Form<TestConnectionInput>,
) -> Result<Html<String>, AppError> {
    let port = input.port.unwrap_or(22);
    let username = input.username.unwrap_or_else(|| "root".to_string());

    tracing::info!(
        "Testing SSH connection to {}@{}:{}",
        username,
        input.host,
        port
    );

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
