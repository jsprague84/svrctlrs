//! A user's own preferences. Every query takes the `user_id`; there is no way to read or write
//! another user's row.

use anyhow::Context;
use sqlx::{Pool, Sqlite};
use svrctlrs_core::{Error, Result};
use tracing::instrument;

use crate::models::UserSetting;

/// The user's value under `key`, or None when they never saved one.
#[instrument(skip(pool))]
pub async fn get(pool: &Pool<Sqlite>, user_id: i64, key: &str) -> Result<Option<UserSetting>> {
    sqlx::query_as::<_, UserSetting>(
        "SELECT user_id, key, value, updated_at FROM user_settings WHERE user_id = ? AND key = ?",
    )
    .bind(user_id)
    .bind(key)
    .fetch_optional(pool)
    .await
    .context("Failed to get user setting")
    .map_err(Error::Database)
}

/// Save the user's value under `key` (insert or replace) and return the stored row.
#[instrument(skip(pool, value))]
pub async fn put(pool: &Pool<Sqlite>, user_id: i64, key: &str, value: &str) -> Result<UserSetting> {
    sqlx::query_as::<_, UserSetting>(
        r#"
        INSERT INTO user_settings (user_id, key, value, updated_at)
        VALUES (?, ?, ?, CURRENT_TIMESTAMP)
        ON CONFLICT(user_id, key) DO UPDATE SET
            value = excluded.value,
            updated_at = CURRENT_TIMESTAMP
        RETURNING user_id, key, value, updated_at
        "#,
    )
    .bind(user_id)
    .bind(key)
    .bind(value)
    .fetch_one(pool)
    .await
    .context("Failed to save user setting")
    .map_err(Error::Database)
}

/// Forget the user's value under `key` (back to the default).
#[instrument(skip(pool))]
pub async fn delete(pool: &Pool<Sqlite>, user_id: i64, key: &str) -> Result<()> {
    sqlx::query("DELETE FROM user_settings WHERE user_id = ? AND key = ?")
        .bind(user_id)
        .bind(key)
        .execute(pool)
        .await
        .context("Failed to delete user setting")
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
        for name in ["ann", "bob"] {
            sqlx::query("INSERT INTO users (username, password_hash) VALUES (?, 'x')")
                .bind(name)
                .execute(&pool)
                .await
                .unwrap();
        }
        pool
    }

    #[tokio::test]
    async fn each_user_has_their_own_value_and_a_save_replaces_it() {
        let pool = db().await;
        assert!(get(&pool, 1, "terminal").await.unwrap().is_none());

        put(&pool, 1, "terminal", r#"{"fontSize":12}"#)
            .await
            .unwrap();
        let saved = put(&pool, 1, "terminal", r#"{"fontSize":16}"#)
            .await
            .unwrap();
        assert_eq!(saved.value, r#"{"fontSize":16}"#);
        assert_eq!(
            get(&pool, 1, "terminal").await.unwrap().unwrap().value,
            saved.value
        );
        assert!(
            get(&pool, 2, "terminal").await.unwrap().is_none(),
            "bob has none"
        );

        put(&pool, 2, "terminal", r#"{"fontSize":20}"#)
            .await
            .unwrap();
        assert_eq!(
            get(&pool, 1, "terminal").await.unwrap().unwrap().value,
            r#"{"fontSize":16}"#
        );
    }

    #[tokio::test]
    async fn deleting_a_user_deletes_their_settings() {
        let pool = db().await;
        put(&pool, 2, "terminal", "{}").await.unwrap();
        sqlx::query("DELETE FROM users WHERE id = 2")
            .execute(&pool)
            .await
            .unwrap();
        let left: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM user_settings")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(left.0, 0);
    }
}
