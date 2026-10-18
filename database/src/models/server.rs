use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// An SSH target.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Server {
    pub id: i64,
    pub name: String,
    /// Hostname or IP address
    pub hostname: String,
    /// SSH port
    pub port: i32,
    /// SSH user; `None` means the credential's user, else `root` (see `ssh::target`)
    pub username: Option<String>,
    /// Foreign key to credentials
    pub credential_id: Option<i64>,
    pub description: Option<String>,
    pub enabled: bool,

    // Status tracking (health monitor and connection attempts)
    pub last_seen_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Server {
    /// `user@host[:port]` for logs and messages; the port only when it is not 22.
    pub fn display_address(&self) -> String {
        let user = self.username.as_deref().unwrap_or("root");
        if self.port != 22 {
            format!("{}@{}:{}", user, self.hostname, self.port)
        } else {
            format!("{}@{}", user, self.hostname)
        }
    }
}

/// Input for creating a new server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateServer {
    pub name: String,
    pub hostname: String,
    #[serde(default = "default_port")]
    pub port: i32,
    pub username: Option<String>,
    pub credential_id: Option<i64>,
    pub description: Option<String>,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

impl CreateServer {
    /// Validate server input
    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("Server name cannot be empty".to_string());
        }
        if self.hostname.trim().is_empty() {
            return Err("A server needs a hostname".to_string());
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
    /// `None` keeps the current credential, `Some(None)` unassigns it, `Some(Some(id))` sets it.
    pub credential_id: Option<Option<i64>>,
    pub description: Option<String>,
    pub enabled: Option<bool>,
    pub last_error: Option<String>,
}

impl UpdateServer {
    /// Check if this update contains any changes
    pub fn has_changes(&self) -> bool {
        self.name.is_some()
            || self.hostname.is_some()
            || self.port.is_some()
            || self.username.is_some()
            || self.credential_id.is_some()
            || self.description.is_some()
            || self.enabled.is_some()
            || self.last_error.is_some()
    }
}

fn default_port() -> i32 {
    22
}

fn default_enabled() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server(hostname: &str, username: Option<&str>, port: i32) -> Server {
        Server {
            id: 1,
            name: "web".to_string(),
            hostname: hostname.to_string(),
            port,
            username: username.map(String::from),
            credential_id: None,
            description: None,
            enabled: true,
            last_seen_at: None,
            last_error: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn display_address_shows_user_host_and_a_non_default_port() {
        assert_eq!(
            server("192.168.1.100", Some("admin"), 22).display_address(),
            "admin@192.168.1.100"
        );
        assert_eq!(
            server("example.com", Some("user"), 2222).display_address(),
            "user@example.com:2222"
        );
        assert_eq!(server("h", None, 22).display_address(), "root@h");
    }

    #[test]
    fn create_server_validation() {
        let valid = CreateServer {
            name: "remote".to_string(),
            hostname: "192.168.1.100".to_string(),
            port: 22,
            username: Some("admin".to_string()),
            credential_id: Some(1),
            description: None,
            enabled: true,
        };
        assert!(valid.validate().is_ok());
        assert!(CreateServer {
            hostname: "  ".to_string(),
            ..valid.clone()
        }
        .validate()
        .is_err());
        assert!(CreateServer {
            port: 99999,
            ..valid.clone()
        }
        .validate()
        .is_err());
        assert!(CreateServer {
            name: String::new(),
            ..valid
        }
        .validate()
        .is_err());
    }
}
