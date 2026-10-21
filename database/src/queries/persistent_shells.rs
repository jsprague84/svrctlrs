//! Shells kept in tmux on their server (ROADMAP 23). Lookups for a user take the `user_id`;
//! the unscoped ones serve the server's own housekeeping (expiry after a restart).

use anyhow::Context;
use sqlx::{Pool, Sqlite};
use svrctlrs_core::{Error, Result};
use tracing::instrument;

use crate::models::PersistentShell;

const COLUMNS: &str = "id, user_id, server_id, tmux_name, created_at";

/// Remember a shell started in tmux.
#[instrument(skip(pool))]
pub async fn insert(
    pool: &Pool<Sqlite>,
    id: &str,
    user_id: i64,
    server_id: i64,
    tmux_name: &str,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO persistent_shells (id, user_id, server_id, tmux_name) VALUES (?, ?, ?, ?)",
    )
    .bind(id)
    .bind(user_id)
    .bind(server_id)
    .bind(tmux_name)
    .execute(pool)
    .await
    .context("Failed to save a persistent shell")
    .map_err(Error::Database)?;
    Ok(())
}

/// The user's shell with this session id, if it is a kept one.
#[instrument(skip(pool))]
pub async fn get_for_user(
    pool: &Pool<Sqlite>,
    id: &str,
    user_id: i64,
) -> Result<Option<PersistentShell>> {
    sqlx::query_as::<_, PersistentShell>(&format!(
        "SELECT {COLUMNS} FROM persistent_shells WHERE id = ? AND user_id = ?"
    ))
    .bind(id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .context("Failed to get a persistent shell")
    .map_err(Error::Database)
}

/// The kept shell with this session id, whoever's it is (the server's own housekeeping).
#[instrument(skip(pool))]
pub async fn get(pool: &Pool<Sqlite>, id: &str) -> Result<Option<PersistentShell>> {
    sqlx::query_as::<_, PersistentShell>(&format!(
        "SELECT {COLUMNS} FROM persistent_shells WHERE id = ?"
    ))
    .bind(id)
    .fetch_optional(pool)
    .await
    .context("Failed to get a persistent shell")
    .map_err(Error::Database)
}

/// The user's kept shells, oldest first.
#[instrument(skip(pool))]
pub async fn list_for_user(pool: &Pool<Sqlite>, user_id: i64) -> Result<Vec<PersistentShell>> {
    sqlx::query_as::<_, PersistentShell>(&format!(
        "SELECT {COLUMNS} FROM persistent_shells WHERE user_id = ? ORDER BY created_at, id"
    ))
    .bind(user_id)
    .fetch_all(pool)
    .await
    .context("Failed to list persistent shells")
    .map_err(Error::Database)
}

/// Every kept shell, for the expiry sweep after a restart.
#[instrument(skip(pool))]
pub async fn list_all(pool: &Pool<Sqlite>) -> Result<Vec<PersistentShell>> {
    sqlx::query_as::<_, PersistentShell>(&format!(
        "SELECT {COLUMNS} FROM persistent_shells ORDER BY created_at, id"
    ))
    .fetch_all(pool)
    .await
    .context("Failed to list persistent shells")
    .map_err(Error::Database)
}

/// Forget a kept shell (it ended for good).
#[instrument(skip(pool))]
pub async fn delete(pool: &Pool<Sqlite>, id: &str) -> Result<()> {
    sqlx::query("DELETE FROM persistent_shells WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .context("Failed to delete a persistent shell")
        .map_err(Error::Database)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn db() -> Pool<Sqlite> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        crate::run_migrations(&pool).await.unwrap();
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&pool)
            .await
            .unwrap();
        for name in ["ann", "bob"] {
            sqlx::query("INSERT INTO users (username, password_hash) VALUES (?, 'x')")
                .bind(name)
                .execute(&pool)
                .await
                .unwrap();
        }
        sqlx::query(
            "INSERT INTO servers (name, hostname, persistent_shell) VALUES ('web', 'h', 1)",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool
    }

    #[tokio::test]
    async fn a_kept_shell_is_found_only_by_its_owner() {
        let pool = db().await;
        insert(&pool, "abc", 1, 1, "svrctlrs-abc").await.unwrap();
        let mine = get_for_user(&pool, "abc", 1).await.unwrap().unwrap();
        assert_eq!(mine.tmux_name, "svrctlrs-abc");
        assert_eq!(mine.server_id, 1);
        assert!(get_for_user(&pool, "abc", 2).await.unwrap().is_none());
        assert_eq!(list_for_user(&pool, 1).await.unwrap().len(), 1);
        assert!(list_for_user(&pool, 2).await.unwrap().is_empty());
        assert_eq!(list_all(&pool).await.unwrap().len(), 1);
        delete(&pool, "abc").await.unwrap();
        assert!(list_all(&pool).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn deleting_the_server_or_the_user_forgets_its_shells() {
        let pool = db().await;
        insert(&pool, "a", 1, 1, "svrctlrs-a").await.unwrap();
        insert(&pool, "b", 2, 1, "svrctlrs-b").await.unwrap();
        sqlx::query("DELETE FROM users WHERE id = 2")
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(list_all(&pool).await.unwrap().len(), 1);
        sqlx::query("DELETE FROM servers WHERE id = 1")
            .execute(&pool)
            .await
            .unwrap();
        assert!(list_all(&pool).await.unwrap().is_empty());
    }

    #[test]
    fn the_migration_adds_the_server_switch_off() {
        // The column default is what every existing server gets
        let sql = include_str!("../../migrations/110_persistent_shells.sql");
        assert!(sql.contains("persistent_shell BOOLEAN NOT NULL DEFAULT 0"));
    }
}
