//! Failed sign-in attempts, one row per attempt and key, for the login limiter
//! (`server/src/login_limiter.rs`). Times are Unix seconds, supplied by the caller so the
//! limiter can be tested without a clock.

use anyhow::Context;
use sqlx::{Pool, Sqlite};
use svrctlrs_core::{Error, Result};
use tracing::instrument;

/// Failures recorded for `key` after `since`, oldest first.
#[instrument(skip(pool))]
pub async fn failures_since(pool: &Pool<Sqlite>, key: &str, since: i64) -> Result<Vec<i64>> {
    sqlx::query_scalar(
        "SELECT failed_at FROM login_failures WHERE key = ? AND failed_at > ? ORDER BY failed_at",
    )
    .bind(key)
    .bind(since)
    .fetch_all(pool)
    .await
    .context("Failed to read login failures")
    .map_err(Error::Database)
}

/// Record one failure for each key, and drop every row at or before `prune_before` (the
/// limiter's window has passed them), in one transaction.
#[instrument(skip(pool))]
pub async fn record(
    pool: &Pool<Sqlite>,
    keys: &[String],
    at: i64,
    prune_before: i64,
) -> Result<()> {
    let mut tx = pool
        .begin()
        .await
        .context("Failed to start a transaction")
        .map_err(Error::Database)?;
    for key in keys {
        sqlx::query("INSERT INTO login_failures (key, failed_at) VALUES (?, ?)")
            .bind(key)
            .bind(at)
            .execute(&mut *tx)
            .await
            .context("Failed to record a login failure")
            .map_err(Error::Database)?;
    }
    sqlx::query("DELETE FROM login_failures WHERE failed_at <= ?")
        .bind(prune_before)
        .execute(&mut *tx)
        .await
        .context("Failed to prune login failures")
        .map_err(Error::Database)?;
    tx.commit()
        .await
        .context("Failed to commit login failures")
        .map_err(Error::Database)
}

/// Forget `key`'s failures (a successful sign-in).
#[instrument(skip(pool))]
pub async fn clear(pool: &Pool<Sqlite>, key: &str) -> Result<()> {
    sqlx::query("DELETE FROM login_failures WHERE key = ?")
        .bind(key)
        .execute(pool)
        .await
        .context("Failed to clear login failures")
        .map_err(Error::Database)?;
    Ok(())
}
