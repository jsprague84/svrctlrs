//! Askama templates for HTMX UI

#![allow(dead_code)]

use askama::Template;
use serde::{Deserialize, Serialize};

// ============================================================================
// Askama Filters
// ============================================================================

pub mod filters {
    use serde::Serialize;

    /// JSON filter for Askama templates
    /// Usage: {{ value|json|safe }}
    pub fn json<T: Serialize>(value: T) -> ::askama::Result<String> {
        serde_json::to_string(&value).map_err(|e| askama::Error::Custom(Box::new(e)))
    }

    /// Length filter for getting the length of a slice/vector
    /// Usage: {{ my_vec|length }}
    pub fn length<T>(value: &[T], _: &dyn askama::Values) -> ::askama::Result<usize> {
        Ok(value.len())
    }
}

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
    pub total_schedules: usize,
    pub active_jobs: usize,
    pub active_tasks: usize,
    pub total_tasks: usize,
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

#[derive(Template)]
#[template(path = "pages/servers.html")]
pub struct ServersTemplate {
    pub user: Option<User>,
    pub servers: Vec<ServerDisplay>,
    pub credentials: Vec<CredentialDisplay>,
    pub tags: Vec<TagDisplay>,
}

#[derive(Template)]
#[template(path = "components/server_list.html")]
pub struct ServerListTemplate {
    pub servers: Vec<ServerDisplay>,
}

#[derive(Template)]
#[template(path = "components/server_form_updated.html")]
pub struct ServerFormTemplate {
    pub server: Option<ServerDisplay>,
    pub credentials: Vec<CredentialDisplay>,
    pub tags: Vec<TagDisplay>,
    pub selected_tags: Vec<i64>, // IDs of tags selected for this server
    pub error: Option<String>,
}

#[derive(Template)]
#[template(path = "components/server_capabilities.html")]
pub struct ServerCapabilitiesTemplate {
    pub server_id: i64,
    pub server: ServerDisplay, // Full server info for display
    pub capabilities: Vec<ServerCapabilityDisplay>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerCapabilityDisplay {
    pub capability: String,
    pub available: bool,
    pub version: Option<String>,
    pub detected_at: String,
}

/// Simple tag info for server display (name + color)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerTagInfo {
    pub name: String,
    pub color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerDisplay {
    pub id: i64,
    pub name: String,
    pub hostname: String, // Empty string if None (display-ready)
    pub host: String,     // Alias for hostname (display-ready)
    pub port: i32,
    pub username: String,    // Empty string if None (display-ready)
    pub description: String, // Empty string if None (display-ready)
    pub credential_id: Option<i64>,
    pub credential_name: String, // Empty string if None (display-ready)
    pub connection_type: String,
    pub connection_string: String, // Empty string if None (display-ready)
    pub is_local: bool,
    pub tag_ids: Vec<i64>,        // For checking if a tag is selected
    pub tags: Vec<ServerTagInfo>, // Tags with colors for display
    pub capabilities: Vec<String>,
    pub os_type: String,         // Empty string if None (display-ready)
    pub os_distro: String,       // Empty string if None (display-ready)
    pub os_version: String,      // Empty string if None (display-ready)
    pub package_manager: String, // Empty string if None (display-ready)
    pub docker_available: bool,
    pub systemd_available: bool,
    pub enabled: bool,
    pub last_seen_at: String, // Empty string if None (display-ready)
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

#[derive(Template)]
#[template(path = "pages/credentials.html")]
pub struct CredentialsTemplate {
    pub user: Option<User>,
    pub credentials: Vec<CredentialDisplay>,
}

#[derive(Template)]
#[template(path = "components/credential_list.html")]
pub struct CredentialListTemplate {
    pub credentials: Vec<CredentialDisplay>,
}

#[derive(Template)]
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
    pub auth_type: String, // Alias for credential_type (for template compatibility)
    pub description: String, // Empty string if None (display-ready)
    pub value_preview: String,
    pub username: String, // Empty string if None (display-ready)
    pub server_count: i64,
    pub created_at: String,
    pub updated_at: String,
}

// ============================================================================
// Tags
// ============================================================================

#[derive(Template)]
#[template(path = "pages/tags.html")]
pub struct TagsTemplate {
    pub user: Option<User>,
    pub tags: Vec<TagDisplay>,
}

#[derive(Template)]
#[template(path = "components/tag_list.html")]
pub struct TagListTemplate {
    pub tags: Vec<TagDisplay>,
}

#[derive(Template)]
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

#[derive(Template)]
#[template(path = "pages/job_types.html")]
pub struct JobTypesTemplate {
    pub user: Option<User>,
    pub job_types: Vec<JobTypeDisplay>,
}

#[derive(Template)]
#[template(path = "components/job_type_list.html")]
pub struct JobTypeListTemplate {
    pub job_types: Vec<JobTypeDisplay>,
}

#[derive(Template)]
#[template(path = "components/job_type_form.html")]
pub struct JobTypeFormTemplate {
    pub job_type: Option<JobTypeDisplay>,
    pub error: Option<String>,
}

#[derive(Template)]
#[template(path = "components/job_type_view.html")]
pub struct JobTypeViewTemplate {
    pub job_type: JobTypeDisplay,
    pub command_templates: Vec<CommandTemplateDisplay>,
}

#[derive(Debug, Clone)]
pub struct JobTypeDisplay {
    pub id: i64,
    pub name: String,
    pub display_name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub execution_type: String, // "local", "remote", "composite"
    pub required_capabilities: Vec<String>,
    pub command_template_count: i64,
    pub job_template_count: i64,
    pub enabled: bool,
    pub created_at: String,
    pub metadata_json: String, // Pre-serialized JSON for templates
}

impl JobTypeDisplay {
    pub fn get_requires_capabilities(&self) -> Vec<String> {
        self.required_capabilities.clone()
    }

