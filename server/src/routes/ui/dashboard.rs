//! Dashboard page routes

use askama::Template;
use axum::{extract::State, response::Html, routing::get, Router};
use svrctlrs_database::queries;

use super::{get_user_from_session, AppError};
use crate::{state::AppState, templates::*};

/// Create dashboard router
pub fn routes() -> Router<AppState> {
    Router::new().route("/", get(dashboard_page))
}

/// Dashboard page handler
async fn dashboard_page(State(state): State<AppState>) -> Result<Html<String>, AppError> {
    let user = get_user_from_session().await;

    // Get stats
    let plugins = state.plugins.read().await;
    let enabled_plugins = plugins.plugins().len();

    // Get server count from database
    let db = state.db().await;
    let servers = queries::servers::list_servers(db.pool()).await?;
    let total_servers = servers.len();

    let stats = DashboardStats {
        total_servers,
        active_tasks: 0, // TODO: Track active tasks
        enabled_plugins,
        total_tasks: 0, // TODO: Track total tasks
    };

    let template = DashboardTemplate { user, stats };
    Ok(Html(template.render()?))
}
