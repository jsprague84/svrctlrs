//! Application state

use std::sync::Arc;
use svrctlrs_core::{NotificationManager, PluginRegistry, RemoteExecutor, Result};
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
    pub plugins: Arc<RwLock<PluginRegistry>>,
    pub scheduler: Arc<RwLock<Option<Scheduler>>>,
    #[allow(dead_code)]
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

    /// Initialize all plugins based on database configuration
    pub async fn init_plugins(&self) -> Result<()> {
        use svrctlrs_database::queries;

        let mut registry = self.plugins.write().await;
        let db = self.database.read().await;

        // Load enabled plugins from database
        let enabled_plugins = queries::plugins::list_enabled_plugins(db.pool()).await?;

        tracing::info!(
            "Loading {} enabled plugins from database",
            enabled_plugins.len()
        );

        // Register each enabled plugin
        for db_plugin in enabled_plugins {
            match db_plugin.id.as_str() {
                #[cfg(feature = "plugin-docker")]
                "docker" => {
                    tracing::info!("Registering Docker plugin (enabled in database)");
                    let config = db_plugin.get_config();
                    let plugin = svrctlrs_plugin_docker::DockerPlugin::from_config(config)?;
                    registry.register(Box::new(plugin))?;
                }

                #[cfg(feature = "plugin-updates")]
                "updates" => {
                    tracing::info!("Registering Updates plugin (enabled in database)");
                    let config = db_plugin.get_config();
                    let plugin = svrctlrs_plugin_updates::UpdatesPlugin::from_config(config)?;
                    registry.register(Box::new(plugin))?;
                }

                #[cfg(feature = "plugin-health")]
                "health" => {
                    tracing::info!("Registering Health plugin (enabled in database)");
                    let config = db_plugin.get_config();
                    let plugin = svrctlrs_plugin_health::HealthPlugin::from_config(config)?;
                    registry.register(Box::new(plugin))?;
                }

                #[cfg(feature = "plugin-weather")]
                "weather" => {
                    tracing::info!("Registering Weather plugin (enabled in database)");
                    let config = db_plugin.get_config();
                    let plugin = svrctlrs_plugin_weather::WeatherPlugin::from_config(config)?;
                    registry.register(Box::new(plugin))?;
                }

                #[cfg(feature = "plugin-speedtest")]
                "speedtest" => {
                    tracing::info!("Registering SpeedTest plugin (enabled in database)");
                    let config = db_plugin.get_config();
                    let plugin = svrctlrs_plugin_speedtest::SpeedTestPlugin::from_config(config)?;
                    registry.register(Box::new(plugin))?;
                }

                _ => {
                    tracing::warn!("Unknown plugin in database: {}", db_plugin.id);
                }
            }
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

        // Type alias for complex handler type
        type TaskHandler = std::sync::Arc<
            dyn Fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send>>
                + Send
                + Sync,
        >;

        // Register each task with the scheduler
        for task in tasks {
            let task_id = task.id;
            let schedule = task.schedule.clone();
            let task_name = task.name.clone();

            // Clone state for the closure
            let state = self.clone();

            // Create async handler that executes the task
            let handler: TaskHandler = std::sync::Arc::new(move || {
                let state = state.clone();
                Box::pin(async move {
                    match crate::executor::execute_task(&state, task_id).await {
                        Ok(result) => {
                            if result.success {
                                tracing::info!("Scheduled task {} completed successfully", task_id);
                                Ok(())
                            } else {
                                let err_msg =
                                    result.error.unwrap_or_else(|| "Unknown error".to_string());
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
            match scheduler
                .add_task(format!("task_{}", task_id), &schedule, handler)
                .await
            {
                Ok(_) => {
                    // Calculate and update next run time
                    match queries::tasks::calculate_next_run(&schedule) {
                        Ok(next_run) => {
                            if let Err(e) = queries::tasks::update_task_next_run(db.pool(), task_id, next_run).await {
                                tracing::warn!("Failed to update next_run_at for task {}: {}", task_id, e);
                            } else {
                                tracing::debug!(
                                    "Updated next_run_at for task {} ({}): {:?}",
                                    task_id,
                                    task_name,
                                    next_run
                                );
                            }
                        }
                        Err(e) => {
                            tracing::warn!("Failed to calculate next_run_at for task {}: {}", task_id, e);
                        }
                    }

                    tracing::info!(
                        "Registered task {} ({}) with schedule: {}",
                        task_id,
                        task_name,
                        schedule
                    );
                }
                Err(e) => {
                    tracing::error!(
                        "Failed to register task {} ({}): {}. Skipping this task.",
                        task_id,
                        task_name,
                        e
                    );
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

    /// Reload configuration from database without restarting
    /// This reloads:
    /// - Plugin configurations
    /// - Task schedules
    /// - Notification backends
    pub async fn reload_config(&self) -> Result<()> {
        use svrctlrs_database::queries;

        tracing::info!("🔄 Reloading configuration from database");

        // 1. Reload plugins
        tracing::info!("Reloading plugins...");
        {
            let mut registry = self.plugins.write().await;
            registry.clear();
            drop(registry);
        }
        self.init_plugins().await?;

        // 2. Reload scheduler tasks
        tracing::info!("Reloading scheduler tasks...");
        if let Some(scheduler) = self.scheduler.read().await.as_ref() {
            // Clear existing tasks
            scheduler.clear_all_tasks().await;

            // Reload tasks from database
            let db = self.database.read().await;
            let enabled_tasks = queries::tasks::list_enabled_tasks(db.pool()).await?;

            tracing::info!(
                "Loading {} enabled tasks into scheduler",
                enabled_tasks.len()
            );

            for task in enabled_tasks {
                let task_id = format!("task_{}", task.id);
                let schedule = task.schedule.clone();
                let state = self.clone();
                let task_id_clone = task.id;

                let handler: svrctlrs_scheduler::AsyncTaskHandler = Arc::new(move || {
                    let state = state.clone();
                    let task_id = task_id_clone;
                    Box::pin(async move {
                        crate::executor::execute_task(&state, task_id)
                            .await
                            .map(|_| ())
                            .map_err(|e| svrctlrs_core::Error::RemoteExecutionError(e.to_string()))
                    })
                });

                scheduler.add_task(&task_id, &schedule, handler).await?;

                // Calculate and update next run time
                match queries::tasks::calculate_next_run(&schedule) {
                    Ok(next_run) => {
                        if let Err(e) = queries::tasks::update_task_next_run(db.pool(), task.id, next_run).await {
                            tracing::warn!("Failed to update next_run_at for task {}: {}", task.id, e);
                        } else {
                            tracing::debug!(
                                "Updated next_run_at for task {} ({}): {:?}",
                                task.id,
                                task.name,
                                next_run
                            );
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Failed to calculate next_run_at for task {}: {}", task.id, e);
                    }
                }

                tracing::info!(
                    "Registered task {} ({}) with schedule: {}",
                    task.id,
                    task.name,
                    task.schedule
                );
            }
        }

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
