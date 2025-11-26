//! API routes

mod api;
mod notifications;
mod plugins;
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
        // Plugin management routes
        .nest("/v1/plugins", plugins::routes())
        // Notification backend management routes
        .nest("/v1/notifications", notifications::routes())
        // Webhook routes
        .nest("/webhooks", webhooks::routes())
        .with_state(state)
}
