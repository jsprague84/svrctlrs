use sqlx::{Pool, Sqlite};
use svrctlrs_core::{Error, Result};

use crate::models::{CreateNotificationBackend, NotificationBackend, UpdateNotificationBackend};

/// List all notification backends
pub async fn list_notification_backends(pool: &Pool<Sqlite>) -> Result<Vec<NotificationBackend>> {
    sqlx::query_as::<_, NotificationBackend>(
        r#"
        SELECT id, type, name, enabled, config, priority, created_at, updated_at
        FROM notification_backends
        ORDER BY priority DESC, name
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| Error::DatabaseError(format!("Failed to list notification backends: {}", e)))
}

/// Get notification backend by ID
pub async fn get_notification_backend(pool: &Pool<Sqlite>, id: i64) -> Result<NotificationBackend> {
    sqlx::query_as::<_, NotificationBackend>(
        r#"
        SELECT id, type, name, enabled, config, priority, created_at, updated_at
        FROM notification_backends
        WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .map_err(|e| Error::DatabaseError(format!("Failed to get notification backend: {}", e)))
}

/// Create notification backend
pub async fn create_notification_backend(
    pool: &Pool<Sqlite>,
    backend: &CreateNotificationBackend,
) -> Result<i64> {
    let config_json = serde_json::to_string(&backend.config)
        .map_err(|e| Error::DatabaseError(format!("Failed to serialize config: {}", e)))?;

    let result = sqlx::query(
        r#"
        INSERT INTO notification_backends (type, name, config, priority)
        VALUES (?, ?, ?, ?)
        "#,
    )
    .bind(&backend.backend_type)
    .bind(&backend.name)
    .bind(config_json)
    .bind(backend.priority)
    .execute(pool)
    .await
    .map_err(|e| Error::DatabaseError(format!("Failed to create notification backend: {}", e)))?;

    Ok(result.last_insert_rowid())
}

/// Update notification backend
pub async fn update_notification_backend(
    pool: &Pool<Sqlite>,
    id: i64,
    update: &UpdateNotificationBackend,
) -> Result<()> {
    let mut query = String::from("UPDATE notification_backends SET updated_at = CURRENT_TIMESTAMP");
    let mut bindings: Vec<String> = Vec::new();

    if let Some(name) = &update.name {
        query.push_str(", name = ?");
        bindings.push(name.clone());
    }
    if let Some(enabled) = update.enabled {
        query.push_str(", enabled = ?");
        bindings.push(if enabled { "1" } else { "0" }.to_string());
    }
    if let Some(config) = &update.config {
        query.push_str(", config = ?");
        bindings.push(serde_json::to_string(config).unwrap_or_else(|_| "{}".to_string()));
    }
    if let Some(priority) = update.priority {
        query.push_str(", priority = ?");
        bindings.push(priority.to_string());
    }

    query.push_str(" WHERE id = ?");
    bindings.push(id.to_string());

    let mut q = sqlx::query(&query);
    for binding in bindings {
        q = q.bind(binding);
    }

    q.execute(pool).await.map_err(|e| {
        Error::DatabaseError(format!("Failed to update notification backend: {}", e))
    })?;

    Ok(())
}

/// Delete notification backend
pub async fn delete_notification_backend(pool: &Pool<Sqlite>, id: i64) -> Result<()> {
    sqlx::query("DELETE FROM notification_backends WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| {
            Error::DatabaseError(format!("Failed to delete notification backend: {}", e))
        })?;

    Ok(())
}

/// List enabled notification backends
pub async fn list_enabled_notification_backends(
    pool: &Pool<Sqlite>,
) -> Result<Vec<NotificationBackend>> {
    sqlx::query_as::<_, NotificationBackend>(
        r#"
        SELECT id, type, name, enabled, config, priority, created_at, updated_at
        FROM notification_backends
        WHERE enabled = 1
        ORDER BY priority DESC, name
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| {
        Error::DatabaseError(format!(
            "Failed to list enabled notification backends: {}",
            e
        ))
    })
}

/// List notification backends by type
pub async fn list_notification_backends_by_type(
    pool: &Pool<Sqlite>,
    backend_type: &str,
) -> Result<Vec<NotificationBackend>> {
    sqlx::query_as::<_, NotificationBackend>(
        r#"
        SELECT id, type, name, enabled, config, priority, created_at, updated_at
        FROM notification_backends
        WHERE type = ? AND enabled = 1
        ORDER BY priority DESC, name
        "#,
    )
    .bind(backend_type)
    .fetch_all(pool)
    .await
    .map_err(|e| {
        Error::DatabaseError(format!(
            "Failed to list notification backends by type: {}",
            e
        ))
    })
}
