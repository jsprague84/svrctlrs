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
    let enabled_plugins = 4; // Built-in features: ssh, docker, updates, health

    // Get server count from database
    let db = state.db().await;
    let servers = queries::servers::list_servers(db.pool()).await?;
    let total_servers = servers.len();

    let stats = DashboardStats {
        total_servers,
        total_schedules: 0, // TODO: Count job schedules
        active_jobs: 0, // TODO: Count running jobs
        active_tasks: 0, // TODO: Track active tasks
        enabled_plugins,
        total_tasks: 0, // TODO: Track total tasks
        recent_runs: vec![], // TODO: Query recent job runs
    };

    let template = DashboardTemplate { user, stats };
    Ok(Html(template.render()?))
}
