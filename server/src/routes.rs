//! API routes

mod api;
mod servers;
pub mod ui;
mod webhooks;

use axum::Router;

use crate::state::AppState;

/// Create main router with all routes
pub fn api_routes(state: AppState) -> Router {
    Router::new()
        // REST API routes
        .nest("/v1", api::routes())
        // Server management routes
        .nest("/v1/servers", servers::routes())
        // Webhook routes
        .nest("/webhooks", webhooks::routes())
        .with_state(state)
}
