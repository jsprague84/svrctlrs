//! API client for fetching data from the backend

use serde::{Deserialize, Serialize};

/// Health check response
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HealthResponse {
    pub status: String,
    pub service: String,
    pub version: String,
}

/// Server status response
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StatusResponse {
    pub status: String,
    pub plugins_loaded: usize,
    pub scheduler_running: bool,
    pub servers: usize,
}

/// Plugin metadata
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PluginInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub author: String,
}

/// Plugin list response
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PluginListResponse {
    pub plugins: Vec<PluginInfo>,
}

/// Server info
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ServerInfo {
    pub name: String,
    pub ssh_host: String,
    pub is_local: bool,
}

/// Server list response
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ServerListResponse {
    pub servers: Vec<ServerInfo>,
}

/// Task info
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TaskInfo {
    pub plugin_id: String,
    pub task_id: String,
    pub description: String,
    pub schedule: String,
    pub enabled: bool,
}

/// Task list response
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TaskListResponse {
    pub tasks: Vec<TaskInfo>,
}

// Note: ApiClient has been removed. Server functions now use FromContext<AppState>
// extraction to access server state directly, eliminating HTTP overhead.
