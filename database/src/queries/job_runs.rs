//! Job run database queries

use anyhow::Context;
use chrono::Utc;
use sqlx::{Pool, Sqlite};
use svrctlrs_core::{Error, Result};
use tracing::instrument;

use crate::models::{JobRun, ServerJobResult, StepExecutionResult};

// ============================================================================
// Job Run Queries
// ============================================================================

/// List recent job runs with limit and offset for pagination
#[instrument(skip(pool))]
pub async fn list_job_runs(pool: &Pool<Sqlite>, limit: i64, offset: i64) -> Result<Vec<JobRun>> {
    sqlx::query_as::<_, JobRun>(
        r#"
        SELECT id, job_schedule_id, job_template_id, server_id, status, started_at, finished_at,
               duration_ms, exit_code, output, error, rendered_command, retry_attempt, is_retry,
               notification_sent, notification_error, metadata
        FROM job_runs
        ORDER BY started_at DESC
        LIMIT ? OFFSET ?
        "#,
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
    .context("Failed to list job runs")
    .map_err(|e| Error::DatabaseError(e.to_string()))
}

/// Extended job run with joined names for display
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct JobRunWithNames {
    pub id: i64,
    pub job_schedule_id: Option<i64>,
    pub job_schedule_name: Option<String>,
    pub job_template_id: i64,
    pub job_template_name: String,
    pub server_id: Option<i64>,
    pub server_name: Option<String>,
    pub status: String,
    pub started_at: chrono::DateTime<Utc>,
    pub finished_at: Option<chrono::DateTime<Utc>>,
    pub duration_ms: Option<i64>,
    pub exit_code: Option<i32>,
    pub output: Option<String>,
    pub error: Option<String>,
    pub rendered_command: Option<String>,
    pub retry_attempt: i32,
    pub is_retry: bool,
    pub notification_sent: bool,
    pub notification_error: Option<String>,
    pub metadata: Option<String>,
}

/// List recent job runs with joined names (optimized for display)
#[instrument(skip(pool))]
pub async fn list_job_runs_with_names(
    pool: &Pool<Sqlite>,
    limit: i64,
    offset: i64,
) -> Result<Vec<JobRunWithNames>> {
    sqlx::query_as::<_, JobRunWithNames>(
        r#"
        SELECT 
            jr.id, jr.job_schedule_id,
            js.name as job_schedule_name,
            jr.job_template_id,
            jt.name as job_template_name,
            jr.server_id,
            s.name as server_name,
            jr.status, jr.started_at, jr.finished_at, jr.duration_ms, jr.exit_code,
            jr.output, jr.error, jr.rendered_command, jr.retry_attempt, jr.is_retry,
            jr.notification_sent, jr.notification_error, jr.metadata
        FROM job_runs jr
        INNER JOIN job_templates jt ON jr.job_template_id = jt.id
        LEFT JOIN job_schedules js ON jr.job_schedule_id = js.id
        LEFT JOIN servers s ON jr.server_id = s.id
        ORDER BY jr.started_at DESC
        LIMIT ? OFFSET ?
        "#,
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
    .context("Failed to list job runs with names")
    .map_err(|e| Error::DatabaseError(e.to_string()))
}

/// Alias for list_job_runs for backwards compatibility
pub use list_job_runs as list_job_runs_paginated;

/// Get job run by ID with server results
#[instrument(skip(pool))]
pub async fn get_job_run(pool: &Pool<Sqlite>, id: i64) -> Result<JobRun> {
    sqlx::query_as::<_, JobRun>(
        r#"
        SELECT id, job_schedule_id, job_template_id, server_id, status, started_at, finished_at,
               duration_ms, exit_code, output, error, rendered_command, retry_attempt, is_retry,
               notification_sent, notification_error, metadata
        FROM job_runs
        WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .context("Failed to get job run")
    .map_err(|e| Error::DatabaseError(e.to_string()))
}

/// Get job runs for a specific template
#[instrument(skip(pool))]
pub async fn get_job_runs_by_template(
    pool: &Pool<Sqlite>,
    template_id: i64,
    limit: i64,
) -> Result<Vec<JobRun>> {
    sqlx::query_as::<_, JobRun>(
        r#"
        SELECT id, job_schedule_id, job_template_id, server_id, status, started_at, finished_at,
               duration_ms, exit_code, output, error, rendered_command, retry_attempt, is_retry,
               notification_sent, notification_error, metadata
        FROM job_runs
        WHERE job_template_id = ?
        ORDER BY started_at DESC
        LIMIT ?
        "#,
    )
    .bind(template_id)
    .bind(limit)
    .fetch_all(pool)
    .await
    .context("Failed to get job runs by template")
    .map_err(|e| Error::DatabaseError(e.to_string()))
}

/// Get job runs for a specific schedule
#[instrument(skip(pool))]
pub async fn get_job_runs_by_schedule(
    pool: &Pool<Sqlite>,
    schedule_id: i64,
    limit: i64,
) -> Result<Vec<JobRun>> {
    sqlx::query_as::<_, JobRun>(
        r#"
        SELECT id, job_schedule_id, job_template_id, server_id, status, started_at, finished_at,
               duration_ms, exit_code, output, error, rendered_command, retry_attempt, is_retry,
               notification_sent, notification_error, metadata
        FROM job_runs
        WHERE job_schedule_id = ?
        ORDER BY started_at DESC
        LIMIT ?
        "#,
    )
    .bind(schedule_id)
    .bind(limit)
    .fetch_all(pool)
    .await
    .context("Failed to get job runs by schedule")
    .map_err(|e| Error::DatabaseError(e.to_string()))
}

/// Create a new job run
#[instrument(skip(pool, metadata))]
pub async fn create_job_run(
    pool: &Pool<Sqlite>,
    job_schedule_id: i64,
    job_template_id: i64,
    server_id: i64,
    retry_attempt: i32,
    is_retry: bool,
    metadata: Option<String>,
) -> Result<i64> {
    let result = sqlx::query(
        r#"
        INSERT INTO job_runs (
            job_schedule_id, job_template_id, server_id, status, retry_attempt, is_retry, metadata
        )
        VALUES (?, ?, ?, 'running', ?, ?, ?)
        "#,
    )
    .bind(job_schedule_id)
    .bind(job_template_id)
    .bind(server_id)
    .bind(retry_attempt)
    .bind(is_retry)
    .bind(metadata)
    .execute(pool)
    .await
    .context("Failed to create job run")
    .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(result.last_insert_rowid())
}

/// Update job run status
#[instrument(skip(pool, metadata))]
pub async fn update_job_run_status(
    pool: &Pool<Sqlite>,
    id: i64,
    status: &str,
    metadata: Option<String>,
) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE job_runs
        SET status = ?, metadata = ?
        WHERE id = ?
        "#,
    )
    .bind(status)
    .bind(metadata)
    .bind(id)
    .execute(pool)
    .await
    .context("Failed to update job run status")
    .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(())
}

/// Mark job run as complete with final results
#[instrument(skip(pool, output, error, rendered_command))]
pub async fn finish_job_run(
    pool: &Pool<Sqlite>,
    id: i64,
    status: &str,
    exit_code: Option<i32>,
    output: Option<String>,
    error: Option<String>,
    rendered_command: Option<String>,
) -> Result<()> {
    let now = Utc::now();

    // Calculate duration from started_at
    let job_run = get_job_run(pool, id).await?;
    let duration_ms = now
        .signed_duration_since(job_run.started_at)
        .num_milliseconds();

    sqlx::query(
        r#"
        UPDATE job_runs
        SET status = ?,
            finished_at = ?,
            duration_ms = ?,
            exit_code = ?,
            output = ?,
            error = ?,
            rendered_command = ?
        WHERE id = ?
        "#,
    )
    .bind(status)
    .bind(now)
    .bind(duration_ms)
    .bind(exit_code)
    .bind(output)
    .bind(error)
    .bind(rendered_command)
    .bind(id)
    .execute(pool)
    .await
    .context("Failed to finish job run")
    .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(())
}

/// Update notification sent status
#[instrument(skip(pool))]
pub async fn update_job_run_notification(
    pool: &Pool<Sqlite>,
    id: i64,
    sent: bool,
    error: Option<String>,
) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE job_runs
        SET notification_sent = ?, notification_error = ?
        WHERE id = ?
        "#,
    )
    .bind(sent)
    .bind(error)
    .bind(id)
    .execute(pool)
    .await
    .context("Failed to update job run notification status")
    .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(())
}

