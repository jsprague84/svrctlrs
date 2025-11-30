//! Job type and command template database queries

use anyhow::Context;
use sqlx::{Pool, Sqlite};
use svrctlrs_core::{Error, Result};
use tracing::instrument;

use crate::models::{
    CommandTemplate, CreateCommandTemplate, CreateJobType, JobType, UpdateCommandTemplate,
    UpdateJobType,
};

// ============================================================================
// Job Type Queries
// ============================================================================

/// List all job types
#[instrument(skip(pool))]
pub async fn list_job_types(pool: &Pool<Sqlite>) -> Result<Vec<JobType>> {
    sqlx::query_as::<_, JobType>(
        r#"
        SELECT id, name, display_name, description, icon, color, requires_capabilities, metadata, enabled, created_at
        FROM job_types
        ORDER BY name
        "#,
    )
    .fetch_all(pool)
    .await
    .context("Failed to list job types")
    .map_err(|e| Error::DatabaseError(e.to_string()))
}

/// Get job type by ID with command templates
#[instrument(skip(pool))]
pub async fn get_job_type(pool: &Pool<Sqlite>, id: i64) -> Result<JobType> {
    sqlx::query_as::<_, JobType>(
        r#"
        SELECT id, name, display_name, description, icon, color, requires_capabilities, metadata, enabled, created_at
        FROM job_types
        WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .context("Failed to get job type")
    .map_err(|e| Error::DatabaseError(e.to_string()))
}

/// Get job types by category (use metadata filtering if needed)
#[instrument(skip(pool))]
pub async fn get_job_types_by_name(pool: &Pool<Sqlite>, name: &str) -> Result<JobType> {
    sqlx::query_as::<_, JobType>(
        r#"
        SELECT id, name, display_name, description, icon, color, requires_capabilities, metadata, enabled, created_at
        FROM job_types
        WHERE name = ?
        "#,
    )
    .bind(name)
    .fetch_one(pool)
    .await
    .context("Failed to get job type by name")
    .map_err(|e| Error::DatabaseError(e.to_string()))
}

/// Create a new job type
#[instrument(skip(pool, input))]
pub async fn create_job_type(pool: &Pool<Sqlite>, input: &CreateJobType) -> Result<i64> {
    let result = sqlx::query(
        r#"
        INSERT INTO job_types (name, display_name, description, icon, color, requires_capabilities, metadata, enabled)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&input.name)
    .bind(&input.display_name)
    .bind(&input.description)
    .bind(&input.icon)
    .bind(&input.color)
    .bind(input.requires_capabilities_string())
    .bind(input.metadata_string())
    .bind(input.enabled)
    .execute(pool)
    .await
    .context("Failed to create job type")
    .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(result.last_insert_rowid())
}

/// Update an existing job type
#[instrument(skip(pool, input))]
pub async fn update_job_type(pool: &Pool<Sqlite>, id: i64, input: &UpdateJobType) -> Result<()> {
    if !input.has_changes() {
        return Ok(());
    }

    let mut query = String::from("UPDATE job_types SET");
    let mut params: Vec<String> = Vec::new();
    let mut first = true;

    if let Some(display_name) = &input.display_name {
        if !first {
            query.push(',');
        }
        query.push_str(" display_name = ?");
        params.push(display_name.clone());
        first = false;
    }
    if let Some(description) = &input.description {
        if !first {
            query.push(',');
        }
        query.push_str(" description = ?");
        params.push(description.clone());
        first = false;
    }
    if let Some(icon) = &input.icon {
        if !first {
            query.push(',');
        }
        query.push_str(" icon = ?");
        params.push(icon.clone());
        first = false;
    }
    if let Some(color) = &input.color {
        if !first {
            query.push(',');
        }
        query.push_str(" color = ?");
        params.push(color.clone());
        first = false;
    }
    if let Some(caps) = input.requires_capabilities_string() {
        if !first {
            query.push(',');
        }
        query.push_str(" requires_capabilities = ?");
        params.push(caps);
        first = false;
    }
    if let Some(metadata) = input.metadata_string() {
        if !first {
            query.push(',');
        }
        query.push_str(" metadata = ?");
        params.push(metadata);
        first = false;
    }
    if let Some(enabled) = input.enabled {
        if !first {
            query.push(',');
        }
        query.push_str(" enabled = ?");
        params.push(if enabled { "1" } else { "0" }.to_string());
    }

    query.push_str(" WHERE id = ?");

    let mut q = sqlx::query(&query);
    for param in params {
        q = q.bind(param);
    }
    q = q.bind(id);

    q.execute(pool)
        .await
        .context("Failed to update job type")
        .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(())
}

/// Delete a job type (check if in use first)
#[instrument(skip(pool))]
pub async fn delete_job_type(pool: &Pool<Sqlite>, id: i64) -> Result<()> {
    // Check if job type is in use by any job templates
    let count: (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*) as count
        FROM job_templates
        WHERE job_type_id = ?
        "#,
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .context("Failed to check if job type is in use")
    .map_err(|e| Error::DatabaseError(e.to_string()))?;

    if count.0 > 0 {
        return Err(Error::DatabaseError(
            "Cannot delete job type: it is in use by one or more job templates".to_string(),
        ));
    }

    sqlx::query("DELETE FROM job_types WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .context("Failed to delete job type")
        .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(())
}

// ============================================================================
// Command Template Queries
// ============================================================================

/// Get all command templates for a job type
#[instrument(skip(pool))]
pub async fn get_command_templates(
    pool: &Pool<Sqlite>,
    job_type_id: i64,
) -> Result<Vec<CommandTemplate>> {
    sqlx::query_as::<_, CommandTemplate>(
        r#"
        SELECT id, job_type_id, name, display_name, description, command, required_capabilities,
               os_filter, timeout_seconds, working_directory, environment, output_format,
               parse_output, output_parser, notify_on_success, notify_on_failure, metadata,
               created_at, updated_at
        FROM command_templates
        WHERE job_type_id = ?
        ORDER BY name
        "#,
    )
    .bind(job_type_id)
    .fetch_all(pool)
    .await
    .context("Failed to get command templates")
    .map_err(|e| Error::DatabaseError(e.to_string()))
}

/// Get command template by ID
#[instrument(skip(pool))]
pub async fn get_command_template(pool: &Pool<Sqlite>, id: i64) -> Result<CommandTemplate> {
    sqlx::query_as::<_, CommandTemplate>(
        r#"
        SELECT id, job_type_id, name, display_name, description, command, required_capabilities,
               os_filter, timeout_seconds, working_directory, environment, output_format,
               parse_output, output_parser, notify_on_success, notify_on_failure, parameter_schema, metadata,
               created_at, updated_at
        FROM command_templates
        WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .context("Failed to get command template")
    .map_err(|e| Error::DatabaseError(e.to_string()))
}

/// Get a command template by name
pub async fn get_command_template_by_name(pool: &Pool<Sqlite>, name: &str) -> Result<CommandTemplate> {
    sqlx::query_as::<_, CommandTemplate>(
        r#"
        SELECT id, job_type_id, name, display_name, description, command, required_capabilities,
               os_filter, timeout_seconds, working_directory, environment, output_format,
               parse_output, output_parser, notify_on_success, notify_on_failure, parameter_schema, metadata,
               created_at, updated_at
        FROM command_templates
        WHERE name = ?
        "#,
    )
    .bind(name)
    .fetch_one(pool)
    .await
    .context("Failed to get command template by name")
    .map_err(|e| Error::DatabaseError(e.to_string()))
}

/// Create a new command template
#[instrument(skip(pool, input))]
pub async fn create_command_template(
    pool: &Pool<Sqlite>,
    input: &CreateCommandTemplate,
) -> Result<i64> {
    let result = sqlx::query(
        r#"
        INSERT INTO command_templates (
            job_type_id, name, display_name, description, command, required_capabilities,
            os_filter, timeout_seconds, working_directory, environment, output_format,
            parse_output, output_parser, notify_on_success, notify_on_failure, parameter_schema, metadata
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(input.job_type_id)
    .bind(&input.name)
    .bind(&input.display_name)
    .bind(&input.description)
    .bind(&input.command)
    .bind(input.required_capabilities_string())
    .bind(input.os_filter_string())
    .bind(input.timeout_seconds)
    .bind(&input.working_directory)
    .bind(input.environment_string())
    .bind(&input.output_format)
    .bind(input.parse_output)
    .bind(input.output_parser_string())
    .bind(input.notify_on_success)
    .bind(input.notify_on_failure)
    .bind(input.parameter_schema_string())
    .bind(input.metadata_string())
    .execute(pool)
    .await
    .context("Failed to create command template")
    .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(result.last_insert_rowid())
}

/// Update an existing command template
#[instrument(skip(pool, input))]
pub async fn update_command_template(
    pool: &Pool<Sqlite>,
    id: i64,
    input: &UpdateCommandTemplate,
) -> Result<()> {
    if !input.has_changes() {
        return Ok(());
    }

    let mut query = String::from("UPDATE command_templates SET updated_at = CURRENT_TIMESTAMP");
    let mut params: Vec<String> = Vec::new();

    if let Some(display_name) = &input.display_name {
        query.push_str(", display_name = ?");
        params.push(display_name.clone());
    }
    if let Some(description) = &input.description {
        query.push_str(", description = ?");
        params.push(description.clone());
    }
    if let Some(command) = &input.command {
        query.push_str(", command = ?");
        params.push(command.clone());
    }
    if let Some(caps) = input.required_capabilities_string() {
        query.push_str(", required_capabilities = ?");
        params.push(caps);
    }
    if let Some(filter) = input.os_filter_string() {
        query.push_str(", os_filter = ?");
        params.push(filter);
    }
    if let Some(timeout) = input.timeout_seconds {
        query.push_str(", timeout_seconds = ?");
        params.push(timeout.to_string());
    }
    if let Some(working_dir) = &input.working_directory {
        query.push_str(", working_directory = ?");
        params.push(working_dir.clone());
    }
    if let Some(env) = input.environment_string() {
        query.push_str(", environment = ?");
        params.push(env);
    }
    if let Some(format) = &input.output_format {
        query.push_str(", output_format = ?");
        params.push(format.clone());
    }
    if let Some(parse) = input.parse_output {
        query.push_str(", parse_output = ?");
        params.push(if parse { "1" } else { "0" }.to_string());
    }
    if let Some(parser) = input.output_parser_string() {
        query.push_str(", output_parser = ?");
        params.push(parser);
    }
    if let Some(notify_success) = input.notify_on_success {
        query.push_str(", notify_on_success = ?");
        params.push(if notify_success { "1" } else { "0" }.to_string());
    }
    if let Some(notify_failure) = input.notify_on_failure {
        query.push_str(", notify_on_failure = ?");
        params.push(if notify_failure { "1" } else { "0" }.to_string());
    }
    if let Some(param_schema) = input.parameter_schema_string() {
        query.push_str(", parameter_schema = ?");
        params.push(param_schema);
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
        .context("Failed to update command template")
        .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(())
}

/// Delete a command template
#[instrument(skip(pool))]
pub async fn delete_command_template(pool: &Pool<Sqlite>, id: i64) -> Result<()> {
    // Check if template is in use by job templates or steps
    let count: (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*) as count
        FROM job_templates
        WHERE command_template_id = ?
        "#,
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .context("Failed to check if command template is in use")
    .map_err(|e| Error::DatabaseError(e.to_string()))?;

    let step_count: (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*) as count
        FROM job_template_steps
        WHERE command_template_id = ?
        "#,
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .context("Failed to check if command template is in use by steps")
    .map_err(|e| Error::DatabaseError(e.to_string()))?;

    if count.0 > 0 || step_count.0 > 0 {
        return Err(Error::DatabaseError(
            "Cannot delete command template: it is in use by one or more job templates or steps"
                .to_string(),
        ));
    }

    sqlx::query("DELETE FROM command_templates WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .context("Failed to delete command template")
        .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(())
}
