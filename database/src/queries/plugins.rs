use sqlx::{Pool, Sqlite};
use svrctlrs_core::{Error, Result};

use crate::models::{Plugin, UpdatePlugin};

/// List all plugins
pub async fn list_plugins(pool: &Pool<Sqlite>) -> Result<Vec<Plugin>> {
    sqlx::query_as::<_, Plugin>(
        r#"
        SELECT id, name, description, enabled, config, created_at, updated_at
        FROM plugins
        ORDER BY name
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| Error::DatabaseError(format!("Failed to list plugins: {}", e)))
}

/// Get plugin by ID
pub async fn get_plugin(pool: &Pool<Sqlite>, id: &str) -> Result<Plugin> {
    sqlx::query_as::<_, Plugin>(
        r#"
        SELECT id, name, description, enabled, config, created_at, updated_at
        FROM plugins
        WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .map_err(|e| Error::DatabaseError(format!("Failed to get plugin: {}", e)))
}

/// Update plugin
pub async fn update_plugin(pool: &Pool<Sqlite>, id: &str, update: &UpdatePlugin) -> Result<()> {
    let mut query = String::from("UPDATE plugins SET updated_at = CURRENT_TIMESTAMP");
    let mut bindings: Vec<String> = Vec::new();

    if let Some(enabled) = update.enabled {
        query.push_str(", enabled = ?");
        bindings.push(if enabled { "1" } else { "0" }.to_string());
    }
    if let Some(config) = &update.config {
        query.push_str(", config = ?");
        bindings.push(serde_json::to_string(config).unwrap_or_else(|_| "{}".to_string()));
    }

    query.push_str(" WHERE id = ?");
    bindings.push(id.to_string());

    let mut q = sqlx::query(&query);
    for binding in bindings {
        q = q.bind(binding);
    }

    q.execute(pool)
        .await
        .map_err(|e| Error::DatabaseError(format!("Failed to update plugin: {}", e)))?;

    Ok(())
}

/// Toggle plugin enabled status
pub async fn toggle_plugin(pool: &Pool<Sqlite>, id: &str) -> Result<bool> {
    // Get current status
    let plugin = get_plugin(pool, id).await?;
    let new_status = !plugin.enabled;

    // Update status
    sqlx::query(
        r#"
        UPDATE plugins 
        SET enabled = ?, updated_at = CURRENT_TIMESTAMP 
        WHERE id = ?
        "#,
    )
    .bind(new_status)
    .bind(id)
    .execute(pool)
    .await
    .map_err(|e| Error::DatabaseError(format!("Failed to toggle plugin: {}", e)))?;

    Ok(new_status)
}

/// List enabled plugins
pub async fn list_enabled_plugins(pool: &Pool<Sqlite>) -> Result<Vec<Plugin>> {
    sqlx::query_as::<_, Plugin>(
        r#"
        SELECT id, name, description, enabled, config, created_at, updated_at
        FROM plugins
        WHERE enabled = 1
        ORDER BY name
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| Error::DatabaseError(format!("Failed to list enabled plugins: {}", e)))
}
