//! Application state

use svrctlrs_core::{NotificationManager, PluginRegistry, RemoteExecutor, Result};
use svrctlrs_scheduler::Scheduler;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::config::Config;

/// Shared application state
pub struct AppState {
    pub config: Config,
    pub plugins: Arc<RwLock<PluginRegistry>>,
    pub scheduler: Arc<RwLock<Option<Scheduler>>>,
    pub executor: RemoteExecutor,
}

impl AppState {
    /// Create new application state
    pub async fn new(config: Config) -> Result<Self> {
        let executor = RemoteExecutor::new(config.ssh_key_path.clone());
        let plugins = Arc::new(RwLock::new(PluginRegistry::new()));

        Ok(Self {
            config,
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

        // Initialize all registered plugins
        registry.init_all().await?;

        Ok(())
    }

    /// Start the scheduler
    pub async fn start_scheduler(&self) -> Result<()> {
        let mut scheduler_lock = self.scheduler.write().await;
        let scheduler = Scheduler::new();

        // TODO: Register scheduled tasks from plugins

        *scheduler_lock = Some(scheduler);
        Ok(())
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

        NotificationManager::new(client, &services)
            .unwrap_or_else(|_| {
                tracing::warn!("Failed to create notification manager");
                // Create empty manager as fallback
                NotificationManager::new(reqwest::Client::new(), &[]).unwrap()
            })
    }
}
