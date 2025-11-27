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

use super::{get_user_from_session, AppError};
use crate::{state::AppState, templates::*};

/// Create tasks router
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/tasks", get(tasks_page).post(task_create))
        .route("/tasks/new", get(task_form_new))
        .route("/tasks/list", get(task_list))
        .route("/tasks/{id}", axum::routing::delete(task_delete))
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
            server_id: t.server_id,
            server_name: t.server_name,
            command: t.command,
            schedule: t.schedule,
            enabled: t.enabled,
            timeout: t.timeout,
            last_run_at: t
                .last_run_at
                .map(|dt| dt.format("%Y-%m-%d %I:%M:%S %p").to_string()),
            next_run_at: t
                .next_run_at
                .map(|dt| dt.format("%Y-%m-%d %I:%M:%S %p").to_string()),
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

/// Show task creation form
async fn task_form_new(State(state): State<AppState>) -> Result<Html<String>, AppError> {
    tracing::info!("Loading task creation form");

    let db = state.db().await;

    // Load available servers and plugins
    let db_servers = queries::servers::list_servers(db.pool()).await?;
    let db_plugins = queries::plugins::list_plugins(db.pool()).await?;

    // Convert to template types
    let servers: Vec<crate::templates::Server> = db_servers
        .into_iter()
        .map(|s| crate::templates::Server {
            id: s.id,
            name: s.name,
            host: s.host.unwrap_or_default(),
            port: Some(s.port),
            username: Some(s.username),
            description: s.description,
            enabled: s.enabled,
        })
        .collect();

    let plugins: Vec<crate::templates::Plugin> = db_plugins
        .into_iter()
        .filter(|p| p.id != "ssh") // Exclude virtual SSH plugin from UI
        .map(|p| crate::templates::Plugin {
            id: p.id,
            name: p.name,
            description: p
                .description
                .unwrap_or_else(|| "No description".to_string()),
            version: "1.0.0".to_string(), // Plugin version not stored in DB
            enabled: p.enabled,
        })
        .collect();

    let template = crate::templates::TaskFormTemplate {
        task: None,
        servers,
        plugins,
        error: None,
    };

    Ok(Html(template.render()?))
}

