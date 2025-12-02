//! API routes

mod api;
pub mod job_runs_ws;
mod servers;
pub mod terminal;
pub mod terminal_pty;
pub mod ui;

use axum::Router;

use crate::state::AppState;

/// Create main router with all routes
pub fn api_routes(state: AppState) -> Router {
    Router::new()
        // REST API routes
        .nest("/v1", api::routes())
        // Server management routes
        .nest("/v1/servers", servers::routes())
        .with_state(state)
}
