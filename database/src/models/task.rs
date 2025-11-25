use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::FromRow;

/// Task model
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Task {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub plugin_id: String,
    pub server_id: Option<i64>,
    pub schedule: String,  // Cron expression
    pub enabled: bool,
    pub command: String,
    pub args: Option<String>,  // JSON string
    pub timeout: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_run_at: Option<DateTime<Utc>>,
    pub next_run_at: Option<DateTime<Utc>>,
    pub run_count: i32,
}

/// Create task input
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTask {
    pub name: String,
    pub description: Option<String>,
    pub plugin_id: String,
    pub server_id: Option<i64>,
    pub schedule: String,
    pub command: String,
    pub args: Option<JsonValue>,
    #[serde(default = "default_timeout")]
    pub timeout: i32,
}

/// Update task input
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateTask {
    pub name: Option<String>,
    pub description: Option<String>,
    pub schedule: Option<String>,
    pub enabled: Option<bool>,
    pub command: Option<String>,
    pub args: Option<JsonValue>,
    pub timeout: Option<i32>,
}

/// Task history model
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TaskHistory {
    pub id: i64,
    pub task_id: String,
    pub plugin_id: String,
    pub server_id: Option<i64>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub duration_ms: Option<i64>,
    pub status: Option<String>,
    pub exit_code: Option<i32>,
    pub stdout: Option<String>,
    pub stderr: Option<String>,
    pub error_message: Option<String>,
    pub triggered_by: Option<String>,
    pub success: bool,
    pub message: Option<String>,
    pub timestamp: DateTime<Utc>,
}

/// Task history entry for recording execution results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskHistoryEntry {
    pub task_id: i64,
    pub success: bool,
    pub output: String,
    pub error: Option<String>,
    pub duration_ms: u64,
    pub executed_at: DateTime<Utc>,
}

fn default_timeout() -> i32 {
    300
}

impl Task {
    /// Get args as JSON value
    pub fn get_args(&self) -> JsonValue {
        self.args
            .as_ref()
            .and_then(|a| serde_json::from_str(a).ok())
            .unwrap_or(JsonValue::Object(serde_json::Map::new()))
    }
}

