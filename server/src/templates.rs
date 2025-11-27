//! Askama templates for HTMX UI

use askama::Template;
use askama_web::WebTemplate;
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

#[derive(Template, WebTemplate)]
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

#[derive(Template, WebTemplate)]
#[template(path = "pages/servers.html")]
pub struct ServersTemplate {
    pub user: Option<User>,
    pub servers: Vec<Server>,
}

#[derive(Template, WebTemplate)]
#[template(path = "components/server_list.html")]
pub struct ServerListTemplate {
    pub servers: Vec<Server>,
}

#[derive(Template, WebTemplate)]
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
    pub description: Option<String>,
    pub enabled: bool,
}

#[derive(Debug, Deserialize)]
pub struct CreateServerInput {
    pub name: String,
    pub host: String,
    pub port: Option<i32>,
    pub username: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateServerInput {
    pub name: Option<String>,
    pub host: Option<String>,
    pub port: Option<i32>,
    pub username: Option<String>,
    pub description: Option<String>,
    pub enabled: Option<bool>,
}

// ============================================================================
// Tasks
// ============================================================================

#[derive(Template, WebTemplate)]
#[template(path = "pages/tasks.html")]
pub struct TasksTemplate {
    pub user: Option<User>,
}

#[derive(Template, WebTemplate)]
#[template(path = "components/task_list.html")]
pub struct TaskListTemplate {
    pub task_groups: Vec<TaskGroup>,
}

#[derive(Template, WebTemplate)]
#[template(path = "components/task_form.html")]
pub struct TaskFormTemplate {
    pub task: Option<Task>,
    pub servers: Vec<Server>,
    pub plugins: Vec<Plugin>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskGroup {
    pub server_name: Option<String>, // None = Local tasks
    pub tasks: Vec<Task>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub plugin_id: String,
    pub server_id: Option<i64>,
    pub server_name: Option<String>, // NULL = local execution
    pub command: String,
    pub schedule: String,
    pub enabled: bool,
    pub timeout: i32,
    pub last_run_at: Option<String>,
    pub next_run_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateTaskInput {
    pub name: String,
    pub description: Option<String>,
    pub server_id: String, // "local" or server ID
    pub plugin_id: Option<String>,
    pub command: Option<String>,
    pub remote_command: Option<String>,
    pub schedule: String,
    pub timeout: Option<i32>,
    pub enabled: Option<String>, // checkbox "on" or None
}

// ============================================================================
// Plugins
// ============================================================================

#[derive(Template, WebTemplate)]
#[template(path = "pages/plugins.html")]
pub struct PluginsTemplate {
    pub user: Option<User>,
    pub plugins: Vec<Plugin>,
}

#[derive(Template, WebTemplate)]
#[template(path = "components/plugin_list.html")]
pub struct PluginListTemplate {
    pub plugins: Vec<Plugin>,
}

#[derive(Template, WebTemplate)]
#[template(path = "components/plugin_config_form.html")]
pub struct PluginConfigFormTemplate {
    pub plugin: Plugin,
    pub config_schedule: String,
    // Weather plugin
    pub config_api_key: String,
    pub config_zip: String,
    pub config_location: String,
    pub config_units: String,
    // Speedtest plugin
    pub config_min_down: String,
    pub config_min_up: String,
    // Docker plugin
    pub config_send_summary: bool,
    pub config_cpu_warn_pct: String,
    pub config_mem_warn_pct: String,
    // Updates plugin
    pub config_updates_send_summary: bool,
    // Health plugin
    pub config_health_send_summary: bool,
    pub config_health_cpu_warn_pct: String,
    pub config_health_mem_warn_pct: String,
    pub config_health_disk_warn_pct: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plugin {
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub enabled: bool,
}

#[derive(Debug, Deserialize)]
pub struct PluginConfigInput {
    // Common to all plugins
    pub schedule: Option<String>,
    // Weather plugin
    pub api_key: Option<String>,
    pub zip: Option<String>,
    pub location: Option<String>,
    pub units: Option<String>,
    // Speedtest plugin
    pub min_down: Option<String>,
    pub min_up: Option<String>,
    // Docker plugin
    pub send_summary: Option<String>, // checkbox "on" or None
    pub cpu_warn_pct: Option<String>,
    pub mem_warn_pct: Option<String>,
    // Updates plugin
    pub updates_send_summary: Option<String>, // checkbox "on" or None
    // Health plugin
    pub health_send_summary: Option<String>, // checkbox "on" or None
    pub health_cpu_warn_pct: Option<String>,
    pub health_mem_warn_pct: Option<String>,
    pub health_disk_warn_pct: Option<String>,
}

// ============================================================================
// Settings
// ============================================================================

#[derive(Template, WebTemplate)]
#[template(path = "pages/settings.html")]
pub struct SettingsTemplate {
    pub user: Option<User>,
}

// ============================================================================
// Notifications
// ============================================================================

#[derive(Template, WebTemplate)]
#[template(path = "pages/notifications.html")]
pub struct NotificationsTemplate {
    pub user: Option<User>,
    pub notifications: Vec<NotificationBackend>,
}

#[derive(Template, WebTemplate)]
#[template(path = "components/notification_list.html")]
pub struct NotificationListTemplate {
    pub notifications: Vec<NotificationBackend>,
}

#[derive(Template, WebTemplate)]
#[template(path = "components/notification_form.html")]
pub struct NotificationFormTemplate {
    pub notification: Option<NotificationBackend>,
    pub config_url: String,
    pub config_token: String,
    pub config_topic: String,
    pub config_username: String,
    pub config_password: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationBackend {
    pub id: i64,
    pub backend_type: String,
    pub name: String,
    pub enabled: bool,
    pub priority: i32,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct CreateNotificationInput {
    pub name: String,
    pub backend_type: String,
    pub url: Option<String>,
    pub token: Option<String>,
    pub topic: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub priority: Option<i32>,
    pub enabled: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct UpdateNotificationInput {
    pub name: Option<String>,
    pub url: Option<String>,
    pub token: Option<String>,
    pub topic: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub priority: Option<i32>,
    pub enabled: Option<String>,
}

// ============================================================================
// Auth
// ============================================================================

#[derive(Template, WebTemplate)]
#[template(path = "pages/login.html")]
pub struct LoginTemplate {
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LoginForm {
    #[allow(dead_code)]
    pub username: String,
    #[allow(dead_code)]
    pub password: String,
}

// ============================================================================
// Error Pages
// ============================================================================

#[derive(Template, WebTemplate)]
#[template(path = "pages/404.html")]
pub struct NotFoundTemplate {
    pub user: Option<User>,
}
