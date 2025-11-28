//! Database layer with SQLite

use sqlx::{sqlite::SqlitePool, Pool, Sqlite};
use svrctlrs_core::{Error, Result};
use tracing::info;

// Export models and queries
pub mod models;
pub mod notification_service;
pub mod queries;

pub use models::*;
pub use notification_service::{NotificationService, ServerResultContext, TemplateContext};
pub use queries::*;

// Re-export sqlx types for convenience
pub use sqlx::{self, Pool as SqlxPool, Sqlite as SqlxSqlite};

// Embed migrations at compile time
static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

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
                        Error::DatabaseError(format!("Failed to create database directory: {}", e))
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
                            return Err(Error::DatabaseError(format!(
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
                .map_err(|e| Error::DatabaseError(format!("Invalid database URL: {}", e)))?
                .create_if_missing(true)
                .journal_mode(SqliteJournalMode::Wal);

            SqlitePool::connect_with(options)
                .await
                .map_err(|e| Error::DatabaseError(format!("Failed to connect: {}", e)))?
        } else {
            SqlitePool::connect(database_url)
                .await
                .map_err(|e| Error::DatabaseError(format!("Failed to connect: {}", e)))?
        };

        Ok(Self { pool })
    }

    /// Run database migrations
    pub async fn migrate(&self) -> Result<()> {
        info!("Running database migrations");

        // Enable foreign keys
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&self.pool)
            .await
            .map_err(|e| Error::DatabaseError(format!("Failed to enable foreign keys: {}", e)))?;

        // Run embedded migrations
        MIGRATOR
            .run(&self.pool)
            .await
            .map_err(|e| Error::DatabaseError(format!("Failed to run migrations: {}", e)))?;

        info!("Database migrations completed successfully");
        Ok(())
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

// ============================================================================
// Helper functions for common database operations
// ============================================================================

/// Record a metric
pub async fn record_metric(
    pool: &Pool<Sqlite>,
    server_id: i64,
    feature_id: &str,
    metric_name: &str,
    metric_value: f64,
    metric_unit: Option<&str>,
    metadata: Option<&str>,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO metrics (server_id, plugin_id, metric_name, metric_value, metric_unit, metadata)
        VALUES (?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(server_id)
    .bind(feature_id)
    .bind(metric_name)
    .bind(metric_value)
    .bind(metric_unit)
    .bind(metadata)
    .execute(pool)
    .await
    .map_err(|e| Error::DatabaseError(format!("Failed to record metric: {}", e)))?;

    Ok(())
}

/// Notification record parameters
pub struct NotificationRecord<'a> {
    pub service: &'a str,
    pub backend: &'a str,
    pub title: &'a str,
    pub body: Option<&'a str>,
    pub priority: u8,
    pub success: bool,
    pub error_message: Option<&'a str>,
}

/// Record a notification
pub async fn record_notification(
    pool: &Pool<Sqlite>,
    record: NotificationRecord<'_>,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO notifications (service, backend, title, body, priority, success, error_message)
        VALUES (?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(record.service)
    .bind(record.backend)
    .bind(record.title)
    .bind(record.body)
    .bind(record.priority as i64)
    .bind(record.success)
    .bind(record.error_message)
    .execute(pool)
    .await
    .map_err(|e| Error::DatabaseError(format!("Failed to record notification: {}", e)))?;

    Ok(())
}

/// Record a webhook invocation
pub async fn record_webhook(
    pool: &Pool<Sqlite>,
    endpoint: &str,
    server: Option<&str>,
    action: &str,
    success: bool,
    duration_ms: Option<i64>,
    error_message: Option<&str>,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO webhooks (endpoint, server, action, success, duration_ms, error_message)
        VALUES (?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(endpoint)
    .bind(server)
    .bind(action)
    .bind(success)
    .bind(duration_ms)
    .bind(error_message)
    .execute(pool)
    .await
    .map_err(|e| Error::DatabaseError(format!("Failed to record webhook: {}", e)))?;

    Ok(())
}

/// Record a task execution
pub async fn record_task_execution(
    pool: &Pool<Sqlite>,
    task_id: &str,
    feature_id: &str,
    server_id: Option<i64>,
    success: bool,
    message: Option<&str>,
    duration_ms: Option<i64>,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO task_history (task_id, feature_id, server_id, success, message, duration_ms)
        VALUES (?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(task_id)
    .bind(feature_id)
    .bind(server_id)
    .bind(success)
    .bind(message)
    .bind(duration_ms)
    .execute(pool)
    .await
    .map_err(|e| Error::DatabaseError(format!("Failed to record task execution: {}", e)))?;

    Ok(())
}
