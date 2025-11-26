//! Shared types

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Server configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Server {
    /// Server name/identifier
    pub name: String,
    /// SSH connection string (user@host) or None for localhost
    pub ssh_host: Option<String>,
}

impl Server {
    /// Create a local server instance
    pub fn local(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ssh_host: None,
        }
    }

    /// Create a remote server instance
    pub fn remote(name: impl Into<String>, ssh_host: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ssh_host: Some(ssh_host.into()),
        }
    }

    /// Is this the local server?
    pub fn is_local(&self) -> bool {
        self.ssh_host.is_none()
    }

    /// Get display string for server
    pub fn display(&self) -> String {
        match &self.ssh_host {
            Some(host) => format!("{} ({})", self.name, host),
            None => format!("{} (local)", self.name),
        }
    }

    /// Parse username from ssh_host (username@host:port)
    pub fn username(&self) -> Option<String> {
        self.ssh_host
            .as_ref()
            .and_then(|host| host.split('@').next().map(|s| s.to_string()))
    }

    /// Parse host from ssh_host (username@host:port)
    pub fn host(&self) -> Option<String> {
        self.ssh_host.as_ref().and_then(|host| {
            host.split('@')
                .nth(1)
                .map(|s| s.split(':').next().unwrap_or(s).to_string())
        })
    }

    /// Parse port from ssh_host (username@host:port), defaults to 22
    pub fn port(&self) -> u16 {
        self.ssh_host
            .as_ref()
            .and_then(|host| host.split(':').nth(1).and_then(|p| p.parse::<u16>().ok()))
            .unwrap_or(22)
    }
}

/// Server status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerStatus {
    pub server: String,
    pub online: bool,
    pub last_check: chrono::DateTime<chrono::Utc>,
    pub metrics: HashMap<String, MetricValue>,
}

/// Metric value
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MetricValue {
    Integer(i64),
    Float(f64),
    String(String),
    Boolean(bool),
}

impl From<i64> for MetricValue {
    fn from(v: i64) -> Self {
        Self::Integer(v)
    }
}

impl From<f64> for MetricValue {
    fn from(v: f64) -> Self {
        Self::Float(v)
    }
}

impl From<String> for MetricValue {
    fn from(v: String) -> Self {
        Self::String(v)
    }
}

impl From<bool> for MetricValue {
    fn from(v: bool) -> Self {
        Self::Boolean(v)
    }
}
