//! API routes

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde_json::json;
use std::sync::Arc;

use crate::state::AppState;

/// Create API router
pub fn api_routes(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/plugins", get(list_plugins))
        .route("/servers", get(list_servers))
        .with_state(state)
}

/// Health check endpoint
async fn health_check() -> impl IntoResponse {
    Json(json!({
        "status": "ok",
        "service": "svrctlrs"
    }))
}

/// List all plugins
async fn list_plugins(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let registry = state.plugins.read().await;
    let plugin_ids = registry.plugin_ids();

    Json(json!({
        "plugins": plugin_ids
    }))
}

/// List all servers
async fn list_servers(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    Json(json!({
        "servers": state.config.servers
    }))
}
