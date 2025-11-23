//! Database layer with SQLite

use sqlx::{sqlite::SqlitePool, Pool, Sqlite};
use svrctlrs_core::{Error, Result};
use tracing::info;

/// Database connection pool
pub struct Database {
    pool: Pool<Sqlite>,
}

impl Database {
    /// Create a new database connection
    pub async fn new(database_url: &str) -> Result<Self> {
        info!(url = %database_url, "Connecting to database");

        let pool = SqlitePool::connect(database_url)
            .await
            .map_err(|e| Error::DatabaseError(format!("Failed to connect: {}", e)))?;

        Ok(Self { pool })
    }

    /// Run database migrations
    pub async fn migrate(&self) -> Result<()> {
        info!("Running database migrations");

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS servers (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE,
                ssh_host TEXT,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| Error::DatabaseError(format!("Migration failed: {}", e)))?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS metrics (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                server_id INTEGER NOT NULL,
                plugin_id TEXT NOT NULL,
                metric_name TEXT NOT NULL,
                metric_value REAL NOT NULL,
                timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (server_id) REFERENCES servers(id)
            )
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| Error::DatabaseError(format!("Migration failed: {}", e)))?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS task_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                task_id TEXT NOT NULL,
                plugin_id TEXT NOT NULL,
                server_id INTEGER,
                success BOOLEAN NOT NULL,
                message TEXT,
                timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (server_id) REFERENCES servers(id)
            )
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| Error::DatabaseError(format!("Migration failed: {}", e)))?;

        info!("Database migrations completed");
        Ok(())
    }

    /// Get the underlying pool
    pub fn pool(&self) -> &Pool<Sqlite> {
        &self.pool
    }
}
