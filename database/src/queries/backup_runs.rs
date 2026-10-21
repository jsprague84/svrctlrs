//! The history of backup attempts (ROADMAP 24): one row per attempt, the newest 100 kept.
//! Server-wide, not per user: the database is one file for everyone.

use anyhow::Context;
use sqlx::{Pool, Sqlite};
use svrctlrs_core::{Error, Result};
use tracing::instrument;

use crate::models::{BackupRun, NewBackupRun};

const COLUMNS: &str = "id, trigger, started_at, finished_at, file_name, size_bytes, removed, error";
/// Rows kept; older attempts are forgotten as new ones arrive.
const KEEP_ROWS: i64 = 100;

/// Record a finished attempt and forget the oldest beyond [`KEEP_ROWS`].
#[instrument(skip(pool, run), fields(trigger = run.trigger))]
pub async fn record(pool: &Pool<Sqlite>, run: &NewBackupRun<'_>) -> Result<i64> {
    let id = sqlx::query(
        "INSERT INTO backup_runs (trigger, started_at, finished_at, file_name, size_bytes, removed, error)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(run.trigger)
    .bind(run.started_at)
    .bind(chrono::Utc::now())
    .bind(run.file_name)
    .bind(run.size_bytes)
    .bind(run.removed)
    .bind(run.error)
    .execute(pool)
    .await
    .context("Failed to record a backup run")
    .map_err(Error::Database)?
    .last_insert_rowid();
    sqlx::query(
        "DELETE FROM backup_runs WHERE id NOT IN (SELECT id FROM backup_runs ORDER BY id DESC LIMIT ?)",
    )
    .bind(KEEP_ROWS)
    .execute(pool)
    .await
    .context("Failed to prune backup runs")
    .map_err(Error::Database)?;
    Ok(id)
}

/// The latest attempt, of any trigger or of one.
#[instrument(skip(pool))]
pub async fn latest(pool: &Pool<Sqlite>, trigger: Option<&str>) -> Result<Option<BackupRun>> {
    sqlx::query_as::<_, BackupRun>(&format!(
        "SELECT {COLUMNS} FROM backup_runs WHERE (?1 IS NULL OR trigger = ?1)
         ORDER BY started_at DESC, id DESC LIMIT 1"
    ))
    .bind(trigger)
    .fetch_optional(pool)
    .await
    .context("Failed to get the latest backup run")
    .map_err(Error::Database)
}

/// The latest attempt that wrote a copy.
#[instrument(skip(pool))]
pub async fn latest_success(pool: &Pool<Sqlite>) -> Result<Option<BackupRun>> {
    sqlx::query_as::<_, BackupRun>(&format!(
        "SELECT {COLUMNS} FROM backup_runs WHERE error IS NULL
         ORDER BY started_at DESC, id DESC LIMIT 1"
    ))
    .fetch_optional(pool)
    .await
    .context("Failed to get the latest backup")
    .map_err(Error::Database)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};

    async fn pool() -> Pool<Sqlite> {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        crate::run_migrations(&pool).await.unwrap();
        pool
    }

    fn run(
        trigger: &'static str,
        minutes_ago: i64,
        error: Option<&'static str>,
    ) -> NewBackupRun<'static> {
        NewBackupRun {
            trigger,
            started_at: Utc::now() - Duration::minutes(minutes_ago),
            file_name: error.is_none().then_some("svrctlrs-x.db"),
            size_bytes: error.is_none().then_some(4096),
            removed: 0,
            error,
        }
    }

    #[tokio::test]
    async fn latest_and_latest_success() {
        let pool = pool().await;
        assert!(latest(&pool, None).await.unwrap().is_none());
        record(&pool, &run("nightly", 60, None)).await.unwrap();
        record(&pool, &run("manual", 30, None)).await.unwrap();
        record(&pool, &run("nightly", 10, Some("disk full")))
            .await
            .unwrap();

        let last = latest(&pool, None).await.unwrap().unwrap();
        assert_eq!(last.error.as_deref(), Some("disk full"));
        let manual = latest(&pool, Some("manual")).await.unwrap().unwrap();
        assert_eq!(manual.trigger, "manual");
        let ok = latest_success(&pool).await.unwrap().unwrap();
        assert_eq!(ok.trigger, "manual");
        assert_eq!(ok.size_bytes, Some(4096));
    }

    #[tokio::test]
    async fn keeps_the_newest_hundred() {
        let pool = pool().await;
        for i in 0..105 {
            record(&pool, &run("svrctl", 200 - i, None)).await.unwrap();
        }
        let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM backup_runs")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(n, 100);
        let oldest: i64 = sqlx::query_scalar("SELECT MIN(id) FROM backup_runs")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(oldest, 6);
    }

    #[tokio::test]
    async fn settings_are_seeded() {
        let pool = pool().await;
        let keep = crate::queries::settings::get_setting_value_or(&pool, "backup.keep", "")
            .await
            .unwrap();
        let hour = crate::queries::settings::get_setting_value_or(&pool, "backup.hour", "")
            .await
            .unwrap();
        assert_eq!((keep.as_str(), hour.as_str()), ("14", "3"));
    }
}
