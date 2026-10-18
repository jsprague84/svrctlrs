//! SSH host keys per server: the trusted ones a connection is checked against, and the one
//! recorded on first contact that waits for the user to confirm its fingerprint.

use anyhow::Context;
use sqlx::{Pool, Sqlite};
use svrctlrs_core::{Error, Result};
use tracing::instrument;

use crate::models::ServerHostKey;

const COLUMNS: &str =
    "id, server_id, key_type, public_key, first_seen_at, last_seen_at, trusted_at";

/// Every host key recorded for a server, trusted or not, regardless of key type.
///
/// Trust decisions must look at all of the trusted ones: a server that has a trusted key of one
/// type and suddenly presents a different type is a mismatch, not a first contact.
#[instrument(skip(pool))]
pub async fn list_host_keys_for_server(
    pool: &Pool<Sqlite>,
    server_id: i64,
) -> Result<Vec<ServerHostKey>> {
    sqlx::query_as::<_, ServerHostKey>(&format!(
        "SELECT {COLUMNS} FROM server_host_keys WHERE server_id = ? ORDER BY first_seen_at, id"
    ))
    .bind(server_id)
    .fetch_all(pool)
    .await
    .context("Failed to list host keys for server")
    .map_err(Error::Database)
}

/// Forget every host key for a server, trusted or waiting, so the next connection records a
/// new one to confirm. Returns how many rows were removed.
#[instrument(skip(pool))]
pub async fn delete_host_keys_for_server(pool: &Pool<Sqlite>, server_id: i64) -> Result<u64> {
    let result = sqlx::query("DELETE FROM server_host_keys WHERE server_id = ?")
        .bind(server_id)
        .execute(pool)
        .await
        .context("Failed to delete host keys for server")
        .map_err(Error::Database)?;
    Ok(result.rows_affected())
}

/// Record the key a server presented on first contact, not yet trusted. Only one unconfirmed
/// key is kept per server: a different key seen on the next attempt replaces it, the same key
/// refreshes `last_seen_at`. Callers only do this when the server has no trusted key.
#[instrument(skip(pool, public_key))]
pub async fn record_unconfirmed_host_key(
    pool: &Pool<Sqlite>,
    server_id: i64,
    key_type: &str,
    public_key: &str,
) -> Result<ServerHostKey> {
    let mut tx = pool
        .begin()
        .await
        .context("Failed to start a transaction")
        .map_err(Error::Database)?;
    sqlx::query(
        "DELETE FROM server_host_keys \
         WHERE server_id = ? AND trusted_at IS NULL AND NOT (key_type = ? AND public_key = ?)",
    )
    .bind(server_id)
    .bind(key_type)
    .bind(public_key)
    .execute(&mut *tx)
    .await
    .context("Failed to replace the unconfirmed host key")
    .map_err(Error::Database)?;
    let row = sqlx::query_as::<_, ServerHostKey>(&format!(
        "INSERT INTO server_host_keys (server_id, key_type, public_key) VALUES (?, ?, ?) \
         ON CONFLICT (server_id, key_type) DO UPDATE SET last_seen_at = datetime('now') \
         RETURNING {COLUMNS}"
    ))
    .bind(server_id)
    .bind(key_type)
    .bind(public_key)
    .fetch_one(&mut *tx)
    .await
    .context("Failed to record the host key")
    .map_err(Error::Database)?;
    tx.commit()
        .await
        .context("Failed to commit the host key")
        .map_err(Error::Database)?;
    Ok(row)
}

/// Trust a recorded key after the user confirmed its fingerprint. `NotFound` when the key is
/// not waiting for confirmation (unknown id, another server's key, or already trusted).
#[instrument(skip(pool))]
pub async fn trust_host_key(pool: &Pool<Sqlite>, server_id: i64, id: i64) -> Result<ServerHostKey> {
    sqlx::query_as::<_, ServerHostKey>(&format!(
        "UPDATE server_host_keys SET trusted_at = datetime('now') \
         WHERE id = ? AND server_id = ? AND trusted_at IS NULL RETURNING {COLUMNS}"
    ))
    .bind(id)
    .bind(server_id)
    .fetch_optional(pool)
    .await
    .context("Failed to trust the host key")
    .map_err(Error::Database)?
    .ok_or_else(|| Error::NotFound(format!("unconfirmed host key {id}")))
}

