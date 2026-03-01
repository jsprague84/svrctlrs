//! Database layer with SQLite

#![allow(ambiguous_glob_reexports)]

use sqlx::{sqlite::SqlitePool, Pool, Sqlite};
use svrctlrs_core::{Error, Result};
use tracing::info;

// Export models and queries
pub mod models;
pub mod queries;

pub use models::*;
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
