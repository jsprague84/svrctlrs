//! Server database queries (updated for new schema)

use anyhow::Context;
use sqlx::{Pool, Sqlite};
use svrctlrs_core::{Error, Result};
use tracing::instrument;

use crate::models::{CreateServer, Server, ServerCapability, UpdateServer};

/// List all servers
#[instrument(skip(pool))]
pub async fn list_servers(pool: &Pool<Sqlite>) -> Result<Vec<Server>> {
    sqlx::query_as::<_, Server>(
        r#"
        SELECT id, name, hostname, port, username, credential_id, description, is_local, enabled,
               os_type, os_distro, package_manager, docker_available, systemd_available, metadata,
               last_seen_at, last_error, created_at, updated_at
        FROM servers
        ORDER BY name
        "#,
    )
    .fetch_all(pool)
    .await
    .context("Failed to list servers")
    .map_err(|e| Error::DatabaseError(e.to_string()))
}

/// Get server by ID
#[instrument(skip(pool))]
pub async fn get_server(pool: &Pool<Sqlite>, id: i64) -> Result<Server> {
    sqlx::query_as::<_, Server>(
        r#"
        SELECT id, name, hostname, port, username, credential_id, description, is_local, enabled,
               os_type, os_distro, package_manager, docker_available, systemd_available, metadata,
               last_seen_at, last_error, created_at, updated_at
        FROM servers
        WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .context("Failed to get server")
    .map_err(|e| Error::DatabaseError(e.to_string()))
}

/// Get server by name
#[instrument(skip(pool))]
pub async fn get_server_by_name(pool: &Pool<Sqlite>, name: &str) -> Result<Server> {
    sqlx::query_as::<_, Server>(
        r#"
        SELECT id, name, hostname, port, username, credential_id, description, is_local, enabled,
               os_type, os_distro, package_manager, docker_available, systemd_available, metadata,
               last_seen_at, last_error, created_at, updated_at
        FROM servers
        WHERE name = ?
        "#,
    )
    .bind(name)
    .fetch_one(pool)
    .await
    .context("Failed to get server by name")
    .map_err(|e| Error::DatabaseError(e.to_string()))
}

/// Get all servers with a specific tag
#[instrument(skip(pool))]
pub async fn get_servers_by_tag(pool: &Pool<Sqlite>, tag_id: i64) -> Result<Vec<Server>> {
    sqlx::query_as::<_, Server>(
        r#"
        SELECT s.id, s.name, s.hostname, s.port, s.username, s.credential_id, s.description,
               s.is_local, s.enabled, s.os_type, s.os_distro, s.package_manager,
               s.docker_available, s.systemd_available, s.metadata, s.last_seen_at, s.last_error,
               s.created_at, s.updated_at
        FROM servers s
        JOIN server_tags st ON s.id = st.server_id
        WHERE st.tag_id = ?
        ORDER BY s.name
        "#,
    )
    .bind(tag_id)
    .fetch_all(pool)
    .await
    .context("Failed to get servers by tag")
    .map_err(|e| Error::DatabaseError(e.to_string()))
}

/// Create a new server with capability detection
#[instrument(skip(pool, input))]
pub async fn create_server(pool: &Pool<Sqlite>, input: &CreateServer) -> Result<i64> {
    // Validate input
    input
        .validate()
        .map_err(|e| Error::DatabaseError(format!("Validation error: {}", e)))?;

    let result = sqlx::query(
        r#"
        INSERT INTO servers (name, hostname, port, username, credential_id, description, is_local, enabled, metadata)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&input.name)
    .bind(&input.hostname)
    .bind(input.port)
    .bind(&input.username)
    .bind(input.credential_id)
    .bind(&input.description)
    .bind(input.is_local)
    .bind(input.enabled)
    .bind(input.metadata_string())
    .execute(pool)
    .await
    .context("Failed to create server")
    .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(result.last_insert_rowid())
}

/// Update an existing server
#[instrument(skip(pool, input))]
pub async fn update_server(pool: &Pool<Sqlite>, id: i64, input: &UpdateServer) -> Result<()> {
    if !input.has_changes() {
        return Ok(());
    }

    let mut query = String::from("UPDATE servers SET updated_at = CURRENT_TIMESTAMP");
    let mut params: Vec<String> = Vec::new();

    if let Some(name) = &input.name {
        query.push_str(", name = ?");
        params.push(name.clone());
    }
    if let Some(hostname) = &input.hostname {
        query.push_str(", hostname = ?");
        params.push(hostname.clone());
    }
    if let Some(port) = input.port {
        query.push_str(", port = ?");
        params.push(port.to_string());
    }
    if let Some(username) = &input.username {
        query.push_str(", username = ?");
        params.push(username.clone());
    }
    if input.credential_id.is_some() {
        query.push_str(", credential_id = ?");
        params.push(input.credential_id.map(|id| id.to_string()).unwrap_or_else(|| "NULL".to_string()));
    }
    if let Some(description) = &input.description {
        query.push_str(", description = ?");
        params.push(description.clone());
    }
    if let Some(enabled) = input.enabled {
        query.push_str(", enabled = ?");
        params.push(if enabled { "1" } else { "0" }.to_string());
    }
    if let Some(os_type) = &input.os_type {
        query.push_str(", os_type = ?");
        params.push(os_type.clone());
    }
    if let Some(os_distro) = &input.os_distro {
        query.push_str(", os_distro = ?");
        params.push(os_distro.clone());
    }
    if let Some(package_manager) = &input.package_manager {
        query.push_str(", package_manager = ?");
        params.push(package_manager.clone());
    }
    if let Some(docker_available) = input.docker_available {
        query.push_str(", docker_available = ?");
        params.push(if docker_available { "1" } else { "0" }.to_string());
    }
    if let Some(systemd_available) = input.systemd_available {
        query.push_str(", systemd_available = ?");
        params.push(if systemd_available { "1" } else { "0" }.to_string());
    }
    if let Some(metadata) = input.metadata_string() {
        query.push_str(", metadata = ?");
        params.push(metadata);
    }
    if let Some(last_error) = &input.last_error {
        query.push_str(", last_error = ?");
        params.push(last_error.clone());
    }

    query.push_str(" WHERE id = ?");

    let mut q = sqlx::query(&query);
    for param in params {
        q = q.bind(param);
    }
    q = q.bind(id);

    q.execute(pool)
        .await
        .context("Failed to update server")
        .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(())
}

/// Delete a server
#[instrument(skip(pool))]
pub async fn delete_server(pool: &Pool<Sqlite>, id: i64) -> Result<()> {
    sqlx::query("DELETE FROM servers WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .context("Failed to delete server")
        .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(())
}

/// Update server health status
#[instrument(skip(pool))]
pub async fn update_server_status(
    pool: &Pool<Sqlite>,
    id: i64,
    last_error: Option<String>,
) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE servers
        SET last_seen_at = CURRENT_TIMESTAMP,
            last_error = ?
        WHERE id = ?
        "#,
    )
    .bind(last_error)
    .bind(id)
    .execute(pool)
    .await
    .context("Failed to update server status")
    .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(())
}

