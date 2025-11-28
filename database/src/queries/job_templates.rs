//! Job template and step database queries

use anyhow::Context;
use sqlx::{Pool, Sqlite};
use svrctlrs_core::{Error, Result};
use tracing::instrument;

use crate::models::{
    CreateJobTemplate, CreateJobTemplateStep, JobTemplate, JobTemplateStep, UpdateJobTemplate,
    UpdateJobTemplateStep,
};

// ============================================================================
// Job Template Queries
// ============================================================================

/// List all job templates
#[instrument(skip(pool))]
pub async fn list_job_templates(pool: &Pool<Sqlite>) -> Result<Vec<JobTemplate>> {
    sqlx::query_as::<_, JobTemplate>(
        r#"
        SELECT id, name, display_name, description, job_type_id, is_composite, command_template_id,
               variables, timeout_seconds, retry_count, retry_delay_seconds, notify_on_success,
               notify_on_failure, notification_policy_id, metadata, created_at, updated_at
        FROM job_templates
        ORDER BY name
        "#,
    )
    .fetch_all(pool)
    .await
    .context("Failed to list job templates")
    .map_err(|e| Error::DatabaseError(e.to_string()))
}

/// Get job template by ID with steps (if composite)
#[instrument(skip(pool))]
pub async fn get_job_template(pool: &Pool<Sqlite>, id: i64) -> Result<JobTemplate> {
    sqlx::query_as::<_, JobTemplate>(
        r#"
        SELECT id, name, display_name, description, job_type_id, is_composite, command_template_id,
               variables, timeout_seconds, retry_count, retry_delay_seconds, notify_on_success,
               notify_on_failure, notification_policy_id, metadata, created_at, updated_at
        FROM job_templates
        WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .context("Failed to get job template")
    .map_err(|e| Error::DatabaseError(e.to_string()))
}

/// Create a new job template
#[instrument(skip(pool, input))]
pub async fn create_job_template(pool: &Pool<Sqlite>, input: &CreateJobTemplate) -> Result<i64> {
    // Validate input
    input
        .validate()
        .map_err(|e| Error::DatabaseError(format!("Validation error: {}", e)))?;

    let result = sqlx::query(
        r#"
        INSERT INTO job_templates (
            name, display_name, description, job_type_id, is_composite, command_template_id,
            variables, timeout_seconds, retry_count, retry_delay_seconds, notify_on_success,
            notify_on_failure, notification_policy_id, metadata
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&input.name)
    .bind(&input.display_name)
    .bind(&input.description)
    .bind(input.job_type_id)
    .bind(input.is_composite)
    .bind(input.command_template_id)
    .bind(input.variables_string())
    .bind(input.timeout_seconds)
    .bind(input.retry_count)
    .bind(input.retry_delay_seconds)
    .bind(input.notify_on_success)
    .bind(input.notify_on_failure)
    .bind(input.notification_policy_id)
    .bind(input.metadata_string())
    .execute(pool)
    .await
    .context("Failed to create job template")
    .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(result.last_insert_rowid())
}

/// Update an existing job template
#[instrument(skip(pool, input))]
pub async fn update_job_template(
    pool: &Pool<Sqlite>,
    id: i64,
    input: &UpdateJobTemplate,
) -> Result<()> {
    if !input.has_changes() {
        return Ok(());
    }

    let mut query = String::from("UPDATE job_templates SET updated_at = CURRENT_TIMESTAMP");
    let mut params: Vec<String> = Vec::new();

    if let Some(display_name) = &input.display_name {
        query.push_str(", display_name = ?");
        params.push(display_name.clone());
    }
    if let Some(description) = &input.description {
        query.push_str(", description = ?");
        params.push(description.clone());
    }
    if let Some(cmd_id) = input.command_template_id {
        query.push_str(", command_template_id = ?");
        params.push(cmd_id.to_string());
    }
    if let Some(vars) = input.variables_string() {
        query.push_str(", variables = ?");
        params.push(vars);
    }
    if let Some(timeout) = input.timeout_seconds {
        query.push_str(", timeout_seconds = ?");
        params.push(timeout.to_string());
    }
    if let Some(retry) = input.retry_count {
        query.push_str(", retry_count = ?");
        params.push(retry.to_string());
    }
    if let Some(delay) = input.retry_delay_seconds {
        query.push_str(", retry_delay_seconds = ?");
        params.push(delay.to_string());
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
        .context("Failed to update job template")
        .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(())
}

/// Delete a job template
#[instrument(skip(pool))]
pub async fn delete_job_template(pool: &Pool<Sqlite>, id: i64) -> Result<()> {
    // Check if template is in use by job schedules
    let count: (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*) as count
        FROM job_schedules
        WHERE job_template_id = ?
        "#,
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .context("Failed to check if job template is in use")
    .map_err(|e| Error::DatabaseError(e.to_string()))?;

    if count.0 > 0 {
        return Err(Error::DatabaseError(
            "Cannot delete job template: it is in use by one or more job schedules".to_string(),
        ));
    }

    sqlx::query("DELETE FROM job_templates WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .context("Failed to delete job template")
        .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(())
}

// ============================================================================
// Job Template Step Queries
// ============================================================================

/// Get all steps for a job template
#[instrument(skip(pool))]
pub async fn get_job_template_steps(pool: &Pool<Sqlite>, template_id: i64) -> Result<Vec<JobTemplateStep>> {
    sqlx::query_as::<_, JobTemplateStep>(
        r#"
        SELECT id, job_template_id, step_order, name, command_template_id, variables,
               continue_on_failure, timeout_seconds, metadata
        FROM job_template_steps
        WHERE job_template_id = ?
        ORDER BY step_order
        "#,
    )
    .bind(template_id)
    .fetch_all(pool)
    .await
    .context("Failed to get job template steps")
    .map_err(|e| Error::DatabaseError(e.to_string()))
}

/// Get a specific job template step by ID
#[instrument(skip(pool))]
pub async fn get_job_template_step(pool: &Pool<Sqlite>, id: i64) -> Result<JobTemplateStep> {
    sqlx::query_as::<_, JobTemplateStep>(
        r#"
        SELECT id, job_template_id, step_order, name, command_template_id, variables,
               continue_on_failure, timeout_seconds, metadata
        FROM job_template_steps
        WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .context("Failed to get job template step")
    .map_err(|e| Error::DatabaseError(e.to_string()))
}

/// Create a new job template step
#[instrument(skip(pool, input))]
pub async fn create_job_template_step(pool: &Pool<Sqlite>, input: &CreateJobTemplateStep) -> Result<i64> {
    // Validate input
    input
        .validate()
        .map_err(|e| Error::DatabaseError(format!("Validation error: {}", e)))?;

    let result = sqlx::query(
        r#"
        INSERT INTO job_template_steps (
            job_template_id, step_order, name, command_template_id, variables,
            continue_on_failure, timeout_seconds, metadata
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(input.job_template_id)
    .bind(input.step_order)
    .bind(&input.name)
    .bind(input.command_template_id)
    .bind(input.variables_string())
    .bind(input.continue_on_failure)
    .bind(input.timeout_seconds)
    .bind(input.metadata_string())
    .execute(pool)
    .await
    .context("Failed to create job template step")
    .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(result.last_insert_rowid())
}

/// Update an existing job template step
#[instrument(skip(pool, input))]
pub async fn update_job_template_step(
    pool: &Pool<Sqlite>,
    id: i64,
    input: &UpdateJobTemplateStep,
) -> Result<()> {
    if !input.has_changes() {
        return Ok(());
    }

    let mut query = String::from("UPDATE job_template_steps SET");
    let mut params: Vec<String> = Vec::new();
    let mut first = true;

    if let Some(order) = input.step_order {
        if !first {
            query.push(',');
        }
        query.push_str(" step_order = ?");
        params.push(order.to_string());
        first = false;
    }
    if let Some(name) = &input.name {
        if !first {
            query.push(',');
        }
        query.push_str(" name = ?");
        params.push(name.clone());
        first = false;
    }
    if let Some(cmd_id) = input.command_template_id {
        if !first {
            query.push(',');
        }
        query.push_str(" command_template_id = ?");
        params.push(cmd_id.to_string());
        first = false;
    }
    if let Some(vars) = input.variables_string() {
        if !first {
            query.push(',');
        }
        query.push_str(" variables = ?");
        params.push(vars);
        first = false;
    }
    if let Some(continue_on_fail) = input.continue_on_failure {
        if !first {
            query.push(',');
        }
        query.push_str(" continue_on_failure = ?");
        params.push(if continue_on_fail { "1" } else { "0" }.to_string());
        first = false;
    }
    if input.timeout_seconds.is_some() {
        if !first {
            query.push(',');
        }
        query.push_str(" timeout_seconds = ?");
        params.push(
            input
                .timeout_seconds
                .map(|t| t.to_string())
                .unwrap_or_else(|| "NULL".to_string()),
        );
        first = false;
    }
    if let Some(metadata) = input.metadata_string() {
        if !first {
            query.push(',');
        }
        query.push_str(" metadata = ?");
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
        .context("Failed to update job template step")
        .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(())
}

/// Delete a job template step
#[instrument(skip(pool))]
pub async fn delete_job_template_step(pool: &Pool<Sqlite>, id: i64) -> Result<()> {
    sqlx::query("DELETE FROM job_template_steps WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .context("Failed to delete job template step")
        .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(())
}

/// Reorder job template steps
#[instrument(skip(pool, step_orders))]
pub async fn reorder_job_template_steps(
    pool: &Pool<Sqlite>,
    template_id: i64,
    step_orders: &[(i64, i32)], // Vec of (step_id, new_order)
) -> Result<()> {
    // Use a transaction to ensure atomicity
    let mut tx = pool
        .begin()
        .await
        .context("Failed to begin transaction")
        .map_err(|e| Error::DatabaseError(e.to_string()))?;

    for (step_id, new_order) in step_orders {
        sqlx::query(
            r#"
            UPDATE job_template_steps
            SET step_order = ?
            WHERE id = ? AND job_template_id = ?
            "#,
        )
        .bind(new_order)
        .bind(step_id)
        .bind(template_id)
        .execute(&mut *tx)
        .await
        .context("Failed to reorder step")
        .map_err(|e| Error::DatabaseError(e.to_string()))?;
    }

    tx.commit()
        .await
        .context("Failed to commit transaction")
        .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(())
}