    pub fn get_metadata(&self) -> String {
        self.metadata_json.clone()
    }
}

/// Convert database JobType to display model
impl From<svrctlrs_database::models::JobType> for JobTypeDisplay {
    fn from(jt: svrctlrs_database::models::JobType) -> Self {
        use chrono::Local;

        // Extract computed values first before moving fields
        let required_capabilities = jt.get_requires_capabilities();
        let metadata_json =
            serde_json::to_string(&jt.get_metadata()).unwrap_or_else(|_| "{}".to_string());
        let created_at = jt
            .created_at
            .with_timezone(&Local)
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();

        Self {
            id: jt.id,
            name: jt.name,
            display_name: jt.display_name,
            description: jt.description,
            icon: jt.icon,
            color: jt.color,
            execution_type: "local".to_string(), // TODO: Add to schema or derive from metadata
            required_capabilities,
            command_template_count: 0, // Will be set by query join
            job_template_count: 0,     // Will be set by query join
            enabled: jt.enabled,
            created_at,
            metadata_json,
        }
    }
}

// ============================================================================
// Command Templates
// ============================================================================

#[derive(Template)]
#[template(path = "components/command_template_list.html")]
pub struct CommandTemplateListTemplate {
    pub job_type_id: i64,
    pub command_templates: Vec<CommandTemplateDisplay>,
}

#[derive(Template)]
#[template(path = "components/command_template_form.html")]
pub struct CommandTemplateFormTemplate {
    pub job_type_id: i64,
    pub command_template: Option<CommandTemplateDisplay>,
    pub error: Option<String>,
}

#[derive(Template)]
#[template(path = "components/command_template_test.html")]
pub struct CommandTemplateTestTemplate {
    pub job_type_id: i64,
    pub template: CommandTemplateDisplay,
    pub param_schema: Vec<ParameterDisplay>,
}

#[derive(Template)]
#[template(path = "components/command_template_test_result.html")]
pub struct CommandTemplateTestResultTemplate {
    pub validation: ValidationResult,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub rendered_command: Option<String>,
    pub errors: Vec<String>,
    pub warnings: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct CommandTemplateDisplay {
    pub id: i64,
    pub job_type_id: i64,
    pub name: String,
    pub display_name: String,
    pub description: Option<String>,
    pub command: String,
    pub required_capabilities: Vec<String>,
    pub os_filter_json: String, // Pre-serialized JSON
    pub timeout_seconds: i32,
    pub working_directory: Option<String>,
    pub environment_json: String, // Pre-serialized JSON
    pub output_format: Option<String>,
    pub parse_output: bool,
    pub output_parser_json: String, // Pre-serialized JSON
    pub notify_on_success: bool,
    pub notify_on_failure: bool,
    pub parameter_schema_json: String, // Pre-serialized JSON
    pub metadata_json: String,         // Pre-serialized JSON
    pub created_at: String,
}

impl CommandTemplateDisplay {
    pub fn get_required_capabilities(&self) -> Vec<String> {
        self.required_capabilities.clone()
    }

    pub fn get_os_filter(&self) -> String {
        self.os_filter_json.clone()
    }

    pub fn get_environment(&self) -> String {
        self.environment_json.clone()
    }

    pub fn get_output_parser(&self) -> String {
        self.output_parser_json.clone()
    }

    pub fn get_parameter_schema(&self) -> String {
        self.parameter_schema_json.clone()
    }

