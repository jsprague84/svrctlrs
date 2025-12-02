//! Terminal profile queries

use anyhow::Result;
use sqlx::{Pool, Sqlite};

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
        .map(|v| serde_json::to_string(v).unwrap_or_default());
    let quick_commands_json = profile
        .quick_commands
        .as_ref()
        .map(|v| serde_json::to_string(v).unwrap_or_default());

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

    let mut query = String::from("UPDATE terminal_profiles SET updated_at = CURRENT_TIMESTAMP");
    let mut params: Vec<String> = Vec::new();

    if let Some(name) = &update.name {
        query.push_str(", name = ?");
        params.push(name.clone());
    }
    if let Some(description) = &update.description {
        query.push_str(", description = ?");
        params.push(description.clone());
    }
    if let Some(layout) = &update.layout {
        query.push_str(", layout = ?");
        params.push(layout.clone());
    }
    if let Some(pane_configs) = &update.pane_configs {
        query.push_str(", pane_configs = ?");
        params.push(serde_json::to_string(pane_configs).unwrap_or_default());
    }
    if let Some(quick_commands) = &update.quick_commands {
        query.push_str(", quick_commands = ?");
        params.push(serde_json::to_string(quick_commands).unwrap_or_default());
    }
    if let Some(is_default) = update.is_default {
        query.push_str(", is_default = ?");
        params.push(if is_default { "1" } else { "0" }.to_string());
    }

    query.push_str(" WHERE id = ?");

    // Build the query dynamically
    let mut qb = sqlx::query(&query);
    for param in &params {
        qb = qb.bind(param);
    }
    qb = qb.bind(id);
    qb.execute(pool).await?;

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
