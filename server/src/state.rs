//! Application state

use std::sync::Arc;
use svrctlrs_core::Result;
use svrctlrs_database::{Database, SqlxPool, SqlxSqlite};
use tokio::sync::RwLock;

use crate::login_limiter::LoginLimiter;
use crate::recordings::RecordingStore;
use crate::sessions::SessionRegistry;
use crate::ws_tickets::WsTickets;

/// Shared application state
///
/// This struct implements Clone to allow it to be used as Axum state
/// All fields are wrapped in Arc for efficient cloning
#[derive(Clone)]
pub struct AppState {
    pub database: Arc<RwLock<Database>>,
    pub pool: SqlxPool<SqlxSqlite>,
    /// Origins of non-browser clients (`ALLOWED_ORIGINS`); used for CORS and to admit their
    /// WebSocket upgrades, whose `Origin` is not this server's host.
    pub allowed_origins: Arc<Vec<String>>,
    /// Failed-login tracking for both login endpoints.
    pub login_limiter: Arc<LoginLimiter>,
    /// One-time tickets that authenticate WebSocket upgrades from native clients.
    pub ws_tickets: Arc<WsTickets>,
    /// Live interactive sessions; they outlive the sockets that use them.
    pub sessions: Arc<SessionRegistry>,
    /// Where session recordings are written, and the recorders still writing.
    pub recordings: Arc<RecordingStore>,
}

impl AppState {
    /// Create new application state
    pub async fn new(database: Database, allowed_origins: Vec<String>) -> Result<Self> {
        let pool = database.pool().clone();

        Ok(Self {
            pool,
            database: Arc::new(RwLock::new(database)),
            allowed_origins: Arc::new(allowed_origins),
            login_limiter: Arc::new(LoginLimiter::with_defaults()),
            ws_tickets: Arc::new(WsTickets::default()),
            sessions: SessionRegistry::from_env(),
            recordings: RecordingStore::from_env(),
        })
    }

    /// Get database reference
    #[allow(dead_code)]
    pub async fn db(&self) -> tokio::sync::RwLockReadGuard<'_, Database> {
        self.database.read().await
    }
}
