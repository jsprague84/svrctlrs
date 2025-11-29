use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::FromRow;

/// JobType model - built-in job categories (replaces plugins)
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct JobType {
    pub id: i64,
    /// Internal name (docker, os, backup, monitoring, custom)
    pub name: String,
    /// Display name for UI ("Docker Operations", "OS Maintenance")
    pub display_name: String,
    pub description: Option<String>,
    /// Icon name for UI
    pub icon: Option<String>,
    /// Color for UI (hex code)
    pub color: Option<String>,
    /// JSON array of required capabilities (e.g., ["docker"], ["apt"])
    pub requires_capabilities: Option<String>,
    /// Additional metadata as JSON string
    pub metadata: Option<String>,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
}

impl JobType {
    /// Get required capabilities as a vector
    pub fn get_requires_capabilities(&self) -> Vec<String> {
        self.requires_capabilities
            .as_ref()
            .and_then(|c| serde_json::from_str(c).ok())
            .unwrap_or_default()
    }

    /// Get metadata as JSON value
    pub fn get_metadata(&self) -> JsonValue {
        self.metadata
            .as_ref()
            .and_then(|m| serde_json::from_str(m).ok())
            .unwrap_or(JsonValue::Object(serde_json::Map::new()))
    }

    /// Check if this job type requires specific capability
    pub fn requires_capability(&self, capability: &str) -> bool {
        self.get_requires_capabilities()
            .iter()
            .any(|c| c == capability)
    }
}

/// CommandTemplate model - reusable command patterns with variable substitution
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CommandTemplate {
    pub id: i64,
    pub job_type_id: i64,
    /// Internal name (list_containers, update_packages, etc.)
    pub name: String,
    /// Display name for UI ("List Docker Containers")
    pub display_name: String,
    pub description: Option<String>,

    // Command definition
    /// Command with {{variables}} for substitution
    pub command: String,
    /// JSON array of required capabilities
    pub required_capabilities: Option<String>,
    /// JSON object for OS filtering (e.g., {"distro": ["ubuntu", "debian"]})
    pub os_filter: Option<String>,

    // Execution settings
    pub timeout_seconds: i32,
    pub working_directory: Option<String>,
    /// JSON object for environment variables (e.g., {"VAR": "value"})
    pub environment: Option<String>,

    // Output handling
    pub output_format: Option<String>,
    pub parse_output: bool,
    /// JSON parser configuration if parse_output = true
    pub output_parser: Option<String>,

    // Notification defaults
    pub notify_on_success: bool,
    pub notify_on_failure: bool,

    /// JSON array defining parameters that can be substituted in the command
    /// Format: [{"name": "var", "type": "string", "required": true, "description": "...", "default": "", "validation": {...}}]
    pub parameter_schema: Option<String>,

    pub metadata: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl CommandTemplate {
    /// Get required capabilities as a vector
    pub fn get_required_capabilities(&self) -> Vec<String> {
        self.required_capabilities
            .as_ref()
            .and_then(|c| serde_json::from_str(c).ok())
            .unwrap_or_default()
    }

    /// Get OS filter as JSON value
    pub fn get_os_filter(&self) -> JsonValue {
        self.os_filter
            .as_ref()
            .and_then(|f| serde_json::from_str(f).ok())
            .unwrap_or(JsonValue::Object(serde_json::Map::new()))
    }

    /// Get environment variables as a map
    pub fn get_environment(&self) -> std::collections::HashMap<String, String> {
        self.environment
            .as_ref()
            .and_then(|e| serde_json::from_str(e).ok())
            .unwrap_or_default()
    }

    /// Get output parser configuration
    pub fn get_output_parser(&self) -> JsonValue {
        self.output_parser
            .as_ref()
            .and_then(|p| serde_json::from_str(p).ok())
            .unwrap_or(JsonValue::Object(serde_json::Map::new()))
    }

    /// Get metadata as JSON value
    pub fn get_metadata(&self) -> JsonValue {
        self.metadata
            .as_ref()
            .and_then(|m| serde_json::from_str(m).ok())
            .unwrap_or(JsonValue::Object(serde_json::Map::new()))
    }

    /// Get parameter schema as JSON value
    pub fn get_parameter_schema(&self) -> JsonValue {
        self.parameter_schema
            .as_ref()
            .and_then(|ps| serde_json::from_str(ps).ok())
            .unwrap_or(JsonValue::Array(Vec::new()))
    }

    /// Substitute variables in the command
    pub fn substitute_variables(
        &self,
        variables: &std::collections::HashMap<String, String>,
    ) -> String {
        let mut command = self.command.clone();
        for (key, value) in variables {
            command = command.replace(&format!("{{{{{}}}}}", key), value);
        }
        command
    }

    /// Check if command matches OS filter
    pub fn matches_os_filter(&self, os_distro: Option<&str>) -> bool {
        let filter = self.get_os_filter();

        if filter.is_null() || !filter.is_object() {
            return true; // No filter means match all
        }

        if let Some(distros) = filter.get("distro").and_then(|d| d.as_array()) {
            if let Some(os) = os_distro {
                return distros.iter().any(|d| d.as_str() == Some(os));
            }
            return false; // Filter requires distro but none provided
        }

        true
    }
}

/// Input for creating a new job type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateJobType {
    pub name: String,
    pub display_name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub requires_capabilities: Option<Vec<String>>,
    pub metadata: Option<JsonValue>,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

impl CreateJobType {
    /// Convert requires_capabilities to JSON string
    pub fn requires_capabilities_string(&self) -> Option<String> {
        self.requires_capabilities
            .as_ref()
            .and_then(|c| serde_json::to_string(c).ok())
    }

    /// Convert metadata to JSON string
    pub fn metadata_string(&self) -> Option<String> {
        self.metadata
            .as_ref()
            .and_then(|m| serde_json::to_string(m).ok())
    }
}

/// Input for updating an existing job type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateJobType {
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub requires_capabilities: Option<Vec<String>>,
    pub metadata: Option<JsonValue>,
    pub enabled: Option<bool>,
}