    pub fn get_metadata(&self) -> String {
        self.metadata_json.clone()
    }
}

/// Convert database CommandTemplate to display model
impl From<svrctlrs_database::models::CommandTemplate> for CommandTemplateDisplay {
    fn from(ct: svrctlrs_database::models::CommandTemplate) -> Self {
        use chrono::Local;

        // Extract and serialize computed values first before moving fields
        let required_capabilities = ct.get_required_capabilities();
        let os_filter_json =
            serde_json::to_string(&ct.get_os_filter()).unwrap_or_else(|_| "{}".to_string());
        let environment_json =
            serde_json::to_string(&ct.get_environment()).unwrap_or_else(|_| "{}".to_string());
        let output_parser_json =
            serde_json::to_string(&ct.get_output_parser()).unwrap_or_else(|_| "{}".to_string());
        let parameter_schema_json =
            serde_json::to_string(&ct.get_parameter_schema()).unwrap_or_else(|_| "[]".to_string());
        let metadata_json =
            serde_json::to_string(&ct.get_metadata()).unwrap_or_else(|_| "{}".to_string());
        let created_at = ct
            .created_at
            .with_timezone(&Local)
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();

        Self {
            id: ct.id,
            job_type_id: ct.job_type_id,
            name: ct.name,
            display_name: ct.display_name,
            description: ct.description,
            command: ct.command,
            required_capabilities,
            os_filter_json,
            timeout_seconds: ct.timeout_seconds,
            working_directory: ct.working_directory,
            environment_json,
            output_format: ct.output_format,
            parse_output: ct.parse_output,
            output_parser_json,
            notify_on_success: ct.notify_on_success,
            notify_on_failure: ct.notify_on_failure,
            parameter_schema_json,
            metadata_json,
            created_at,
        }
    }
}

/// Template for rendering command template parameter form fields
#[derive(Template)]
#[template(path = "components/command_template_parameters.html")]
pub struct CommandTemplateParametersTemplate {
    pub parameters: Vec<ParameterDisplay>,
}

/// Display model for a single parameter from parameter schema
#[derive(Debug, Clone)]
pub struct ParameterDisplay {
    pub name: String,
    pub param_type: String, // "string", "number", "boolean", "select"
    pub required: bool,
    pub description: Option<String>,
    pub default: Option<String>,
    pub options: Option<Vec<String>>, // For select type
}

impl ParameterDisplay {
    pub fn from_json(
        value: &serde_json::Value,
        existing_vars: &std::collections::HashMap<String, String>,
    ) -> Option<Self> {
        let name = value.get("name")?.as_str()?.to_string();
        let param_type = value.get("type")?.as_str()?.to_string();
        let required = value
            .get("required")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let description = value
            .get("description")
            .and_then(|v| v.as_str())
            .map(String::from);

        // Get default from schema
        let schema_default = value.get("default").and_then(|v| {
            if v.is_string() {
                v.as_str().map(String::from)
            } else if v.is_number() {
                Some(v.to_string())
            } else if v.is_boolean() {
                Some(v.as_bool().unwrap().to_string())
            } else {
                None
            }
        });

        // Use existing value if available, otherwise fall back to schema default
        let default = existing_vars
            .get(&name)
            .map(String::from)
            .or(schema_default);

        let options = value.get("options").and_then(|v| {
            v.as_array().map(|arr| {
                arr.iter()
                    .filter_map(|item| item.as_str().map(String::from))
                    .collect()
            })
        });

        Some(Self {
            name,
            param_type,
            required,
            description,
            default,
            options,
        })
    }
}

// ============================================================================
// Job Templates
// ============================================================================

#[derive(Template)]
#[template(path = "pages/job_templates.html")]
pub struct JobTemplatesTemplate {
    pub user: Option<User>,
    pub job_templates: Vec<JobTemplateDisplay>,
    pub job_types: Vec<JobTypeDisplay>,
}

#[derive(Template)]
#[template(path = "components/job_template_list.html")]
pub struct JobTemplateListTemplate {
    pub job_templates: Vec<JobTemplateDisplay>,
}

#[derive(Template)]
#[template(path = "components/job_template_form.html")]
pub struct JobTemplateFormTemplate {
    pub job_template: Option<JobTemplateDisplay>,
    pub job_types: Vec<JobTypeDisplay>,
    pub command_templates: Vec<CommandTemplateDisplay>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct JobTemplateDisplay {
    pub id: i64,
    pub name: String,
    pub display_name: String,
    pub description: Option<String>,
    pub job_type_id: i64,
    pub job_type_name: String,
    pub is_composite: bool,
    pub command_template_id: Option<i64>,
    pub variables_json: String,
    pub timeout_seconds: i32,
    pub retry_count: i32,
    pub retry_delay_seconds: i32,
    pub notify_on_success: bool,
    pub notify_on_failure: bool,
    pub notification_policy_id: Option<i64>,
    // Display-only computed fields (not in DB)
    pub step_count: i64,
    pub schedule_count: i64,
    pub metadata_json: String,
    pub created_at: String,
    pub updated_at: String,
}

impl JobTemplateDisplay {
    pub fn get_variables(&self) -> String {
        self.variables_json.clone()
    }

    pub fn get_metadata(&self) -> String {
        self.metadata_json.clone()
    }
}

// Optimized From implementation using joined data
impl From<svrctlrs_database::queries::JobTemplateWithNames> for JobTemplateDisplay {
    fn from(jt: svrctlrs_database::queries::JobTemplateWithNames) -> Self {
        use chrono::Local;

        let variables_json = jt
            .variables
            .as_ref()
            .map(|v| v.to_string())
            .unwrap_or_else(|| "{}".to_string());
        let metadata_json = jt
            .metadata
            .as_ref()
            .map(|m| m.to_string())
            .unwrap_or_else(|| "{}".to_string());
        let created_at = jt
            .created_at
            .with_timezone(&Local)
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        let updated_at = jt
            .updated_at
            .with_timezone(&Local)
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();

        Self {
            id: jt.id,
            name: jt.name,
            display_name: jt.display_name,
            description: jt.description,
            job_type_id: jt.job_type_id,
            job_type_name: jt.job_type_name, // ✅ From JOIN
            is_composite: jt.is_composite,
            command_template_id: jt.command_template_id,
            variables_json,
            timeout_seconds: jt.timeout_seconds,
            retry_count: jt.retry_count,
            retry_delay_seconds: jt.retry_delay_seconds,
            notify_on_success: jt.notify_on_success,
            notify_on_failure: jt.notify_on_failure,
            notification_policy_id: jt.notification_policy_id,
            step_count: 0,     // TODO: Count from job_template_steps table
            schedule_count: 0, // TODO: Count from job_schedules table
            metadata_json,
            created_at,
            updated_at,
        }
    }
}

// Optimized From implementation with counts from database query
impl From<svrctlrs_database::queries::JobTemplateWithCounts> for JobTemplateDisplay {
    fn from(jt: svrctlrs_database::queries::JobTemplateWithCounts) -> Self {
        use chrono::Local;

        let variables_json = jt
            .variables
            .as_ref()
            .map(|v| v.to_string())
            .unwrap_or_else(|| "{}".to_string());
        let metadata_json = jt
            .metadata
            .as_ref()
            .map(|m| m.to_string())
            .unwrap_or_else(|| "{}".to_string());
        let created_at = jt
            .created_at
            .with_timezone(&Local)
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        let updated_at = jt
            .updated_at
            .with_timezone(&Local)
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();

        Self {
            id: jt.id,
            name: jt.name,
            display_name: jt.display_name,
            description: jt.description,
            job_type_id: jt.job_type_id,
            job_type_name: jt.job_type_name, // ✅ From JOIN
            is_composite: jt.is_composite,
            command_template_id: jt.command_template_id,
            variables_json,
            timeout_seconds: jt.timeout_seconds,
            retry_count: jt.retry_count,
            retry_delay_seconds: jt.retry_delay_seconds,
            notify_on_success: jt.notify_on_success,
            notify_on_failure: jt.notify_on_failure,
            notification_policy_id: jt.notification_policy_id,
            step_count: jt.step_count,         // ✅ From database COUNT
            schedule_count: jt.schedule_count, // ✅ From database COUNT
            metadata_json,
            created_at,
            updated_at,
        }
    }
}

// ============================================================================
// Job Template Steps
// ============================================================================

#[derive(Template)]
#[template(path = "components/job_template_steps.html")]
pub struct JobTemplateStepsTemplate {
    pub job_template_id: i64,
    pub steps: Vec<JobTemplateStepDisplay>,
}

#[derive(Template)]
#[template(path = "components/job_template_step_form.html")]
pub struct JobTemplateStepFormTemplate {
    pub job_template_id: i64,
    pub step: Option<JobTemplateStepDisplay>,
    pub command_templates: Vec<CommandTemplateDisplay>,
    pub job_types: Vec<JobTypeDisplay>, // For filtering command templates by type
    pub next_order_index: i32,          // For new steps
    pub error: Option<String>,
}

#[derive(Template)]
#[template(path = "components/job_template_steps.html")]
pub struct JobTemplateStepListTemplate {
    pub job_template_id: i64,
    pub steps: Vec<JobTemplateStepDisplay>,
}

#[derive(Debug, Clone)]
pub struct JobTemplateStepDisplay {
    pub id: i64,
    pub job_template_id: i64,
    pub step_order: i32,
    pub order_index: i32, // Alias for step_order (for template compatibility)
    pub name: String,
    pub description: String, // Alias for name (for template compatibility)
    pub command_template_id: i64,
    pub command_template_name: String,
    pub job_type_id: Option<i64>,      // From command_template JOIN
    pub job_type_name: Option<String>, // From command_template JOIN
    pub variables_json: String,
    pub continue_on_failure: bool,
    pub timeout_seconds: Option<i32>,
    pub metadata_json: String,
}

impl JobTemplateStepDisplay {
    pub fn get_variables(&self) -> String {
        self.variables_json.clone()
    }

