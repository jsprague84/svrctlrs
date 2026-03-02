//! Servers API endpoints
//!
//! Provides CRUD operations for managing servers.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;
use svrctlrs_database::{queries::servers, CreateServer, UpdateServer};
use tracing::{error, info, instrument};

use super::ApiError;
use crate::state::AppState;

/// Create servers router
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list_servers).post(create_server))
        .route(
            "/{id}",
            get(get_server).put(update_server).delete(delete_server),
        )
        .route("/{id}/test", post(test_connection))
        .route("/{id}/capabilities", get(get_server_capabilities))
}

/// List all servers
#[instrument(skip(state))]
async fn list_servers(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    let servers_list = servers::list_servers(&state.pool).await.map_err(|e| {
        error!(error = %e, "Failed to list servers");
        ApiError::internal_error(format!("Failed to list servers: {}", e))
    })?;

    Ok(Json(json!({ "servers": servers_list })))
}

/// Get server by ID
#[instrument(skip(state))]
async fn get_server(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    let server = servers::get_server(&state.pool, id).await.map_err(|e| {
        error!(error = %e, id = id, "Failed to get server");
        ApiError::not_found("Server")
    })?;

    let capabilities = servers::get_server_capabilities(&state.pool, id)
        .await
        .unwrap_or_default();

    Ok(Json(json!({
        "server": server,
        "capabilities": capabilities
    })))
}

/// Create server input
#[derive(Debug, Deserialize)]
struct CreateServerInput {
    name: String,
    #[serde(default)]
    hostname: Option<String>,
    #[serde(default = "default_port")]
    port: i32,
    #[serde(default)]
    username: Option<String>,
    #[serde(default)]
    credential_id: Option<i64>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    is_local: bool,
    #[serde(default = "default_enabled")]
    enabled: bool,
}

fn default_port() -> i32 {
    22
}

fn default_enabled() -> bool {
    true
}

/// Create a new server
#[instrument(skip(state, input))]
async fn create_server(
    State(state): State<AppState>,
    Json(input): Json<CreateServerInput>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    info!(name = %input.name, "Creating server");

    let create = CreateServer {
        name: input.name,
        hostname: input.hostname,
        port: input.port,
        username: input.username,
        credential_id: input.credential_id,
        description: input.description,
        is_local: input.is_local,
        enabled: input.enabled,
        metadata: None,
    };

    let id = servers::create_server(&state.pool, &create)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to create server");
            ApiError::internal_error(format!("Failed to create server: {}", e))
        })?;

    info!(id = id, "Server created successfully");

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "id": id,
            "message": "Server created successfully"
        })),
    ))
}

/// Update server input
#[derive(Debug, Deserialize)]
struct UpdateServerInput {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    hostname: Option<String>,
    #[serde(default)]
    port: Option<i32>,
    #[serde(default)]
    username: Option<String>,
    #[serde(default)]
    credential_id: Option<i64>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    enabled: Option<bool>,
}

/// Update an existing server
#[instrument(skip(state, input))]
async fn update_server(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(input): Json<UpdateServerInput>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    info!(id = id, "Updating server");

    // Verify server exists
    servers::get_server(&state.pool, id)
        .await
        .map_err(|_| ApiError::not_found("Server"))?;

    let update = UpdateServer {
        name: input.name,
        hostname: input.hostname,
        port: input.port,
        username: input.username,
        credential_id: input.credential_id,
        description: input.description,
        enabled: input.enabled,
        os_type: None,
        os_distro: None,
        package_manager: None,
        docker_available: None,
        systemd_available: None,
        metadata: None,
        last_error: None,
    };

    servers::update_server(&state.pool, id, &update)
        .await
        .map_err(|e| {
            error!(error = %e, id = id, "Failed to update server");
            ApiError::internal_error(format!("Failed to update server: {}", e))
        })?;

    info!(id = id, "Server updated successfully");

    Ok(Json(json!({
        "id": id,
        "message": "Server updated successfully"
    })))
}

/// Delete a server
#[instrument(skip(state))]
async fn delete_server(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    info!(id = id, "Deleting server");

    servers::delete_server(&state.pool, id).await.map_err(|e| {
        error!(error = %e, id = id, "Failed to delete server");
        ApiError::internal_error(format!("Failed to delete server: {}", e))
    })?;

    info!(id = id, "Server deleted successfully");

    Ok(Json(json!({
        "id": id,
        "message": "Server deleted successfully"
    })))
}

/// Test server SSH connection
#[instrument(skip(state))]
async fn test_connection(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    info!(id = id, "Testing server connection");

    let server = servers::get_server(&state.pool, id).await.map_err(|e| {
        error!(error = %e, id = id, "Server not found");
        ApiError::not_found("Server")
    })?;

    let hostname = server.hostname.as_deref().unwrap_or("").to_string();
    if hostname.is_empty() {
        return Err(ApiError::bad_request("Server has no hostname configured"));
    }

    // Load credential if assigned
    let key_path = if let Some(cred_id) = server.credential_id {
        match svrctlrs_database::queries::credentials::get_credential(&state.pool, cred_id).await {
            Ok(cred) => Some(cred.value.clone()),
            Err(e) => {
                tracing::warn!(error = %e, "Failed to load credential for connection test");
                None
            }
        }
    } else {
        None
    };

    let config = crate::ssh::SshConfig {
        host: hostname,
        port: server.port as u16,
        username: server.username.unwrap_or_else(|| "root".to_string()),
        key_path,
        ..Default::default()
    };

    let started = std::time::Instant::now();
    match crate::ssh::test_connection(&config).await {
        Ok(output) => {
            let elapsed = started.elapsed();
            info!(
                id = id,
                elapsed_ms = elapsed.as_millis(),
                "Connection test succeeded"
            );

            Ok(Json(json!({
                "success": true,
                "message": output,
                "server_id": id,
                "hostname": server.hostname,
                "port": server.port,
                "latency_ms": elapsed.as_millis()
            })))
        }
        Err(e) => {
            let elapsed = started.elapsed();
            error!(id = id, error = %e, elapsed_ms = elapsed.as_millis(), "Connection test failed");

            Ok(Json(json!({
                "success": false,
                "message": format!("{}", e),
                "server_id": id,
                "hostname": server.hostname,
                "port": server.port,
                "latency_ms": elapsed.as_millis()
            })))
        }
    }
}

/// Get server capabilities
#[instrument(skip(state))]
async fn get_server_capabilities(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    // Verify server exists
    servers::get_server(&state.pool, id)
        .await
        .map_err(|_| ApiError::not_found("Server"))?;

    let capabilities = servers::get_server_capabilities(&state.pool, id)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to get server capabilities");
            ApiError::internal_error(format!("Failed to get server capabilities: {}", e))
        })?;

    Ok(Json(json!({ "capabilities": capabilities })))
}
