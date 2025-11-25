//! Server management API endpoints

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post, put},
    Json, Router,
};
use serde_json::json;
use tracing::{error, info, instrument};

use svrctlrs_database::{queries, CreateServer, UpdateServer};

use crate::state::AppState;

/// Create servers API router
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list_servers).post(create_server))
        .route("/{id}", get(get_server).put(update_server).delete(delete_server))
        .route("/{id}/test", post(test_server_connection))
}

/// List all servers
#[instrument(skip(state))]
async fn list_servers(State(state): State<AppState>) -> Result<impl IntoResponse, (StatusCode, String)> {
    let db = state.database.read().await;
    let pool = db.pool();

    let servers = queries::list_servers(pool)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to list servers");
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to list servers: {}", e))
        })?;

    Ok(Json(json!({
        "servers": servers
    })))
}

/// Get server by ID
#[instrument(skip(state))]
async fn get_server(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let db = state.database.read().await;
    let pool = db.pool();

    let server = queries::get_server(pool, id)
        .await
        .map_err(|e| {
            error!(error = %e, id = id, "Failed to get server");
            (StatusCode::NOT_FOUND, format!("Server not found: {}", e))
        })?;

    Ok(Json(server))
}

/// Create a new server
#[instrument(skip(state))]
async fn create_server(
    State(state): State<AppState>,
    Json(server): Json<CreateServer>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    info!(name = %server.name, host = %server.host, "Creating server");

    let db = state.database.read().await;
    let pool = db.pool();

    let server_id = queries::create_server(pool, &server)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to create server");
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to create server: {}", e))
        })?;

    // Fetch the created server
    let created_server = queries::get_server(pool, server_id)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to fetch created server");
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to fetch created server: {}", e))
        })?;

    info!(id = server_id, "Server created successfully");

    Ok((StatusCode::CREATED, Json(created_server)))
}

/// Update a server
#[instrument(skip(state))]
async fn update_server(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(update): Json<UpdateServer>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    info!(id = id, "Updating server");

    let db = state.database.read().await;
    let pool = db.pool();

    // Verify server exists
    queries::get_server(pool, id)
        .await
        .map_err(|e| {
            error!(error = %e, id = id, "Server not found");
            (StatusCode::NOT_FOUND, format!("Server not found: {}", e))
        })?;

    // Update server
    queries::update_server(pool, id, &update)
        .await
        .map_err(|e| {
            error!(error = %e, id = id, "Failed to update server");
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to update server: {}", e))
        })?;

    // Fetch updated server
    let updated_server = queries::get_server(pool, id)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to fetch updated server");
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to fetch updated server: {}", e))
        })?;

    info!(id = id, "Server updated successfully");

    Ok(Json(updated_server))
}

/// Delete a server
#[instrument(skip(state))]
async fn delete_server(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    info!(id = id, "Deleting server");

    let db = state.database.read().await;
    let pool = db.pool();

    // Verify server exists
    let server = queries::get_server(pool, id)
        .await
        .map_err(|e| {
            error!(error = %e, id = id, "Server not found");
            (StatusCode::NOT_FOUND, format!("Server not found: {}", e))
        })?;

    // Delete server
    queries::delete_server(pool, id)
        .await
        .map_err(|e| {
            error!(error = %e, id = id, "Failed to delete server");
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to delete server: {}", e))
        })?;

    info!(id = id, name = %server.name, "Server deleted successfully");

    Ok((
        StatusCode::OK,
        Json(json!({
            "message": "Server deleted successfully",
            "id": id
        }))
    ))
}

/// Test server SSH connection
#[instrument(skip(state))]
async fn test_server_connection(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    info!(id = id, "Testing server connection");

    let db = state.database.read().await;
    let pool = db.pool();

    let server = queries::get_server(pool, id)
        .await
        .map_err(|e| {
            error!(error = %e, id = id, "Server not found");
            (StatusCode::NOT_FOUND, format!("Server not found: {}", e))
        })?;

    // TODO: Implement actual SSH connection test
    // For now, just return a mock response
    
    info!(id = id, host = ?server.host, "Connection test completed");

    Ok(Json(json!({
        "success": true,
        "message": "Connection test not yet implemented",
        "server_id": id,
        "host": server.host,
        "port": server.port
    })))
}