/// List enabled servers only
#[instrument(skip(pool))]
pub async fn list_enabled_servers(pool: &Pool<Sqlite>) -> Result<Vec<Server>> {
    sqlx::query_as::<_, Server>(
        r#"
        SELECT id, name, hostname, port, username, credential_id, description, is_local, enabled,
               os_type, os_distro, package_manager, docker_available, systemd_available, metadata,
               last_seen_at, last_error, created_at, updated_at
        FROM servers
        WHERE enabled = 1
        ORDER BY name
        "#,
    )
    .fetch_all(pool)
    .await
    .context("Failed to list enabled servers")
    .map_err(|e| Error::DatabaseError(e.to_string()))
}

// ============================================================================
// Capability Queries
// ============================================================================

/// Get all capabilities for a server
#[instrument(skip(pool))]
pub async fn get_server_capabilities(pool: &Pool<Sqlite>, server_id: i64) -> Result<Vec<ServerCapability>> {
    sqlx::query_as::<_, ServerCapability>(
        r#"
        SELECT id, server_id, capability, available, version, detected_at
        FROM server_capabilities
        WHERE server_id = ?
        ORDER BY capability
        "#,
    )
    .bind(server_id)
    .fetch_all(pool)
    .await
    .context("Failed to get server capabilities")
    .map_err(|e| Error::DatabaseError(e.to_string()))
}

