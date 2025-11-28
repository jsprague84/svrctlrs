//! Server management routes (updated for new schema)

use askama::Template;
use axum::{
    extract::{Path, State},
    response::{Html, IntoResponse},
    routing::{get, post, put},
    Form, Router,
};
use serde::Deserialize;
use svrctlrs_database::{
    models::{CreateServer, Server, UpdateServer},
    queries::{
        credentials, servers_new as servers_queries, tags,
    },
};

use super::{get_user_from_session, AppError};
use crate::{state::AppState, templates::*};

/// Create servers router
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/servers", get(servers_page).post(server_create))
        .route("/servers/new", get(server_form_new))
        .route("/servers/list", get(server_list))
        .route("/servers/test", post(server_test_connection))
        .route("/servers/{id}/edit", get(server_form_edit))
        .route(
            "/servers/{id}",
            put(server_update).delete(server_delete),
        )
        .route("/servers/{id}/test", post(server_test_by_id))
        .route("/servers/{id}/capabilities", get(server_capabilities))
        .route("/servers/{id}/tags", post(server_add_tag).delete(server_remove_tag))
}

/// Servers page handler
async fn servers_page(State(state): State<AppState>) -> Result<Html<String>, AppError> {
    let user = get_user_from_session().await;

    // Load servers, credentials, and tags from database
    let db = state.db().await;
    let servers = servers_queries::list_servers(db.pool()).await?;
    let credentials_list = credentials::list_credentials(db.pool()).await?;
    let tags_list = tags::list_tags(db.pool()).await?;

    // Convert to display models
    let servers_display: Vec<ServerDisplay> = servers.into_iter().map(|s| server_to_display(&s)).collect();
    let credentials_display: Vec<CredentialDisplay> = credentials_list
        .into_iter()
        .map(|c| credential_to_display(&c))
        .collect();
    let tags_display: Vec<TagDisplay> = tags_list
        .into_iter()
        .map(|t| TagDisplay {
            id: t.id,
            name: t.name.clone(),
            color: t.color_or_default(),
            description: t.description.clone(),
            server_count: 0, // Will be filled by tags query if needed
            created_at: t.created_at.to_rfc3339(),
        })
        .collect();

    let template = ServersTemplate {
        user,
        servers: servers_display,
        credentials: credentials_display,
        tags: tags_display,
    };
    Ok(Html(template.render()?))
}

/// Get server list (HTMX component)
async fn server_list(State(state): State<AppState>) -> Result<Html<String>, AppError> {
    let db = state.db().await;
    let servers = servers_queries::list_servers(db.pool()).await?;
    let servers_display: Vec<ServerDisplay> = servers.into_iter().map(|s| server_to_display(&s)).collect();

    let template = ServerListTemplate { servers: servers_display };
    Ok(Html(template.render()?))
}

/// New server form
async fn server_form_new(State(state): State<AppState>) -> Result<Html<String>, AppError> {
    let db = state.db().await;
    let credentials_list = credentials::list_credentials(db.pool()).await?;
    let tags_list = tags::list_tags(db.pool()).await?;

    let credentials_display: Vec<CredentialDisplay> = credentials_list
        .into_iter()
        .map(|c| credential_to_display(&c))
        .collect();
    let tags_display: Vec<TagDisplay> = tags_list
        .into_iter()
        .map(|t| TagDisplay {
            id: t.id,
            name: t.name.clone(),
            color: t.color_or_default(),
            description: t.description.clone(),
            server_count: 0,
            created_at: t.created_at.to_rfc3339(),
        })
        .collect();

    let template = ServerFormTemplate {
        server: None,
        credentials: credentials_display,
        tags: tags_display,
        selected_tags: vec![],
        error: None,
    };
    Ok(Html(template.render()?))
}

