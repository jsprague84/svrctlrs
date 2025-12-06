//! Job schedule database queries

use anyhow::Context;
use chrono::{DateTime, Utc};
use sqlx::{Pool, Sqlite};
use svrctlrs_core::{Error, Result};
use tracing::instrument;

use crate::models::{CreateJobSchedule, JobSchedule, UpdateJobSchedule};

/// List all job schedules
#[instrument(skip(pool))]
pub async fn list_job_schedules(pool: &Pool<Sqlite>) -> Result<Vec<JobSchedule>> {
    sqlx::query_as::<_, JobSchedule>(
        r#"
        SELECT id, name, description, job_template_id, server_id, schedule, enabled,
               timeout_seconds, retry_count, notify_on_success, notify_on_failure,
               notification_policy_id, last_run_at, last_run_status, next_run_at,
               success_count, failure_count, metadata, created_at, updated_at
        FROM job_schedules
        ORDER BY name
        "#,
    )
    .fetch_all(pool)
    .await
    .context("Failed to list job schedules")
    .map_err(|e| Error::DatabaseError(e.to_string()))
}

/// Extended job schedule with joined names for display
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct JobScheduleWithNames {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub job_template_id: i64,
    pub job_template_name: String,
    pub server_id: Option<i64>,
    pub server_name: Option<String>,
    pub schedule: String,
    pub enabled: bool,
    pub timeout_seconds: Option<i32>,
    pub retry_count: Option<i32>,
    pub notify_on_success: bool,
    pub notify_on_failure: bool,
    pub notification_policy_id: Option<i64>,
    pub last_run_at: Option<DateTime<Utc>>,
    pub last_run_status: Option<String>,
    pub next_run_at: Option<DateTime<Utc>>,
    pub success_count: i64,
    pub failure_count: i64,
    pub metadata: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// List all job schedules with joined names (optimized for display)
#[instrument(skip(pool))]
pub async fn list_job_schedules_with_names(
    pool: &Pool<Sqlite>,
) -> Result<Vec<JobScheduleWithNames>> {
    sqlx::query_as::<_, JobScheduleWithNames>(
        r#"
        SELECT 
            js.id, js.name, js.description, js.job_template_id,
            jt.name as job_template_name,
            js.server_id,
            s.name as server_name,
            js.schedule, js.enabled, js.timeout_seconds, js.retry_count,
            js.notify_on_success, js.notify_on_failure, js.notification_policy_id,
            js.last_run_at, js.last_run_status, js.next_run_at,
            js.success_count, js.failure_count, js.metadata,
            js.created_at, js.updated_at
        FROM job_schedules js
        INNER JOIN job_templates jt ON js.job_template_id = jt.id
        LEFT JOIN servers s ON js.server_id = s.id
        ORDER BY js.name
        "#,
    )
    .fetch_all(pool)
    .await
    .context("Failed to list job schedules with names")
    .map_err(|e| Error::DatabaseError(e.to_string()))
}

/// List enabled job schedules only
#[instrument(skip(pool))]
pub async fn list_enabled_schedules(pool: &Pool<Sqlite>) -> Result<Vec<JobSchedule>> {
    sqlx::query_as::<_, JobSchedule>(
        r#"
        SELECT id, name, description, job_template_id, server_id, schedule, enabled,
               timeout_seconds, retry_count, notify_on_success, notify_on_failure,
               notification_policy_id, last_run_at, last_run_status, next_run_at,
               success_count, failure_count, metadata, created_at, updated_at
        FROM job_schedules
        WHERE enabled = 1
        ORDER BY next_run_at
        "#,
    )
    .fetch_all(pool)
    .await
    .context("Failed to list enabled job schedules")
    .map_err(|e| Error::DatabaseError(e.to_string()))
}

/// Get job schedule by ID
#[instrument(skip(pool))]
pub async fn get_job_schedule(pool: &Pool<Sqlite>, id: i64) -> Result<JobSchedule> {
    sqlx::query_as::<_, JobSchedule>(
        r#"
        SELECT id, name, description, job_template_id, server_id, schedule, enabled,
               timeout_seconds, retry_count, notify_on_success, notify_on_failure,
               notification_policy_id, last_run_at, last_run_status, next_run_at,
               success_count, failure_count, metadata, created_at, updated_at
        FROM job_schedules
        WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .context("Failed to get job schedule")
    .map_err(|e| Error::DatabaseError(e.to_string()))
}

/// Get job schedule by ID with joined names (optimized for display)
#[instrument(skip(pool))]
pub async fn get_job_schedule_with_names(
    pool: &Pool<Sqlite>,
    id: i64,
) -> Result<JobScheduleWithNames> {
    sqlx::query_as::<_, JobScheduleWithNames>(
        r#"
        SELECT
            js.id, js.name, js.description, js.job_template_id,
            jt.name as job_template_name,
            js.server_id,
            s.name as server_name,
            js.schedule, js.enabled, js.timeout_seconds, js.retry_count,
            js.notify_on_success, js.notify_on_failure, js.notification_policy_id,
            js.last_run_at, js.last_run_status, js.next_run_at,
            js.success_count, js.failure_count, js.metadata,
            js.created_at, js.updated_at
        FROM job_schedules js
        INNER JOIN job_templates jt ON js.job_template_id = jt.id
        LEFT JOIN servers s ON js.server_id = s.id
        WHERE js.id = ?
        "#,
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .context("Failed to get job schedule with names")
    .map_err(|e| Error::DatabaseError(e.to_string()))
}

/// Get all schedules for a job template
#[instrument(skip(pool))]
pub async fn get_schedules_by_template(
    pool: &Pool<Sqlite>,
    template_id: i64,
) -> Result<Vec<JobSchedule>> {
    sqlx::query_as::<_, JobSchedule>(
        r#"
        SELECT id, name, description, job_template_id, server_id, schedule, enabled,
               timeout_seconds, retry_count, notify_on_success, notify_on_failure,
               notification_policy_id, last_run_at, last_run_status, next_run_at,
               success_count, failure_count, metadata, created_at, updated_at
        FROM job_schedules
        WHERE job_template_id = ?
        ORDER BY name
        "#,
    )
    .bind(template_id)
    .fetch_all(pool)
    .await
    .context("Failed to get schedules by template")
    .map_err(|e| Error::DatabaseError(e.to_string()))
}

/// Create a new job schedule
#[instrument(skip(pool, input))]
pub async fn create_job_schedule(pool: &Pool<Sqlite>, input: &CreateJobSchedule) -> Result<i64> {
    // Validate input
    input
        .validate()
        .map_err(|e| Error::DatabaseError(format!("Validation error: {}", e)))?;

    let result = sqlx::query(
        r#"
        INSERT INTO job_schedules (
            name, description, job_template_id, server_id, schedule, enabled,
            timeout_seconds, retry_count, notify_on_success, notify_on_failure,
            notification_policy_id, metadata
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&input.name)
    .bind(&input.description)
    .bind(input.job_template_id)
    .bind(input.server_id)
    .bind(&input.schedule)
    .bind(input.enabled)
    .bind(input.timeout_seconds)
    .bind(input.retry_count)
    .bind(input.notify_on_success)
    .bind(input.notify_on_failure)
    .bind(input.notification_policy_id)
    .bind(input.metadata_string())
    .execute(pool)
    .await
    .context("Failed to create job schedule")
    .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(result.last_insert_rowid())
}

/// Update an existing job schedule
#[instrument(skip(pool, input))]
pub async fn update_job_schedule(
    pool: &Pool<Sqlite>,
    id: i64,
    input: &UpdateJobSchedule,
) -> Result<()> {
    if !input.has_changes() {
        return Ok(());
    }

    let mut query = String::from("UPDATE job_schedules SET updated_at = CURRENT_TIMESTAMP");
    let mut params: Vec<String> = Vec::new();

    if let Some(name) = &input.name {
        query.push_str(", name = ?");
        params.push(name.clone());
    }
    if let Some(description) = &input.description {
        query.push_str(", description = ?");
        params.push(description.clone());
    }
    if let Some(schedule) = &input.schedule {
        query.push_str(", schedule = ?");
        params.push(schedule.clone());
    }
    if let Some(enabled) = input.enabled {
        query.push_str(", enabled = ?");
        params.push(if enabled { "1" } else { "0" }.to_string());
    }
    if let Some(timeout) = input.timeout_seconds {
        query.push_str(", timeout_seconds = ?");
        params.push(timeout.to_string());
    }
    if let Some(retry) = input.retry_count {
        query.push_str(", retry_count = ?");
        params.push(retry.to_string());
    }
    if let Some(notify_success) = input.notify_on_success {
        query.push_str(", notify_on_success = ?");
        params.push(if notify_success { "1" } else { "0" }.to_string());
    }
    if let Some(notify_failure) = input.notify_on_failure {
        query.push_str(", notify_on_failure = ?");
        params.push(if notify_failure { "1" } else { "0" }.to_string());
    }
    if input.notification_policy_id.is_some() {
        query.push_str(", notification_policy_id = ?");
        params.push(
            input
                .notification_policy_id
                .map(|id| id.to_string())
                .unwrap_or_else(|| "NULL".to_string()),
        );
    }
    if let Some(next_run) = input.next_run_at {
        query.push_str(", next_run_at = ?");
        params.push(next_run.to_rfc3339());
    }
    if let Some(metadata) = input.metadata_string() {
        query.push_str(", metadata = ?");
        params.push(metadata);
    }

    query.push_str(" WHERE id = ?");

    let mut q = sqlx::query(&query);
    for param in params {
        q = q.bind(param);
    }
    q = q.bind(id);

    q.execute(pool)
        .await
        .context("Failed to update job schedule")
        .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(())
}

/// Delete a job schedule
#[instrument(skip(pool))]
pub async fn delete_job_schedule(pool: &Pool<Sqlite>, id: i64) -> Result<()> {
    sqlx::query("DELETE FROM job_schedules WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .context("Failed to delete job schedule")
        .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(())
}

/// Get schedules that are due to run
#[instrument(skip(pool))]
pub async fn get_schedules_due(pool: &Pool<Sqlite>) -> Result<Vec<JobSchedule>> {
    let now = Utc::now();

    sqlx::query_as::<_, JobSchedule>(
        r#"
        SELECT id, name, description, job_template_id, server_id, schedule, enabled,
               timeout_seconds, retry_count, notify_on_success, notify_on_failure,
               notification_policy_id, last_run_at, last_run_status, next_run_at,
               success_count, failure_count, metadata, created_at, updated_at
        FROM job_schedules
        WHERE enabled = 1
          AND (next_run_at IS NULL OR next_run_at <= ?)
        ORDER BY next_run_at
        "#,
    )
    .bind(now)
    .fetch_all(pool)
    .await
    .context("Failed to get due schedules")
    .map_err(|e| Error::DatabaseError(e.to_string()))
}

/// Update schedule next run time
#[instrument(skip(pool))]
pub async fn update_schedule_next_run(
    pool: &Pool<Sqlite>,
    id: i64,
    next_run: DateTime<Utc>,
) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE job_schedules
        SET next_run_at = ?
        WHERE id = ?
        "#,
    )
    .bind(next_run)
    .bind(id)
    .execute(pool)
    .await
    .context("Failed to update schedule next run time")
    .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(())
}

/// Record a schedule run (update last_run_at timestamp and counters)
#[instrument(skip(pool))]
pub async fn record_schedule_run(
    pool: &Pool<Sqlite>,
    id: i64,
    status: &str,
    next_run: Option<DateTime<Utc>>,
) -> Result<()> {
    // Increment appropriate counter
    let counter_field = if status == "success" {
        "success_count"
    } else {
        "failure_count"
    };

    let query_str = format!(
        r#"
        UPDATE job_schedules
        SET last_run_at = CURRENT_TIMESTAMP,
            last_run_status = ?,
            next_run_at = ?,
            {} = {} + 1
        WHERE id = ?
        "#,
        counter_field, counter_field
    );

    sqlx::query(&query_str)
        .bind(status)
        .bind(next_run)
        .bind(id)
        .execute(pool)
        .await
        .context("Failed to record schedule run")
        .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(())
}

/// Count total job schedules
#[instrument(skip(pool))]
pub async fn count_job_schedules(pool: &Pool<Sqlite>) -> Result<i64> {
    let count: (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*) as count
        FROM job_schedules
        "#,
    )
    .fetch_one(pool)
    .await
    .context("Failed to count job schedules")
    .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(count.0)
}

/// Count enabled job schedules
#[instrument(skip(pool))]
pub async fn count_enabled_schedules(pool: &Pool<Sqlite>) -> Result<i64> {
    let count: (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*) as count
        FROM job_schedules
        WHERE enabled = 1
        "#,
    )
    .fetch_one(pool)
    .await
    .context("Failed to count enabled job schedules")
    .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(count.0)
}

/// Job schedule with most recent run info for dashboard display
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct JobScheduleWithLastRun {
    // Schedule fields
    pub schedule_id: i64,
    pub schedule_name: String,
    pub job_template_name: String,
    pub server_name: Option<String>,
    pub cron_expression: String,
    pub schedule_enabled: bool,
    pub next_run_at: Option<DateTime<Utc>>,
    pub success_count: i64,
    pub failure_count: i64,
    // Most recent run fields (nullable if no runs yet)
    pub last_run_id: Option<i64>,
    pub last_run_status: Option<String>,
    pub last_run_started_at: Option<DateTime<Utc>>,
    pub last_run_finished_at: Option<DateTime<Utc>>,
    pub last_run_duration_ms: Option<i64>,
}

/// Get all job schedules with their most recent job run
#[instrument(skip(pool))]
pub async fn list_schedules_with_last_run(pool: &Pool<Sqlite>) -> Result<Vec<JobScheduleWithLastRun>> {
    sqlx::query_as::<_, JobScheduleWithLastRun>(
        r#"
        SELECT
            js.id as schedule_id,
            js.name as schedule_name,
            jt.name as job_template_name,
            s.name as server_name,
            js.schedule as cron_expression,
            js.enabled as schedule_enabled,
            js.next_run_at,
            js.success_count,
            js.failure_count,
            jr.id as last_run_id,
            jr.status as last_run_status,
            jr.started_at as last_run_started_at,
            jr.finished_at as last_run_finished_at,
            jr.duration_ms as last_run_duration_ms
        FROM job_schedules js
        INNER JOIN job_templates jt ON js.job_template_id = jt.id
        LEFT JOIN servers s ON js.server_id = s.id
        LEFT JOIN job_runs jr ON jr.id = (
            SELECT jr2.id
            FROM job_runs jr2
            WHERE jr2.job_schedule_id = js.id
            ORDER BY jr2.started_at DESC
            LIMIT 1
        )
        ORDER BY js.name
        "#,
    )
    .fetch_all(pool)
    .await
    .context("Failed to list schedules with last run")
    .map_err(|e| Error::DatabaseError(e.to_string()))
}
