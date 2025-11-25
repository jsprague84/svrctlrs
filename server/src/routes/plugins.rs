use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, put},
    Json, Router,
};
use serde_json::json;
use tracing::{error, info, instrument};

use crate::state::AppState;
use svrctlrs_database::models::plugin::{Plugin, UpdatePlugin};
use svrctlrs_database::queries;

/// Create plugin API router
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list_plugins))
        .route("/{id}", get(get_plugin).put(update_plugin))
        .route("/{id}/toggle", put(toggle_plugin))
}

/// List all plugins
#[instrument(skip(state))]
async fn list_plugins(State(state): State<AppState>) -> Result<Json<Vec<Plugin>>, (StatusCode, String)> {
    let db = state.db().await;
    let plugins = queries::plugins::list_plugins(db.pool())
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to list plugins");
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;
    Ok(Json(plugins))
}

/// Get a plugin by ID
#[instrument(skip(state))]
async fn get_plugin(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let db = state.db().await;
    let plugin = queries::plugins::get_plugin(db.pool(), &id)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to get plugin");
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;
    Ok(Json(plugin))
}

/// Update a plugin
#[instrument(skip(state, update_plugin_input))]
async fn update_plugin(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(update_plugin_input): Json<UpdatePlugin>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    info!(plugin_id = %id, "Updating plugin");
    let db = state.db().await;
    
    queries::plugins::update_plugin(db.pool(), &id, &update_plugin_input)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to update plugin");
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;
    
    // Return updated plugin
    let plugin = queries::plugins::get_plugin(db.pool(), &id)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to get updated plugin");
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;
    
    Ok(Json(plugin))
}

/// Toggle plugin enabled status
#[instrument(skip(state))]
async fn toggle_plugin(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    info!(plugin_id = %id, "Toggling plugin");
    let db = state.db().await;
    
    let new_status = queries::plugins::toggle_plugin(db.pool(), &id)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to toggle plugin");
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;
    
    Ok(Json(json!({
        "plugin_id": id,
        "enabled": new_status
    })))
}

