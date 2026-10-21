//! Database layer with SQLite: the connection pool, embedded migrations, and the `models` and
//! `queries` modules. Import from those modules by path; the crate root re-exports nothing of
//! theirs (the two globs used to collide on their shared module names).

use sqlx::{sqlite::SqlitePool, Pool, Sqlite};
use svrctlrs_core::{Error, Result};
use tracing::info;

pub mod backup;
pub mod models;
pub mod queries;

// Re-export sqlx types for convenience
pub use sqlx::{self, Pool as SqlxPool, Sqlite as SqlxSqlite};

/// The current chain: `100_baseline.sql` and everything after it.
static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");
/// The frozen pre-baseline chain (`000`–`024`) for databases that already ran part of it.
static LEGACY_MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations-legacy");
/// Version of the baseline migration in [`MIGRATOR`].
pub const BASELINE_VERSION: i64 = 100;

/// Database connection pool
pub struct Database {
    pool: Pool<Sqlite>,
}

impl Database {
    /// Create a new database connection
    pub async fn new(database_url: &str) -> Result<Self> {
        info!(url = %database_url, "Connecting to database");

        // Extract file path from SQLite URL and ensure parent directory exists with proper permissions
        if database_url.starts_with("sqlite:") {
            let path = database_url.strip_prefix("sqlite:").unwrap_or(database_url);
            if let Some(parent) = std::path::Path::new(path).parent() {
                // Try to create directory if it doesn't exist
                if !parent.exists() {
                    info!(dir = ?parent, "Creating database directory");
                    std::fs::create_dir_all(parent).map_err(|e| {
                        Error::Database(anyhow::anyhow!(
                            "Failed to create database directory: {}",
                            e
                        ))
                    })?;
                } else {
                    // Directory exists, check if we can write to it
                    info!(dir = ?parent, "Database directory exists, checking permissions");

                    // Try to create a test file to verify write permissions
                    let test_file = parent.join(".write_test");
                    match std::fs::write(&test_file, b"test") {
                        Ok(_) => {
                            // Clean up test file
                            let _ = std::fs::remove_file(&test_file);
                            info!("Database directory is writable");
                        }
                        Err(e) => {
                            return Err(Error::Database(anyhow::anyhow!(
                                "Database directory {:?} is not writable: {}. Check volume permissions.",
                                parent, e
                            )));
                        }
                    }
                }
            }
        }

        // For SQLite, we need to use connect_with to set options
        use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode};
        use std::str::FromStr;

        let pool = if database_url.starts_with("sqlite:") {
            let options = SqliteConnectOptions::from_str(database_url)
                .map_err(|e| Error::Database(anyhow::anyhow!("Invalid database URL: {}", e)))?
                .create_if_missing(true)
                .journal_mode(SqliteJournalMode::Wal);

            SqlitePool::connect_with(options)
                .await
                .map_err(|e| Error::Database(anyhow::anyhow!("Failed to connect: {}", e)))?
        } else {
            SqlitePool::connect(database_url)
                .await
                .map_err(|e| Error::Database(anyhow::anyhow!("Failed to connect: {}", e)))?
        };

        Ok(Self { pool })
    }

    /// Run database migrations
    pub async fn migrate(&self) -> Result<()> {
        run_migrations(&self.pool).await
    }

    /// Get the underlying pool
    pub fn pool(&self) -> &Pool<Sqlite> {
        &self.pool
    }

    /// Close the database connection
    pub async fn close(self) -> Result<()> {
        self.pool.close().await;
        Ok(())
    }
}

/// Bring a database to the current schema.
///
/// A fresh database runs the current chain only (the baseline and later). A database that
/// predates the baseline (any `_sqlx_migrations` row below [`BASELINE_VERSION`]) first finishes
/// the legacy chain, whose last step converges on the baseline shape, and is then marked as
/// having the baseline applied so the current chain continues from there. Both migrators
/// ignore versions they do not know, since each sees the other's rows.
pub async fn run_migrations(pool: &Pool<Sqlite>) -> Result<()> {
    info!("Running database migrations");

    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(pool)
        .await
        .map_err(|e| Error::Database(anyhow::anyhow!("Failed to enable foreign keys: {}", e)))?;

    if has_legacy_history(pool).await? {
        info!("Database predates the baseline; finishing the legacy migration chain");
        tolerant(&LEGACY_MIGRATOR).run(pool).await.map_err(|e| {
            Error::Database(anyhow::anyhow!("Failed to run legacy migrations: {}", e))
        })?;
        mark_baseline_applied(pool).await?;
    }

    tolerant(&MIGRATOR)
        .run(pool)
        .await
        .map_err(|e| Error::Database(anyhow::anyhow!("Failed to run migrations: {}", e)))?;

    info!("Database migrations completed successfully");
    Ok(())
}

/// A copy of `migrator` that ignores applied versions it does not contain.
fn tolerant(migrator: &sqlx::migrate::Migrator) -> sqlx::migrate::Migrator {
    sqlx::migrate::Migrator {
        migrations: std::borrow::Cow::Owned(migrator.iter().cloned().collect()),
        ignore_missing: true,
        ..sqlx::migrate::Migrator::DEFAULT
    }
}