impl UpdateJobType {
    /// Convert requires_capabilities to JSON string
    pub fn requires_capabilities_string(&self) -> Option<String> {
        self.requires_capabilities
            .as_ref()
            .and_then(|c| serde_json::to_string(c).ok())
    }

    /// Convert metadata to JSON string
    pub fn metadata_string(&self) -> Option<String> {
        self.metadata
            .as_ref()
            .and_then(|m| serde_json::to_string(m).ok())
    }

    /// Check if this update contains any changes
    pub fn has_changes(&self) -> bool {
        self.display_name.is_some()
            || self.description.is_some()
            || self.icon.is_some()
            || self.color.is_some()
            || self.requires_capabilities.is_some()
            || self.metadata.is_some()
            || self.enabled.is_some()
    }
}

/// Input for creating a new command template
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCommandTemplate {
    pub job_type_id: i64,
    pub name: String,
    pub display_name: String,
    pub description: Option<String>,
    pub command: String,
    pub required_capabilities: Option<Vec<String>>,
    pub os_filter: Option<JsonValue>,
    #[serde(default = "default_timeout")]
    pub timeout_seconds: i32,
    pub working_directory: Option<String>,
    pub environment: Option<std::collections::HashMap<String, String>>,
    pub output_format: Option<String>,
    #[serde(default)]
    pub parse_output: bool,
    pub output_parser: Option<JsonValue>,
    #[serde(default)]
    pub notify_on_success: bool,
    #[serde(default = "default_notify_on_failure")]
    pub notify_on_failure: bool,
    pub parameter_schema: Option<JsonValue>,
    pub metadata: Option<JsonValue>,
}

impl CreateCommandTemplate {
    /// Convert required_capabilities to JSON string
    pub fn required_capabilities_string(&self) -> Option<String> {
        self.required_capabilities
            .as_ref()
            .and_then(|c| serde_json::to_string(c).ok())
    }

    /// Convert os_filter to JSON string
    pub fn os_filter_string(&self) -> Option<String> {
        self.os_filter
            .as_ref()
            .and_then(|f| serde_json::to_string(f).ok())
    }

    /// Convert environment to JSON string
    pub fn environment_string(&self) -> Option<String> {
        self.environment
            .as_ref()
            .and_then(|e| serde_json::to_string(e).ok())
    }

    /// Convert output_parser to JSON string
    pub fn output_parser_string(&self) -> Option<String> {
        self.output_parser
            .as_ref()
            .and_then(|p| serde_json::to_string(p).ok())
    }

    /// Convert parameter_schema to JSON string
    pub fn parameter_schema_string(&self) -> Option<String> {
        self.parameter_schema
            .as_ref()
            .and_then(|ps| serde_json::to_string(ps).ok())
    }

    /// Convert metadata to JSON string
    pub fn metadata_string(&self) -> Option<String> {
        self.metadata
            .as_ref()
            .and_then(|m| serde_json::to_string(m).ok())
    }
}

