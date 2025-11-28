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

    // Get recent job runs (last 10)
    let recent_job_runs = queries::job_runs::list_job_runs(db.pool(), 10, 0)
        .await
        .unwrap_or_default();

    // Convert to display format with server/template names
    let mut recent_runs = Vec::new();
    for run in recent_job_runs {
        // Get server name
        let server_name = match queries::servers::get_server(db.pool(), run.server_id).await {
            Ok(server) => server.name,
            Err(_) => format!("Server #{}", run.server_id),
        };

        // Get job template name
        let job_name =
            match queries::job_templates::get_job_template(db.pool(), run.job_template_id).await {
                Ok(template) => template.display_name,
                Err(_) => format!("Job #{}", run.job_template_id),
            };

        recent_runs.push(crate::templates::RecentJobRun {
            id: run.id,
            job_name,
            server_name,
            status: run.status_str.clone(),
            started_at: run.started_at.format("%Y-%m-%d %H:%M:%S").to_string(),
            duration_seconds: run.duration_ms.map(|ms| ms as f64 / 1000.0),
        });
    }

    let stats = DashboardStats {
        total_servers,
        total_schedules,
        active_jobs,
        active_tasks,
        enabled_plugins,
        total_tasks: total_schedules, // Same as total_schedules for now
        recent_runs,
    };

    let template = DashboardTemplate { user, stats };
    Ok(Html(template.render()?))
}
