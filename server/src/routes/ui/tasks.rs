//! Task management routes

use askama::Template;
use axum::{
    extract::{Path, State},
    response::Html,
    routing::{get, post, put},
    Form, Router,
};
use serde::Deserialize;
use svrctlrs_database::queries;

use crate::{state::AppState, templates::*};
use super::{get_user_from_session, AppError};

/// Create tasks router
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/tasks", get(tasks_page))
        .route("/tasks/list", get(task_list))
        .route("/tasks/{id}/run", post(task_run_now))
        .route("/tasks/{id}/schedule", put(task_update_schedule))
}

/// Tasks page handler
async fn tasks_page(State(_state): State<AppState>) -> Result<Html<String>, AppError> {
    let user = get_user_from_session().await;

    let template = TasksTemplate { user };
    Ok(Html(template.render()?))
}

/// Task list component (for auto-refresh)
async fn task_list(State(state): State<AppState>) -> Result<Html<String>, AppError> {
    let tasks = get_tasks(&state).await;

    tracing::info!("📋 Loaded {} tasks for grouping", tasks.len());

    // Group tasks by server
    let mut task_groups = std::collections::HashMap::<Option<String>, Vec<Task>>::new();
    for task in tasks {
        tracing::info!(
            "  Task '{}' has server_name: {:?}",
            task.name,
            task.server_name
        );
        task_groups
            .entry(task.server_name.clone())
            .or_default()
            .push(task);
    }

    // Convert to sorted vector: Local first, then alphabetically by server name
    let mut groups: Vec<TaskGroup> = task_groups
        .into_iter()
        .map(|(server_name, tasks)| TaskGroup { server_name, tasks })
        .collect();

    groups.sort_by(|a, b| {
        match (&a.server_name, &b.server_name) {
            (None, None) => std::cmp::Ordering::Equal,
            (None, Some(_)) => std::cmp::Ordering::Less, // Local first
            (Some(_), None) => std::cmp::Ordering::Greater,
            (Some(a_name), Some(b_name)) => a_name.cmp(b_name), // Alphabetical
        }
    });

    tracing::info!("📊 Created {} task groups", groups.len());
    for group in &groups {
        tracing::info!(
            "  Group '{:?}' has {} tasks",
            group.server_name,
            group.tasks.len()
        );
    }

    let template = TaskListTemplate {
        task_groups: groups,
    };
    Ok(Html(template.render()?))
}

/// Helper to load and convert tasks from database
async fn get_tasks(state: &AppState) -> Vec<Task> {
    // Load tasks from database
    let db = state.db().await;
    let db_tasks = queries::tasks::list_tasks(db.pool())
        .await
        .unwrap_or_default();

    db_tasks
        .into_iter()
        .map(|t| Task {
            id: t.id,
            name: t.name,
            description: t.description,
            plugin_id: t.plugin_id,
            server_name: t.server_name,
            schedule: t.schedule,
            last_run_at: t.last_run_at.map(|dt| dt.to_rfc3339()),
            next_run_at: t.next_run_at.map(|dt| dt.to_rfc3339()),
        })
        .collect()
}

/// Update task schedule input
#[derive(Debug, Deserialize)]
struct UpdateScheduleInput {
    schedule: String,
}

/// Update task schedule handler
async fn task_update_schedule(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Form(input): Form<UpdateScheduleInput>,
) -> Result<Html<String>, AppError> {
    tracing::info!("Updating schedule for task {} to: {}", id, input.schedule);

    // Validate cron expression
    use cron::Schedule;
    use std::str::FromStr;

    if let Err(e) = Schedule::from_str(&input.schedule) {
        return Ok(Html(format!(
            r#"<code class="schedule-display" id="schedule-display-{}" onclick="editSchedule({}, '{}')" style="color: red;" title="Invalid cron: {}">{}</code>"#,
            id, id, input.schedule, e, input.schedule
        )));
    }

    // Update in database
    let db = state.db().await;
    let update_task = svrctlrs_database::models::task::UpdateTask {
        name: None,
        description: None,
        schedule: Some(input.schedule.clone()),
        enabled: None,
        command: None,
        args: None,
        timeout: None,
    };
    queries::tasks::update_task(db.pool(), id, &update_task).await?;

    // Calculate and update next run time for the new schedule
    match queries::tasks::calculate_next_run(&input.schedule) {
        Ok(next_run) => {
            if let Err(e) = queries::tasks::update_task_next_run(db.pool(), id, next_run).await {
                tracing::warn!("Failed to update next_run_at for task {}: {}", id, e);
            } else {
                tracing::debug!("Updated next_run_at for task {}: {:?}", id, next_run);
            }
        }
        Err(e) => {
            tracing::warn!("Failed to calculate next_run_at for task {}: {}", id, e);
        }
    }

    // Reload scheduler to pick up new schedule
    state.reload_config().await?;

    // Return updated display
    Ok(Html(format!(
        r#"<code class="schedule-display" id="schedule-display-{}" onclick="editSchedule({}, '{}')">{}</code>"#,
        id, id, input.schedule, input.schedule
    )))
}

/// Run task manually handler
async fn task_run_now(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Html<String>, AppError> {
    tracing::info!("Running task {} manually", id);

    // Execute task using the executor
    match crate::executor::execute_task(&state, id).await {
        Ok(result) => {
            if result.success {
                // Escape HTML in output to prevent XSS
                let escaped_output = result
                    .output
                    .replace('&', "&amp;")
                    .replace('<', "&lt;")
                    .replace('>', "&gt;")
                    .replace('"', "&quot;");

                Ok(Html(format!(
                    r#"<div class="alert alert-success">✓ Task executed successfully in {}ms<br><pre>{}</pre></div>"#,
                    result.duration_ms, escaped_output
                )))
            } else {
                Ok(Html(format!(
                    r#"<div class="alert alert-error">✗ Task execution failed: {}</div>"#,
                    result.error.unwrap_or_else(|| "Unknown error".to_string())
                )))
            }
        }
        Err(e) => {
            tracing::error!("Failed to execute task {}: {}", id, e);
            Ok(Html(format!(
                r#"<div class="alert alert-error">✗ Failed to execute task: {}</div>"#,
                e
            )))
        }
    }
}