    pub fn get_metadata(&self) -> String {
        self.metadata_json.clone()
    }
}

/// Optimized From implementation with joined names from database query
impl From<svrctlrs_database::queries::JobTemplateStepWithNames> for JobTemplateStepDisplay {
    fn from(step: svrctlrs_database::queries::JobTemplateStepWithNames) -> Self {
        let variables_json = step
            .variables
            .as_ref()
            .and_then(|v| serde_json::to_string(v).ok())
            .unwrap_or_else(|| "{}".to_string());
        let metadata_json = step
            .metadata
            .as_ref()
            .and_then(|m| serde_json::to_string(m).ok())
            .unwrap_or_else(|| "{}".to_string());

        Self {
            id: step.id,
            job_template_id: step.job_template_id,
            step_order: step.step_order,
            order_index: step.step_order, // Alias for template compatibility
            name: step.name.clone(),
            description: step.name, // Alias for template compatibility
            command_template_id: step.command_template_id,
            command_template_name: step.command_template_name, // ✅ From database JOIN
            job_type_id: step.job_type_id,                     // ✅ From database JOIN
            job_type_name: step.job_type_name,                 // ✅ From database JOIN
            variables_json,
            continue_on_failure: step.continue_on_failure,
            timeout_seconds: step.timeout_seconds,
            metadata_json,
        }
    }
}

// ============================================================================
// Job Schedules
// ============================================================================

#[derive(Template)]
#[template(path = "pages/job_schedules.html")]
pub struct JobSchedulesTemplate {
    pub user: Option<User>,
    pub schedules: Vec<JobScheduleDisplay>,
    pub schedule_groups: Vec<ServerScheduleGroup>, // Grouped by server
    pub job_templates: Vec<JobTemplateDisplay>,
    pub servers: Vec<ServerDisplay>,
}

#[derive(Template)]
#[template(path = "components/job_schedule_list.html")]
pub struct JobScheduleListTemplate {
    pub schedules: Vec<JobScheduleDisplay>,
    pub schedule_groups: Vec<ServerScheduleGroup>, // Grouped by server
}

#[derive(Template)]
#[template(path = "components/job_schedule_form.html")]
pub struct JobScheduleFormTemplate {
    pub schedule: Option<JobScheduleDisplay>, // Template uses "schedule" not "job_schedule"
    pub job_templates: Vec<JobTemplateDisplay>,
    pub servers: Vec<ServerDisplay>,
    pub error: Option<String>,
}

#[derive(Template)]
#[template(path = "components/grouped_schedules.html")]
pub struct GroupedSchedulesTemplate {
    pub grouped_schedules: Vec<ServerScheduleGroup>,
    pub groups: Vec<ServerScheduleGroup>, // Alias for template compatibility
}

#[derive(Debug, Clone)]
pub struct ServerScheduleGroup {
    pub server_id: Option<i64>,
    pub server_name: Option<String>,
    pub schedules: Vec<JobScheduleDisplay>,
}

#[derive(Debug, Clone)]
pub struct JobScheduleDisplay {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub job_template_id: i64,
    pub job_template_name: String,
    pub server_id: Option<i64>,
    pub server_name: Option<String>,
    pub schedule: String,
    pub cron_expression: String, // Alias for schedule (template compatibility)
    pub enabled: bool,
    pub timeout_seconds: Option<i32>,
    pub retry_count: Option<i32>,
    pub notify_on_success: Option<bool>,
    pub notify_on_failure: Option<bool>,
    pub notification_policy_id: Option<i64>,
    pub last_run_at: Option<String>,
    pub last_run: Option<String>, // Alias for last_run_at
    pub last_run_status: Option<String>,
    pub next_run_at: Option<String>,
    pub next_run: Option<String>, // Alias for next_run_at
    pub success_count: i64,
    pub failure_count: i64,
    pub metadata_json: String,
    pub created_at: String,
}

impl JobScheduleDisplay {
    pub fn get_metadata(&self) -> String {
        self.metadata_json.clone()
    }

