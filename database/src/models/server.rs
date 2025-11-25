use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// Server model
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Server {
    pub id: i64,
    pub name: String,
    pub host: Option<String>,
    pub port: i32,
    pub username: String,
    pub ssh_key_path: Option<String>,
    pub enabled: bool,
    pub description: Option<String>,
    pub tags: Option<String>,  // JSON array
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_seen_at: Option<DateTime<Utc>>,
    pub os_type: Option<String>,
    pub os_version: Option<String>,
    pub docker_installed: bool,
    pub connection_timeout: i32,
    pub retry_attempts: i32,
}

/// Create server input
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateServer {
    pub name: String,
    pub host: String,
    #[serde(default = "default_port")]
    pub port: i32,
    #[serde(default = "default_username")]
    pub username: String,
    pub ssh_key_path: Option<String>,
    pub description: Option<String>,
    pub tags: Option<Vec<String>>,
}

/// Update server input
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateServer {
    pub name: Option<String>,
    pub host: Option<String>,
    pub port: Option<i32>,
    pub username: Option<String>,
    pub ssh_key_path: Option<String>,
    pub enabled: Option<bool>,
    pub description: Option<String>,
    pub tags: Option<Vec<String>>,
    pub connection_timeout: Option<i32>,
    pub retry_attempts: Option<i32>,
}

fn default_port() -> i32 {
    22
}

fn default_username() -> String {
    "root".to_string()
}

impl Server {
    /// Get tags as a vector
    pub fn get_tags(&self) -> Vec<String> {
        self.tags
            .as_ref()
            .and_then(|t| serde_json::from_str(t).ok())
            .unwrap_or_default()
    }
}