// ============================================================================
// Server Job Result Queries (for multi-server jobs)
// ============================================================================

/// Get all server results for a job run
#[instrument(skip(pool))]
pub async fn get_server_job_results(
    pool: &Pool<Sqlite>,
    run_id: i64,
) -> Result<Vec<ServerJobResult>> {
    sqlx::query_as::<_, ServerJobResult>(
        r#"
        SELECT id, job_run_id, server_id, status, started_at, finished_at, duration_ms,
               exit_code, output, error, metadata
        FROM server_job_results
        WHERE job_run_id = ?
        ORDER BY server_id
        "#,
    )
    .bind(run_id)
    .fetch_all(pool)
    .await
    .context("Failed to get server job results")
    .map_err(|e| Error::DatabaseError(e.to_string()))
}

/// Create a server job result
#[instrument(skip(pool, metadata))]
pub async fn create_server_job_result(
    pool: &Pool<Sqlite>,
    job_run_id: i64,
    server_id: i64,
    metadata: Option<String>,
) -> Result<i64> {
    let result = sqlx::query(
        r#"
        INSERT INTO server_job_results (job_run_id, server_id, status, started_at, metadata)
        VALUES (?, ?, 'success', CURRENT_TIMESTAMP, ?)
        "#,
    )
    .bind(job_run_id)
    .bind(server_id)
    .bind(metadata)
    .execute(pool)
    .await
    .context("Failed to create server job result")
    .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(result.last_insert_rowid())
}

