//! Plugin system traits and types

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{Error, NotificationManager, Result, Server};

/// Plugin metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMetadata {
    /// Plugin unique identifier
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Description
    pub description: String,
    /// Version
    pub version: String,
    /// Author
    pub author: String,
}

/// Plugin information returned at runtime
#[derive(Debug, Clone)]
pub struct PluginInfo {
    pub metadata: PluginMetadata,
    pub enabled: bool,
    pub scheduled_tasks: Vec<ScheduledTask>,
}

/// Scheduled task configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledTask {
    /// Unique task identifier
    pub id: String,
    /// Cron expression (e.g., "0 */5 * * * *")
    pub schedule: String,
    /// Task description
    pub description: String,
    /// Whether task is enabled by default
    pub enabled: bool,
}

/// Plugin execution context
#[derive(Debug, Clone)]
pub struct PluginContext {
    /// Servers to monitor/operate on
    pub servers: Vec<Server>,
    /// Plugin-specific configuration
    pub config: HashMap<String, String>,
    /// Notification manager for sending alerts
    pub notification_manager: NotificationManager,
}

/// Plugin execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginResult {
    /// Whether execution was successful
    pub success: bool,
    /// Human-readable message
    pub message: String,
    /// Structured data (optional)
    pub data: Option<serde_json::Value>,
    /// Metrics collected (optional)
    pub metrics: Option<HashMap<String, f64>>,
}

/// Main plugin trait
///
/// All monitoring plugins must implement this trait.
#[async_trait]
pub trait Plugin: Send + Sync {
    /// Get plugin metadata
    fn metadata(&self) -> PluginMetadata;

    /// Get scheduled tasks for this plugin
    fn scheduled_tasks(&self) -> Vec<ScheduledTask> {
        Vec::new()
    }

    /// Initialize plugin
    ///
    /// Called once when the plugin is loaded.
    async fn init(&mut self) -> Result<()> {
        Ok(())
    }

    /// Shutdown plugin
    ///
    /// Called when the plugin is unloaded or server is shutting down.
    async fn shutdown(&mut self) -> Result<()> {
        Ok(())
    }

    /// Execute plugin for a scheduled task
    ///
    /// # Arguments
    ///
    /// * `task_id` - ID of the scheduled task being executed
    /// * `context` - Execution context with servers and config
    async fn execute(&self, task_id: &str, context: &PluginContext) -> Result<PluginResult>;

    /// Health check for the plugin itself
    ///
    /// Returns Ok if plugin is healthy, Err otherwise.
    async fn health_check(&self) -> Result<()> {
        Ok(())
    }
}

/// Plugin registry
///
/// Manages all loaded plugins.
pub struct PluginRegistry {
    plugins: HashMap<String, Box<dyn Plugin>>,
}

impl PluginRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
        }
    }

    /// Register a plugin
    pub fn register(&mut self, plugin: Box<dyn Plugin>) -> Result<()> {
        let id = plugin.metadata().id.clone();

        if self.plugins.contains_key(&id) {
            return Err(Error::PluginError(format!(
                "Plugin '{}' is already registered",
                id
            )));
        }

        tracing::info!(
            plugin_id = %id,
            plugin_name = %plugin.metadata().name,
            "Registering plugin"
        );

        self.plugins.insert(id, plugin);
        Ok(())
    }

    /// Get a plugin by ID
    pub fn get(&self, id: &str) -> Option<&dyn Plugin> {
        self.plugins.get(id).map(|p| p.as_ref())
    }

    /// Get all plugin IDs
    pub fn plugin_ids(&self) -> Vec<String> {
        self.plugins.keys().cloned().collect()
    }

    /// Get all plugins
    pub fn plugins(&self) -> Vec<&dyn Plugin> {
        self.plugins.values().map(|p| p.as_ref()).collect()
    }

    /// Initialize all plugins
    pub async fn init_all(&mut self) -> Result<()> {
        for (id, plugin) in self.plugins.iter_mut() {
            tracing::info!(plugin_id = %id, "Initializing plugin");
            plugin.init().await?;
        }
        Ok(())
    }

    /// Shutdown all plugins
    pub async fn shutdown_all(&mut self) -> Result<()> {
        for (id, plugin) in self.plugins.iter_mut() {
            tracing::info!(plugin_id = %id, "Shutting down plugin");
            if let Err(e) = plugin.shutdown().await {
                tracing::error!(
                    plugin_id = %id,
                    error = %e,
                    "Failed to shutdown plugin"
                );
            }
        }
        Ok(())
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}
