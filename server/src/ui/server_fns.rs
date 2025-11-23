//! Server functions for UI interactions
//!
//! These functions run on the server but can be called from client-side components.

use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

// Re-export API types for convenience
pub use crate::ui::api::{
    PluginInfo, PluginListResponse, ServerListResponse, StatusResponse, TaskListResponse,
};

/// Result type for server functions
pub type ServerFnResult<T> = Result<T, ServerFnError>;

/// Get server status
#[server]
pub async fn get_status() -> ServerFnResult<StatusResponse> {
    let client = crate::ui::api::ApiClient::default();
    client
        .status()
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

/// List all plugins
#[server]
pub async fn list_plugins() -> ServerFnResult<PluginListResponse> {
    let client = crate::ui::api::ApiClient::default();
    client
        .plugins()
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

/// Get plugin details
#[server]
pub async fn get_plugin(plugin_id: String) -> ServerFnResult<PluginInfo> {
    let client = crate::ui::api::ApiClient::default();
    client
        .plugin(&plugin_id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

/// List all servers
#[server]
pub async fn list_servers() -> ServerFnResult<ServerListResponse> {
    let client = crate::ui::api::ApiClient::default();
    client
        .servers()
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

/// List all tasks
#[server]
pub async fn list_tasks() -> ServerFnResult<TaskListResponse> {
    let client = crate::ui::api::ApiClient::default();
    client
        .tasks()
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

/// Execute a task
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ExecuteTaskRequest {
    pub plugin_id: String,
    pub task_id: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ExecuteTaskResponse {
    pub success: bool,
    pub message: String,
}

#[server]
pub async fn execute_task(req: ExecuteTaskRequest) -> ServerFnResult<ExecuteTaskResponse> {
    // Call the backend API endpoint
    let client = reqwest::Client::new();
    let response = client
        .post("http://localhost:8080/api/v1/tasks/execute")
        .json(&serde_json::json!({
            "plugin_id": req.plugin_id,
            "task_id": req.task_id
        }))
        .send()
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    if !response.status().is_success() {
        return Err(ServerFnError::new(format!(
            "Task execution failed: {}",
            response.status()
        )));
    }

    let result: serde_json::Value = response
        .json()
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(ExecuteTaskResponse {
        success: result["success"].as_bool().unwrap_or(false),
        message: result["message"]
            .as_str()
            .unwrap_or("Unknown")
            .to_string(),
    })
}

/// Toggle task enabled/disabled
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ToggleTaskRequest {
    pub plugin_id: String,
    pub task_id: String,
    pub enabled: bool,
}

#[server]
pub async fn toggle_task(req: ToggleTaskRequest) -> ServerFnResult<bool> {
    // TODO: Implement backend endpoint for toggling tasks
    // For now, return success
    tracing::info!(
        "Toggle task: plugin={}, task={}, enabled={}",
        req.plugin_id,
        req.task_id,
        req.enabled
    );
    Ok(req.enabled)
}

/// Get server details
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

#[server]
pub async fn get_server_details(name: String) -> ServerFnResult<ServerDetails> {
    // TODO: Implement backend endpoint for server details
    // For now, return mock data
    tracing::info!("Get server details: {}", name);
    Ok(ServerDetails {
        name: name.clone(),
        ssh_host: "unknown".to_string(),
        is_local: false,
        status: "unknown".to_string(),
        uptime: None,
        cpu_usage: None,
        memory_usage: None,
        disk_usage: None,
    })
}

/// Update settings
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct UpdateSettingsRequest {
    pub settings: std::collections::HashMap<String, String>,
}

#[server]
pub async fn update_settings(req: UpdateSettingsRequest) -> ServerFnResult<bool> {
    // TODO: Implement backend endpoint for updating settings
    tracing::info!("Update settings: {:?}", req.settings);
    Ok(true)
}

/// Enable/disable plugin
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TogglePluginRequest {
    pub plugin_id: String,
    pub enabled: bool,
}

#[server]
pub async fn toggle_plugin(req: TogglePluginRequest) -> ServerFnResult<bool> {
    // TODO: Implement backend endpoint for toggling plugins
    tracing::info!(
        "Toggle plugin: plugin={}, enabled={}",
        req.plugin_id,
        req.enabled
    );
    Ok(req.enabled)
}
