//! Askama templates for HTMX UI

use askama::Template;
use askama_web::WebTemplate;
use serde::{Deserialize, Serialize};
use svrctlrs_database::models::{
    Credential, CredentialType, JobTemplate, JobTemplateStep, JobType, CommandTemplate,
    JobSchedule, JobRun, ServerJobResult, StepExecutionResult, Server, Tag,
};

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
    pub total_schedules: usize,
    pub active_jobs: usize,
    pub recent_runs: Vec<RecentJobRun>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentJobRun {
    pub id: i64,
    pub job_name: String,
    pub server_name: String,
    pub status: String,
    pub started_at: String,
    pub duration_seconds: Option<f64>,
}

// ============================================================================
// Servers
// ============================================================================

#[derive(Template, WebTemplate)]
#[template(path = "pages/servers.html")]
pub struct ServersTemplate {
    pub user: Option<User>,
    pub servers: Vec<ServerDisplay>,
}

#[derive(Template, WebTemplate)]
#[template(path = "components/server_list.html")]
pub struct ServerListTemplate {
    pub servers: Vec<ServerDisplay>,
}

#[derive(Template, WebTemplate)]
#[template(path = "components/server_form.html")]
pub struct ServerFormTemplate {
    pub server: Option<ServerDisplay>,
    pub credentials: Vec<CredentialDisplay>,
    pub tags: Vec<TagDisplay>,
    pub error: Option<String>,
}

