//! Server database queries (updated for new schema)

use anyhow::Context;
use sqlx::{Pool, QueryBuilder, Sqlite};
use svrctlrs_core::{Error, Result};
use tracing::instrument;

use crate::models::{normalize_tags, CreateServer, Server, UpdateServer};

/// List all servers
#[instrument(skip(pool))]
pub async fn list_servers(pool: &Pool<Sqlite>) -> Result<Vec<Server>> {
    sqlx::query_as::<_, Server>(
        r#"
        SELECT id, name, hostname, port, username, credential_id, via_server_id, tags,
               startup_command, record_sessions, description, enabled, last_seen_at,
               last_error, created_at, updated_at
        FROM servers
        ORDER BY name
        "#,
    )
    .fetch_all(pool)
    .await
    .context("Failed to list servers")
    .map_err(Error::Database)
}

/// Get server by ID
#[instrument(skip(pool))]
pub async fn get_server(pool: &Pool<Sqlite>, id: i64) -> Result<Server> {
    sqlx::query_as::<_, Server>(
        r#"
        SELECT id, name, hostname, port, username, credential_id, via_server_id, tags,
               startup_command, record_sessions, description, enabled, last_seen_at,
               last_error, created_at, updated_at
        FROM servers
        WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .context("Failed to get server")
    .map_err(Error::Database)?
    .ok_or_else(|| Error::NotFound(format!("server {id}")))
}

/// Get server by name
#[instrument(skip(pool))]
pub async fn get_server_by_name(pool: &Pool<Sqlite>, name: &str) -> Result<Server> {
    sqlx::query_as::<_, Server>(
        r#"
        SELECT id, name, hostname, port, username, credential_id, via_server_id, tags,
               startup_command, record_sessions, description, enabled, last_seen_at,
               last_error, created_at, updated_at
        FROM servers
        WHERE name = ?
        "#,
    )
    .bind(name)
    .fetch_optional(pool)
    .await
    .context("Failed to get server by name")
    .map_err(Error::Database)?
    .ok_or_else(|| Error::NotFound(format!("server {name:?}")))
}

/// Create a new server
#[instrument(skip(pool, input))]
pub async fn create_server(pool: &Pool<Sqlite>, input: &CreateServer) -> Result<i64> {
    // Validate input
    input.validate().map_err(Error::InvalidInput)?;

    let result = sqlx::query(
        r#"
        INSERT INTO servers (name, hostname, port, username, credential_id, via_server_id,
                             tags, startup_command, record_sessions, description, enabled)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&input.name)
    .bind(&input.hostname)
    .bind(input.port)
    .bind(&input.username)
    .bind(input.credential_id)
    .bind(input.via_server_id)
    .bind(sqlx::types::Json(normalize_tags(&input.tags)))
    .bind(
        input
            .startup_command
            .as_deref()
            .map(str::trim)
            .filter(|c| !c.is_empty()),
    )
    .bind(input.record_sessions)
    .bind(&input.description)
    .bind(input.enabled)
    .execute(pool)
    .await
    .map_err(super::write_error("server", "create"))?;

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
    // Nullable text columns: an empty string from the form means "clear"
    if let Some(hostname) = &input.hostname {
        qb.push(", hostname = NULLIF(")
            .push_bind(hostname.clone())
            .push(", '')");
    }
    if let Some(port) = input.port {
        qb.push(", port = ").push_bind(port);
    }
    if let Some(username) = &input.username {
        qb.push(", username = NULLIF(")
            .push_bind(username.clone())
            .push(", '')");
    }
    if let Some(credential_id) = input.credential_id {
        // `Some(None)` binds NULL and unassigns the credential
        qb.push(", credential_id = ").push_bind(credential_id);
    }
    if let Some(via_server_id) = input.via_server_id {
        qb.push(", via_server_id = ").push_bind(via_server_id);
    }
    if let Some(tags) = &input.tags {
        qb.push(", tags = ")
            .push_bind(sqlx::types::Json(normalize_tags(tags)));
    }
    if let Some(startup_command) = &input.startup_command {
        qb.push(", startup_command = NULLIF(")
            .push_bind(startup_command.trim().to_string())
            .push(", '')");
    }
    if let Some(record) = input.record_sessions {
        qb.push(", record_sessions = ").push_bind(record);
    }
    if let Some(description) = &input.description {
        qb.push(", description = NULLIF(")
            .push_bind(description.clone())
            .push(", '')");
    }
    if let Some(enabled) = input.enabled {
        qb.push(", enabled = ").push_bind(enabled);
    }
    if let Some(last_error) = &input.last_error {
        qb.push(", last_error = ").push_bind(last_error.clone());
    }

    qb.push(" WHERE id = ").push_bind(id);

    qb.build()
        .execute(pool)
        .await
        .map_err(super::write_error("server", "update"))?;

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
        .map_err(Error::Database)?;

    Ok(())
}

/// Record why a server could not be reached, leaving `last_seen_at` as the last success.
#[instrument(skip(pool))]
pub async fn update_server_last_error(pool: &Pool<Sqlite>, id: i64, error: &str) -> Result<()> {
    sqlx::query("UPDATE servers SET last_error = ? WHERE id = ?")
        .bind(error)
        .bind(id)
        .execute(pool)
        .await
        .context("Failed to update server error")
        .map_err(Error::Database)?;
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
    .map_err(Error::Database)?;

    Ok(())
}

/// List enabled servers only
#[instrument(skip(pool))]
pub async fn list_enabled_servers(pool: &Pool<Sqlite>) -> Result<Vec<Server>> {
    sqlx::query_as::<_, Server>(
        r#"
        SELECT id, name, hostname, port, username, credential_id, via_server_id, tags,
               startup_command, record_sessions, description, enabled, last_seen_at,
               last_error, created_at, updated_at
        FROM servers
        WHERE enabled = 1
        ORDER BY name
        "#,
    )
    .fetch_all(pool)
    .await
    .context("Failed to list enabled servers")
    .map_err(Error::Database)
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

    fn host(name: &str) -> CreateServer {
        CreateServer {
            name: name.to_string(),
            hostname: "10.0.0.9".to_string(),
            port: 22,
            username: Some("root".to_string()),
            credential_id: None,
            via_server_id: None,
            tags: vec![],
            startup_command: None,
            record_sessions: false,
            description: None,
            enabled: true,
        }
    }

    #[tokio::test]
    async fn duplicate_name_is_a_conflict_not_a_database_error() {
        let pool = setup_test_db().await;
        create_server(&pool, &host("web")).await.unwrap();
        let err = create_server(&pool, &host("web")).await.unwrap_err();
        assert!(matches!(err, Error::Conflict(_)), "{err}");
        assert_eq!(err.to_string(), "A server with that name already exists");

        let other = create_server(&pool, &host("db")).await.unwrap();
        let rename = UpdateServer {
            name: Some("web".to_string()),
            ..Default::default()
        };
        let err = update_server(&pool, other, &rename).await.unwrap_err();
        assert!(matches!(err, Error::Conflict(_)), "{err}");
    }

    #[tokio::test]
    async fn test_server_lifecycle() {
        let pool = setup_test_db().await;

        // Create server
        let input = CreateServer {
            name: "test-server".to_string(),
            hostname: "192.168.1.100".to_string(),
            port: 22,
            username: Some("admin".to_string()),
            credential_id: None,
            via_server_id: None,
            tags: vec!["Prod".into(), "prod".into(), " web ".into()],
            startup_command: Some(" tmux new -A -s main ".into()),
            record_sessions: true,
            description: Some("Test server".to_string()),
            enabled: true,
        };

        let id = create_server(&pool, &input).await.unwrap();
        assert!(id > 0);

        // Get server
        let server = get_server(&pool, id).await.unwrap();
        assert_eq!(server.name, "test-server");
        assert_eq!(server.hostname, "192.168.1.100");
        assert_eq!(server.tags.0, ["Prod", "web"]);
        assert_eq!(
            server.startup_command.as_deref(),
            Some("tmux new -A -s main")
        );
        assert!(server.record_sessions);

        // Update server
        let update = UpdateServer {
            description: Some("Updated description".to_string()),
            port: Some(2222),
            ..Default::default()
        };
        update_server(&pool, id, &update).await.unwrap();

        let server = get_server(&pool, id).await.unwrap();
        assert_eq!(server.description, Some("Updated description".to_string()));
        assert_eq!(server.port, 2222);
        assert_eq!(server.tags.0, ["Prod", "web"], "tags kept when absent");
        update_server(
            &pool,
            id,
            &UpdateServer {
                tags: Some(vec![]),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert!(get_server(&pool, id).await.unwrap().tags.0.is_empty());
        update_server(
            &pool,
            id,
            &UpdateServer {
                startup_command: Some("  ".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert_eq!(get_server(&pool, id).await.unwrap().startup_command, None);
        assert!(
            get_server(&pool, id).await.unwrap().record_sessions,
            "kept when absent"
        );
        update_server(
            &pool,
            id,
            &UpdateServer {
                record_sessions: Some(false),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert!(!get_server(&pool, id).await.unwrap().record_sessions);

        // Delete server
        delete_server(&pool, id).await.unwrap();

        // Verify deleted
        assert!(get_server(&pool, id).await.is_err());
    }

    #[tokio::test]
    async fn deleting_a_jump_host_makes_its_dependants_direct() {
        let pool = setup_test_db().await;
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&pool)
            .await
            .unwrap();
        let jump = create_server(&pool, &host("bastion")).await.unwrap();
        let inner = create_server(
            &pool,
            &CreateServer {
                via_server_id: Some(jump),
                ..host("inner")
            },
        )
        .await
        .unwrap();
        assert_eq!(
            get_server(&pool, inner).await.unwrap().via_server_id,
            Some(jump)
        );
        // `Some(None)` clears it; `None` keeps it
        update_server(
            &pool,
            inner,
            &UpdateServer {
                via_server_id: Some(None),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert_eq!(get_server(&pool, inner).await.unwrap().via_server_id, None);
        update_server(
            &pool,
            inner,
            &UpdateServer {
                via_server_id: Some(Some(jump)),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        delete_server(&pool, jump).await.unwrap();
        assert_eq!(get_server(&pool, inner).await.unwrap().via_server_id, None);
    }

    #[tokio::test]
    async fn missing_server_is_not_found_not_a_database_error() {
        let pool = setup_test_db().await;
        assert!(matches!(
            get_server(&pool, 4242).await.unwrap_err(),
            Error::NotFound(ref what) if what == "server 4242"
        ));
        assert!(matches!(
            get_server_by_name(&pool, "nope").await.unwrap_err(),
            Error::NotFound(_)
        ));
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

/// List all servers with their credential names (for display)
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
            .map_err(Error::Database)?;
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

/// Get a single server by ID with its credential name
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
        .map_err(Error::Database)?;
        cred.map(|(name,)| name)
    } else {
        None
    };

    Ok(ServerWithDetails {
        server,
        credential_name,
    })
}