/// Create new task handler
async fn task_create(
    State(state): State<AppState>,
    Form(input): Form<crate::templates::CreateTaskInput>,
) -> Result<Html<String>, AppError> {
    use cron::Schedule;
    use std::str::FromStr;

    tracing::info!("Creating new task: {}", input.name);

    // Validate cron expression
    if let Err(e) = Schedule::from_str(&input.schedule) {
        tracing::warn!("Invalid cron expression: {}", e);

        // Return error in form
        let db = state.db().await;
        let db_servers = queries::servers::list_servers(db.pool()).await?;
        let db_plugins = queries::plugins::list_plugins(db.pool()).await?;

        let servers: Vec<crate::templates::Server> = db_servers
            .into_iter()
            .map(|s| crate::templates::Server {
                id: s.id,
                name: s.name,
                host: s.host.unwrap_or_default(),
                port: Some(s.port),
                username: Some(s.username),
                description: s.description,
                enabled: s.enabled,
            })
            .collect();

        let plugins: Vec<crate::templates::Plugin> = db_plugins
            .into_iter()
            .filter(|p| p.id != "ssh") // Exclude virtual SSH plugin from UI
            .map(|p| crate::templates::Plugin {
                id: p.id,
                name: p.name,
                description: p
                    .description
                    .unwrap_or_else(|| "No description".to_string()),
                version: "1.0.0".to_string(),
                enabled: p.enabled,
            })
            .collect();

        let template = crate::templates::TaskFormTemplate {
            task: None,
            servers,
            plugins,
            error: Some(format!("Invalid cron expression: {}", e)),
        };

        return Ok(Html(template.render()?));
    }

    let db = state.db().await;

    // Parse server_id and determine task type
    let (server_id, server_name, plugin_id, command) = if input.server_id == "local" {
        // Local plugin task
        let plugin_id = input
            .plugin_id
            .ok_or_else(|| anyhow::anyhow!("Plugin ID required for local tasks"))?;
        let command = input.command.unwrap_or_else(|| "plugin_task".to_string());

        (None, Some("localhost".to_string()), plugin_id, command)
    } else {
        // Remote SSH task
        let id = input
            .server_id
            .parse::<i64>()
            .map_err(|_| anyhow::anyhow!("Invalid server ID"))?;

        let server = queries::servers::get_server(db.pool(), id).await?;
        let remote_command = input
            .remote_command
            .ok_or_else(|| anyhow::anyhow!("Command required for remote tasks"))?;

        (
            Some(id),
            Some(server.name),
            "ssh".to_string(),
            remote_command,
        )
    };

    // Create task
    let create_task = svrctlrs_database::models::task::CreateTask {
        name: input.name,
        description: input.description,
        plugin_id,
        server_id,
        server_name,
        schedule: input.schedule.clone(),
        command,
        args: None,
        timeout: input.timeout.unwrap_or(300),
    };

    let task_id = queries::tasks::create_task(db.pool(), &create_task).await?;
    tracing::info!("Created task with ID: {}", task_id);

    // Calculate and set next_run_at
    match queries::tasks::calculate_next_run(&input.schedule) {
        Ok(next_run) => {
            if let Err(e) = queries::tasks::update_task_next_run(db.pool(), task_id, next_run).await
            {
                tracing::warn!("Failed to update next_run_at for task {}: {}", task_id, e);
            } else {
                tracing::debug!("Updated next_run_at for task {}: {:?}", task_id, next_run);
            }
        }
        Err(e) => {
            tracing::warn!(
                "Failed to calculate next_run_at for task {}: {}",
                task_id,
                e
            );
        }
    }

    // If task is enabled, update enabled status
    if input.enabled.is_some() {
        let update_task = svrctlrs_database::models::task::UpdateTask {
            name: None,
            description: None,
            schedule: None,
            enabled: Some(true),
            command: None,
            args: None,
            timeout: None,
        };
        queries::tasks::update_task(db.pool(), task_id, &update_task).await?;
    }

    // Reload scheduler to pick up new task
    state.reload_config().await?;

    // Return updated task list
    let tasks = get_tasks(&state).await;
    let mut task_groups = std::collections::HashMap::<Option<String>, Vec<Task>>::new();
    for task in tasks {
        task_groups
            .entry(task.server_name.clone())
            .or_default()
            .push(task);
    }

    let mut groups: Vec<crate::templates::TaskGroup> = task_groups
        .into_iter()
        .map(|(server_name, tasks)| crate::templates::TaskGroup { server_name, tasks })
        .collect();

    groups.sort_by(|a, b| match (&a.server_name, &b.server_name) {
        (None, None) => std::cmp::Ordering::Equal,
        (None, Some(_)) => std::cmp::Ordering::Less,
        (Some(_), None) => std::cmp::Ordering::Greater,
        (Some(a_name), Some(b_name)) => a_name.cmp(b_name),
    });

    let template = TaskListTemplate {
        task_groups: groups,
    };
    Ok(Html(template.render()?))
}

/// Delete task handler
async fn task_delete(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Html<String>, AppError> {
    tracing::info!("Deleting task {}", id);

    let db = state.db().await;

    // Delete the task
    queries::tasks::delete_task(db.pool(), id).await?;

    // Reload scheduler to remove the task
    state.reload_config().await?;

    // Return updated task list
    let tasks = get_tasks(&state).await;
    let mut task_groups = std::collections::HashMap::<Option<String>, Vec<Task>>::new();
    for task in tasks {
        task_groups
            .entry(task.server_name.clone())
            .or_default()
            .push(task);
    }

    let mut groups: Vec<crate::templates::TaskGroup> = task_groups
        .into_iter()
        .map(|(server_name, tasks)| crate::templates::TaskGroup { server_name, tasks })
        .collect();

    groups.sort_by(|a, b| match (&a.server_name, &b.server_name) {
        (None, None) => std::cmp::Ordering::Equal,
        (None, Some(_)) => std::cmp::Ordering::Less,
        (Some(_), None) => std::cmp::Ordering::Greater,
        (Some(a_name), Some(b_name)) => a_name.cmp(b_name),
    });

    let template = TaskListTemplate {
        task_groups: groups,
    };
    Ok(Html(template.render()?))
}
