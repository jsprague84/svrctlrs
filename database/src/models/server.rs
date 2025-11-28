use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::FromRow;

/// Server model - represents execution targets (local or remote)
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Server {
    pub id: i64,
    pub name: String,
    /// Hostname or IP address (NULL for local server)
    pub hostname: Option<String>,
    /// SSH port for remote servers
    pub port: i32,
    /// SSH username for remote servers
    pub username: Option<String>,
    /// Foreign key to credentials table for SSH key
    pub credential_id: Option<i64>,
    pub description: Option<String>,
    /// Whether this is the local server
    pub is_local: bool,
    pub enabled: bool,

    // Detected metadata
    /// Detected OS type (linux, windows, macos)
    pub os_type: Option<String>,
    /// Detected OS distribution (ubuntu, fedora, debian, arch, etc.)
    pub os_distro: Option<String>,
    /// Detected package manager (apt, dnf, pacman, yum, etc.)
    pub package_manager: Option<String>,
    pub docker_available: bool,
    pub systemd_available: bool,
    /// Additional metadata as JSON string
    pub metadata: Option<String>,

    // Status tracking
    pub last_seen_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Server {
    /// Get metadata as JSON value
    pub fn get_metadata(&self) -> JsonValue {
        self.metadata
            .as_ref()
            .and_then(|m| serde_json::from_str(m).ok())
            .unwrap_or(JsonValue::Object(serde_json::Map::new()))
    }

    /// Check if this is a remote server
    pub fn is_remote(&self) -> bool {
        !self.is_local
    }

    /// Get the display address for this server
    pub fn display_address(&self) -> String {
        if self.is_local {
            "localhost".to_string()
        } else if let Some(ref hostname) = self.hostname {
            if let Some(ref username) = self.username {
                if self.port != 22 {
                    format!("{}@{}:{}", username, hostname, self.port)
                } else {
                    format!("{}@{}", username, hostname)
                }
            } else {
                hostname.clone()
            }
        } else {
            "unknown".to_string()
        }
    }

    /// Check if server has required capability
    pub fn has_capability(&self, capability: &str) -> bool {
        match capability {
            "docker" => self.docker_available,
            "systemd" => self.systemd_available,
            "apt" => self.package_manager.as_deref() == Some("apt"),
            "dnf" => self.package_manager.as_deref() == Some("dnf"),
            "pacman" => self.package_manager.as_deref() == Some("pacman"),
            "yum" => self.package_manager.as_deref() == Some("yum"),
            _ => false,
        }
    }

    /// Check if server has all required capabilities
    pub fn has_all_capabilities(&self, capabilities: &[String]) -> bool {
        capabilities.iter().all(|cap| self.has_capability(cap))
    }

    /// Check if server is healthy (recently seen and no errors)
    pub fn is_healthy(&self) -> bool {
        self.last_error.is_none() && self.last_seen_at.is_some()
    }
}

/// ServerCapability model - tracks detected capabilities
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ServerCapability {
    pub id: i64,
    pub server_id: i64,
    /// Capability name (docker, systemd, apt, dnf, pacman, etc.)
    pub capability: String,
    pub available: bool,
    /// Version if applicable
    pub version: Option<String>,
    pub detected_at: DateTime<Utc>,
}

/// Input for creating a new server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateServer {
    pub name: String,
    pub hostname: Option<String>,
    #[serde(default = "default_port")]
    pub port: i32,
    pub username: Option<String>,
    pub credential_id: Option<i64>,
    pub description: Option<String>,
    #[serde(default)]
    pub is_local: bool,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    pub metadata: Option<JsonValue>,
}

impl CreateServer {
    /// Convert metadata to JSON string for database storage
    pub fn metadata_string(&self) -> Option<String> {
        self.metadata
            .as_ref()
            .and_then(|m| serde_json::to_string(m).ok())
    }

    /// Validate server input
    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("Server name cannot be empty".to_string());
        }

        if !self.is_local {
            if self.hostname.is_none() {
                return Err("Remote servers must have a hostname".to_string());
            }
            if self.username.is_none() {
                return Err("Remote servers must have a username".to_string());
            }
        }

        if self.port < 1 || self.port > 65535 {
            return Err(format!("Invalid port number: {}", self.port));
        }

        Ok(())
    }
}

/// Input for updating an existing server
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateServer {
    pub name: Option<String>,
    pub hostname: Option<String>,
    pub port: Option<i32>,
    pub username: Option<String>,
    pub credential_id: Option<i64>,
    pub description: Option<String>,
    pub enabled: Option<bool>,
    pub os_type: Option<String>,
    pub os_distro: Option<String>,
    pub package_manager: Option<String>,
    pub docker_available: Option<bool>,
    pub systemd_available: Option<bool>,
    pub metadata: Option<JsonValue>,
    pub last_error: Option<String>,
}

