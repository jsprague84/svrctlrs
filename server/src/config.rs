//! Configuration management

use serde::{Deserialize, Serialize};
use svrctlrs_core::{Error, Result, Server};

/// Application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Database URL
    pub database_url: String,

    /// Servers to monitor
    pub servers: Vec<Server>,

    /// SSH key path for remote execution
    pub ssh_key_path: Option<String>,
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
        let database_url =
            std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:data/svrctlrs.db".to_string());

        // Parse servers from environment
        let servers = if let Ok(server_str) = std::env::var("SERVERS") {
            Self::parse_servers(&server_str)?
        } else {
            vec![Server::local("localhost")]
        };

        // Support file-based SSH key path (Docker/K8s secrets)
        let ssh_key_path = get_secret("SSH_KEY_PATH");

        Ok(Config {
            database_url,
            servers,
            ssh_key_path,
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

/// Get secret from environment variable or file
///
/// Supports both direct environment variables and file-based secrets (Docker/Kubernetes pattern).
/// If `VAR_NAME` is not found, tries `VAR_NAME_FILE` which should point to a file containing the secret.
///
/// # Examples
///
/// ```no_run
/// // Direct environment variable
/// std::env::set_var("API_KEY", "secret123");
/// let key = get_secret("API_KEY");
/// assert!(key.is_some());
///
/// // File-based secret (Docker/K8s)
/// std::env::set_var("API_KEY_FILE", "/run/secrets/api_key");
/// let key = get_secret("API_KEY");
/// assert!(key.is_some());
/// ```
pub fn get_secret(var_name: &str) -> Option<String> {
    // Try environment variable first
    if let Ok(value) = std::env::var(var_name) {
        return Some(value);
    }

    // Try file-based secret (Docker secrets / Kubernetes)
    let file_var = format!("{}_FILE", var_name);
    if let Ok(path) = std::env::var(&file_var) {
        if let Ok(contents) = std::fs::read_to_string(&path) {
            return Some(contents.trim().to_string());
        }
    }

    None
}