/// Set or update a server capability
#[instrument(skip(pool))]
pub async fn set_server_capability(
    pool: &Pool<Sqlite>,
    server_id: i64,
    name: &str,
    available: bool,
    version: Option<&str>,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO server_capabilities (server_id, capability, available, version, detected_at)
        VALUES (?, ?, ?, ?, CURRENT_TIMESTAMP)
        ON CONFLICT(server_id, capability)
        DO UPDATE SET available = ?, version = ?, detected_at = CURRENT_TIMESTAMP
        "#,
    )
    .bind(server_id)
    .bind(name)
    .bind(available)
    .bind(version)
    .bind(available)
    .bind(version)
    .execute(pool)
    .await
    .context("Failed to set server capability")
    .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(())
}

/// Check if a server has a specific capability
#[instrument(skip(pool))]
pub async fn has_capability(pool: &Pool<Sqlite>, server_id: i64, name: &str) -> Result<bool> {
    let result: Option<(bool,)> = sqlx::query_as(
        r#"
        SELECT available
        FROM server_capabilities
        WHERE server_id = ? AND capability = ?
        "#,
    )
    .bind(server_id)
    .bind(name)
    .fetch_optional(pool)
    .await
    .context("Failed to check server capability")
    .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(result.map(|(available,)| available).unwrap_or(false))
}

/// Find all servers that have a specific capability
#[instrument(skip(pool))]
pub async fn get_servers_with_capability(pool: &Pool<Sqlite>, name: &str) -> Result<Vec<Server>> {
    sqlx::query_as::<_, Server>(
        r#"
        SELECT s.id, s.name, s.hostname, s.port, s.username, s.credential_id, s.description,
               s.is_local, s.enabled, s.os_type, s.os_distro, s.package_manager,
               s.docker_available, s.systemd_available, s.metadata, s.last_seen_at, s.last_error,
               s.created_at, s.updated_at
        FROM servers s
        JOIN server_capabilities sc ON s.id = sc.server_id
        WHERE sc.capability = ? AND sc.available = 1
        ORDER BY s.name
        "#,
    )
    .bind(name)
    .fetch_all(pool)
    .await
    .context("Failed to get servers with capability")
    .map_err(|e| Error::DatabaseError(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup_test_db() -> Pool<Sqlite> {
        let pool = SqlitePool::connect(":memory:").await.unwrap();
        sqlx::migrate!("../migrations").run(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn test_server_lifecycle() {
        let pool = setup_test_db().await;

        // Create server
        let input = CreateServer {
            name: "test-server".to_string(),
            hostname: Some("192.168.1.100".to_string()),
            port: 22,
            username: Some("admin".to_string()),
            credential_id: None,
            description: Some("Test server".to_string()),
            is_local: false,
            enabled: true,
            metadata: None,
        };

        let id = create_server(&pool, &input).await.unwrap();
        assert!(id > 0);

        // Get server
        let server = get_server(&pool, id).await.unwrap();
        assert_eq!(server.name, "test-server");
        assert_eq!(server.hostname, Some("192.168.1.100".to_string()));

        // Update server
        let update = UpdateServer {
            description: Some("Updated description".to_string()),
            os_type: Some("linux".to_string()),
            os_distro: Some("ubuntu".to_string()),
            package_manager: Some("apt".to_string()),
            docker_available: Some(true),
            ..Default::default()
        };
        update_server(&pool, id, &update).await.unwrap();

        let server = get_server(&pool, id).await.unwrap();
        assert_eq!(server.description, Some("Updated description".to_string()));
        assert_eq!(server.os_type, Some("linux".to_string()));
        assert!(server.docker_available);

        // Set capabilities
        set_server_capability(&pool, id, "docker", true, Some("20.10.0"))
            .await
            .unwrap();
        set_server_capability(&pool, id, "systemd", true, None)
            .await
            .unwrap();

        // Check capabilities
        assert!(has_capability(&pool, id, "docker").await.unwrap());
        assert!(has_capability(&pool, id, "systemd").await.unwrap());
        assert!(!has_capability(&pool, id, "nonexistent").await.unwrap());

        let caps = get_server_capabilities(&pool, id).await.unwrap();
        assert_eq!(caps.len(), 2);

        // Get servers with capability
        let servers = get_servers_with_capability(&pool, "docker")
            .await
            .unwrap();
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].id, id);

        // Delete server
        delete_server(&pool, id).await.unwrap();

        // Verify deleted
        assert!(get_server(&pool, id).await.is_err());
    }
}
