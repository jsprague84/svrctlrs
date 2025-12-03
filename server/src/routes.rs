//! Route modules
//!
//! This module organizes all HTTP routes for the SvrCtlRS server.
//!
//! ## Route Structure
//!
//! ```text
//! /api/v1/          REST API endpoints (JSON)
//! /ws/              WebSocket endpoints
//! /                 HTMX UI routes (HTML)
//! /static/          Static assets
//! ```

pub mod api;
pub mod job_runs_ws;
pub mod terminal;
pub mod terminal_pty;
pub mod ui;

use axum::Router;

use crate::state::AppState;

/// Create REST API router
///
/// All API endpoints are nested under /api/v1/
pub fn api_routes(state: AppState) -> Router {
    Router::new().nest("/v1", api::routes()).with_state(state)
}
