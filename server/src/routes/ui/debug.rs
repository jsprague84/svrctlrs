//! Terminal page routes - Multi-terminal server interface
//!
//! Provides a page with multiple terminal panes for simultaneously
//! executing commands across multiple servers.

use askama::Template;
use axum::{extract::State, response::Html, routing::get, Router};
use svrctlrs_database::queries;

use super::{get_user_from_session, AppError};
use crate::{state::AppState, templates::*};

/// Create terminal router
pub fn routes() -> Router<AppState> {
    Router::new().route("/terminal", get(terminal_page))
}

/// Terminal page handler - provides multi-terminal interface
async fn terminal_page(State(state): State<AppState>) -> Result<Html<String>, AppError> {
    let user = get_user_from_session().await;

    // Get all enabled servers for the terminal dropdowns
    let db = state.db().await;
    let servers_with_details = queries::servers::list_servers_with_details(db.pool()).await?;

    // Convert to display format and filter to enabled only
    let servers: Vec<ServerDisplay> = servers_with_details
        .into_iter()
        .filter(|s| s.server.enabled)
        .map(Into::into)
        .collect();

    let template = TerminalPageTemplate { user, servers };
    Ok(Html(template.render()?))
}
