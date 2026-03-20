//! Terminal profile queries

use anyhow::{Context, Result};
use sqlx::{Pool, QueryBuilder, Sqlite};

use crate::models::{CreateTerminalProfile, TerminalProfile, UpdateTerminalProfile};

/// List all terminal profiles
pub async fn list_terminal_profiles(pool: &Pool<Sqlite>) -> Result<Vec<TerminalProfile>> {
    let profiles = sqlx::query_as::<_, TerminalProfile>(
        r#"
        SELECT id, name, description, layout, pane_configs, quick_commands, is_default, created_at, updated_at
        FROM terminal_profiles
        ORDER BY is_default DESC, name ASC
        "#,
    )
    .fetch_all(pool)
    .await?;

    Ok(profiles)
}

/// Get a terminal profile by ID
pub async fn get_terminal_profile(pool: &Pool<Sqlite>, id: i64) -> Result<TerminalProfile> {
    let profile = sqlx::query_as::<_, TerminalProfile>(
        r#"
        SELECT id, name, description, layout, pane_configs, quick_commands, is_default, created_at, updated_at
        FROM terminal_profiles
        WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_one(pool)
    .await?;

    Ok(profile)
}

/// Get the default terminal profile
pub async fn get_default_terminal_profile(pool: &Pool<Sqlite>) -> Result<Option<TerminalProfile>> {
    let profile = sqlx::query_as::<_, TerminalProfile>(
        r#"
        SELECT id, name, description, layout, pane_configs, quick_commands, is_default, created_at, updated_at
        FROM terminal_profiles
        WHERE is_default = 1
        LIMIT 1
        "#,
    )
    .fetch_optional(pool)
    .await?;

    Ok(profile)
}

/// Create a new terminal profile
pub async fn create_terminal_profile(
    pool: &Pool<Sqlite>,
    profile: &CreateTerminalProfile,
) -> Result<i64> {
    // If this profile is set as default, unset any existing default
    if profile.is_default {
        sqlx::query("UPDATE terminal_profiles SET is_default = 0 WHERE is_default = 1")
            .execute(pool)
            .await?;
    }

    let pane_configs_json = profile
        .pane_configs
        .as_ref()
        .map(serde_json::to_string)
        .transpose()
        .context("Failed to serialize pane configs")?;
    let quick_commands_json = profile
        .quick_commands
        .as_ref()
        .map(serde_json::to_string)
        .transpose()
        .context("Failed to serialize quick commands")?;

    let result = sqlx::query(
        r#"
        INSERT INTO terminal_profiles (name, description, layout, pane_configs, quick_commands, is_default)
        VALUES (?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&profile.name)
    .bind(&profile.description)
    .bind(&profile.layout)
    .bind(&pane_configs_json)
    .bind(&quick_commands_json)
    .bind(profile.is_default)
    .execute(pool)
    .await?;

    Ok(result.last_insert_rowid())
}

/// Update an existing terminal profile
pub async fn update_terminal_profile(
    pool: &Pool<Sqlite>,
    id: i64,
    update: &UpdateTerminalProfile,
) -> Result<()> {
    // If this profile is being set as default, unset any existing default
    if update.is_default == Some(true) {
        sqlx::query("UPDATE terminal_profiles SET is_default = 0 WHERE is_default = 1 AND id != ?")
            .bind(id)
            .execute(pool)
            .await?;
    }

    let mut qb: QueryBuilder<Sqlite> =
        QueryBuilder::new("UPDATE terminal_profiles SET updated_at = CURRENT_TIMESTAMP");

    if let Some(name) = &update.name {
        qb.push(", name = ").push_bind(name.clone());
    }
    if let Some(description) = &update.description {
        qb.push(", description = ").push_bind(description.clone());
    }
    if let Some(layout) = &update.layout {
        qb.push(", layout = ").push_bind(layout.clone());
    }
    if let Some(pane_configs) = &update.pane_configs {
        let json = serde_json::to_string(pane_configs)
            .context("Failed to serialize pane configs")?;
        qb.push(", pane_configs = ").push_bind(json);
    }
    if let Some(quick_commands) = &update.quick_commands {
        let json = serde_json::to_string(quick_commands)
            .context("Failed to serialize quick commands")?;
        qb.push(", quick_commands = ").push_bind(json);
    }
    if let Some(is_default) = update.is_default {
        qb.push(", is_default = ").push_bind(is_default);
    }

    qb.push(" WHERE id = ").push_bind(id);

    qb.build().execute(pool).await?;

    Ok(())
}

/// Delete a terminal profile
pub async fn delete_terminal_profile(pool: &Pool<Sqlite>, id: i64) -> Result<()> {
    sqlx::query("DELETE FROM terminal_profiles WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;

    Ok(())
}
