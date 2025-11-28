//! Job schedule database queries

use anyhow::Context;
use chrono::{DateTime, Utc};
use sqlx::{Pool, Sqlite};
use svrctlrs_core::{Error, Result};
use tracing::instrument;

use crate::models::{CreateJobSchedule, JobSchedule, JobScheduleWithDetails, UpdateJobSchedule};

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

/// Get all schedules for a job template
#[instrument(skip(pool))]
pub async fn get_schedules_by_template(pool: &Pool<Sqlite>, template_id: i64) -> Result<Vec<JobSchedule>> {
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
