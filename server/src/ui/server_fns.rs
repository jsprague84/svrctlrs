//! Server-side functions using Dioxus Fullstack.
//!
//! These functions access shared `AppState` via static global accessor.

use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

// Re-export API types for convenience
pub use crate::ui::api::{
    PluginInfo, PluginListResponse, ServerInfo, ServerListResponse, StatusResponse, TaskInfo,
    TaskListResponse,
};

#[cfg(feature = "server")]
use crate::state::AppState;

/// Server function: get current server status
#[server]
pub async fn get_status() -> Result<StatusResponse, ServerFnError> {
    let state = AppState::global();
    let plugins = state.plugins.read().await;
    let plugin_count = plugins.plugin_ids().len();

    let scheduler = state.scheduler.read().await;
    let scheduler_running = scheduler.is_some();

    Ok(StatusResponse {
        status: "running".to_string(),
        plugins_loaded: plugin_count,
        scheduler_running,
        servers: state.config.servers.len(),
    })
}

/// Server function: list all plugins
#[server]
pub async fn list_plugins() -> Result<PluginListResponse, ServerFnError> {
    let state = AppState::global();
    let registry = state.plugins.read().await;
    let plugins: Vec<PluginInfo> = registry
        .plugin_ids()
        .into_iter()
        .filter_map(|id| registry.get(&id))
        .map(|plugin| {
            let meta = plugin.metadata();
            PluginInfo {
                id: meta.id,
                name: meta.name,
                description: meta.description,
                version: meta.version,
                author: meta.author,
            }
        })
        .collect();

    Ok(PluginListResponse { plugins })
}

/// Server function: get plugin details
#[server]
pub async fn get_plugin(plugin_id: String) -> Result<PluginInfo, ServerFnError> {
    let state = AppState::global();
    let registry = state.plugins.read().await;

    let plugin = registry
        .get(&plugin_id)
        .ok_or_else(|| ServerFnError::new(format!("Plugin {} not found", plugin_id)))?;

    let meta = plugin.metadata();

    Ok(PluginInfo {
        id: meta.id,
        name: meta.name,
        description: meta.description,
        version: meta.version,
        author: meta.author,
    })
}

/// Server function: list all servers
#[server]
pub async fn list_servers() -> Result<ServerListResponse, ServerFnError> {
    let state = AppState::global();
    let servers: Vec<ServerInfo> = state
        .config
        .servers
        .iter()
        .map(|s| ServerInfo {
            name: s.name.clone(),
            ssh_host: s
                .ssh_host
                .clone()
                .unwrap_or_else(|| "localhost".to_string()),
            is_local: s.is_local(),
        })
        .collect();

    Ok(ServerListResponse { servers })
}

/// Server function: list all tasks
#[server]
pub async fn list_tasks() -> Result<TaskListResponse, ServerFnError> {
    let state = AppState::global();
    let registry = state.plugins.read().await;
    let mut all_tasks = Vec::new();

    for plugin_id in registry.plugin_ids() {
        if let Some(plugin) = registry.get(&plugin_id) {
            let tasks = plugin.scheduled_tasks();
            for task in tasks {
                all_tasks.push(TaskInfo {
                    plugin_id: plugin_id.clone(),
                    task_id: task.id,
                    description: task.description,
                    schedule: task.schedule,
                    enabled: task.enabled,
                });
            }
        }
    }

    Ok(TaskListResponse { tasks: all_tasks })
}

/// Execute a task request
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ExecuteTaskRequest {
    pub plugin_id: String,
    pub task_id: String,
}

/// Execute a task response
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ExecuteTaskResponse {
    pub success: bool,
    pub message: String,
}

/// Server function: execute a plugin task
#[server]
pub async fn execute_task(req: ExecuteTaskRequest) -> Result<ExecuteTaskResponse, ServerFnError> {
    use std::collections::HashMap;

    tracing::info!(
        plugin_id = %req.plugin_id,
        task_id = %req.task_id,
        "Manual task execution requested"
    );

    let state = AppState::global();
    let registry = state.plugins.read().await;

    let plugin = registry
        .get(&req.plugin_id)
        .ok_or_else(|| ServerFnError::new(format!("Plugin {} not found", req.plugin_id)))?;

    // Create plugin context
    let context = svrctlrs_core::PluginContext {
        servers: state.config.servers.clone(),
        config: HashMap::new(),
        notification_manager: state.notification_manager().await,
    };

    // Execute the task
    let result = plugin.execute(&req.task_id, &context).await.map_err(|e| {
        tracing::error!(error = %e, "Task execution failed");
        ServerFnError::new(format!("Task execution failed: {}", e))
    })?;

    tracing::info!(success = result.success, "Task execution completed");

    Ok(ExecuteTaskResponse {
        success: result.success,
        message: result.message,
    })
}

/// Toggle task request
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ToggleTaskRequest {
    pub plugin_id: String,
    pub task_id: String,
    pub enabled: bool,
}

/// Server function: toggle a scheduled task's enabled state
#[server]
pub async fn toggle_task(req: ToggleTaskRequest) -> Result<bool, ServerFnError> {
    // TODO: Implement task state persistence in scheduler
    // For now, just log the request
    tracing::info!(
        "Toggle task: plugin={}, task={}, enabled={}",
        req.plugin_id,
        req.task_id,
        req.enabled
    );

    Ok(req.enabled)
}

/// Server details
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ServerDetails {
    pub name: String,
    pub ssh_host: String,
    pub is_local: bool,
    pub status: String,
    pub uptime: Option<String>,
    pub cpu_usage: Option<f32>,
    pub memory_usage: Option<f32>,
    pub disk_usage: Option<f32>,
}

/// Server function: get detailed server information
#[server]
pub async fn get_server_details(name: String) -> Result<ServerDetails, ServerFnError> {
    tracing::info!("Get server details: {}", name);

    let state = AppState::global();

    // Find server in config
    let server = state
        .config
        .servers
        .iter()
        .find(|s| s.name == name)
        .ok_or_else(|| ServerFnError::new(format!("Server {} not found", name)))?;

    // TODO: Query health plugin for real-time metrics
    Ok(ServerDetails {
        name: server.name.clone(),
        ssh_host: server
            .ssh_host
            .clone()
            .unwrap_or_else(|| "localhost".to_string()),
        is_local: server.is_local(),
        status: "unknown".to_string(),
        uptime: None,
        cpu_usage: None,
        memory_usage: None,
        disk_usage: None,
    })
}

/// Update settings request
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct UpdateSettingsRequest {
    pub settings: std::collections::HashMap<String, String>,
}

/// Server function: update application settings
#[server]
pub async fn update_settings(req: UpdateSettingsRequest) -> Result<bool, ServerFnError> {
    // TODO: Persist settings to config file or database
    tracing::info!("Update settings: {:?}", req.settings);

    Ok(true)
}

/// Toggle plugin request
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TogglePluginRequest {
    pub plugin_id: String,
    pub enabled: bool,
}

/// Server function: toggle plugin enabled state
#[server]
pub async fn toggle_plugin(req: TogglePluginRequest) -> Result<bool, ServerFnError> {
    let state = AppState::global();

    // Verify plugin exists
    let registry = state.plugins.read().await;
    registry
        .get(&req.plugin_id)
        .ok_or_else(|| ServerFnError::new(format!("Plugin {} not found", req.plugin_id)))?;

    // TODO: Implement runtime plugin enable/disable in PluginRegistry
    tracing::info!(
        "Toggle plugin: plugin={}, enabled={}",
        req.plugin_id,
        req.enabled
    );

    Ok(req.enabled)
}
