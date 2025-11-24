//! API routes

mod webhooks;
mod api;

use axum::Router;

use crate::state::AppState;

/// Create main router with all routes
pub fn api_routes(state: AppState) -> Router {
    Router::new()
        // REST API routes
        .nest("/v1", api::routes())
        // Webhook routes
        .nest("/webhooks", webhooks::routes())
        .with_state(state)
}