/// Refresh `last_seen_at` after a trusted key matched.
#[instrument(skip(pool))]
pub async fn update_host_key_last_seen(
    pool: &Pool<Sqlite>,
    server_id: i64,
    key_type: &str,
) -> Result<()> {
    sqlx::query(
        "UPDATE server_host_keys SET last_seen_at = datetime('now') \
         WHERE server_id = ? AND key_type = ?",
    )
    .bind(server_id)
    .bind(key_type)
    .execute(pool)
    .await
    .context("Failed to update host key last_seen_at")
    .map_err(Error::Database)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::SqlitePool;

    async fn setup_test_db() -> Pool<Sqlite> {
        let pool = SqlitePool::connect(":memory:").await.unwrap();
        sqlx::migrate!().run(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO servers (name, hostname, port, username, enabled) \
             VALUES ('one', 'one.example', 22, 'root', 1), ('two', 'two.example', 22, 'root', 1)",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool
    }

    #[tokio::test]
    async fn first_contact_records_an_unconfirmed_key_and_trusting_it_sticks() {
        let pool = setup_test_db().await;
        let seen = record_unconfirmed_host_key(&pool, 1, "ssh-ed25519", "ssh-ed25519 AAAA1")
            .await
            .unwrap();
        assert!(!seen.is_trusted());
        assert_eq!(seen.public_key, "ssh-ed25519 AAAA1");

        // Seeing the same key again keeps the row
        let again = record_unconfirmed_host_key(&pool, 1, "ssh-ed25519", "ssh-ed25519 AAAA1")
            .await
            .unwrap();
        assert_eq!(again.id, seen.id);

        let trusted = trust_host_key(&pool, 1, seen.id).await.unwrap();
        assert!(trusted.is_trusted());
        let keys = list_host_keys_for_server(&pool, 1).await.unwrap();
        assert_eq!(keys.len(), 1);
        assert!(keys[0].is_trusted());

        // Trusting twice, trusting through another server, or an unknown id: not found
        assert!(matches!(
            trust_host_key(&pool, 1, seen.id).await.unwrap_err(),
            Error::NotFound(_)
        ));
        assert!(matches!(
            trust_host_key(&pool, 2, seen.id).await.unwrap_err(),
            Error::NotFound(_)
        ));
        assert!(matches!(
            trust_host_key(&pool, 1, 999).await.unwrap_err(),
            Error::NotFound(_)
        ));
    }

    #[tokio::test]
    async fn a_different_unconfirmed_key_replaces_the_waiting_one() {
        let pool = setup_test_db().await;
        record_unconfirmed_host_key(&pool, 1, "ssh-ed25519", "ssh-ed25519 AAAA1")
            .await
            .unwrap();
        // Same type, different key (the host was reinstalled between attempts)
        let replaced = record_unconfirmed_host_key(&pool, 1, "ssh-ed25519", "ssh-ed25519 BBBB2")
            .await
            .unwrap();
        assert_eq!(replaced.public_key, "ssh-ed25519 BBBB2");
        // Different type (a different algorithm was negotiated)
        record_unconfirmed_host_key(&pool, 1, "ssh-rsa", "ssh-rsa CCCC3")
            .await
            .unwrap();
        let keys = list_host_keys_for_server(&pool, 1).await.unwrap();
        assert_eq!(keys.len(), 1, "{keys:?}");
        assert_eq!(keys[0].key_type, "ssh-rsa");
        assert!(!keys[0].is_trusted());
        // Other servers are untouched
        assert!(list_host_keys_for_server(&pool, 2)
            .await
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn last_seen_moves_and_keys_can_be_forgotten() {
        let pool = setup_test_db().await;
        let k = record_unconfirmed_host_key(&pool, 1, "ssh-ed25519", "ssh-ed25519 AAAA1")
            .await
            .unwrap();
        trust_host_key(&pool, 1, k.id).await.unwrap();
        update_host_key_last_seen(&pool, 1, "ssh-ed25519")
            .await
            .unwrap();
        let after = &list_host_keys_for_server(&pool, 1).await.unwrap()[0];
        assert!(after.last_seen_at >= k.last_seen_at);

        assert_eq!(delete_host_keys_for_server(&pool, 1).await.unwrap(), 1);
        assert!(list_host_keys_for_server(&pool, 1)
            .await
            .unwrap()
            .is_empty());
        assert_eq!(delete_host_keys_for_server(&pool, 1).await.unwrap(), 0);
    }
}