/// Input for updating an existing command template
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateCommandTemplate {
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub command: Option<String>,
    pub required_capabilities: Option<Vec<String>>,
    pub os_filter: Option<JsonValue>,
    pub timeout_seconds: Option<i32>,
    pub working_directory: Option<String>,
    pub environment: Option<std::collections::HashMap<String, String>>,
    pub output_format: Option<String>,
    pub parse_output: Option<bool>,
    pub output_parser: Option<JsonValue>,
    pub notify_on_success: Option<bool>,
    pub notify_on_failure: Option<bool>,
    pub parameter_schema: Option<JsonValue>,
    pub metadata: Option<JsonValue>,
}

impl UpdateCommandTemplate {
    /// Convert required_capabilities to JSON string
    pub fn required_capabilities_string(&self) -> Option<String> {
        self.required_capabilities
            .as_ref()
            .and_then(|c| serde_json::to_string(c).ok())
    }

    /// Convert os_filter to JSON string
    pub fn os_filter_string(&self) -> Option<String> {
        self.os_filter
            .as_ref()
            .and_then(|f| serde_json::to_string(f).ok())
    }

    /// Convert environment to JSON string
    pub fn environment_string(&self) -> Option<String> {
        self.environment
            .as_ref()
            .and_then(|e| serde_json::to_string(e).ok())
    }

    /// Convert output_parser to JSON string
    pub fn output_parser_string(&self) -> Option<String> {
        self.output_parser
            .as_ref()
            .and_then(|p| serde_json::to_string(p).ok())
    }

    /// Convert parameter_schema to JSON string
    pub fn parameter_schema_string(&self) -> Option<String> {
        self.parameter_schema
            .as_ref()
            .and_then(|ps| serde_json::to_string(ps).ok())
    }

    /// Convert metadata to JSON string
    pub fn metadata_string(&self) -> Option<String> {
        self.metadata
            .as_ref()
            .and_then(|m| serde_json::to_string(m).ok())
    }

    /// Check if this update contains any changes
    pub fn has_changes(&self) -> bool {
        self.display_name.is_some()
            || self.description.is_some()
            || self.command.is_some()
            || self.required_capabilities.is_some()
            || self.os_filter.is_some()
            || self.timeout_seconds.is_some()
            || self.working_directory.is_some()
            || self.environment.is_some()
            || self.output_format.is_some()
            || self.parse_output.is_some()
            || self.output_parser.is_some()
            || self.notify_on_success.is_some()
            || self.notify_on_failure.is_some()
            || self.metadata.is_some()
    }
}

fn default_enabled() -> bool {
    true
}

fn default_timeout() -> i32 {
    300
}

fn default_notify_on_failure() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_type_capabilities() {
        let job_type = JobType {
            id: 1,
            name: "docker".to_string(),
            display_name: "Docker Operations".to_string(),
            description: None,
            icon: Some("docker".to_string()),
            color: Some("#2496ED".to_string()),
            requires_capabilities: Some(r#"["docker"]"#.to_string()),
            metadata: None,
            enabled: true,
            created_at: Utc::now(),
        };

        assert_eq!(job_type.get_requires_capabilities(), vec!["docker"]);
        assert!(job_type.requires_capability("docker"));
        assert!(!job_type.requires_capability("systemd"));
    }

    #[test]
    fn test_command_template_variable_substitution() {
        let template = CommandTemplate {
            id: 1,
            job_type_id: 1,
            name: "test".to_string(),
            display_name: "Test".to_string(),
            description: None,
            command: "echo {{message}} {{value}}".to_string(),
            required_capabilities: None,
            os_filter: None,
            timeout_seconds: 300,
            working_directory: None,
            environment: None,
            output_format: None,
            parse_output: false,
            output_parser: None,
            notify_on_success: false,
            notify_on_failure: true,
            parameter_schema: None,
            metadata: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let mut vars = std::collections::HashMap::new();
        vars.insert("message".to_string(), "Hello".to_string());
        vars.insert("value".to_string(), "World".to_string());

        assert_eq!(template.substitute_variables(&vars), "echo Hello World");
    }

    #[test]
    fn test_command_template_os_filter() {
        let template = CommandTemplate {
            id: 1,
            job_type_id: 1,
            name: "test".to_string(),
            display_name: "Test".to_string(),
            description: None,
            command: "test".to_string(),
            required_capabilities: None,
            os_filter: Some(r#"{"distro": ["ubuntu", "debian"]}"#.to_string()),
            timeout_seconds: 300,
            working_directory: None,
            environment: None,
            output_format: None,
            parse_output: false,
            output_parser: None,
            notify_on_success: false,
            notify_on_failure: true,
            parameter_schema: None,
            metadata: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        assert!(template.matches_os_filter(Some("ubuntu")));
        assert!(template.matches_os_filter(Some("debian")));
        assert!(!template.matches_os_filter(Some("fedora")));
        assert!(!template.matches_os_filter(None));
    }
}