    pub fn success_rate(&self) -> f64 {
        let total = self.success_count + self.failure_count;
        if total == 0 {
            0.0
        } else {
            (self.success_count as f64 / total as f64) * 100.0
        }
    }
}

// ============================================================================
// Job Runs
// ============================================================================

#[derive(Template)]
#[template(path = "pages/job_runs.html")]
pub struct JobRunsTemplate {
    pub user: Option<User>,
    pub job_runs: Vec<JobRunDisplay>,
    pub current_page: usize,
    pub total_pages: usize,
    pub per_page: usize,
}

#[derive(Template)]
#[template(path = "components/job_run_list.html")]
pub struct JobRunListTemplate {
    pub job_runs: Vec<JobRunDisplay>,
    pub current_page: usize,
    pub total_pages: usize,
    pub per_page: usize,
}

#[derive(Template)]
#[template(path = "pages/job_run_detail.html")]
pub struct JobRunDetailTemplate {
    pub user: Option<User>,
    pub job_run: JobRunDisplay,
    pub server_results: Vec<ServerJobResultDisplay>,
    pub results: Vec<ServerJobResultDisplay>, // Alias for template compatibility
    pub step_results: Vec<StepExecutionResultDisplay>,
    pub servers: Vec<ServerDisplay>,
}

#[derive(Debug, Clone)]
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
    pub completed_at: Option<String>, // Alias for finished_at
    pub duration_ms: Option<i64>,
    pub duration_seconds: f64, // Computed field (not method)
    pub exit_code: Option<i32>,
    pub output: Option<String>,
    pub error: Option<String>,
    pub rendered_command: Option<String>,
    pub retry_attempt: i32,
    pub is_retry: bool,
    pub notification_sent: bool,
    pub notification_error: Option<String>,
    pub metadata_json: String,
    // Display-only fields
    pub trigger_type: String, // "scheduled", "manual", "retry"
    pub total_servers: i64,   // For multi-server jobs
    pub success_count: i64,   // Successful server results
    pub failed_count: i64,    // Failed server results
}

impl JobRunDisplay {
    pub fn get_metadata(&self) -> String {
        self.metadata_json.clone()
    }

    pub fn formatted_duration(&self) -> String {
        match self.duration_ms {
            Some(ms) => {
                let secs = ms / 1000;
                let mins = secs / 60;
                let hours = mins / 60;
                if hours > 0 {
                    format!("{}h {}m {}s", hours, mins % 60, secs % 60)
                } else if mins > 0 {
                    format!("{}m {}s", mins, secs % 60)
                } else {
                    format!("{}s", secs)
                }
            }
            None => "".to_string(),
        }
    }
}

/// Convert database JobRunWithNames (from JOIN query) to display model
impl From<svrctlrs_database::queries::JobRunWithNames> for JobRunDisplay {
    fn from(jr: svrctlrs_database::queries::JobRunWithNames) -> Self {
        use chrono::Local;

        let metadata_json = jr.metadata.unwrap_or_else(|| "{}".to_string());
        let started_at = jr
            .started_at
            .with_timezone(&Local)
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        let finished_at = jr.finished_at.map(|dt| {
            dt.with_timezone(&Local)
                .format("%Y-%m-%d %H:%M:%S")
                .to_string()
        });
        let duration_seconds = jr.duration_ms.map(|ms| ms as f64 / 1000.0).unwrap_or(0.0);
        let trigger_type = if jr.is_retry {
            "retry"
        } else if jr.job_schedule_id.is_none() {
            "manual"
        } else {
            "scheduled"
        };

        Self {
            id: jr.id,
            job_schedule_id: jr.job_schedule_id.unwrap_or(0),
            job_schedule_name: jr.job_schedule_name.unwrap_or_else(|| "Manual".to_string()),
            job_template_id: jr.job_template_id,
            job_template_name: jr.job_template_name,
            server_id: jr.server_id.unwrap_or(0),
            server_name: jr.server_name.unwrap_or_else(|| "Local".to_string()),
            status: jr.status.clone(),
            started_at,
            finished_at: finished_at.clone(),
            completed_at: finished_at, // Alias
            duration_ms: jr.duration_ms,
            duration_seconds,
            exit_code: jr.exit_code,
            output: jr.output,
            error: jr.error,
            rendered_command: jr.rendered_command,
            retry_attempt: jr.retry_attempt,
            is_retry: jr.is_retry,
            notification_sent: jr.notification_sent,
            notification_error: jr.notification_error,
            metadata_json,
            trigger_type: trigger_type.to_string(),
            total_servers: 1, // Single server job
            success_count: if jr.status == "success" { 1 } else { 0 },
            failed_count: if jr.status == "failed" { 1 } else { 0 },
        }
    }
}

#[derive(Debug, Clone)]
pub struct ServerJobResultDisplay {
    pub id: i64,
    pub job_run_id: i64,
    pub server_id: i64,
    pub server_name: String,
    pub status: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub completed_at: Option<String>, // Alias for finished_at
    pub duration_ms: Option<i64>,
    pub duration_seconds: f64, // Computed field
    pub exit_code: Option<i32>,
    pub output: Option<String>,
    pub stdout: Option<String>, // Alias for output
    pub error: Option<String>,
    pub stderr: Option<String>,        // Alias for error
    pub error_message: Option<String>, // Alias for error
    pub metadata_json: String,
    pub timestamp: String, // Alias for started_at
}

impl ServerJobResultDisplay {
    pub fn duration_seconds(&self) -> f64 {
        self.duration_ms.map(|ms| ms as f64 / 1000.0).unwrap_or(0.0)
    }
}

#[derive(Template)]
#[template(path = "components/server_job_results.html")]
pub struct ServerJobResultsTemplate {
    pub job_run_id: i64,
    pub server_results: Vec<ServerJobResultDisplay>,
    pub results: Vec<ServerJobResultDisplay>, // Alias for template compatibility
    pub servers: Vec<ServerDisplay>,
}

#[derive(Template)]
#[template(path = "components/server_job_result_detail.html")]
pub struct ServerJobResultDetailTemplate {
    pub result: ServerJobResultDisplay,
}

// Step Execution Results (for composite jobs)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepExecutionResultDisplay {
    pub id: i64,
    pub job_run_id: i64,
    pub step_order: i32,
    pub step_name: String,
    pub command_template_id: i64,
    pub status: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub duration_ms: Option<i64>,
    pub duration_seconds: f64,
    pub exit_code: Option<i32>,
    pub output: Option<String>,
    pub error: Option<String>,
    pub metadata_json: String,
}

impl StepExecutionResultDisplay {
    pub fn duration_seconds(&self) -> f64 {
        self.duration_ms.map(|ms| ms as f64 / 1000.0).unwrap_or(0.0)
    }
}

// ============================================================================
// Notification Channels
// ============================================================================

#[derive(Template)]
#[template(path = "pages/notification_channels.html")]
pub struct NotificationChannelsTemplate {
    pub user: Option<User>,
    pub channels: Vec<NotificationChannelDisplay>,
}

#[derive(Template)]
#[template(path = "components/notification_channel_list.html")]
pub struct NotificationChannelListTemplate {
    pub channels: Vec<NotificationChannelDisplay>,
}

#[derive(Template)]
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
    pub endpoint: String,    // Empty string if None (display-ready)
    pub description: String, // Empty string if None (display-ready)
    pub config_preview: String,
    pub enabled: bool,
    pub created_at: String,
}

