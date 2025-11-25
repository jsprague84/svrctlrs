use sqlx::{Pool, Sqlite};
use svrctlrs_core::{Error, Result};

use crate::models::{CreateServer, Server, UpdateServer};

/// List all servers
pub async fn list_servers(pool: &Pool<Sqlite>) -> Result<Vec<Server>> {
    sqlx::query_as::<_, Server>(
        r#"
        SELECT id, name, host, port, username, ssh_key_path, enabled, description, tags,
               created_at, updated_at, last_seen_at, os_type, os_version, docker_installed,
               connection_timeout, retry_attempts
        FROM servers
        ORDER BY name
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| Error::DatabaseError(format!("Failed to list servers: {}", e)))
}

/// Get server by ID
pub async fn get_server(pool: &Pool<Sqlite>, id: i64) -> Result<Server> {
    sqlx::query_as::<_, Server>(
        r#"
        SELECT id, name, host, port, username, ssh_key_path, enabled, description, tags,
               created_at, updated_at, last_seen_at, os_type, os_version, docker_installed,
               connection_timeout, retry_attempts
        FROM servers
        WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .map_err(|e| Error::DatabaseError(format!("Failed to get server: {}", e)))
}

/// Get server by name
pub async fn get_server_by_name(pool: &Pool<Sqlite>, name: &str) -> Result<Server> {
    sqlx::query_as::<_, Server>(
        r#"
        SELECT id, name, host, port, username, ssh_key_path, enabled, description, tags,
               created_at, updated_at, last_seen_at, os_type, os_version, docker_installed,
               connection_timeout, retry_attempts
        FROM servers
        WHERE name = ?
        "#,
    )
    .bind(name)
    .fetch_one(pool)
    .await
    .map_err(|e| Error::DatabaseError(format!("Failed to get server by name: {}", e)))
}

/// Create a new server
pub async fn create_server(pool: &Pool<Sqlite>, server: &CreateServer) -> Result<i64> {
    let tags_json = server
        .tags
        .as_ref()
        .map(|t| serde_json::to_string(t).unwrap_or_else(|_| "[]".to_string()));

    let result = sqlx::query(
        r#"
        INSERT INTO servers (name, host, port, username, ssh_key_path, description, tags)
        VALUES (?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&server.name)
    .bind(&server.host)
    .bind(server.port)
    .bind(&server.username)
    .bind(&server.ssh_key_path)
    .bind(&server.description)
    .bind(tags_json)
    .execute(pool)
    .await
    .map_err(|e| Error::DatabaseError(format!("Failed to create server: {}", e)))?;

    Ok(result.last_insert_rowid())
}

/// Update a server
pub async fn update_server(
    pool: &Pool<Sqlite>,
    id: i64,
    update: &UpdateServer,
) -> Result<()> {
    let mut query = String::from("UPDATE servers SET updated_at = CURRENT_TIMESTAMP");
    let mut bindings: Vec<String> = Vec::new();

    if let Some(name) = &update.name {
        query.push_str(", name = ?");
        bindings.push(name.clone());
    }
    if let Some(host) = &update.host {
        query.push_str(", host = ?");
        bindings.push(host.clone());
    }
    if let Some(port) = update.port {
        query.push_str(", port = ?");
        bindings.push(port.to_string());
    }
    if let Some(username) = &update.username {
        query.push_str(", username = ?");
        bindings.push(username.clone());
    }
    if let Some(ssh_key_path) = &update.ssh_key_path {
        query.push_str(", ssh_key_path = ?");
        bindings.push(ssh_key_path.clone());
    }
    if let Some(enabled) = update.enabled {
        query.push_str(", enabled = ?");
        bindings.push(if enabled { "1" } else { "0" }.to_string());
    }
    if let Some(description) = &update.description {
        query.push_str(", description = ?");
        bindings.push(description.clone());
    }
    if let Some(tags) = &update.tags {
        query.push_str(", tags = ?");
        bindings.push(serde_json::to_string(tags).unwrap_or_else(|_| "[]".to_string()));
    }
    if let Some(timeout) = update.connection_timeout {
        query.push_str(", connection_timeout = ?");
        bindings.push(timeout.to_string());
    }
    if let Some(retry) = update.retry_attempts {
        query.push_str(", retry_attempts = ?");
        bindings.push(retry.to_string());
    }

    query.push_str(" WHERE id = ?");
    bindings.push(id.to_string());

    let mut q = sqlx::query(&query);
    for binding in bindings {
        q = q.bind(binding);
    }

    q.execute(pool)
        .await
        .map_err(|e| Error::DatabaseError(format!("Failed to update server: {}", e)))?;

    Ok(())
}

/// Delete a server
pub async fn delete_server(pool: &Pool<Sqlite>, id: i64) -> Result<()> {
    sqlx::query("DELETE FROM servers WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| Error::DatabaseError(format!("Failed to delete server: {}", e)))?;

    Ok(())
}

/// Update server last seen timestamp
pub async fn update_server_last_seen(pool: &Pool<Sqlite>, id: i64) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE servers 
        SET last_seen_at = CURRENT_TIMESTAMP 
        WHERE id = ?
        "#,
    )
    .bind(id)
    .execute(pool)
    .await
    .map_err(|e| Error::DatabaseError(format!("Failed to update last seen: {}", e)))?;

    Ok(())
}

/// Update server OS information
pub async fn update_server_os_info(
    pool: &Pool<Sqlite>,
    id: i64,
    os_type: &str,
    os_version: &str,
    docker_installed: bool,
) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE servers 
        SET os_type = ?, os_version = ?, docker_installed = ?
        WHERE id = ?
        "#,
    )
    .bind(os_type)
    .bind(os_version)
    .bind(docker_installed)
    .bind(id)
    .execute(pool)
    .await
    .map_err(|e| Error::DatabaseError(format!("Failed to update OS info: {}", e)))?;

    Ok(())
}

/// List enabled servers
pub async fn list_enabled_servers(pool: &Pool<Sqlite>) -> Result<Vec<Server>> {
    sqlx::query_as::<_, Server>(
        r#"
        SELECT id, name, host, port, username, ssh_key_path, enabled, description, tags,
               created_at, updated_at, last_seen_at, os_type, os_version, docker_installed,
               connection_timeout, retry_attempts
        FROM servers
        WHERE enabled = 1
        ORDER BY name
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| Error::DatabaseError(format!("Failed to list enabled servers: {}", e)))
}

