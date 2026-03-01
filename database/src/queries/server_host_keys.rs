//! SSH host key database queries for TOFU (Trust On First Use) verification

use anyhow::Context;
use sqlx::{Pool, Sqlite};
use svrctlrs_core::{Error, Result};
use tracing::instrument;

use crate::models::ServerHostKey;

/// Get the stored host key for a server and key type
#[instrument(skip(pool))]
pub async fn get_host_key_by_server(
    pool: &Pool<Sqlite>,
    server_id: i64,
    key_type: &str,
) -> Result<Option<ServerHostKey>> {
    sqlx::query_as::<_, ServerHostKey>(
        r#"
        SELECT id, server_id, key_type, public_key, first_seen_at, last_seen_at
        FROM server_host_keys
        WHERE server_id = ? AND key_type = ?
        "#,
    )
    .bind(server_id)
    .bind(key_type)
    .fetch_optional(pool)
    .await
    .context("Failed to get host key by server")
    .map_err(|e| Error::DatabaseError(e.to_string()))
}

/// Store a new host key for a server (TOFU - first connection)
#[instrument(skip(pool))]
pub async fn store_host_key(
    pool: &Pool<Sqlite>,
    server_id: i64,
    key_type: &str,
    public_key: &str,
) -> Result<i64> {
    let result = sqlx::query(
        r#"
        INSERT INTO server_host_keys (server_id, key_type, public_key)
        VALUES (?, ?, ?)
        "#,
    )
    .bind(server_id)
    .bind(key_type)
    .bind(public_key)
    .execute(pool)
    .await
    .context("Failed to store host key")
    .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(result.last_insert_rowid())
}

/// Update the last_seen_at timestamp for a server's host key
#[instrument(skip(pool))]
pub async fn update_host_key_last_seen(
    pool: &Pool<Sqlite>,
    server_id: i64,
    key_type: &str,
) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE server_host_keys
        SET last_seen_at = datetime('now')
        WHERE server_id = ? AND key_type = ?
        "#,
    )
    .bind(server_id)
    .bind(key_type)
    .execute(pool)
    .await
    .context("Failed to update host key last_seen_at")
    .map_err(|e| Error::DatabaseError(e.to_string()))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::SqlitePool;

    async fn setup_test_db() -> Pool<Sqlite> {
        let pool = SqlitePool::connect(":memory:").await.unwrap();
        sqlx::migrate!().run(&pool).await.unwrap();

        // Create a test server (non-local requires hostname + username per CHECK constraint)
        sqlx::query(
            "INSERT INTO servers (name, hostname, port, username, is_local, enabled) VALUES ('test-server', 'test.example.com', 22, 'root', 0, 1)"
        )
        .execute(&pool)
        .await
        .unwrap();

        pool
    }

    #[tokio::test]
    async fn test_store_and_get_host_key() {
        let pool = setup_test_db().await;

        let key_type = "ssh-ed25519";
        let public_key = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIFakeKeyDataHere";

        let id = store_host_key(&pool, 1, key_type, public_key).await.unwrap();
        assert!(id > 0);

        let stored = get_host_key_by_server(&pool, 1, key_type).await.unwrap();
        assert!(stored.is_some());
        let stored = stored.unwrap();
        assert_eq!(stored.server_id, 1);
        assert_eq!(stored.key_type, key_type);
        assert_eq!(stored.public_key, public_key);
    }

    #[tokio::test]
    async fn test_get_nonexistent_host_key() {
        let pool = setup_test_db().await;

        let result = get_host_key_by_server(&pool, 1, "ssh-ed25519").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_update_last_seen() {
        let pool = setup_test_db().await;

        let key_type = "ssh-ed25519";
        store_host_key(&pool, 1, key_type, "ssh-ed25519 AAAAC3fakekeydata")
            .await
            .unwrap();

        let before = get_host_key_by_server(&pool, 1, key_type).await.unwrap().unwrap();

        // Update last_seen
        update_host_key_last_seen(&pool, 1, key_type).await.unwrap();

        let after = get_host_key_by_server(&pool, 1, key_type).await.unwrap().unwrap();
        assert!(after.last_seen_at >= before.last_seen_at);
    }

    #[tokio::test]
    async fn test_multiple_key_types_per_server() {
        let pool = setup_test_db().await;

        store_host_key(&pool, 1, "ssh-ed25519", "ssh-ed25519 AAAAC3key1")
            .await
            .unwrap();

        // Different key type for same server should succeed
        store_host_key(&pool, 1, "ssh-rsa", "ssh-rsa AAAAB3key2")
            .await
            .unwrap();

        // Verify both keys stored independently
        let ed_key = get_host_key_by_server(&pool, 1, "ssh-ed25519").await.unwrap();
        assert!(ed_key.is_some());
        assert_eq!(ed_key.unwrap().public_key, "ssh-ed25519 AAAAC3key1");

        let rsa_key = get_host_key_by_server(&pool, 1, "ssh-rsa").await.unwrap();
        assert!(rsa_key.is_some());
        assert_eq!(rsa_key.unwrap().public_key, "ssh-rsa AAAAB3key2");
    }

    #[tokio::test]
    async fn test_duplicate_server_and_key_type_fails() {
        let pool = setup_test_db().await;

        store_host_key(&pool, 1, "ssh-ed25519", "ssh-ed25519 AAAAC3key1")
            .await
            .unwrap();

        // Same server + same key type should fail (UNIQUE constraint)
        let result = store_host_key(&pool, 1, "ssh-ed25519", "ssh-ed25519 AAAAC3key2").await;
        assert!(result.is_err());
    }
}