impl UpdateServer {
    /// Convert metadata to JSON string for database storage
    pub fn metadata_string(&self) -> Option<String> {
        self.metadata
            .as_ref()
            .and_then(|m| serde_json::to_string(m).ok())
    }

    /// Check if this update contains any changes
    pub fn has_changes(&self) -> bool {
        self.name.is_some()
            || self.hostname.is_some()
            || self.port.is_some()
            || self.username.is_some()
            || self.credential_id.is_some()
            || self.description.is_some()
            || self.enabled.is_some()
            || self.os_type.is_some()
            || self.os_distro.is_some()
            || self.package_manager.is_some()
            || self.docker_available.is_some()
            || self.systemd_available.is_some()
            || self.metadata.is_some()
            || self.last_error.is_some()
    }
}

fn default_port() -> i32 {
    22
}

fn default_enabled() -> bool {
    true
}

/// Server with tags (for UI display)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerWithTags {
    #[serde(flatten)]
    pub server: Server,
    pub tags: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_display_address() {
        let local = Server {
            id: 1,
            name: "localhost".to_string(),
            hostname: None,
            port: 22,
            username: None,
            credential_id: None,
            description: None,
            is_local: true,
            enabled: true,
            os_type: None,
            os_distro: None,
            package_manager: None,
            docker_available: false,
            systemd_available: false,
            metadata: None,
            last_seen_at: None,
            last_error: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        assert_eq!(local.display_address(), "localhost");

        let remote = Server {
            id: 2,
            name: "web-server".to_string(),
            hostname: Some("192.168.1.100".to_string()),
            port: 22,
            username: Some("admin".to_string()),
            credential_id: Some(1),
            description: None,
            is_local: false,
            enabled: true,
            os_type: Some("linux".to_string()),
            os_distro: Some("ubuntu".to_string()),
            package_manager: Some("apt".to_string()),
            docker_available: true,
            systemd_available: true,
            metadata: None,
            last_seen_at: None,
            last_error: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        assert_eq!(remote.display_address(), "admin@192.168.1.100");

        let remote_custom_port = Server {
            id: 3,
            name: "custom-port".to_string(),
            hostname: Some("example.com".to_string()),
            port: 2222,
            username: Some("user".to_string()),
            credential_id: Some(1),
            description: None,
            is_local: false,
            enabled: true,
            os_type: None,
            os_distro: None,
            package_manager: None,
            docker_available: false,
            systemd_available: false,
            metadata: None,
            last_seen_at: None,
            last_error: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        assert_eq!(
            remote_custom_port.display_address(),
            "user@example.com:2222"
        );
    }

    #[test]
    fn test_server_capabilities() {
        let server = Server {
            id: 1,
            name: "test".to_string(),
            hostname: Some("test.local".to_string()),
            port: 22,
            username: Some("user".to_string()),
            credential_id: None,
            description: None,
            is_local: false,
            enabled: true,
            os_type: Some("linux".to_string()),
            os_distro: Some("ubuntu".to_string()),
            package_manager: Some("apt".to_string()),
            docker_available: true,
            systemd_available: true,
            metadata: None,
            last_seen_at: None,
            last_error: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        assert!(server.has_capability("docker"));
        assert!(server.has_capability("systemd"));
        assert!(server.has_capability("apt"));
        assert!(!server.has_capability("dnf"));
        assert!(!server.has_capability("pacman"));

        assert!(server.has_all_capabilities(&["docker".to_string(), "apt".to_string()]));
        assert!(!server.has_all_capabilities(&["docker".to_string(), "dnf".to_string()]));
    }

    #[test]
    fn test_create_server_validation() {
        let valid_local = CreateServer {
            name: "localhost".to_string(),
            hostname: None,
            port: 22,
            username: None,
            credential_id: None,
            description: None,
            is_local: true,
            enabled: true,
            metadata: None,
        };
        assert!(valid_local.validate().is_ok());

        let valid_remote = CreateServer {
            name: "remote".to_string(),
            hostname: Some("192.168.1.100".to_string()),
            port: 22,
            username: Some("admin".to_string()),
            credential_id: Some(1),
            description: None,
            is_local: false,
            enabled: true,
            metadata: None,
        };
        assert!(valid_remote.validate().is_ok());

        let invalid_remote_no_hostname = CreateServer {
            name: "remote".to_string(),
            hostname: None,
            port: 22,
            username: Some("admin".to_string()),
            credential_id: None,
            description: None,
            is_local: false,
            enabled: true,
            metadata: None,
        };
        assert!(invalid_remote_no_hostname.validate().is_err());

        let invalid_port = CreateServer {
            name: "test".to_string(),
            hostname: Some("test.local".to_string()),
            port: 99999,
            username: Some("user".to_string()),
            credential_id: None,
            description: None,
            is_local: false,
            enabled: true,
            metadata: None,
        };
        assert!(invalid_port.validate().is_err());
    }
}
