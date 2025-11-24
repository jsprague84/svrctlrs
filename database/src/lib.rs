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

        // Enable foreign keys
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&self.pool)
            .await
            .map_err(|e| Error::DatabaseError(format!("Failed to enable foreign keys: {}", e)))?;

        // Servers table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS servers (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE,
                ssh_host TEXT,
                enabled BOOLEAN NOT NULL DEFAULT 1,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| Error::DatabaseError(format!("Failed to create servers table: {}", e)))?;

        // Metrics history table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS metrics (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                server_id INTEGER NOT NULL,
                plugin_id TEXT NOT NULL,
                metric_name TEXT NOT NULL,
                metric_value REAL NOT NULL,
                metric_unit TEXT,
                metadata TEXT,
                timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (server_id) REFERENCES servers(id) ON DELETE CASCADE
            )
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| Error::DatabaseError(format!("Failed to create metrics table: {}", e)))?;

        // Create index for fast metric queries
        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_metrics_server_plugin
            ON metrics(server_id, plugin_id, timestamp DESC)
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| Error::DatabaseError(format!("Failed to create metrics index: {}", e)))?;

        // Notification log table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS notifications (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                service TEXT NOT NULL,
                backend TEXT NOT NULL,
                title TEXT NOT NULL,
                body TEXT,
                priority INTEGER DEFAULT 3,
                success BOOLEAN NOT NULL,
                error_message TEXT,
                timestamp DATETIME DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| {
            Error::DatabaseError(format!("Failed to create notifications table: {}", e))
        })?;

        // Webhook invocation log table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS webhooks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                endpoint TEXT NOT NULL,
                server TEXT,
                action TEXT NOT NULL,
                success BOOLEAN NOT NULL,
                duration_ms INTEGER,
                error_message TEXT,
                timestamp DATETIME DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| Error::DatabaseError(format!("Failed to create webhooks table: {}", e)))?;

        // Task execution history table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS task_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                task_id TEXT NOT NULL,
                plugin_id TEXT NOT NULL,
                server_id INTEGER,
                success BOOLEAN NOT NULL,
                message TEXT,
                duration_ms INTEGER,
                timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (server_id) REFERENCES servers(id) ON DELETE SET NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| Error::DatabaseError(format!("Failed to create task_history table: {}", e)))?;

        // Create index for task history queries
        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_task_history_plugin
            ON task_history(plugin_id, timestamp DESC)
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| Error::DatabaseError(format!("Failed to create task history index: {}", e)))?;

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
    plugin_id: &str,
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
    .bind(plugin_id)
    .bind(metric_name)
    .bind(metric_value)
    .bind(metric_unit)
    .bind(metadata)
    .execute(pool)
    .await
    .map_err(|e| Error::DatabaseError(format!("Failed to record metric: {}", e)))?;

    Ok(())
}

/// Record a notification
pub async fn record_notification(
    pool: &Pool<Sqlite>,
    service: &str,
    backend: &str,
    title: &str,
    body: Option<&str>,
    priority: u8,
    success: bool,
    error_message: Option<&str>,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO notifications (service, backend, title, body, priority, success, error_message)
        VALUES (?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(service)
    .bind(backend)
    .bind(title)
    .bind(body)
    .bind(priority as i64)
    .bind(success)
    .bind(error_message)
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
    plugin_id: &str,
    server_id: Option<i64>,
    success: bool,
    message: Option<&str>,
    duration_ms: Option<i64>,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO task_history (task_id, plugin_id, server_id, success, message, duration_ms)
        VALUES (?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(task_id)
    .bind(plugin_id)
    .bind(server_id)
    .bind(success)
    .bind(message)
    .bind(duration_ms)
    .execute(pool)
    .await
    .map_err(|e| Error::DatabaseError(format!("Failed to record task execution: {}", e)))?;

    Ok(())
}
