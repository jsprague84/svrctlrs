//! Signed-in devices. Everything a user does with their devices is scoped by `user_id`; the
//! authentication lookup ([`find_by_hash`]) and the housekeeping ([`all_hashes`],
//! [`delete_ids`]) go by the session's hash alone.

use anyhow::Context;
use sqlx::{Pool, Sqlite};
use svrctlrs_core::{Error, Result};
use tracing::instrument;

use crate::models::{Device, NewDevice};

const COLUMNS: &str = "id, user_id, session_hash, method, user_agent, first_ip, last_ip, \
                       created_at, last_seen_at";

/// Record a sign-in. Signing the same session in again (an id collision is not possible, but
/// a repeated adoption is) keeps the first row.
#[instrument(skip(pool, device), fields(user_id = device.user_id, method = %device.method))]
pub async fn create(pool: &Pool<Sqlite>, device: &NewDevice) -> Result<Device> {
    sqlx::query_as::<_, Device>(&format!(
        r#"
        INSERT INTO devices (user_id, session_hash, method, user_agent, first_ip, last_ip)
        VALUES (?, ?, ?, ?, ?, ?)
        ON CONFLICT(session_hash) DO UPDATE SET session_hash = excluded.session_hash
        RETURNING {COLUMNS}
        "#
    ))
    .bind(device.user_id)
    .bind(&device.session_hash)
    .bind(&device.method)
    .bind(&device.user_agent)
    .bind(&device.ip)
    .bind(&device.ip)
    .fetch_one(pool)
    .await
    .context("Failed to record the sign-in")
    .map_err(Error::Database)
}

/// The device of the session with this hash, whoever owns it (the auth layer compares the
/// owner with the session's user).
#[instrument(skip(pool, session_hash))]
pub async fn find_by_hash(pool: &Pool<Sqlite>, session_hash: &str) -> Result<Option<Device>> {
    sqlx::query_as::<_, Device>(&format!(
        "SELECT {COLUMNS} FROM devices WHERE session_hash = ?"
    ))
    .bind(session_hash)
    .fetch_optional(pool)
    .await
    .context("Failed to look up the device")
    .map_err(Error::Database)
}

/// The user's devices, most recently used first
#[instrument(skip(pool))]
pub async fn list_for_user(pool: &Pool<Sqlite>, user_id: i64) -> Result<Vec<Device>> {
    sqlx::query_as::<_, Device>(&format!(
        "SELECT {COLUMNS} FROM devices WHERE user_id = ? ORDER BY last_seen_at DESC, id DESC"
    ))
    .bind(user_id)
    .fetch_all(pool)
    .await
    .context("Failed to list devices")
    .map_err(Error::Database)
}

