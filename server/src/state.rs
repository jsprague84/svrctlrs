//! Application state

use std::sync::Arc;
use svrctlrs_core::{NotificationManager, PluginRegistry, RemoteExecutor, Result};
use svrctlrs_database::Database;
use svrctlrs_scheduler::Scheduler;
use tokio::sync::{OnceCell, RwLock};

use crate::config::Config;

/// Global application state for server functions
static APP_STATE: OnceCell<AppState> = OnceCell::const_new();

/// Shared application state
///
/// This struct implements Clone to allow it to be used as Axum state
/// All fields are wrapped in Arc for efficient cloning
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub database: Arc<RwLock<Database>>,
    pub plugins: Arc<RwLock<PluginRegistry>>,
    pub scheduler: Arc<RwLock<Option<Scheduler>>>,
    pub executor: Arc<RemoteExecutor>,
}

impl AppState {
    /// Create new application state
    pub async fn new(config: Config, database: Database) -> Result<Self> {
        let executor = Arc::new(RemoteExecutor::new(config.ssh_key_path.clone()));
        let plugins = Arc::new(RwLock::new(PluginRegistry::new()));

        Ok(Self {
            config: Arc::new(config),
            database: Arc::new(RwLock::new(database)),
            plugins,
            scheduler: Arc::new(RwLock::new(None)),
            executor,
        })
    }

    /// Initialize all plugins
    pub async fn init_plugins(&self) -> Result<()> {
        let mut registry = self.plugins.write().await;

        // Register plugins based on configuration
        #[cfg(feature = "plugin-docker")]
        if self.config.plugins.docker_enabled {
            tracing::info!("Registering Docker plugin");
            let plugin = svrctlrs_plugin_docker::DockerPlugin::new();
            registry.register(Box::new(plugin))?;
        }

        #[cfg(feature = "plugin-updates")]
        if self.config.plugins.updates_enabled {
            tracing::info!("Registering Updates plugin");
            let plugin = svrctlrs_plugin_updates::UpdatesPlugin::new();
            registry.register(Box::new(plugin))?;
        }

        #[cfg(feature = "plugin-health")]
        if self.config.plugins.health_enabled {
            tracing::info!("Registering Health plugin");
            let plugin = svrctlrs_plugin_health::HealthPlugin::new();
            registry.register(Box::new(plugin))?;
        }

        // Add-on plugins (optional, disabled by default)
        #[cfg(feature = "plugin-weather")]
        if self.config.plugins.weather_enabled {
            tracing::info!("Registering Weather plugin (add-on)");
            let plugin = svrctlrs_plugin_weather::WeatherPlugin::new();
            registry.register(Box::new(plugin))?;
        }

        #[cfg(feature = "plugin-speedtest")]
        if self.config.plugins.speedtest_enabled {
            tracing::info!("Registering SpeedTest plugin (add-on)");
            let plugin = svrctlrs_plugin_speedtest::SpeedTestPlugin::new();
            registry.register(Box::new(plugin))?;
        }

        // Initialize all registered plugins
        registry.init_all().await?;

        Ok(())
    }

    /// Start the scheduler
    pub async fn start_scheduler(&self) -> Result<()> {
        use svrctlrs_database::queries;
        use tracing::info;
        
        let mut scheduler_lock = self.scheduler.write().await;
        let scheduler = Scheduler::new();

        // Load enabled tasks from database
        let db = self.database.read().await;
        let tasks = queries::tasks::list_enabled_tasks(db.pool()).await?;
        
        info!("Loading {} enabled tasks into scheduler", tasks.len());
        
        // Register each task with the scheduler
        for task in tasks {
            let task_id = task.id;
            let schedule = task.schedule.clone();
            let task_name = task.name.clone();
            
            // Clone state for the closure
            let state = self.clone();
            
            // Create async handler that executes the task
            let handler: std::sync::Arc<dyn Fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send>> + Send + Sync> = 
                std::sync::Arc::new(move || {
                    let state = state.clone();
                    Box::pin(async move {
                        match crate::executor::execute_task(&state, task_id).await {
                            Ok(result) => {
                                if result.success {
                                    tracing::info!("Scheduled task {} completed successfully", task_id);
                                    Ok(())
                                } else {
                                    let err_msg = result.error.unwrap_or_else(|| "Unknown error".to_string());
                                    tracing::error!("Scheduled task {} failed: {}", task_id, err_msg);
                                    Err(svrctlrs_core::Error::RemoteExecutionError(err_msg))
                                }
                            }
                            Err(e) => {
                                tracing::error!("Failed to execute scheduled task {}: {}", task_id, e);
                                Err(svrctlrs_core::Error::RemoteExecutionError(e.to_string()))
                            }
                        }
                    })
                });
            
            // Try to add task, but don't fail if cron expression is invalid
            match scheduler.add_task(
                format!("task_{}", task_id),
                &schedule,
                handler,
            ).await {
                Ok(_) => {
                    tracing::info!("Registered task {} ({}) with schedule: {}", task_id, task_name, schedule);
                }
                Err(e) => {
                    tracing::error!("Failed to register task {} ({}): {}. Skipping this task.", task_id, task_name, e);
                    // Continue with other tasks instead of failing
                }
            }
        }
        
        // Start the scheduler
        scheduler.start().await?;

        *scheduler_lock = Some(scheduler);
        Ok(())
    }

    /// Get database reference
    pub async fn db(&self) -> tokio::sync::RwLockReadGuard<'_, Database> {
        self.database.read().await
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
            Ok(backends) => backends.into_iter().filter(|b| b.enabled).collect::<Vec<_>>(),
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
                        // Load service-specific keys if available
                        let mut services = Vec::new();
                        if self.config.plugins.docker_enabled {
                            services.push("docker");
                        }
                        if self.config.plugins.updates_enabled {
                            services.push("updates");
                        }
                        if self.config.plugins.health_enabled {
                            services.push("health");
                        }
                        gb.load_service_keys(&services);
                        gotify_backend = Some(gb);
                        info!("Initialized Gotify backend: {}", backend.name);
                        break; // Use first enabled Gotify backend
                    }
                    Err(e) => {
                        warn!("Failed to initialize Gotify backend {}: {}", backend.name, e);
                    }
                }
            }
        }

        // Initialize ntfy backends
        let mut ntfy_backend: Option<NtfyBackend> = None;
        for backend in backends.iter().filter(|b| b.backend_type == "ntfy") {
            let config = backend.get_config();
            if let (Some(url), Some(topic)) = (
                config.get("url").and_then(|v| v.as_str()),
                config.get("topic").and_then(|v| v.as_str()),
            ) {
                match NtfyBackend::with_url_and_topic(client.clone(), url, topic) {
                    Ok(mut nb) => {
                        // Load service-specific topics if available
                        let mut services = Vec::new();
                        if self.config.plugins.docker_enabled {
                            services.push("docker");
                        }
                        if self.config.plugins.updates_enabled {
                            services.push("updates");
                        }
                        if self.config.plugins.health_enabled {
                            services.push("health");
                        }
                        nb.load_service_topics(&services);
                        ntfy_backend = Some(nb);
                        info!("Initialized ntfy backend: {}", backend.name);
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

    /// Initialize the global app state (call once at startup)
    pub fn set_global(state: AppState) {
        if APP_STATE.set(state).is_err() {
            panic!("AppState already initialized");
        }
    }

    /// Get the global app state (for use in server functions)
    pub fn global() -> AppState {
        APP_STATE
            .get()
            .expect("AppState not initialized - call AppState::set_global() first")
            .clone()
    }
}