#[derive(Template, WebTemplate)]
#[template(path = "components/server_capabilities.html")]
pub struct ServerCapabilitiesTemplate {
    pub server_id: i64,
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerDisplay {
    pub id: i64,
    pub name: String,
    pub hostname: String,
    pub description: Option<String>,
    pub credential_id: Option<i64>,
    pub credential_name: Option<String>,
    pub connection_type: String,
    pub connection_string: Option<String>,
    pub tags: Vec<String>,
    pub capabilities: Vec<String>,
    pub os_distro: Option<String>,
    pub os_version: Option<String>,
    pub enabled: bool,
    pub last_seen_at: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateServerInput {
    pub name: String,
    pub hostname: String,
    pub description: Option<String>,
    pub credential_id: Option<String>,
    pub connection_type: String,
    pub connection_string: Option<String>,
    pub enabled: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateServerInput {
    pub name: Option<String>,
    pub hostname: Option<String>,
    pub description: Option<String>,
    pub credential_id: Option<String>,
    pub connection_type: Option<String>,
    pub connection_string: Option<String>,
    pub enabled: Option<String>,
}

// ============================================================================
// Credentials
// ============================================================================

#[derive(Template, WebTemplate)]
#[template(path = "pages/credentials.html")]
pub struct CredentialsTemplate {
    pub user: Option<User>,
    pub credentials: Vec<CredentialDisplay>,
}

#[derive(Template, WebTemplate)]
#[template(path = "components/credential_list.html")]
pub struct CredentialListTemplate {
    pub credentials: Vec<CredentialDisplay>,
}

#[derive(Template, WebTemplate)]
#[template(path = "components/credential_form.html")]
pub struct CredentialFormTemplate {
    pub credential: Option<CredentialDisplay>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialDisplay {
    pub id: i64,
    pub name: String,
    pub credential_type: String,
    pub credential_type_display: String,
    pub description: Option<String>,
    pub value_preview: String,
    pub username: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

// ============================================================================
// Tags
// ============================================================================

#[derive(Template, WebTemplate)]
#[template(path = "pages/tags.html")]
pub struct TagsTemplate {
    pub user: Option<User>,
    pub tags: Vec<TagDisplay>,
}

#[derive(Template, WebTemplate)]
#[template(path = "components/tag_list.html")]
pub struct TagListTemplate {
    pub tags: Vec<TagDisplay>,
}

#[derive(Template, WebTemplate)]
#[template(path = "components/tag_form.html")]
pub struct TagFormTemplate {
    pub tag: Option<TagDisplay>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagDisplay {
    pub id: i64,
    pub name: String,
    pub color: String,
    pub description: Option<String>,
    pub server_count: i64,
    pub created_at: String,
}

// ============================================================================
// Job Types
// ============================================================================

#[derive(Template, WebTemplate)]
#[template(path = "pages/job_types.html")]
pub struct JobTypesTemplate {
    pub user: Option<User>,
    pub job_types: Vec<JobTypeDisplay>,
}

#[derive(Template, WebTemplate)]
#[template(path = "components/job_type_list.html")]
pub struct JobTypeListTemplate {
    pub job_types: Vec<JobTypeDisplay>,
}

#[derive(Template, WebTemplate)]
#[template(path = "components/job_type_form.html")]
pub struct JobTypeFormTemplate {
    pub job_type: Option<JobTypeDisplay>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobTypeDisplay {
    pub id: i64,
    pub name: String,
    pub display_name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub requires_capabilities: Vec<String>,
    pub enabled: bool,
    pub created_at: String,
}

// ============================================================================
// Command Templates
// ============================================================================

#[derive(Template, WebTemplate)]
#[template(path = "components/command_template_list.html")]
pub struct CommandTemplateListTemplate {
    pub job_type_id: i64,
    pub templates: Vec<CommandTemplateDisplay>,
}

#[derive(Template, WebTemplate)]
#[template(path = "components/command_template_form.html")]
pub struct CommandTemplateFormTemplate {
    pub job_type_id: i64,
    pub template: Option<CommandTemplateDisplay>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandTemplateDisplay {
    pub id: i64,
    pub job_type_id: i64,
    pub name: String,
    pub display_name: String,
    pub description: Option<String>,
    pub command: String,
    pub required_capabilities: Vec<String>,
    pub timeout_seconds: i32,
    pub notify_on_success: bool,
    pub notify_on_failure: bool,
    pub created_at: String,
}

// ============================================================================
// Job Templates
// ============================================================================

#[derive(Template, WebTemplate)]
#[template(path = "pages/job_templates.html")]
pub struct JobTemplatesTemplate {
    pub user: Option<User>,
    pub job_templates: Vec<JobTemplateDisplay>,
}

#[derive(Template, WebTemplate)]
#[template(path = "components/job_template_list.html")]
pub struct JobTemplateListTemplate {
    pub job_templates: Vec<JobTemplateDisplay>,
}

#[derive(Template, WebTemplate)]
#[template(path = "components/job_template_form.html")]
pub struct JobTemplateFormTemplate {
    pub job_template: Option<JobTemplateDisplay>,
    pub job_types: Vec<JobTypeDisplay>,
    pub command_templates: Vec<CommandTemplateDisplay>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobTemplateDisplay {
    pub id: i64,
    pub name: String,
    pub display_name: String,
    pub description: Option<String>,
    pub job_type_id: i64,
    pub job_type_name: String,
    pub is_composite: bool,
    pub command_template_id: Option<i64>,
    pub timeout_seconds: i32,
    pub retry_count: i32,
    pub notify_on_success: bool,
    pub notify_on_failure: bool,
    pub created_at: String,
}

// ============================================================================
// Job Template Steps
// ============================================================================

#[derive(Template, WebTemplate)]
#[template(path = "components/job_template_steps.html")]
pub struct JobTemplateStepsTemplate {
    pub job_template_id: i64,
    pub steps: Vec<JobTemplateStepDisplay>,
}

#[derive(Template, WebTemplate)]
#[template(path = "components/job_template_step_form.html")]
pub struct JobTemplateStepFormTemplate {
    pub job_template_id: i64,
    pub step: Option<JobTemplateStepDisplay>,
    pub command_templates: Vec<CommandTemplateDisplay>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobTemplateStepDisplay {
    pub id: i64,
    pub job_template_id: i64,
    pub step_order: i32,
    pub name: String,
    pub command_template_id: i64,
    pub command_template_name: String,
    pub continue_on_failure: bool,
    pub timeout_seconds: Option<i32>,
}

// ============================================================================
// Job Schedules
// ============================================================================

#[derive(Template, WebTemplate)]
#[template(path = "pages/job_schedules.html")]
pub struct JobSchedulesTemplate {
    pub user: Option<User>,
    pub schedules: Vec<JobScheduleDisplay>,
}

#[derive(Template, WebTemplate)]
#[template(path = "components/job_schedule_list.html")]
pub struct JobScheduleListTemplate {
    pub schedules: Vec<JobScheduleDisplay>,
}

#[derive(Template, WebTemplate)]
#[template(path = "components/job_schedule_form.html")]
pub struct JobScheduleFormTemplate {
    pub schedule: Option<JobScheduleDisplay>,
    pub job_templates: Vec<JobTemplateDisplay>,
    pub servers: Vec<ServerDisplay>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobScheduleDisplay {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub job_template_id: i64,
    pub job_template_name: String,
    pub server_id: i64,
    pub server_name: String,
    pub schedule: String,
    pub enabled: bool,
    pub last_run_at: Option<String>,
    pub last_run_status: Option<String>,
    pub next_run_at: Option<String>,
    pub success_count: i64,
    pub failure_count: i64,
    pub success_rate: f64,
    pub created_at: String,
}

// ============================================================================
// Job Runs
// ============================================================================

#[derive(Template, WebTemplate)]
#[template(path = "pages/job_runs.html")]
pub struct JobRunsTemplate {
    pub user: Option<User>,
    pub runs: Vec<JobRunDisplay>,
}

#[derive(Template, WebTemplate)]
#[template(path = "components/job_run_list.html")]
pub struct JobRunListTemplate {
    pub runs: Vec<JobRunDisplay>,
}

#[derive(Template, WebTemplate)]
#[template(path = "components/job_run_detail.html")]
pub struct JobRunDetailTemplate {
    pub run: JobRunDisplay,
    pub steps: Vec<StepResultDisplay>,
    pub server_results: Vec<ServerJobResultDisplay>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobRunDisplay {
    pub id: i64,
    pub job_schedule_id: i64,
    pub job_schedule_name: String,
    pub job_template_id: i64,
    pub job_template_name: String,
    pub server_id: i64,
    pub server_name: String,
    pub status: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub duration_seconds: Option<f64>,
    pub exit_code: Option<i32>,
    pub output: Option<String>,
    pub error: Option<String>,
    pub retry_attempt: i32,
    pub notification_sent: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepResultDisplay {
    pub id: i64,
    pub step_order: i32,
    pub step_name: String,
    pub status: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub duration_seconds: Option<f64>,
    pub exit_code: Option<i32>,
    pub output: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerJobResultDisplay {
    pub id: i64,
    pub server_id: i64,
    pub server_name: String,
    pub status: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub duration_seconds: Option<f64>,
    pub exit_code: Option<i32>,
    pub output: Option<String>,
    pub error: Option<String>,
}

#[derive(Template, WebTemplate)]
#[template(path = "components/server_job_results.html")]
pub struct ServerJobResultsTemplate {
    pub job_run_id: i64,
    pub server_results: Vec<ServerJobResultDisplay>,
}

// ============================================================================
// Notification Channels
// ============================================================================

#[derive(Template, WebTemplate)]
#[template(path = "pages/notification_channels.html")]
pub struct NotificationChannelsTemplate {
    pub user: Option<User>,
    pub channels: Vec<NotificationChannelDisplay>,
}

#[derive(Template, WebTemplate)]
#[template(path = "components/notification_channel_list.html")]
pub struct NotificationChannelListTemplate {
    pub channels: Vec<NotificationChannelDisplay>,
}

#[derive(Template, WebTemplate)]
#[template(path = "components/notification_channel_form.html")]
pub struct NotificationChannelFormTemplate {
    pub channel: Option<NotificationChannelDisplay>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationChannelDisplay {
    pub id: i64,
    pub name: String,
    pub channel_type: String,
    pub channel_type_display: String,
    pub description: Option<String>,
    pub config_preview: String,
    pub enabled: bool,
    pub created_at: String,
}

// ============================================================================
// Notification Policies
// ============================================================================

#[derive(Template, WebTemplate)]
#[template(path = "pages/notification_policies.html")]
pub struct NotificationPoliciesTemplate {
    pub user: Option<User>,
    pub policies: Vec<NotificationPolicyDisplay>,
}

#[derive(Template, WebTemplate)]
#[template(path = "components/notification_policy_list.html")]
pub struct NotificationPolicyListTemplate {
    pub policies: Vec<NotificationPolicyDisplay>,
}

#[derive(Template, WebTemplate)]
#[template(path = "components/notification_policy_form.html")]
pub struct NotificationPolicyFormTemplate {
    pub policy: Option<NotificationPolicyDisplay>,
    pub channels: Vec<NotificationChannelDisplay>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationPolicyDisplay {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub channel_id: i64,
    pub channel_name: String,
    pub notify_on_success: bool,
    pub notify_on_failure: bool,
    pub notify_on_timeout: bool,
    pub enabled: bool,
    pub created_at: String,
}

// ============================================================================
// Tasks (Legacy - Keep for backward compatibility)
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
    pub servers: Vec<ServerDisplay>,
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
    pub feature_id: String, // Identifies the feature (docker, updates, health, ssh)
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
    pub server_id: String,          // "local" or server ID
    pub feature_id: Option<String>,
    pub command: Option<String>,
    pub remote_command: Option<String>,
    pub schedule: String,
    pub timeout: Option<i32>,
    pub enabled: Option<String>, // checkbox "on" or None
}

// ============================================================================
// Plugins (Legacy - Keep for backward compatibility)
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
// Notifications (Legacy Backend Configuration)
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
