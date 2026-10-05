//! Servers API endpoints
//!
//! Provides CRUD operations for managing servers.

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;
use svrctlrs_database::{models::NewAuditEntry, queries::servers, CreateServer, UpdateServer};

use crate::routes::ui::auth::{client_ip, CurrentUser};
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
        .route(
            "/{id}/host-keys",
            get(list_host_keys).delete(forget_host_keys),
        )
}

/// Stored SSH host keys for a server, with OpenSSH-style fingerprints (no key material).
#[instrument(skip(state))]
async fn list_host_keys(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    use svrctlrs_database::queries::server_host_keys;

    servers::get_server(&state.pool, id)
        .await
        .map_err(|_| ApiError::not_found("Server"))?;
    let keys = server_host_keys::list_host_keys_for_server(&state.pool, id)
        .await
        .map_err(|e| {
            error!(error = %e, id = id, "Failed to list host keys");
            ApiError::internal_error(format!("Failed to list host keys: {e}"))
        })?;
    let keys: Vec<_> = keys
        .iter()
        .map(|k| {
            json!({
                "id": k.id,
                "key_type": k.key_type,
                "fingerprint": crate::host_keys::fingerprint(&k.public_key),
                "first_seen_at": k.first_seen_at,
                "last_seen_at": k.last_seen_at,
            })
        })
        .collect();
    Ok(Json(json!({ "server_id": id, "host_keys": keys })))
}

/// Forget every stored host key for a server. The next connection trusts the key it sees
/// (TOFU), so do this only after confirming out of band that the server's key changed.
#[instrument(skip(state))]
async fn forget_host_keys(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    use svrctlrs_database::queries::server_host_keys;

    servers::get_server(&state.pool, id)
        .await
        .map_err(|_| ApiError::not_found("Server"))?;
    let removed = server_host_keys::delete_host_keys_for_server(&state.pool, id)
        .await
        .map_err(|e| {
            error!(error = %e, id = id, "Failed to forget host keys");
            ApiError::internal_error(format!("Failed to forget host keys: {e}"))
        })?;
    tracing::warn!(
        server_id = id,
        removed,
        "Stored host keys forgotten; next connection re-trusts"
    );
    Ok(Json(json!({ "server_id": id, "removed": removed })))
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

    Ok(Json(json!({
        "server": server
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
    CurrentUser(me): CurrentUser,
    headers: HeaderMap,
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
    crate::audit::record(
        &state.pool,
        NewAuditEntry::new("server.created")
            .user(me)
            .client_ip(Some(client_ip(&headers)))
            .target("server", id, &create.name),
    );

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
    CurrentUser(me): CurrentUser,
    headers: HeaderMap,
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
    crate::audit::record(
        &state.pool,
        NewAuditEntry::new("server.updated")
            .user(me)
            .client_ip(Some(client_ip(&headers)))
            .target("server", id, ""),
    );

    Ok(Json(json!({
        "id": id,
        "message": "Server updated successfully"
    })))
}

/// Delete a server
#[instrument(skip(state))]
async fn delete_server(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    info!(id = id, "Deleting server");

    servers::delete_server(&state.pool, id).await.map_err(|e| {
        error!(error = %e, id = id, "Failed to delete server");
        ApiError::internal_error(format!("Failed to delete server: {}", e))
    })?;

    info!(id = id, "Server deleted successfully");
    crate::audit::record(
        &state.pool,
        NewAuditEntry::new("server.deleted")
            .user(me)
            .client_ip(Some(client_ip(&headers)))
            .target("server", id, ""),
    );

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

    let port = u16::try_from(server.port)
        .map_err(|_| ApiError::bad_request("Server port is out of range"))?;

    // Load credential if assigned
    let credential = match server.credential_id {
        Some(cred_id) => {
            match svrctlrs_database::queries::credentials::get_credential(&state.pool, cred_id)
                .await
            {
                Ok(cred) => Some(cred),
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to load credential for connection test");
                    None
                }
            }
        }
        None => None,
    };

    let started = std::time::Instant::now();
    // The same connector as command mode: the host key is verified (trust on first use)
    // before any authentication, so a test never talks to an unverified server.
    let result =
        match crate::routes::terminal::verify_host_key(&state.pool, server.id, &hostname, port)
            .await
        {
            Ok(verified_key) => crate::routes::terminal::execute_ssh_command(
                &server,
                credential.as_ref(),
                "echo svrctlrs-connection-test",
                None,
                verified_key.as_deref(),
            )
            .await
            .map(|(stdout, stderr, exit_code)| {
                if exit_code == 0 {
                    format!("Connected: {}", stdout.trim())
                } else {
                    format!(
                        "Connected, but the test command exited {exit_code}: {}",
                        stderr.trim()
                    )
                }
            }),
            Err(e) => Err(e),
        };

    match result {
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
                "message": e,
                "server_id": id,
                "hostname": server.hostname,
                "port": server.port,
                "latency_ms": elapsed.as_millis()
            })))
        }
    }
}
