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

    // Get server count from database
    let db = state.db().await;
    let servers = queries::servers::list_servers_with_details(db.pool()).await?;
    let total_servers = servers.len();

    // Get job schedule counts
    let total_schedules = queries::job_schedules::count_job_schedules(db.pool())
        .await
        .unwrap_or(0) as usize;

    // Get running jobs count from database
    let active_jobs = queries::job_runs::count_running_jobs(db.pool())
        .await
        .unwrap_or(0) as usize;

    // Get active tasks count from scheduler
    let scheduler = state.scheduler.read().await;
    let active_tasks = if let Some(ref sched) = *scheduler {
        sched.task_count().await
    } else {
        0
    };

    // Get schedules with their most recent job run
    let schedules_with_last_run = queries::job_schedules::list_schedules_with_last_run(db.pool())
        .await
        .unwrap_or_default();

    // Convert to display format
    let schedules_with_runs: Vec<ScheduleWithLastRunDisplay> = schedules_with_last_run
        .into_iter()
        .map(Into::into)
        .collect();

    let stats = DashboardStats {
        total_servers,
        total_schedules,
        active_jobs,
        active_tasks,
        total_tasks: total_schedules,
        schedules_with_runs,
    };

    let template = DashboardTemplate { user, stats };
    Ok(Html(template.render()?))
}