/// Remove one of the user's devices and return what it was. Another user's device, or a
/// missing one, is [`Error::NotFound`].
#[instrument(skip(pool))]
pub async fn delete(pool: &Pool<Sqlite>, user_id: i64, id: i64) -> Result<Device> {
    sqlx::query_as::<_, Device>(&format!(
        "DELETE FROM devices WHERE id = ? AND user_id = ? RETURNING {COLUMNS}"
    ))
    .bind(id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .context("Failed to remove the device")
    .map_err(Error::Database)?
    .ok_or_else(|| Error::NotFound(format!("device {id}")))
}

/// Remove all of the user's devices except `keep` (None: all of them) and return them.
#[instrument(skip(pool))]
pub async fn delete_all_except(
    pool: &Pool<Sqlite>,
    user_id: i64,
    keep: Option<i64>,
) -> Result<Vec<Device>> {
    sqlx::query_as::<_, Device>(&format!(
        "DELETE FROM devices WHERE user_id = ? AND id IS NOT ? RETURNING {COLUMNS}"
    ))
    .bind(user_id)
    .bind(keep)
    .fetch_all(pool)
    .await
    .context("Failed to remove devices")
    .map_err(Error::Database)
}

/// Note that the device was just used, from `ip`; the user agent is filled in when the
/// sign-in did not have one (a sign-in from before devices were listed).
#[instrument(skip(pool, user_agent))]
pub async fn touch(pool: &Pool<Sqlite>, id: i64, ip: &str, user_agent: Option<&str>) -> Result<()> {
    sqlx::query(
        "UPDATE devices SET last_seen_at = CURRENT_TIMESTAMP, last_ip = ?, \
         user_agent = COALESCE(user_agent, ?) WHERE id = ?",
    )
    .bind(ip)
    .bind(user_agent)
    .bind(id)
    .execute(pool)
    .await
    .context("Failed to record device use")
    .map_err(Error::Database)?;
    Ok(())
}

/// Every device's id and session hash, for matching against the live sessions
#[instrument(skip(pool))]
pub async fn all_hashes(pool: &Pool<Sqlite>) -> Result<Vec<(i64, String)>> {
    sqlx::query_as("SELECT id, session_hash FROM devices")
        .fetch_all(pool)
        .await
        .context("Failed to list device hashes")
        .map_err(Error::Database)
}

/// Whether the device is still signed in
#[instrument(skip(pool))]
pub async fn exists(pool: &Pool<Sqlite>, id: i64) -> Result<bool> {
    sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM devices WHERE id = ?")
        .bind(id)
        .fetch_one(pool)
        .await
        .map(|n| n > 0)
        .context("Failed to look up the device")
        .map_err(Error::Database)
}

/// Remove these devices (their sessions are gone)
#[instrument(skip(pool, ids), fields(count = ids.len()))]
pub async fn delete_ids(pool: &Pool<Sqlite>, ids: &[i64]) -> Result<u64> {
    let mut removed = 0;
    for id in ids {
        removed += sqlx::query("DELETE FROM devices WHERE id = ?")
            .bind(id)
            .execute(pool)
            .await
            .context("Failed to remove a device")
            .map_err(Error::Database)?
            .rows_affected();
    }
    Ok(removed)
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

    fn new(user_id: i64, hash: &str) -> NewDevice {
        NewDevice {
            user_id,
            session_hash: hash.to_string(),
            method: "oidc".into(),
            user_agent: None,
            ip: Some("10.0.0.1".into()),
        }
    }

    #[tokio::test]
    async fn devices_are_per_user() {
        let pool = db().await;
        let a1 = create(&pool, &new(1, "h1")).await.unwrap();
        let a2 = create(&pool, &new(1, "h2")).await.unwrap();
        let b1 = create(&pool, &new(2, "h3")).await.unwrap();
        // The same session recorded twice is one device
        assert_eq!(create(&pool, &new(1, "h1")).await.unwrap().id, a1.id);
        assert_eq!(list_for_user(&pool, 1).await.unwrap().len(), 2);

        // Another user's device is not found, and stays
        assert!(matches!(
            delete(&pool, 1, b1.id).await,
            Err(Error::NotFound(_))
        ));
        assert_eq!(find_by_hash(&pool, "h3").await.unwrap().unwrap().id, b1.id);

        touch(&pool, a2.id, "10.0.0.9", Some("Firefox"))
            .await
            .unwrap();
        touch(&pool, a2.id, "10.0.0.9", Some("Chrome"))
            .await
            .unwrap();
        let a2 = find_by_hash(&pool, "h2").await.unwrap().unwrap();
        assert_eq!(a2.last_ip.as_deref(), Some("10.0.0.9"));
        assert_eq!(a2.first_ip.as_deref(), Some("10.0.0.1"));
        assert_eq!(a2.user_agent.as_deref(), Some("Firefox"));

        let others = delete_all_except(&pool, 1, Some(a1.id)).await.unwrap();
        assert_eq!(others.iter().map(|d| d.id).collect::<Vec<_>>(), [a2.id]);
        assert_eq!(delete(&pool, 1, a1.id).await.unwrap().session_hash, "h1");
        assert!(list_for_user(&pool, 1).await.unwrap().is_empty());
        assert_eq!(list_for_user(&pool, 2).await.unwrap().len(), 1);

        assert!(exists(&pool, b1.id).await.unwrap());
        assert_eq!(delete_ids(&pool, &[b1.id]).await.unwrap(), 1);
        assert!(!exists(&pool, b1.id).await.unwrap());
        assert!(all_hashes(&pool).await.unwrap().is_empty());
    }
}
