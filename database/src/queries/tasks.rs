use chrono::{DateTime, Utc};
use cron::Schedule;
use sqlx::{Pool, Sqlite};
use std::str::FromStr;
use svrctlrs_core::{Error, Result};

use crate::models::{CreateTask, Task, TaskHistory, TaskHistoryEntry, UpdateTask};

/// List all tasks
pub async fn list_tasks(pool: &Pool<Sqlite>) -> Result<Vec<Task>> {
    sqlx::query_as::<_, Task>(
        r#"
        SELECT id, name, description, plugin_id, server_id, server_name, schedule, enabled, command, args,
               timeout, created_at, updated_at, last_run_at, next_run_at, run_count
        FROM tasks
        ORDER BY server_name, name
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| Error::DatabaseError(format!("Failed to list tasks: {}", e)))
}

/// Get task by ID
pub async fn get_task(pool: &Pool<Sqlite>, id: i64) -> Result<Task> {
    sqlx::query_as::<_, Task>(
        r#"
        SELECT id, name, description, plugin_id, server_id, server_name, schedule, enabled, command, args,
               timeout, created_at, updated_at, last_run_at, next_run_at, run_count
        FROM tasks
        WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .map_err(|e| Error::DatabaseError(format!("Failed to get task: {}", e)))
}

/// Create task
pub async fn create_task(pool: &Pool<Sqlite>, task: &CreateTask) -> Result<i64> {
    let args_json = task
        .args
        .as_ref()
        .map(|a| serde_json::to_string(a).unwrap_or_else(|_| "{}".to_string()));

    let result = sqlx::query(
        r#"
        INSERT INTO tasks (name, description, plugin_id, server_id, server_name, schedule, command, args, timeout)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&task.name)
    .bind(&task.description)
    .bind(&task.plugin_id)
    .bind(task.server_id)
    .bind(&task.server_name)
    .bind(&task.schedule)
    .bind(&task.command)
    .bind(args_json)
    .bind(task.timeout)
    .execute(pool)
    .await
    .map_err(|e| Error::DatabaseError(format!("Failed to create task: {}", e)))?;

    Ok(result.last_insert_rowid())
}

/// Update task
pub async fn update_task(pool: &Pool<Sqlite>, id: i64, update: &UpdateTask) -> Result<()> {
    let mut query = String::from("UPDATE tasks SET updated_at = CURRENT_TIMESTAMP");
    let mut bindings: Vec<String> = Vec::new();

    if let Some(name) = &update.name {
        query.push_str(", name = ?");
        bindings.push(name.clone());
    }
    if let Some(description) = &update.description {
        query.push_str(", description = ?");
        bindings.push(description.clone());
    }
    if let Some(schedule) = &update.schedule {
        query.push_str(", schedule = ?");
        bindings.push(schedule.clone());
    }
    if let Some(enabled) = update.enabled {
        query.push_str(", enabled = ?");
        bindings.push(if enabled { "1" } else { "0" }.to_string());
    }
    if let Some(command) = &update.command {
        query.push_str(", command = ?");
        bindings.push(command.clone());
    }
    if let Some(args) = &update.args {
        query.push_str(", args = ?");
        bindings.push(serde_json::to_string(args).unwrap_or_else(|_| "{}".to_string()));
    }
    if let Some(timeout) = update.timeout {
        query.push_str(", timeout = ?");
        bindings.push(timeout.to_string());
    }

    query.push_str(" WHERE id = ?");
    bindings.push(id.to_string());

    let mut q = sqlx::query(&query);
    for binding in bindings {
        q = q.bind(binding);
    }

    q.execute(pool)
        .await
        .map_err(|e| Error::DatabaseError(format!("Failed to update task: {}", e)))?;

    Ok(())
}

/// Delete task
pub async fn delete_task(pool: &Pool<Sqlite>, id: i64) -> Result<()> {
    sqlx::query("DELETE FROM tasks WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| Error::DatabaseError(format!("Failed to delete task: {}", e)))?;

    Ok(())
}

/// Calculate next run time for a cron schedule
pub fn calculate_next_run(cron_expr: &str) -> Result<Option<DateTime<Utc>>> {
    let schedule = Schedule::from_str(cron_expr)
        .map_err(|e| Error::SchedulerError(format!("Invalid cron expression: {}", e)))?;

    Ok(schedule.upcoming(Utc).next())
}

/// Update task's next_run_at field only
pub async fn update_task_next_run(
    pool: &Pool<Sqlite>,
    id: i64,
    next_run_at: Option<DateTime<Utc>>,
) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE tasks
        SET next_run_at = ?
        WHERE id = ?
        "#,
    )
    .bind(next_run_at)
    .bind(id)
    .execute(pool)
    .await
    .map_err(|e| Error::DatabaseError(format!("Failed to update task next_run_at: {}", e)))?;

    Ok(())
}

