//! Notification channel, policy, and log database queries

use anyhow::Context;
use sqlx::{Pool, Sqlite};
use svrctlrs_core::{Error, Result};
use tracing::instrument;

use crate::models::{
    CreateNotificationChannel, CreateNotificationPolicy, NotificationChannel, NotificationLog,
    NotificationPolicy, UpdateNotificationChannel, UpdateNotificationPolicy,
};

// ============================================================================
// Notification Channel Queries
// ============================================================================

/// List all notification channels
#[instrument(skip(pool))]
pub async fn list_notification_channels(pool: &Pool<Sqlite>) -> Result<Vec<NotificationChannel>> {
    sqlx::query_as::<_, NotificationChannel>(
        r#"
        SELECT id, name, channel_type, description, config, enabled, default_priority,
               last_test_at, last_test_success, last_test_error, metadata, created_at, updated_at
        FROM notification_channels
        ORDER BY name
        "#,
    )
    .fetch_all(pool)
    .await
    .context("Failed to list notification channels")
    .map_err(|e| Error::DatabaseError(e.to_string()))
}

/// Get notification channel by ID
#[instrument(skip(pool))]
pub async fn get_notification_channel(pool: &Pool<Sqlite>, id: i64) -> Result<NotificationChannel> {
    sqlx::query_as::<_, NotificationChannel>(
        r#"
        SELECT id, name, channel_type, description, config, enabled, default_priority,
               last_test_at, last_test_success, last_test_error, metadata, created_at, updated_at
        FROM notification_channels
        WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .context("Failed to get notification channel")
    .map_err(|e| Error::DatabaseError(e.to_string()))
}

/// Create a new notification channel
#[instrument(skip(pool, input))]
pub async fn create_notification_channel(
    pool: &Pool<Sqlite>,
    input: &CreateNotificationChannel,
) -> Result<i64> {
    let result = sqlx::query(
        r#"
        INSERT INTO notification_channels (
            name, channel_type, description, config, enabled, default_priority, metadata
        )
        VALUES (?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&input.name)
    .bind(input.channel_type.as_str())
    .bind(&input.description)
    .bind(input.config_string())
    .bind(input.enabled)
    .bind(input.default_priority)
    .bind(input.metadata_string())
    .execute(pool)
    .await
    .context("Failed to create notification channel")
    .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(result.last_insert_rowid())
}

/// Update an existing notification channel
#[instrument(skip(pool, input))]
pub async fn update_notification_channel(
    pool: &Pool<Sqlite>,
    id: i64,
    input: &UpdateNotificationChannel,
) -> Result<()> {
    if !input.has_changes() {
        return Ok(());
    }

    let mut query = String::from("UPDATE notification_channels SET updated_at = CURRENT_TIMESTAMP");
    let mut params: Vec<String> = Vec::new();

    if let Some(name) = &input.name {
        query.push_str(", name = ?");
        params.push(name.clone());
    }
    if let Some(description) = &input.description {
        query.push_str(", description = ?");
        params.push(description.clone());
    }
    if let Some(config) = input.config_string() {
        query.push_str(", config = ?");
        params.push(config);
    }
    if let Some(enabled) = input.enabled {
        query.push_str(", enabled = ?");
        params.push(if enabled { "1" } else { "0" }.to_string());
    }
    if let Some(priority) = input.default_priority {
        query.push_str(", default_priority = ?");
        params.push(priority.to_string());
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
        .context("Failed to update notification channel")
        .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(())
}

/// Delete a notification channel
#[instrument(skip(pool))]
pub async fn delete_notification_channel(pool: &Pool<Sqlite>, id: i64) -> Result<()> {
    sqlx::query("DELETE FROM notification_channels WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .context("Failed to delete notification channel")
        .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(())
}

/// Update channel last used timestamp and error status
#[instrument(skip(pool))]
pub async fn update_channel_last_used(
    pool: &Pool<Sqlite>,
    id: i64,
    success: bool,
    error: Option<String>,
) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE notification_channels
        SET last_test_at = CURRENT_TIMESTAMP,
            last_test_success = ?,
            last_test_error = ?
        WHERE id = ?
        "#,
    )
    .bind(success)
    .bind(error)
    .bind(id)
    .execute(pool)
    .await
    .context("Failed to update channel last used")
    .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(())
}

// ============================================================================
// Notification Policy Queries
// ============================================================================

/// List all notification policies
#[instrument(skip(pool))]
pub async fn list_notification_policies(pool: &Pool<Sqlite>) -> Result<Vec<NotificationPolicy>> {
    sqlx::query_as::<_, NotificationPolicy>(
        r#"
        SELECT id, name, description, on_success, on_failure, on_timeout, job_type_filter,
               server_filter, tag_filter, min_severity, max_per_hour, title_template,
               body_template, enabled, metadata, created_at, updated_at
        FROM notification_policies
        ORDER BY name
        "#,
    )
    .fetch_all(pool)
    .await
    .context("Failed to list notification policies")
    .map_err(|e| Error::DatabaseError(e.to_string()))
}

/// Get notification policy by ID with channels
#[instrument(skip(pool))]
pub async fn get_notification_policy(pool: &Pool<Sqlite>, id: i64) -> Result<NotificationPolicy> {
    sqlx::query_as::<_, NotificationPolicy>(
        r#"
        SELECT id, name, description, on_success, on_failure, on_timeout, job_type_filter,
               server_filter, tag_filter, min_severity, max_per_hour, title_template,
               body_template, enabled, metadata, created_at, updated_at
        FROM notification_policies
        WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .context("Failed to get notification policy")
    .map_err(|e| Error::DatabaseError(e.to_string()))
}

/// Get channels linked to a policy
#[instrument(skip(pool))]
pub async fn get_policy_channels(
    pool: &Pool<Sqlite>,
    policy_id: i64,
) -> Result<Vec<NotificationChannel>> {
    sqlx::query_as::<_, NotificationChannel>(
        r#"
        SELECT nc.id, nc.name, nc.channel_type, nc.description, nc.config, nc.enabled,
               nc.default_priority, nc.last_test_at, nc.last_test_success, nc.last_test_error,
               nc.metadata, nc.created_at, nc.updated_at
        FROM notification_channels nc
        JOIN notification_policy_channels npc ON nc.id = npc.channel_id
        WHERE npc.policy_id = ?
        ORDER BY nc.name
        "#,
    )
    .bind(policy_id)
    .fetch_all(pool)
    .await
    .context("Failed to get policy channels")
    .map_err(|e| Error::DatabaseError(e.to_string()))
}

/// Create a new notification policy
#[instrument(skip(pool, input))]
pub async fn create_notification_policy(
    pool: &Pool<Sqlite>,
    input: &CreateNotificationPolicy,
) -> Result<i64> {
    let result = sqlx::query(
        r#"
        INSERT INTO notification_policies (
            name, description, on_success, on_failure, on_timeout, job_type_filter,
            server_filter, tag_filter, min_severity, max_per_hour, title_template,
            body_template, enabled, metadata
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&input.name)
    .bind(&input.description)
    .bind(input.on_success)
    .bind(input.on_failure)
    .bind(input.on_timeout)
    .bind(input.job_type_filter_string())
    .bind(input.server_filter_string())
    .bind(input.tag_filter_string())
    .bind(input.min_severity)
    .bind(input.max_per_hour)
    .bind(&input.title_template)
    .bind(&input.body_template)
    .bind(input.enabled)
    .bind(input.metadata_string())
    .execute(pool)
    .await
    .context("Failed to create notification policy")
    .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(result.last_insert_rowid())
}

/// Update an existing notification policy
#[instrument(skip(pool, input))]
pub async fn update_notification_policy(
    pool: &Pool<Sqlite>,
    id: i64,
    input: &UpdateNotificationPolicy,
) -> Result<()> {
    if !input.has_changes() {
        return Ok(());
    }

    let mut query = String::from("UPDATE notification_policies SET updated_at = CURRENT_TIMESTAMP");
    let mut params: Vec<String> = Vec::new();

    if let Some(name) = &input.name {
        query.push_str(", name = ?");
        params.push(name.clone());
    }
    if let Some(description) = &input.description {
        query.push_str(", description = ?");
        params.push(description.clone());
    }
    if let Some(on_success) = input.on_success {
        query.push_str(", on_success = ?");
        params.push(if on_success { "1" } else { "0" }.to_string());
    }
    if let Some(on_failure) = input.on_failure {
        query.push_str(", on_failure = ?");
        params.push(if on_failure { "1" } else { "0" }.to_string());
    }
    if let Some(on_timeout) = input.on_timeout {
        query.push_str(", on_timeout = ?");
        params.push(if on_timeout { "1" } else { "0" }.to_string());
    }
    if let Some(filter) = input.job_type_filter_string() {
        query.push_str(", job_type_filter = ?");
        params.push(filter);
    }
    if let Some(filter) = input.server_filter_string() {
        query.push_str(", server_filter = ?");
        params.push(filter);
    }
    if let Some(filter) = input.tag_filter_string() {
        query.push_str(", tag_filter = ?");
        params.push(filter);
    }
    if let Some(severity) = input.min_severity {
        query.push_str(", min_severity = ?");
        params.push(severity.to_string());
    }
    if let Some(max) = input.max_per_hour {
        query.push_str(", max_per_hour = ?");
        params.push(max.to_string());
    }
    if let Some(template) = &input.title_template {
        query.push_str(", title_template = ?");
        params.push(template.clone());
    }
    if let Some(template) = &input.body_template {
        query.push_str(", body_template = ?");
        params.push(template.clone());
    }
    if let Some(enabled) = input.enabled {
        query.push_str(", enabled = ?");
        params.push(if enabled { "1" } else { "0" }.to_string());
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
        .context("Failed to update notification policy")
        .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(())
}

/// Delete a notification policy
#[instrument(skip(pool))]
pub async fn delete_notification_policy(pool: &Pool<Sqlite>, id: i64) -> Result<()> {
    sqlx::query("DELETE FROM notification_policies WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .context("Failed to delete notification policy")
        .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(())
}

/// Link a channel to a policy
#[instrument(skip(pool))]
pub async fn add_policy_channel(
    pool: &Pool<Sqlite>,
    policy_id: i64,
    channel_id: i64,
    priority_override: Option<i32>,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT OR IGNORE INTO notification_policy_channels (policy_id, channel_id, priority_override)
        VALUES (?, ?, ?)
        "#,
    )
    .bind(policy_id)
    .bind(channel_id)
    .bind(priority_override)
    .execute(pool)
    .await
    .context("Failed to add channel to policy")
    .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(())
}

/// Unlink a channel from a policy
#[instrument(skip(pool))]
pub async fn remove_policy_channel(
    pool: &Pool<Sqlite>,
    policy_id: i64,
    channel_id: i64,
) -> Result<()> {
    sqlx::query(
        r#"
        DELETE FROM notification_policy_channels
        WHERE policy_id = ? AND channel_id = ?
        "#,
    )
    .bind(policy_id)
    .bind(channel_id)
    .execute(pool)
    .await
    .context("Failed to remove channel from policy")
    .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(())
}

// ============================================================================
// Notification Log Queries
// ============================================================================

/// Log a sent notification
#[allow(clippy::too_many_arguments)]
#[instrument(skip(pool))]
pub async fn log_notification(
    pool: &Pool<Sqlite>,
    channel_id: i64,
    policy_id: Option<i64>,
    job_run_id: Option<i64>,
    title: &str,
    body: Option<&str>,
    priority: i32,
    success: bool,
    error_message: Option<&str>,
    retry_count: i32,
) -> Result<i64> {
    let result = sqlx::query(
        r#"
        INSERT INTO notification_log (
            channel_id, policy_id, job_run_id, title, body, priority,
            success, error_message, retry_count
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(channel_id)
    .bind(policy_id)
    .bind(job_run_id)
    .bind(title)
    .bind(body)
    .bind(priority)
    .bind(success)
    .bind(error_message)
    .bind(retry_count)
    .execute(pool)
    .await
    .context("Failed to log notification")
    .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(result.last_insert_rowid())
}

/// Get recent notification logs
#[instrument(skip(pool))]
pub async fn get_notification_log(
    pool: &Pool<Sqlite>,
    limit: i64,
    offset: i64,
) -> Result<Vec<NotificationLog>> {
    sqlx::query_as::<_, NotificationLog>(
        r#"
        SELECT id, channel_id, policy_id, job_run_id, title, body, priority,
               success, error_message, retry_count, sent_at
        FROM notification_log
        ORDER BY sent_at DESC
        LIMIT ? OFFSET ?
        "#,
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
    .context("Failed to get notification log")
    .map_err(|e| Error::DatabaseError(e.to_string()))
}

/// Get notification logs for a specific job run
#[instrument(skip(pool))]
pub async fn get_notification_logs_for_run(
    pool: &Pool<Sqlite>,
    run_id: i64,
) -> Result<Vec<NotificationLog>> {
    sqlx::query_as::<_, NotificationLog>(
        r#"
        SELECT id, channel_id, policy_id, job_run_id, title, body, priority,
               success, error_message, retry_count, sent_at
        FROM notification_log
        WHERE job_run_id = ?
        ORDER BY sent_at DESC
        "#,
    )
    .bind(run_id)
    .fetch_all(pool)
    .await
    .context("Failed to get notification logs for run")
    .map_err(|e| Error::DatabaseError(e.to_string()))
}