/// Update a server job result
#[instrument(skip(pool, output, error))]
pub async fn update_server_job_result(
    pool: &Pool<Sqlite>,
    id: i64,
    status: &str,
    exit_code: Option<i32>,
    output: Option<String>,
    error: Option<String>,
) -> Result<()> {
    let now = Utc::now();

    // Calculate duration
    let result = sqlx::query_as::<_, ServerJobResult>(
        r#"
        SELECT id, job_run_id, server_id, status, started_at, finished_at, duration_ms,
               exit_code, output, error, metadata
        FROM server_job_results
        WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .context("Failed to get server job result for update")
    .map_err(|e| Error::DatabaseError(e.to_string()))?;

    let duration_ms = now
        .signed_duration_since(result.started_at)
        .num_milliseconds();

    sqlx::query(
        r#"
        UPDATE server_job_results
        SET status = ?, finished_at = ?, duration_ms = ?, exit_code = ?, output = ?, error = ?
        WHERE id = ?
        "#,
    )
    .bind(status)
    .bind(now)
    .bind(duration_ms)
    .bind(exit_code)
    .bind(output)
    .bind(error)
    .bind(id)
    .execute(pool)
    .await
    .context("Failed to update server job result")
    .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(())
}

// ============================================================================
// Step Execution Result Queries (for composite jobs)
// ============================================================================

/// Get all step execution results for a job run
#[instrument(skip(pool))]
pub async fn get_step_execution_results(
    pool: &Pool<Sqlite>,
    job_run_id: i64,
) -> Result<Vec<StepExecutionResult>> {
    sqlx::query_as::<_, StepExecutionResult>(
        r#"
        SELECT id, job_run_id, step_order, step_name, command_template_id, status, started_at,
               finished_at, duration_ms, exit_code, output, error, metadata
        FROM step_execution_results
        WHERE job_run_id = ?
        ORDER BY step_order
        "#,
    )
    .bind(job_run_id)
    .fetch_all(pool)
    .await
    .context("Failed to get step execution results")
    .map_err(|e| Error::DatabaseError(e.to_string()))
}

/// Create a step execution result
#[instrument(skip(pool, metadata))]
pub async fn create_step_execution_result(
    pool: &Pool<Sqlite>,
    job_run_id: i64,
    step_order: i32,
    step_name: &str,
    command_template_id: i64,
    metadata: Option<String>,
) -> Result<i64> {
    let result = sqlx::query(
        r#"
        INSERT INTO step_execution_results (
            job_run_id, step_order, step_name, command_template_id, status, started_at, metadata
        )
        VALUES (?, ?, ?, ?, 'running', CURRENT_TIMESTAMP, ?)
        "#,
    )
    .bind(job_run_id)
    .bind(step_order)
    .bind(step_name)
    .bind(command_template_id)
    .bind(metadata)
    .execute(pool)
    .await
    .context("Failed to create step execution result")
    .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(result.last_insert_rowid())
}

/// Update a step execution result
#[instrument(skip(pool, output, error))]
pub async fn update_step_execution_result(
    pool: &Pool<Sqlite>,
    id: i64,
    status: &str,
    exit_code: Option<i32>,
    output: Option<String>,
    error: Option<String>,
) -> Result<()> {
    let now = Utc::now();

    // Calculate duration
    let result = sqlx::query_as::<_, StepExecutionResult>(
        r#"
        SELECT id, job_run_id, step_order, step_name, command_template_id, status, started_at,
               finished_at, duration_ms, exit_code, output, error, metadata
        FROM step_execution_results
        WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .context("Failed to get step execution result for update")
    .map_err(|e| Error::DatabaseError(e.to_string()))?;

    let duration_ms = now
        .signed_duration_since(result.started_at)
        .num_milliseconds();

    sqlx::query(
        r#"
        UPDATE step_execution_results
        SET status = ?, finished_at = ?, duration_ms = ?, exit_code = ?, output = ?, error = ?
        WHERE id = ?
        "#,
    )
    .bind(status)
    .bind(now)
    .bind(duration_ms)
    .bind(exit_code)
    .bind(output)
    .bind(error)
    .bind(id)
    .execute(pool)
    .await
    .context("Failed to update step execution result")
    .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(())
}

// ============================================================================
// Statistics Queries
// ============================================================================

/// Count currently running job runs
#[instrument(skip(pool))]
pub async fn count_running_jobs(pool: &Pool<Sqlite>) -> Result<i64> {
    let count: (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*) as count
        FROM job_runs
        WHERE status = 'running'
        "#,
    )
    .fetch_one(pool)
    .await
    .context("Failed to count running jobs")
    .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(count.0)
}

/// Count total job runs
#[instrument(skip(pool))]
pub async fn count_job_runs(pool: &Pool<Sqlite>) -> Result<i64> {
    let count: (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*) as count
        FROM job_runs
        "#,
    )
    .fetch_one(pool)
    .await
    .context("Failed to count job runs")
    .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(count.0)
}