/// Edit server form
async fn server_form_edit(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Html<String>, AppError> {
    let db = state.db().await;
    let server_result = servers_queries::get_server(db.pool(), id).await;
    let credentials_list = credentials::list_credentials(db.pool()).await?;
    let tags_list = tags::list_tags(db.pool()).await?;
    let server_tags = tags::get_tags_for_server(db.pool(), id).await.unwrap_or_default();

    let credentials_display: Vec<CredentialDisplay> = credentials_list
        .into_iter()
        .map(|c| credential_to_display(&c))
        .collect();
    let tags_display: Vec<TagDisplay> = tags_list
        .into_iter()
        .map(|t| TagDisplay {
            id: t.id,
            name: t.name.clone(),
            color: t.color_or_default(),
            description: t.description.clone(),
            server_count: 0,
            created_at: t.created_at.to_rfc3339(),
        })
        .collect();
    let selected_tags: Vec<i64> = server_tags.iter().map(|t| t.id).collect();

    let (server, error) = match server_result {
        Ok(s) => (Some(server_to_display(&s)), None),
        Err(e) => {
            tracing::warn!("Failed to load server {}: {}", id, e);
            (None, Some(format!("Server with ID {} not found", id)))
        }
    };

    let template = ServerFormTemplate {
        server,
        credentials: credentials_display,
        tags: tags_display,
        selected_tags,
        error,
    };
    Ok(Html(template.render()?))
}

/// Create server input (from form)
#[derive(Debug, Deserialize)]
struct CreateServerInput {
    name: String,
    hostname: Option<String>,
    port: Option<i32>,
    username: Option<String>,
    credential_id: Option<String>, // Can be "none" or numeric ID
    description: Option<String>,
    is_local: Option<String>, // checkbox "on" or None
    enabled: Option<String>,   // checkbox "on" or None
    tags: Option<Vec<i64>>,    // Multi-select tag IDs
}

/// Create server handler
async fn server_create(
    State(state): State<AppState>,
    Form(input): Form<CreateServerInput>,
) -> Result<Html<String>, AppError> {
    // Parse credential ID
    let credential_id = match input.credential_id.as_deref() {
        Some("none") | Some("") | None => None,
        Some(id_str) => id_str.parse::<i64>().ok(),
    };

    // Parse checkboxes
    let is_local = input.is_local.is_some();
    let enabled = input.enabled.unwrap_or_else(|| "on".to_string()) == "on";

    // Create server
    let create_server = CreateServer {
        name: input.name.clone(),
        hostname: input.hostname.clone(),
        port: input.port.unwrap_or(22),
        username: input.username.clone(),
        credential_id,
        description: input.description.clone(),
        is_local,
        enabled,
        metadata: None,
    };

    // Validate
    if let Err(e) = create_server.validate() {
        let db = state.db().await;
        let credentials_list = credentials::list_credentials(db.pool()).await?;
        let tags_list = tags::list_tags(db.pool()).await?;

        return Ok(Html(format!(
            r#"<div class="alert alert-error">Validation error: {}</div>"#,
            e
        )));
    }

    // Save to database
    tracing::info!("Creating server: {} @ {:?}", create_server.name, create_server.hostname);
    let db = state.db().await;

    match servers_queries::create_server(db.pool(), &create_server).await {
        Ok(server_id) => {
            // Add tags if provided
            if let Some(tag_ids) = input.tags {
                for tag_id in tag_ids {
                    let _ = tags::add_server_tag(db.pool(), server_id, tag_id).await;
                }
            }

            // Success - return updated list with success message
            let servers = servers_queries::list_servers(db.pool()).await?;
            let servers_display: Vec<ServerDisplay> = servers.into_iter().map(|s| server_to_display(&s)).collect();
            let template = ServerListTemplate { servers: servers_display };
            let list_html = template.render()?;

            Ok(Html(format!(
                r#"<div class="alert alert-success">Server '{}' created successfully!</div>{}"#,
                create_server.name, list_html
            )))
        }
        Err(e) => {
            // Check for duplicate name error
            let error_msg = e.to_string();
            if error_msg.contains("UNIQUE constraint") && error_msg.contains("servers.name") {
                Ok(Html(format!(
                    r#"<div class="alert alert-error">A server with the name '{}' already exists. Please use a different name.</div>"#,
                    create_server.name
                )))
            } else {
                Err(e.into())
            }
        }
    }
}

/// Update server input (from form)
#[derive(Debug, Deserialize)]
struct UpdateServerInput {
    name: Option<String>,
    hostname: Option<String>,
    port: Option<i32>,
    username: Option<String>,
    credential_id: Option<String>,
    description: Option<String>,
    enabled: Option<String>,
}

/// Update server handler
async fn server_update(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Form(input): Form<UpdateServerInput>,
) -> Result<Html<String>, AppError> {
    let db = state.db().await;

    // Parse credential ID
    let credential_id = match input.credential_id.as_deref() {
        Some("none") | Some("") => Some(None),
        Some(id_str) => id_str.parse::<i64>().ok().map(Some),
        None => None,
    };

    // Get server name for success message
    let server_name = if let Some(ref name) = input.name {
        name.clone()
    } else {
        servers_queries::get_server(db.pool(), id).await?.name
    };

    let update_server = UpdateServer {
        name: input.name,
        hostname: input.hostname,
        port: input.port,
        username: input.username,
        credential_id,
        description: input.description,
        enabled: input.enabled.map(|_| true),
        os_type: None,
        os_distro: None,
        package_manager: None,
        docker_available: None,
        systemd_available: None,
        metadata: None,
        last_error: None,
    };

    match servers_queries::update_server(db.pool(), id, &update_server).await {
        Ok(_) => {
            // Success - return updated list
            let servers = servers_queries::list_servers(db.pool()).await?;
            let servers_display: Vec<ServerDisplay> = servers.into_iter().map(|s| server_to_display(&s)).collect();
            let template = ServerListTemplate { servers: servers_display };
            let list_html = template.render()?;

            Ok(Html(format!(
                r#"<div class="alert alert-success">Server '{}' updated successfully!</div>{}"#,
                server_name, list_html
            )))
        }
        Err(e) => {
            let error_msg = e.to_string();
            if error_msg.contains("UNIQUE constraint") && error_msg.contains("servers.name") {
                Ok(Html(
                    r#"<div class="alert alert-error">A server with that name already exists. Please use a different name.</div>"#.to_string()
                ))
            } else {
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
    let db = state.db().await;
    let server_name = servers_queries::get_server(db.pool(), id)
        .await
        .map(|s| s.name)
        .unwrap_or_else(|_| format!("Server {}", id));

    // Delete from database
    tracing::info!("Deleting server {}", id);
    servers_queries::delete_server(db.pool(), id).await?;

    Ok(Html(format!(
        r#"<div class="alert alert-success">Server '{}' deleted successfully!</div>"#,
        server_name
    )))
}

/// Test connection input
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

    tracing::info!("Testing SSH connection to {}@{}:{}", username, input.host, port);

    // TODO: Implement actual SSH connection test
    // For now, return a placeholder response
    Ok(Html(format!(
        r#"<div class="alert alert-info">Connection test for {}@{}:{} - Feature coming soon</div>"#,
        username, input.host, port
    )))
}

/// Test connection for existing server
async fn server_test_by_id(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Html<String>, AppError> {
    let db = state.db().await;
    let server = servers_queries::get_server(db.pool(), id).await?;

    tracing::info!("Testing connection for server: {}", server.name);

    // TODO: Implement actual capability detection
    Ok(Html(format!(
        r#"<div class="alert alert-info">Connection test for '{}' - Feature coming soon</div>"#,
        server.name
    )))
}

/// Get server capabilities
async fn server_capabilities(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Html<String>, AppError> {
    let db = state.db().await;
    let server = servers_queries::get_server(db.pool(), id).await?;
    let capabilities = servers_queries::get_server_capabilities(db.pool(), id).await?;

    let template = ServerCapabilitiesTemplate {
        server: server_to_display(&server),
        capabilities: capabilities
            .into_iter()
            .map(|c| CapabilityDisplay {
                name: c.capability,
                available: c.available,
                version: c.version,
                detected_at: c.detected_at.to_rfc3339(),
            })
            .collect(),
    };

    Ok(Html(template.render()?))
}

/// Add tag to server input
#[derive(Debug, Deserialize)]
struct AddTagInput {
    tag_id: i64,
}

/// Add tag to server
async fn server_add_tag(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Form(input): Form<AddTagInput>,
) -> Result<Html<String>, AppError> {
    let db = state.db().await;
    tags::add_server_tag(db.pool(), id, input.tag_id).await?;

    Ok(Html(
        r#"<div class="alert alert-success">Tag added successfully!</div>"#.to_string(),
    ))
}

/// Remove tag from server input
#[derive(Debug, Deserialize)]
struct RemoveTagInput {
    tag_id: i64,
}

/// Remove tag from server
async fn server_remove_tag(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Form(input): Form<RemoveTagInput>,
) -> Result<Html<String>, AppError> {
    let db = state.db().await;
    tags::remove_server_tag(db.pool(), id, input.tag_id).await?;

    Ok(Html(
        r#"<div class="alert alert-success">Tag removed successfully!</div>"#.to_string(),
    ))
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Convert DB server to UI display model
fn server_to_display(server: &Server) -> ServerDisplay {
    ServerDisplay {
        id: server.id,
        name: server.name.clone(),
        hostname: server.hostname.clone(),
        port: server.port,
        username: server.username.clone(),
        credential_id: server.credential_id,
        description: server.description.clone(),
        is_local: server.is_local,
        enabled: server.enabled,
        os_type: server.os_type.clone(),
        os_distro: server.os_distro.clone(),
        package_manager: server.package_manager.clone(),
        docker_available: server.docker_available,
        systemd_available: server.systemd_available,
        last_seen_at: server.last_seen_at.map(|t| t.to_rfc3339()),
        last_error: server.last_error.clone(),
        tags: vec![], // Will be filled by join query if needed
        created_at: server.created_at.to_rfc3339(),
        updated_at: server.updated_at.to_rfc3339(),
    }
}

/// Convert DB credential to UI display model
fn credential_to_display(cred: &svrctlrs_database::models::Credential) -> CredentialDisplay {
    let type_display = match cred.credential_type_str.as_str() {
        "ssh_key" => "SSH Key",
        "api_token" => "API Token",
        "password" => "Password",
        "certificate" => "Certificate",
        _ => "Unknown",
    };

    let value_preview = if cred.is_ssh_key() {
        cred.value.clone() // Show path
    } else {
        format!("{}...", &cred.value.chars().take(8).collect::<String>())
    };

    CredentialDisplay {
        id: cred.id,
        name: cred.name.clone(),
        credential_type: cred.credential_type_str.clone(),
        credential_type_display: type_display.to_string(),
        description: cred.description.clone(),
        value_preview,
        username: cred.username.clone(),
        created_at: cred.created_at.to_rfc3339(),
        updated_at: cred.updated_at.to_rfc3339(),
    }
}
