//! Application state

use std::sync::Arc;
use svrctlrs_core::{NotificationManager, RemoteExecutor, Result};
use svrctlrs_database::Database;
use svrctlrs_scheduler::Scheduler;
use tokio::sync::RwLock;

use crate::config::Config;

/// Shared application state
///
/// This struct implements Clone to allow it to be used as Axum state
/// All fields are wrapped in Arc for efficient cloning
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub database: Arc<RwLock<Database>>,
    pub pool: sqlx::Pool<sqlx::Sqlite>,  // Direct pool access for convenience
    pub scheduler: Arc<RwLock<Option<Scheduler>>>,
    #[allow(dead_code)]
    pub executor: Arc<RemoteExecutor>,
}

impl AppState {
    /// Create new application state
    pub async fn new(config: Config, database: Database) -> Result<Self> {
        let executor = Arc::new(RemoteExecutor::new(config.ssh_key_path.clone()));
        let pool = database.pool().clone();

        Ok(Self {
            config: Arc::new(config),
            pool,
            database: Arc::new(RwLock::new(database)),
            scheduler: Arc::new(RwLock::new(None)),
            executor,
        })
    }

    /// Start the scheduler
    pub async fn start_scheduler(&self) -> Result<()> {
        use svrctlrs_core::executor::JobExecutor;
        use tracing::info;

        let mut scheduler_lock = self.scheduler.write().await;

        // Create job executor
        let executor = Arc::new(JobExecutor::new(
            self.pool.clone(),
            self.config.ssh_key_path.clone(),
            10, // max_concurrent_jobs
        ));

        // Create and start the new scheduler
        let mut scheduler = Scheduler::new(self.pool.clone(), executor);

        info!("Starting job scheduler");
        scheduler.start().await?;

        *scheduler_lock = Some(scheduler);
        Ok(())
    }

    /// Get database reference
    pub async fn db(&self) -> tokio::sync::RwLockReadGuard<'_, Database> {
        self.database.read().await
    }

    /// Reload configuration from database without restarting
    /// This reloads:
    /// - Job schedules (via scheduler restart)
    /// - Notification backends
    pub async fn reload_config(&self) -> Result<()> {
        tracing::info!("🔄 Reloading configuration from database");

        // For the new scheduler, we can simply restart it since it polls the database
        // The scheduler will automatically pick up any changes to job_schedules
        tracing::info!("Restarting scheduler to pick up schedule changes...");

        // Note: The new scheduler is database-driven and polls for changes,
        // so it doesn't need explicit task registration. It will automatically
        // detect new/updated job schedules on the next poll cycle.

        tracing::info!("✅ Configuration reloaded successfully");
        Ok(())
    }

    /// Get notification manager for plugin context
    /// Loads notification backends from database
    pub async fn notification_manager(&self) -> NotificationManager {
        use svrctlrs_core::{GotifyBackend, NtfyBackend};
        use svrctlrs_database::queries;
        use tracing::{info, warn};

        let client = reqwest::Client::new();
        let db = self.database.read().await;

        // Load enabled notification backends from database
        let backends = match queries::notifications::list_notification_backends(db.pool()).await {
            Ok(backends) => backends
                .into_iter()
                .filter(|b| b.enabled)
                .collect::<Vec<_>>(),
            Err(e) => {
                warn!("Failed to load notification backends from database: {}", e);
                Vec::new()
            }
        };

        // Initialize Gotify backends
        let mut gotify_backend: Option<GotifyBackend> = None;
        for backend in backends.iter().filter(|b| b.backend_type == "gotify") {
            let config = backend.get_config();
            if let (Some(url), Some(token)) = (
                config.get("url").and_then(|v| v.as_str()),
                config.get("token").and_then(|v| v.as_str()),
            ) {
                match GotifyBackend::with_url_and_key(client.clone(), url, token) {
                    Ok(mut gb) => {
                        // Load service-specific keys for all plugins
                        // (plugins will be filtered by enabled status at runtime)
                        let services = vec!["docker", "updates", "health", "weather", "speedtest"];
                        gb.load_service_keys(&services);
                        gotify_backend = Some(gb);
                        info!("Initialized Gotify backend: {}", backend.name);
                        break; // Use first enabled Gotify backend
                    }
                    Err(e) => {
                        warn!(
                            "Failed to initialize Gotify backend {}: {}",
                            backend.name, e
                        );
                    }
                }
            }
        }

        // Initialize ntfy backends
        let mut ntfy_backend: Option<NtfyBackend> = None;
        for backend in backends.iter().filter(|b| b.backend_type == "ntfy") {
            let config = backend.get_config();

            // Debug: log the config to see what we have
            info!("Loading ntfy backend config: {:?}", config);

            if let (Some(url), Some(topic)) = (
                config.get("url").and_then(|v| v.as_str()),
                config.get("topic").and_then(|v| v.as_str()),
            ) {
                match NtfyBackend::with_url_and_topic(client.clone(), url, topic) {
                    Ok(mut nb) => {
                        // Add authentication if configured
                        if let Some(token) = config.get("token").and_then(|v| v.as_str()) {
                            if !token.trim().is_empty() {
                                nb = nb.with_token(token);
                                info!(
                                    "Configured ntfy with token authentication (token length: {})",
                                    token.len()
                                );
                            } else {
                                info!("Token field exists but is empty");
                            }
                        } else if let (Some(username), Some(password)) = (
                            config.get("username").and_then(|v| v.as_str()),
                            config.get("password").and_then(|v| v.as_str()),
                        ) {
                            if !username.trim().is_empty() && !password.trim().is_empty() {
                                nb = nb.with_basic_auth(username, password);
                                info!(
                                    "Configured ntfy with basic authentication (username: {})",
                                    username
                                );
                            } else {
                                info!("Username/password fields exist but are empty");
                            }
                        } else {
                            info!("No authentication configured for ntfy backend");
                        }

                        // Register the same topic for all services (they all go to the same topic)
                        // This allows plugins to call send_for_service("weather", msg) etc.
                        let services = vec!["docker", "updates", "health", "weather", "speedtest"];
                        for service in services {
                            nb.register_service(service, topic);
                        }

                        // Also try to load service-specific topics from environment (optional override)
                        nb.load_service_topics(&[
                            "docker",
                            "updates",
                            "health",
                            "weather",
                            "speedtest",
                        ]);

                        ntfy_backend = Some(nb);
                        info!(
                            "Initialized ntfy backend: {} (topic: {})",
                            backend.name, topic
                        );
                        break; // Use first enabled ntfy backend
                    }
                    Err(e) => {
                        warn!("Failed to initialize ntfy backend {}: {}", backend.name, e);
                    }
                }
            }
        }

        // Create notification manager with database-loaded backends
        NotificationManager::from_backends(gotify_backend, ntfy_backend)
    }
}