// ============================================================================
// Delete Queries
// ============================================================================

/// Delete a single job run and its related results
#[instrument(skip(pool))]
pub async fn delete_job_run(pool: &Pool<Sqlite>, id: i64) -> Result<()> {
    // Delete related step execution results first
    sqlx::query("DELETE FROM step_execution_results WHERE job_run_id = ?")
        .bind(id)
        .execute(pool)
        .await
        .context("Failed to delete step execution results")
        .map_err(|e| Error::DatabaseError(e.to_string()))?;

    // Delete related server job results
    sqlx::query("DELETE FROM server_job_results WHERE job_run_id = ?")
        .bind(id)
        .execute(pool)
        .await
        .context("Failed to delete server job results")
        .map_err(|e| Error::DatabaseError(e.to_string()))?;

    // Delete the job run
    sqlx::query("DELETE FROM job_runs WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .context("Failed to delete job run")
        .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(())
}

/// Delete all completed job runs (excludes running jobs)
#[instrument(skip(pool))]
pub async fn delete_completed_job_runs(pool: &Pool<Sqlite>) -> Result<i64> {
    // Get IDs of completed job runs
    let completed_ids: Vec<(i64,)> = sqlx::query_as(
        r#"
        SELECT id FROM job_runs WHERE status != 'running'
        "#,
    )
    .fetch_all(pool)
    .await
    .context("Failed to get completed job run IDs")
    .map_err(|e| Error::DatabaseError(e.to_string()))?;

    let count = completed_ids.len() as i64;

    // Delete related step execution results
    sqlx::query(
        r#"
        DELETE FROM step_execution_results
        WHERE job_run_id IN (SELECT id FROM job_runs WHERE status != 'running')
        "#,
    )
    .execute(pool)
    .await
    .context("Failed to delete step execution results")
    .map_err(|e| Error::DatabaseError(e.to_string()))?;

    // Delete related server job results
    sqlx::query(
        r#"
        DELETE FROM server_job_results
        WHERE job_run_id IN (SELECT id FROM job_runs WHERE status != 'running')
        "#,
    )
    .execute(pool)
    .await
    .context("Failed to delete server job results")
    .map_err(|e| Error::DatabaseError(e.to_string()))?;

    // Delete the completed job runs
    sqlx::query("DELETE FROM job_runs WHERE status != 'running'")
        .execute(pool)
        .await
        .context("Failed to delete completed job runs")
        .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(count)
}

/// Delete all job runs (including running - use with caution)
#[instrument(skip(pool))]
pub async fn delete_all_job_runs(pool: &Pool<Sqlite>) -> Result<i64> {
    let count = count_job_runs(pool).await?;

    // Delete all step execution results
    sqlx::query("DELETE FROM step_execution_results")
        .execute(pool)
        .await
        .context("Failed to delete all step execution results")
        .map_err(|e| Error::DatabaseError(e.to_string()))?;

    // Delete all server job results
    sqlx::query("DELETE FROM server_job_results")
        .execute(pool)
        .await
        .context("Failed to delete all server job results")
        .map_err(|e| Error::DatabaseError(e.to_string()))?;

    // Delete all job runs
    sqlx::query("DELETE FROM job_runs")
        .execute(pool)
        .await
        .context("Failed to delete all job runs")
        .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(count)
}
