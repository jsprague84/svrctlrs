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
            
            scheduler.add_task(
                format!("task_{}", task_id),
                &schedule,
                handler,
            ).await?;
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
    pub async fn notification_manager(&self) -> NotificationManager {
        let client = reqwest::Client::new();
        let mut services = Vec::new();

        // Add enabled services
        if self.config.plugins.docker_enabled {
            services.push("docker");
        }
        if self.config.plugins.updates_enabled {
            services.push("updates");
        }
        if self.config.plugins.health_enabled {
            services.push("health");
        }

        NotificationManager::new(client, &services).unwrap_or_else(|_| {
            tracing::warn!("Failed to create notification manager");
            // Create empty manager as fallback
            NotificationManager::new(reqwest::Client::new(), &[]).unwrap()
        })
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
