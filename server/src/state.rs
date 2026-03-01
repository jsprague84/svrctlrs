//! Application state

use std::sync::Arc;
use svrctlrs_core::Result;
use svrctlrs_database::{Database, SqlxPool, SqlxSqlite};
use tokio::sync::RwLock;

use crate::config::Config;

/// Shared application state
///
/// This struct implements Clone to allow it to be used as Axum state
/// All fields are wrapped in Arc for efficient cloning
#[derive(Clone)]
#[allow(dead_code)]
pub struct AppState {
    pub config: Arc<Config>,
    pub database: Arc<RwLock<Database>>,
    pub pool: SqlxPool<SqlxSqlite>,
}

impl AppState {
    /// Create new application state
    pub async fn new(config: Config, database: Database) -> Result<Self> {
        let pool = database.pool().clone();

        Ok(Self {
            config: Arc::new(config),
            pool,
            database: Arc::new(RwLock::new(database)),
        })
    }

    /// Get database reference
    #[allow(dead_code)]
    pub async fn db(&self) -> tokio::sync::RwLockReadGuard<'_, Database> {
        self.database.read().await
    }
}