// ============================================================================
// Notification Policies (COMMENTED OUT - Template errors)
// ============================================================================

#[derive(Template)]
#[template(path = "pages/notification_policies.html")]
pub struct NotificationPoliciesTemplate {
    pub user: Option<User>,
    pub policies: Vec<NotificationPolicyDisplay>,
    pub channels: Vec<NotificationChannelDisplay>,
}

#[derive(Template)]
#[template(path = "components/notification_policy_list.html")]
pub struct NotificationPolicyListTemplate {
    pub policies: Vec<NotificationPolicyDisplay>,
    pub channels: Vec<NotificationChannelDisplay>,
}

#[derive(Template)]
#[template(path = "components/notification_policy_form.html")]
pub struct NotificationPolicyFormTemplate {
    pub policy: Option<NotificationPolicyDisplay>,
    pub channels: Vec<NotificationChannelDisplay>,
    pub job_types: Vec<JobTypeDisplay>, // For scoping policy to job type
    pub job_templates: Vec<JobTemplateDisplay>, // For scoping policy to job template
    pub servers: Vec<ServerDisplay>,    // For server filtering
    pub tags: Vec<TagDisplay>,          // For tag filtering
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyChannelAssignment {
    pub channel_id: i64,
    pub channel_name: String,
    pub priority_override: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationPolicyDisplay {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub scope_type: String, // "global", "job_type", "job_template", "job_schedule"
    pub channel_id: i64,
    pub channel_name: String,
    pub on_success: bool,
    pub on_failure: bool,
    pub on_timeout: bool,
    pub enabled: bool,
    pub title_template: Option<String>,
    pub body_template: Option<String>,
    pub created_at: String,
    // Scope-specific fields (populated based on scope_type)
    pub job_type_id: Option<i64>,
    pub job_type_name: Option<String>,
    pub job_template_id: Option<i64>,
    pub job_templates: Vec<String>, // Template names if scoped to multiple
    pub job_template_count: i64,    // Count of templates using this policy
    // Filtering fields
    pub job_type_filter: Option<Vec<String>>,
    pub server_filter: Option<Vec<i64>>,
    pub tag_filter: Option<Vec<String>>,
    // Throttling fields
    pub min_severity: Option<i32>,
    pub max_per_hour: Option<i32>,
    // Multi-channel support
    pub policy_channels: Vec<PolicyChannelAssignment>,
}

// ============================================================================
// Notification Log (Audit Trail)
// ============================================================================

#[derive(Template)]
#[template(path = "pages/notification_log.html")]
pub struct NotificationLogPageTemplate {
    pub user: Option<User>,
    pub logs: Vec<NotificationLogDisplay>,
    pub channels: Vec<NotificationChannelDisplay>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationLogDisplay {
    pub id: i64,
    pub channel_id: i64,
    pub channel_name: String,
    pub policy_id: Option<i64>,
    pub policy_name: Option<String>,
    pub job_run_id: Option<i64>,
    pub title: String,
    pub body: Option<String>,
    pub priority: i32,
    pub success: bool,
    pub error_message: Option<String>,
    pub retry_count: i32,
    pub sent_at: String,
}

// ============================================================================
// Settings
// ============================================================================

#[derive(Template)]
#[template(path = "pages/settings.html")]
pub struct SettingsTemplate {
    pub user: Option<User>,
}

#[derive(Template)]
#[template(path = "pages/settings_general.html")]
pub struct GeneralSettingsTemplate {
    pub user: Option<User>,
    pub settings: Vec<SettingDisplay>,
}

#[derive(Template)]
#[template(path = "components/settings_list.html")]
pub struct SettingsListTemplate {
    pub settings: Vec<SettingDisplay>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingDisplay {
    pub key: String,
    pub value: String,
    pub value_type: String,
    pub description: Option<String>,
    pub updated_at: String,
}

// ============================================================================
// Notifications (Legacy Backend Configuration - COMMENTED OUT)
// ============================================================================

// Legacy notification templates (using old naming)
#[derive(Template)]
#[template(path = "pages/notifications.html")]
pub struct NotificationsTemplate {
    pub user: Option<User>,
    pub notifications: Vec<NotificationChannelDisplay>,
}

#[derive(Template)]
#[template(path = "components/notification_list.html")]
pub struct NotificationListTemplate {
    pub notifications: Vec<NotificationChannelDisplay>,
}

#[derive(Template)]
#[template(path = "components/notification_form.html")]
pub struct NotificationFormTemplate {
    pub notification: Option<NotificationChannelDisplay>,
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

#[derive(Template)]
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

/// Convert database ServerWithDetails to display model (optimized with JOINed data)
impl From<svrctlrs_database::ServerWithDetails> for ServerDisplay {
    fn from(swd: svrctlrs_database::ServerWithDetails) -> Self {
        use chrono::Local;

        let s = swd.server;

        let last_seen = s
            .last_seen_at
            .map(|dt| {
                dt.with_timezone(&Local)
                    .format("%Y-%m-%d %H:%M:%S")
                    .to_string()
            })
            .unwrap_or_default();

        let created = s
            .created_at
            .with_timezone(&Local)
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();

        let hostname_str = s.hostname.clone().unwrap_or_default();
        let connection_type = if s.is_local {
            "Local".to_string()
        } else {
            "SSH".to_string()
        };

        let connection_string = if s.is_local {
            "localhost".to_string()
        } else {
            format!(
                "{}@{}:{}",
                s.username.as_ref().unwrap_or(&String::new()),
                hostname_str,
                s.port
            )
        };

        // Extract tag IDs and names from joined Tag models
        let tag_ids: Vec<i64> = swd.tags.iter().map(|t| t.id).collect();
        let tags: Vec<ServerTagInfo> = swd
            .tags
            .iter()
            .map(|t| ServerTagInfo {
                name: t.name.clone(),
                color: t.color_or_default(),
            })
            .collect();

        // Extract capability names from joined ServerCapability models
        let capabilities: Vec<String> = swd
            .capabilities
            .iter()
            .filter(|c| c.available)
            .map(|c| c.capability.clone())
            .collect();

        Self {
            id: s.id,
            name: s.name,
            hostname: hostname_str.clone(),
            host: hostname_str, // Alias
            port: s.port,
            username: s.username.unwrap_or_default(),
            description: s.description.unwrap_or_default(),
            credential_id: s.credential_id,
            credential_name: swd.credential_name.unwrap_or_default(),
            connection_type,
            connection_string,
            is_local: s.is_local,
            tag_ids,
            tags,
            capabilities,
            os_type: s.os_type.unwrap_or_default(),
            os_distro: s.os_distro.unwrap_or_default(),
            os_version: String::new(), // Not in current schema
            package_manager: s.package_manager.unwrap_or_default(),
            docker_available: s.docker_available,
            systemd_available: s.systemd_available,
            enabled: s.enabled,
            last_seen_at: last_seen,
            created_at: created,
        }
    }
}

/// Convert database TagWithCount to display model (optimized with server count)
impl From<svrctlrs_database::TagWithCount> for TagDisplay {
    fn from(twc: svrctlrs_database::TagWithCount) -> Self {
        use chrono::Local;

        let created = twc
            .tag
            .created_at
            .with_timezone(&Local)
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();

        Self {
            id: twc.tag.id,
            name: twc.tag.name,
            color: twc.tag.color.unwrap_or_else(|| "#6B7280".to_string()), // Default gray color
            description: twc.tag.description,
            server_count: twc.server_count,
            created_at: created,
        }
    }
}

/// Convert database JobScheduleWithNames to display model
impl From<svrctlrs_database::queries::JobScheduleWithNames> for JobScheduleDisplay {
    fn from(js: svrctlrs_database::queries::JobScheduleWithNames) -> Self {
        use chrono::Local;

        let last_run = js.last_run_at.map(|dt| {
            dt.with_timezone(&Local)
                .format("%Y-%m-%d %H:%M:%S")
                .to_string()
        });

        let next_run = js.next_run_at.map(|dt| {
            dt.with_timezone(&Local)
                .format("%Y-%m-%d %H:%M:%S")
                .to_string()
        });

        let created = js
            .created_at
            .with_timezone(&Local)
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();

        let metadata_json = js
            .metadata
            .as_ref()
            .and_then(|m| serde_json::to_string(m).ok())
            .unwrap_or_else(|| "{}".to_string());

        Self {
            id: js.id,
            name: js.name,
            description: js.description,
            job_template_id: js.job_template_id,
            job_template_name: js.job_template_name,
            server_id: js.server_id,
            server_name: js.server_name,
            schedule: js.schedule.clone(),
            cron_expression: js.schedule, // Alias
            enabled: js.enabled,
            timeout_seconds: js.timeout_seconds,
            retry_count: js.retry_count,
            notify_on_success: Some(js.notify_on_success),
            notify_on_failure: Some(js.notify_on_failure),
            notification_policy_id: js.notification_policy_id,
            last_run_at: last_run.clone(),
            last_run, // Alias
            last_run_status: js.last_run_status,
            next_run_at: next_run.clone(),
            next_run, // Alias
            success_count: js.success_count,
            failure_count: js.failure_count,
            metadata_json,
            created_at: created,
        }
    }
}

/// Convert database ServerJobResult to display model
impl From<svrctlrs_database::ServerJobResult> for ServerJobResultDisplay {
    fn from(r: svrctlrs_database::ServerJobResult) -> Self {
        use chrono::Local;

        let started = r
            .started_at
            .with_timezone(&Local)
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();

        let finished = r.finished_at.map(|dt| {
            dt.with_timezone(&Local)
                .format("%Y-%m-%d %H:%M:%S")
                .to_string()
        });

        let completed = finished.clone();

        let duration_seconds = r.duration_ms.map(|ms| ms as f64 / 1000.0).unwrap_or(0.0);

        let metadata_json = r
            .metadata
            .as_ref()
            .and_then(|m| serde_json::to_string(m).ok())
            .unwrap_or_else(|| "{}".to_string());

        Self {
            id: r.id,
            job_run_id: r.job_run_id,
            server_id: r.server_id,
            server_name: String::new(), // TODO: Join from servers table
            status: r.status_str.clone(),
            started_at: started.clone(),
            finished_at: finished.clone(),
            completed_at: completed, // Alias
            duration_ms: r.duration_ms,
            duration_seconds,
            exit_code: r.exit_code,
            output: r.output.clone(),
            stdout: r.output, // Alias
            error: r.error.clone(),
            stderr: r.error.clone(), // Alias
            error_message: r.error,
            metadata_json,
            timestamp: started.clone(), // Alias for started_at
        }
    }
}

/// Convert database StepExecutionResult to display model
impl From<svrctlrs_database::StepExecutionResult> for StepExecutionResultDisplay {
    fn from(s: svrctlrs_database::StepExecutionResult) -> Self {
        use chrono::Local;

        let started = s
            .started_at
            .with_timezone(&Local)
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();

        let finished = s.finished_at.map(|dt| {
            dt.with_timezone(&Local)
                .format("%Y-%m-%d %H:%M:%S")
                .to_string()
        });

        let duration_seconds = s.duration_ms.map(|ms| ms as f64 / 1000.0).unwrap_or(0.0);

        let metadata_json = s
            .metadata
            .as_ref()
            .and_then(|m| serde_json::to_string(m).ok())
            .unwrap_or_else(|| "{}".to_string());

        Self {
            id: s.id,
            job_run_id: s.job_run_id,
            step_order: s.step_order,
            step_name: s.step_name,
            command_template_id: s.command_template_id,
            status: s.status_str,
            started_at: started,
            finished_at: finished,
            duration_ms: s.duration_ms,
            duration_seconds,
            exit_code: s.exit_code,
            output: s.output,
            error: s.error,
            metadata_json,
        }
    }
}

/// Convert database NotificationChannel to display model
impl From<svrctlrs_database::NotificationChannel> for NotificationChannelDisplay {
    fn from(nc: svrctlrs_database::NotificationChannel) -> Self {
        use chrono::Local;

        // Parse config to extract endpoint
        let config_json: serde_json::Value =
            serde_json::from_str(&nc.config).unwrap_or(serde_json::json!({}));

        let endpoint = config_json
            .get("url")
            .or_else(|| config_json.get("endpoint"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let channel_type_display = match nc.channel_type_str.as_str() {
            "gotify" => "Gotify",
            "ntfy" => "ntfy.sh",
            "email" => "Email",
            "webhook" => "Webhook",
            _ => &nc.channel_type_str,
        }
        .to_string();

        let config_preview = if endpoint.is_empty() {
            format!("{} channel", channel_type_display)
        } else {
            format!("{} → {}", channel_type_display, endpoint)
        };

        let created = nc
            .created_at
            .with_timezone(&Local)
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();

        Self {
            id: nc.id,
            name: nc.name,
            channel_type: nc.channel_type_str,
            channel_type_display,
            endpoint,
            description: nc.description.unwrap_or_default(),
            config_preview,
            enabled: nc.enabled,
            created_at: created,
        }
    }
}

/// Convert database NotificationPolicy to display model
impl From<svrctlrs_database::NotificationPolicy> for NotificationPolicyDisplay {
    fn from(np: svrctlrs_database::NotificationPolicy) -> Self {
        use chrono::Local;

        // Parse filters from JSON strings
        let job_type_filter_vec: Vec<String> = np
            .job_type_filter
            .as_ref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();

        let server_filter_vec: Vec<i64> = np
            .server_filter
            .as_ref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();

        let tag_filter_vec: Vec<String> = np
            .tag_filter
            .as_ref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();

        // Determine scope_type from filters
        let scope_type = if !job_type_filter_vec.is_empty() {
            "job_type".to_string()
        } else if !server_filter_vec.is_empty() {
            "specific_servers".to_string()
        } else if !tag_filter_vec.is_empty() {
            "tags".to_string()
        } else {
            "global".to_string()
        };

        let created = np
            .created_at
            .with_timezone(&Local)
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();

        Self {
            id: np.id,
            name: np.name,
            description: np.description,
            scope_type,
            channel_id: 0, // Legacy field - use policy_channels instead
            channel_name: String::from("(Not loaded)"), // Legacy field - use policy_channels instead
            on_success: np.on_success,
            on_failure: np.on_failure,
            on_timeout: np.on_timeout,
            enabled: np.enabled,
            title_template: np.title_template,
            body_template: np.body_template,
            created_at: created,
            job_type_id: None, // Derived from job_type_filter
            job_type_name: None,
            job_template_id: None, // Not in current schema
            job_templates: Vec::new(),
            job_template_count: 0,
            // Filtering fields
            job_type_filter: if job_type_filter_vec.is_empty() {
                None
            } else {
                Some(job_type_filter_vec)
            },
            server_filter: if server_filter_vec.is_empty() {
                None
            } else {
                Some(server_filter_vec)
            },
            tag_filter: if tag_filter_vec.is_empty() {
                None
            } else {
                Some(tag_filter_vec)
            },
            // Throttling fields
            min_severity: Some(np.min_severity),
            max_per_hour: np.max_per_hour,
            // Multi-channel support (populated separately in edit handler)
            policy_channels: Vec::new(),
        }
    }
}

/// Convert database Setting to display model
impl From<svrctlrs_database::models::Setting> for SettingDisplay {
    fn from(s: svrctlrs_database::models::Setting) -> Self {
        use chrono::Local;

        let updated_at = s
            .updated_at
            .with_timezone(&Local)
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();

        Self {
            key: s.key,
            value: s.value,
            value_type: s.value_type,
            description: s.description,
            updated_at,
        }
    }
}

// ============================================================================
// Error Pages
// ============================================================================

#[derive(Template)]
#[template(path = "pages/404.html")]
pub struct NotFoundTemplate {
    pub user: Option<User>,
}
