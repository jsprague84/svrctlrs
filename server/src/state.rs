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
    /// Origins of non-browser clients (`ALLOWED_ORIGINS`); used for CORS and to admit their
    /// WebSocket upgrades, whose `Origin` is not this server's host.
    pub allowed_origins: Arc<Vec<String>>,
}

impl AppState {
    /// Create new application state
    pub async fn new(
        config: Config,
        database: Database,
        allowed_origins: Vec<String>,
    ) -> Result<Self> {
        let pool = database.pool().clone();

        Ok(Self {
            config: Arc::new(config),
            pool,
            database: Arc::new(RwLock::new(database)),
            allowed_origins: Arc::new(allowed_origins),
        })
    }

    /// Get database reference
    #[allow(dead_code)]
    pub async fn db(&self) -> tokio::sync::RwLockReadGuard<'_, Database> {
        self.database.read().await
    }
}
