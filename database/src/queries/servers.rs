//! Server database queries (updated for new schema)

use anyhow::Context;
use sqlx::{Pool, QueryBuilder, Sqlite};
use svrctlrs_core::{Error, Result};
use tracing::instrument;

use crate::models::{CreateServer, Server, UpdateServer};

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

    let mut qb: QueryBuilder<Sqlite> =
        QueryBuilder::new("UPDATE servers SET updated_at = CURRENT_TIMESTAMP");

    if let Some(name) = &input.name {
        qb.push(", name = ").push_bind(name.clone());
    }
    if let Some(hostname) = &input.hostname {
        qb.push(", hostname = ").push_bind(hostname.clone());
    }
    if let Some(port) = input.port {
        qb.push(", port = ").push_bind(port);
    }
    if let Some(username) = &input.username {
        qb.push(", username = ").push_bind(username.clone());
    }
    if let Some(credential_id) = input.credential_id {
        qb.push(", credential_id = ").push_bind(credential_id);
    }
    if let Some(description) = &input.description {
        qb.push(", description = ").push_bind(description.clone());
    }
    if let Some(enabled) = input.enabled {
        qb.push(", enabled = ").push_bind(enabled);
    }
    if let Some(os_type) = &input.os_type {
        qb.push(", os_type = ").push_bind(os_type.clone());
    }
    if let Some(os_distro) = &input.os_distro {
        qb.push(", os_distro = ").push_bind(os_distro.clone());
    }
    if let Some(package_manager) = &input.package_manager {
        qb.push(", package_manager = ")
            .push_bind(package_manager.clone());
    }
    if let Some(docker_available) = input.docker_available {
        qb.push(", docker_available = ").push_bind(docker_available);
    }
    if let Some(systemd_available) = input.systemd_available {
        qb.push(", systemd_available = ")
            .push_bind(systemd_available);
    }
    if let Some(metadata) = input.metadata_string() {
        qb.push(", metadata = ").push_bind(metadata);
    }
    if let Some(last_error) = &input.last_error {
        qb.push(", last_error = ").push_bind(last_error.clone());
    }

    qb.push(" WHERE id = ").push_bind(id);

    qb.build()
        .execute(pool)
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

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::SqlitePool;

    async fn setup_test_db() -> Pool<Sqlite> {
        let pool = SqlitePool::connect(":memory:").await.unwrap();
        sqlx::migrate!().run(&pool).await.unwrap();
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

        // Delete server
        delete_server(&pool, id).await.unwrap();

        // Verify deleted
        assert!(get_server(&pool, id).await.is_err());
    }
}

// ============================================================================
// Optimized Queries with Joined Data
// ============================================================================

/// Extended server with credential name for display
#[derive(Debug, Clone)]
pub struct ServerWithDetails {
    pub server: Server,
    pub credential_name: Option<String>,
}

/// List all servers with credential names, tags, and capabilities (optimized for display)
#[instrument(skip(pool))]
pub async fn list_servers_with_details(pool: &Pool<Sqlite>) -> Result<Vec<ServerWithDetails>> {
    let servers = list_servers(pool).await?;
    let mut result = Vec::new();

    for server in servers {
        // Get credential name if set
        let credential_name = if let Some(cred_id) = server.credential_id {
            let cred: Option<(String,)> = sqlx::query_as(
                r#"
                SELECT name
                FROM credentials
                WHERE id = ?
                "#,
            )
            .bind(cred_id)
            .fetch_optional(pool)
            .await
            .context("Failed to get credential name")
            .map_err(|e| Error::DatabaseError(e.to_string()))?;
            cred.map(|(name,)| name)
        } else {
            None
        };

        result.push(ServerWithDetails {
            server,
            credential_name,
        });
    }

    Ok(result)
}

/// Get a single server by ID with credential name, tags, and capabilities
#[instrument(skip(pool))]
pub async fn get_server_with_details(pool: &Pool<Sqlite>, id: i64) -> Result<ServerWithDetails> {
    let server = get_server(pool, id).await?;

    // Get credential name if set
    let credential_name = if let Some(cred_id) = server.credential_id {
        let cred: Option<(String,)> = sqlx::query_as(
            r#"
            SELECT name
            FROM credentials
            WHERE id = ?
            "#,
        )
        .bind(cred_id)
        .fetch_optional(pool)
        .await
        .context("Failed to get credential name")
        .map_err(|e| Error::DatabaseError(e.to_string()))?;
        cred.map(|(name,)| name)
    } else {
        None
    };

    Ok(ServerWithDetails {
        server,
        credential_name,
    })
}