/// Update task run info
pub async fn update_task_run_info(
    pool: &Pool<Sqlite>,
    id: i64,
    next_run_at: Option<chrono::DateTime<chrono::Utc>>,
) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE tasks
        SET last_run_at = CURRENT_TIMESTAMP,
            next_run_at = ?,
            run_count = run_count + 1
        WHERE id = ?
        "#,
    )
    .bind(next_run_at)
    .bind(id)
    .execute(pool)
    .await
    .map_err(|e| Error::DatabaseError(format!("Failed to update task run info: {}", e)))?;

    Ok(())
}

/// List enabled tasks
pub async fn list_enabled_tasks(pool: &Pool<Sqlite>) -> Result<Vec<Task>> {
    sqlx::query_as::<_, Task>(
        r#"
        SELECT id, name, description, plugin_id, server_id, server_name, schedule, enabled, command, args,
               timeout, created_at, updated_at, last_run_at, next_run_at, run_count
        FROM tasks
        WHERE enabled = 1
        ORDER BY next_run_at
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| Error::DatabaseError(format!("Failed to list enabled tasks: {}", e)))
}

/// List tasks by plugin
pub async fn list_tasks_by_plugin(pool: &Pool<Sqlite>, plugin_id: &str) -> Result<Vec<Task>> {
    sqlx::query_as::<_, Task>(
        r#"
        SELECT id, name, description, plugin_id, server_id, server_name, schedule, enabled, command, args,
               timeout, created_at, updated_at, last_run_at, next_run_at, run_count
        FROM tasks
        WHERE plugin_id = ?
        ORDER BY server_name, name
        "#,
    )
    .bind(plugin_id)
    .fetch_all(pool)
    .await
    .map_err(|e| Error::DatabaseError(format!("Failed to list tasks by plugin: {}", e)))
}

/// List tasks by server
pub async fn list_tasks_by_server(pool: &Pool<Sqlite>, server_id: i64) -> Result<Vec<Task>> {
    sqlx::query_as::<_, Task>(
        r#"
        SELECT id, name, description, plugin_id, server_id, server_name, schedule, enabled, command, args,
               timeout, created_at, updated_at, last_run_at, next_run_at, run_count
        FROM tasks
        WHERE server_id = ?
        ORDER BY name
        "#,
    )
    .bind(server_id)
    .fetch_all(pool)
    .await
    .map_err(|e| Error::DatabaseError(format!("Failed to list tasks by server: {}", e)))
}

// ============================================================================
// Task History
// ============================================================================

/// Get task history
pub async fn get_task_history(
    pool: &Pool<Sqlite>,
    task_id: i64,
    limit: i64,
) -> Result<Vec<TaskHistory>> {
    sqlx::query_as::<_, TaskHistory>(
        r#"
        SELECT id, task_id, plugin_id, server_id, started_at, finished_at, duration_ms,
               status, exit_code, stdout, stderr, error_message, triggered_by, success, message, timestamp
        FROM task_history
        WHERE task_id = ?
        ORDER BY started_at DESC
        LIMIT ?
        "#,
    )
    .bind(task_id.to_string())
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(|e| Error::DatabaseError(format!("Failed to get task history: {}", e)))
}

