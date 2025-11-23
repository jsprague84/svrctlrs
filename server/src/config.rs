//! Configuration management

use serde::{Deserialize, Serialize};
use svrctlrs_core::{Error, Result, Server};
use std::path::Path;

/// Application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Database URL
    pub database_url: String,

    /// Servers to monitor
    pub servers: Vec<Server>,

    /// SSH key path for remote execution
    pub ssh_key_path: Option<String>,

    /// Notification configuration
    pub notifications: NotificationConfig,

    /// Plugin configuration
    pub plugins: PluginConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationConfig {
    pub gotify_url: Option<String>,
    pub gotify_key: Option<String>,
    pub ntfy_url: Option<String>,
    pub ntfy_topic: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfig {
    pub docker_enabled: bool,
    pub updates_enabled: bool,
    pub health_enabled: bool,
}

impl Config {
    /// Load configuration from file or environment
    pub fn load(path: Option<&str>) -> Result<Self> {
        if let Some(p) = path {
            Self::load_from_file(p)
        } else {
            Self::load_from_env()
        }
    }

    /// Load from configuration file
    fn load_from_file(path: &str) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| Error::ConfigError(format!("Failed to read config: {}", e)))?;

        toml::from_str(&content)
            .map_err(|e| Error::ConfigError(format!("Failed to parse config: {}", e)))
    }

    /// Load from environment variables
    fn load_from_env() -> Result<Self> {
        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "sqlite:data/svrctlrs.db".to_string());

        // Parse servers from environment
        let servers = if let Ok(server_str) = std::env::var("SERVERS") {
            Self::parse_servers(&server_str)?
        } else {
            vec![Server::local("localhost")]
        };

        let ssh_key_path = std::env::var("SSH_KEY_PATH").ok();

        Ok(Config {
            database_url,
            servers,
            ssh_key_path,
            notifications: NotificationConfig {
                gotify_url: std::env::var("GOTIFY_URL").ok(),
                gotify_key: std::env::var("GOTIFY_KEY").ok(),
                ntfy_url: std::env::var("NTFY_URL").ok(),
                ntfy_topic: std::env::var("NTFY_TOPIC").ok(),
            },
            plugins: PluginConfig {
                docker_enabled: std::env::var("ENABLE_DOCKER_PLUGIN")
                    .map(|v| v == "true" || v == "1")
                    .unwrap_or(true),
                updates_enabled: std::env::var("ENABLE_UPDATES_PLUGIN")
                    .map(|v| v == "true" || v == "1")
                    .unwrap_or(true),
                health_enabled: std::env::var("ENABLE_HEALTH_PLUGIN")
                    .map(|v| v == "true" || v == "1")
                    .unwrap_or(true),
            },
        })
    }

    /// Parse server list from string
    fn parse_servers(input: &str) -> Result<Vec<Server>> {
        input
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| {
                let parts: Vec<&str> = s.split(':').collect();
                match parts.len() {
                    1 => Ok(Server::local(parts[0])),
                    2 => Ok(Server::remote(parts[0], parts[1])),
                    _ => Err(Error::ConfigError(format!("Invalid server format: {}", s))),
                }
            })
            .collect()
    }
}
