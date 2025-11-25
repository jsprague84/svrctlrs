//! Askama templates for HTMX UI

use askama::Template;
use serde::{Deserialize, Serialize};

// ============================================================================
// User & Auth
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: i64,
    pub username: String,
}

// ============================================================================
// Dashboard
// ============================================================================

#[derive(Template)]
#[template(path = "pages/dashboard.html")]
pub struct DashboardTemplate {
    pub user: Option<User>,
    pub stats: DashboardStats,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DashboardStats {
    pub total_servers: usize,
    pub active_tasks: usize,
    pub enabled_plugins: usize,
    pub total_tasks: usize,
}

// ============================================================================
// Servers
// ============================================================================

#[derive(Template)]
#[template(path = "pages/servers.html")]
pub struct ServersTemplate {
    pub user: Option<User>,
    pub servers: Vec<Server>,
}

#[derive(Template)]
#[template(path = "components/server_list.html")]
pub struct ServerListTemplate {
    pub servers: Vec<Server>,
}

#[derive(Template)]
#[template(path = "components/server_form.html")]
pub struct ServerFormTemplate {
    pub server: Option<Server>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Server {
    pub id: i64,
    pub name: String,
    pub host: String,
    pub port: Option<i32>,
    pub username: Option<String>,
    pub enabled: bool,
}

#[derive(Debug, Deserialize)]
pub struct CreateServerInput {
    pub name: String,
    pub host: String,
    pub port: Option<i32>,
    pub username: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateServerInput {
    pub name: Option<String>,
    pub host: Option<String>,
    pub port: Option<i32>,
    pub username: Option<String>,
    pub enabled: Option<bool>,
}

// ============================================================================
// Tasks
// ============================================================================

#[derive(Template)]
#[template(path = "pages/tasks.html")]
pub struct TasksTemplate {
    pub user: Option<User>,
    pub tasks: Vec<Task>,
}

#[derive(Template)]
#[template(path = "components/task_list.html")]
pub struct TaskListTemplate {
    pub tasks: Vec<Task>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub name: String,
    pub status: TaskStatus,
    pub progress: f32,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
}

impl TaskStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskStatus::Pending => "pending",
            TaskStatus::Running => "running",
            TaskStatus::Completed => "completed",
            TaskStatus::Failed => "failed",
        }
    }

    pub fn badge_class(&self) -> &'static str {
        match self {
            TaskStatus::Pending => "badge-warning",
            TaskStatus::Running => "badge-info",
            TaskStatus::Completed => "badge-success",
            TaskStatus::Failed => "badge-error",
        }
    }
}

// ============================================================================
// Plugins
// ============================================================================

#[derive(Template)]
#[template(path = "pages/plugins.html")]
pub struct PluginsTemplate {
    pub user: Option<User>,
    pub plugins: Vec<Plugin>,
}

#[derive(Template)]
#[template(path = "components/plugin_list.html")]
pub struct PluginListTemplate {
    pub plugins: Vec<Plugin>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plugin {
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub enabled: bool,
}

// ============================================================================
// Settings
// ============================================================================

#[derive(Template)]
#[template(path = "pages/settings.html")]
pub struct SettingsTemplate {
    pub user: Option<User>,
}

// ============================================================================
// Auth
// ============================================================================

#[derive(Template)]
#[template(path = "pages/login.html")]
pub struct LoginTemplate {
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LoginForm {
    pub username: String,
    pub password: String,
}

// ============================================================================
// Error Pages
// ============================================================================

#[derive(Template)]
#[template(path = "pages/404.html")]
pub struct NotFoundTemplate {
    pub user: Option<User>,
}