/// Get recent task history (all tasks)
pub async fn get_recent_task_history(pool: &Pool<Sqlite>, limit: i64) -> Result<Vec<TaskHistory>> {
    sqlx::query_as::<_, TaskHistory>(
        r#"
        SELECT id, task_id, plugin_id, server_id, started_at, finished_at, duration_ms,
               status, exit_code, stdout, stderr, error_message, triggered_by, success, message, timestamp
        FROM task_history
        ORDER BY started_at DESC
        LIMIT ?
        "#,
    )
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(|e| Error::DatabaseError(format!("Failed to get recent task history: {}", e)))
}

/// Clean old task history
pub async fn clean_old_task_history(pool: &Pool<Sqlite>, days: i64) -> Result<u64> {
    let result = sqlx::query(
        r#"
        DELETE FROM task_history
        WHERE timestamp < datetime('now', '-' || ? || ' days')
        "#,
    )
    .bind(days)
    .execute(pool)
    .await
    .map_err(|e| Error::DatabaseError(format!("Failed to clean old task history: {}", e)))?;

    Ok(result.rows_affected())
}

/// Record task execution in history
pub async fn record_task_execution(
    pool: &Pool<Sqlite>,
    entry: &crate::models::task::TaskHistoryEntry,
) -> Result<i64> {
    let result = sqlx::query(
        r#"
        INSERT INTO task_history (task_id, plugin_id, server_id, success, message, duration_ms, timestamp)
        VALUES (?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(entry.task_id.to_string()) // Convert i64 to TEXT
    .bind(&entry.plugin_id)
    .bind(entry.server_id)
    .bind(entry.success)
    .bind(&entry.output)
    .bind(entry.duration_ms as i64)
    .bind(entry.executed_at)
    .execute(pool)
    .await
    .map_err(|e| Error::DatabaseError(format!("Failed to record task execution: {}", e)))?;

    Ok(result.last_insert_rowid())
}

/// Update task statistics after execution
pub async fn update_task_stats(pool: &Pool<Sqlite>, task_id: i64) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE tasks
        SET last_run_at = CURRENT_TIMESTAMP,
            run_count = run_count + 1
        WHERE id = ?
        "#,
    )
    .bind(task_id)
    .execute(pool)
    .await
    .map_err(|e| Error::DatabaseError(format!("Failed to update task stats: {}", e)))?;

    Ok(())
}

/// Record task execution and update stats in a single transaction
///
/// This ensures atomicity - either both operations succeed or both fail,
/// preventing inconsistent state where history is recorded but stats aren't updated.
pub async fn record_task_execution_with_stats(
    pool: &Pool<Sqlite>,
    history_entry: &TaskHistoryEntry,
) -> Result<i64> {
    // Begin transaction
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| Error::DatabaseError(format!("Failed to begin transaction: {}", e)))?;

    // Insert execution history
    let result = sqlx::query(
        r#"
        INSERT INTO task_executions (task_id, plugin_id, server_id, success, output, error, duration_ms, executed_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(history_entry.task_id)
    .bind(&history_entry.plugin_id)
    .bind(history_entry.server_id)
    .bind(history_entry.success)
    .bind(&history_entry.output)
    .bind(&history_entry.error)
    .bind(history_entry.duration_ms as i64)
    .bind(history_entry.executed_at)
    .execute(&mut *tx)
    .await
    .map_err(|e| Error::DatabaseError(format!("Failed to record task execution: {}", e)))?;

    let execution_id = result.last_insert_rowid();

    // Update task statistics
    sqlx::query(
        r#"
        UPDATE tasks
        SET last_run_at = CURRENT_TIMESTAMP,
            run_count = run_count + 1
        WHERE id = ?
        "#,
    )
    .bind(history_entry.task_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| Error::DatabaseError(format!("Failed to update task stats: {}", e)))?;

    // Commit transaction
    tx.commit()
        .await
        .map_err(|e| Error::DatabaseError(format!("Failed to commit transaction: {}", e)))?;

    Ok(execution_id)
}
