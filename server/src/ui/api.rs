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

/// API client for server-side data fetching
pub struct ApiClient {
    base_url: String,
    client: reqwest::Client,
}

impl ApiClient {
    /// Create a new API client
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            client: reqwest::Client::new(),
        }
    }

    /// Fetch health status
    pub async fn health(&self) -> Result<HealthResponse, reqwest::Error> {
        let url = format!("{}/api/v1/health", self.base_url);
        self.client.get(&url).send().await?.json().await
    }

    /// Fetch server status
    pub async fn status(&self) -> Result<StatusResponse, reqwest::Error> {
        let url = format!("{}/api/v1/status", self.base_url);
        self.client.get(&url).send().await?.json().await
    }

    /// Fetch plugin list
    pub async fn plugins(&self) -> Result<PluginListResponse, reqwest::Error> {
        let url = format!("{}/api/v1/plugins", self.base_url);
        self.client.get(&url).send().await?.json().await
    }

    /// Fetch plugin info
    pub async fn plugin(&self, plugin_id: &str) -> Result<PluginInfo, reqwest::Error> {
        let url = format!("{}/api/v1/plugins/{}", self.base_url, plugin_id);
        self.client.get(&url).send().await?.json().await
    }

    /// Fetch server list
    pub async fn servers(&self) -> Result<ServerListResponse, reqwest::Error> {
        let url = format!("{}/api/v1/servers", self.base_url);
        self.client.get(&url).send().await?.json().await
    }

    /// Fetch task list
    pub async fn tasks(&self) -> Result<TaskListResponse, reqwest::Error> {
        let url = format!("{}/api/v1/tasks", self.base_url);
        self.client.get(&url).send().await?.json().await
    }
}

impl Default for ApiClient {
    fn default() -> Self {
        // Default to localhost for server-side rendering
        Self::new("http://localhost:8080")
    }
}