async fn has_legacy_history(pool: &Pool<Sqlite>) -> Result<bool> {
    let has_table: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = '_sqlx_migrations')",
    )
    .fetch_one(pool)
    .await
    .map_err(|e| Error::Database(anyhow::anyhow!("Failed to inspect the schema: {}", e)))?;
    if !has_table {
        return Ok(false);
    }
    sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM _sqlx_migrations WHERE version < ?)")
        .bind(BASELINE_VERSION)
        .fetch_one(pool)
        .await
        .map_err(|e| Error::Database(anyhow::anyhow!("Failed to read migration history: {}", e)))
}

/// Record the baseline as applied (with its real checksum, so sqlx accepts it) without running
/// it: the legacy chain has already produced that schema.
async fn mark_baseline_applied(pool: &Pool<Sqlite>) -> Result<()> {
    let baseline = MIGRATOR
        .iter()
        .find(|m| m.version == BASELINE_VERSION)
        .ok_or_else(|| Error::Config("the baseline migration is missing from the build".into()))?;
    sqlx::query(
        "INSERT OR IGNORE INTO _sqlx_migrations \
         (version, description, installed_on, success, checksum, execution_time) \
         VALUES (?, ?, CURRENT_TIMESTAMP, 1, ?, 0)",
    )
    .bind(baseline.version)
    .bind(baseline.description.as_ref())
    .bind(baseline.checksum.as_ref())
    .execute(pool)
    .await
    .map_err(|e| Error::Database(anyhow::anyhow!("Failed to record the baseline: {}", e)))?;
    Ok(())
}

#[cfg(test)]
mod migration_tests {
    use super::*;
    use sqlx::SqlitePool;
    use std::collections::BTreeSet;

    async fn versions(pool: &Pool<Sqlite>) -> Vec<i64> {
        sqlx::query_scalar("SELECT version FROM _sqlx_migrations ORDER BY version")
            .fetch_all(pool)
            .await
            .unwrap()
    }

    async fn columns(pool: &Pool<Sqlite>, table: &str) -> BTreeSet<String> {
        sqlx::query_scalar::<_, String>(&format!("SELECT name FROM pragma_table_info('{table}')"))
            .fetch_all(pool)
            .await
            .unwrap()
            .into_iter()
            .collect()
    }

    /// A database as production had it before the baseline existed: legacy chain through 023.
    async fn legacy_at_023() -> Pool<Sqlite> {
        let pool = SqlitePool::connect(":memory:").await.unwrap();
        let through_023 = sqlx::migrate::Migrator {
            migrations: std::borrow::Cow::Owned(
                LEGACY_MIGRATOR
                    .iter()
                    .filter(|m| m.version <= 23)
                    .cloned()
                    .collect(),
            ),
            ..sqlx::migrate::Migrator::DEFAULT
        };
        through_023.run(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn fresh_database_runs_only_the_current_chain() {
        let pool = SqlitePool::connect(":memory:").await.unwrap();
        run_migrations(&pool).await.unwrap();
        let applied = versions(&pool).await;
        assert_eq!(applied[0], BASELINE_VERSION);
        assert!(applied.iter().all(|v| *v >= BASELINE_VERSION));
        // Idempotent.
        run_migrations(&pool).await.unwrap();
        assert_eq!(versions(&pool).await, applied);
    }

    #[tokio::test]
    async fn legacy_database_converges_on_the_baseline_and_keeps_its_rows() {
        let pool = legacy_at_023().await;
        sqlx::query(
            "INSERT INTO servers (name, hostname, port, username, is_local, enabled, os_type) \
             VALUES ('box', NULL, 22, NULL, 1, 1, 'linux'), ('web', 'web.example', 22, 'ops', 0, 1, NULL)",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO terminal_profiles (name, layout, quick_commands) VALUES ('p', 'single', '[\"uptime\"]')",
        )
        .execute(&pool)
        .await
        .unwrap();

        run_migrations(&pool).await.unwrap();

        let applied = versions(&pool).await;
        assert!(applied.contains(&24), "legacy chain finished: {applied:?}");
        assert!(
            applied.contains(&BASELINE_VERSION),
            "baseline marked: {applied:?}"
        );

        // Retired local rows (ours, and the `localhost` one the 2025 chain seeded) are kept,
        // disabled, and satisfy the model's NOT NULL hostname.
        let rows: Vec<(String, String, bool)> =
            sqlx::query_as("SELECT name, hostname, enabled FROM servers ORDER BY name")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert_eq!(
            rows,
            vec![
                ("box".to_string(), "localhost".to_string(), false),
                ("localhost".to_string(), "localhost".to_string(), false),
                ("web".to_string(), "web.example".to_string(), true),
            ]
        );
        // The model's queries work on the converged schema.
        assert_eq!(
            queries::servers::list_servers(&pool).await.unwrap().len(),
            3
        );
        assert_eq!(
            queries::terminal_profiles::list_terminal_profiles(&pool)
                .await
                .unwrap()
                .len(),
            1
        );

        // Same columns as a fresh baseline, except the CHECK-bound `is_local`.
        let fresh = SqlitePool::connect(":memory:").await.unwrap();
        run_migrations(&fresh).await.unwrap();
        for table in [
            "users",
            "credentials",
            "servers",
            "server_host_keys",
            "terminal_profiles",
            "quick_commands",
            "settings",
            "audit_log",
        ] {
            let mut legacy = columns(&pool, table).await;
            legacy.remove("is_local");
            assert_eq!(legacy, columns(&fresh, table).await, "columns of {table}");
        }

        // A second start is a no-op.
        run_migrations(&pool).await.unwrap();
        assert_eq!(versions(&pool).await, applied);
    }
}
