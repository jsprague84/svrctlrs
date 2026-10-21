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
    /// Failed-login tracking for every sign-in step.
    pub login_limiter: Arc<LoginLimiter>,
    /// How people may sign in (password, provider); fixed at start-up.
    pub sign_in: Arc<crate::sign_in::SignInConfig>,
    /// One-time tickets that authenticate WebSocket upgrades from native clients.
    pub ws_tickets: Arc<WsTickets>,
    /// One-time codes that hand a browser sign-in to the desktop app.
    pub app_codes: Arc<crate::sign_in::app_codes::AppCodes>,
    /// Live interactive sessions; they outlive the sockets that use them.
    pub sessions: Arc<SessionRegistry>,
    /// Kept shells (tmux) waiting for a client, and revivals in progress.
    pub persistent: Arc<crate::persistent_shells::PersistentShells>,
    /// Where database backups go, and the one being written.
    pub backups: Arc<crate::backups::Backups>,
    /// Where session recordings are written, and the recorders still writing.
    pub recordings: Arc<RecordingStore>,
    /// Ids of API tokens as they are revoked or removed, so terminals opened with one end.
    pub token_revocations: tokio::sync::broadcast::Sender<i64>,
    /// Ids of devices as they are signed out, so terminals they have open end.
    pub device_sign_outs: tokio::sync::broadcast::Sender<i64>,
    /// Ids of approval requests as they are decided, for callers waiting on one.
    pub approval_events: tokio::sync::broadcast::Sender<String>,
    /// The API routes without the auth layer, set once by `app::router`: an approved request
    /// is made through it as the token that asked (`approvals.rs`).
    pub api_router: Arc<std::sync::OnceLock<axum::Router>>,
}

impl AppState {
    /// Create new application state
    pub async fn new(database: Database, allowed_origins: Vec<String>) -> Result<Self> {
        let pool = database.pool().clone();
        let login_limiter = Arc::new(LoginLimiter::with_defaults(pool.clone()));

        Ok(Self {
            pool,
            database: Arc::new(RwLock::new(database)),
            allowed_origins: Arc::new(allowed_origins),
            login_limiter,
            sign_in: Arc::new(crate::sign_in::SignInConfig::default()),
            ws_tickets: Arc::new(WsTickets::default()),
            app_codes: Arc::default(),
            sessions: SessionRegistry::from_env(),
            persistent: Arc::default(),
            backups: crate::backups::Backups::from_env(),
            recordings: RecordingStore::from_env(),
            token_revocations: tokio::sync::broadcast::channel(64).0,
            device_sign_outs: tokio::sync::broadcast::channel(64).0,
            approval_events: tokio::sync::broadcast::channel(64).0,
            api_router: Arc::new(std::sync::OnceLock::new()),
        })
    }

    /// Get database reference
    #[allow(dead_code)]
    pub async fn db(&self) -> tokio::sync::RwLockReadGuard<'_, Database> {
        self.database.read().await
    }
}
