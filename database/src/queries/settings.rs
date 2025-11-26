use sqlx::{Pool, Sqlite};
use svrctlrs_core::{Error, Result};

use crate::models::{Setting, UpdateSetting};

/// List all settings
pub async fn list_settings(pool: &Pool<Sqlite>) -> Result<Vec<Setting>> {
    sqlx::query_as::<_, Setting>(
        r#"
        SELECT key, value, type, description, updated_at
        FROM settings
        ORDER BY key
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| Error::DatabaseError(format!("Failed to list settings: {}", e)))
}

/// Get setting by key
pub async fn get_setting(pool: &Pool<Sqlite>, key: &str) -> Result<Setting> {
    sqlx::query_as::<_, Setting>(
        r#"
        SELECT key, value, type, description, updated_at
        FROM settings
        WHERE key = ?
        "#,
    )
    .bind(key)
    .fetch_one(pool)
    .await
    .map_err(|e| Error::DatabaseError(format!("Failed to get setting: {}", e)))
}

/// Get setting value as string
pub async fn get_setting_value(pool: &Pool<Sqlite>, key: &str) -> Result<String> {
    let setting = get_setting(pool, key).await?;
    Ok(setting.value)
}

/// Get setting value with default
pub async fn get_setting_value_or(pool: &Pool<Sqlite>, key: &str, default: &str) -> Result<String> {
    match get_setting_value(pool, key).await {
        Ok(value) => Ok(value),
        Err(_) => Ok(default.to_string()),
    }
}

/// Update setting
pub async fn update_setting(pool: &Pool<Sqlite>, key: &str, update: &UpdateSetting) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE settings 
        SET value = ?, updated_at = CURRENT_TIMESTAMP 
        WHERE key = ?
        "#,
    )
    .bind(&update.value)
    .bind(key)
    .execute(pool)
    .await
    .map_err(|e| Error::DatabaseError(format!("Failed to update setting: {}", e)))?;

    Ok(())
}

/// Set setting (insert or update)
pub async fn set_setting(pool: &Pool<Sqlite>, key: &str, value: &str) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO settings (key, value, type, updated_at)
        VALUES (?, ?, 'string', CURRENT_TIMESTAMP)
        ON CONFLICT(key) DO UPDATE SET 
            value = excluded.value,
            updated_at = CURRENT_TIMESTAMP
        "#,
    )
    .bind(key)
    .bind(value)
    .execute(pool)
    .await
    .map_err(|e| Error::DatabaseError(format!("Failed to set setting: {}", e)))?;

    Ok(())
}

/// Delete setting
pub async fn delete_setting(pool: &Pool<Sqlite>, key: &str) -> Result<()> {
    sqlx::query("DELETE FROM settings WHERE key = ?")
        .bind(key)
        .execute(pool)
        .await
        .map_err(|e| Error::DatabaseError(format!("Failed to delete setting: {}", e)))?;

    Ok(())
}
