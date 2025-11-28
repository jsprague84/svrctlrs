use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::FromRow;

/// JobTemplate model - user-defined reusable jobs (simple or composite)
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct JobTemplate {
    pub id: i64,
    pub name: String,
    pub display_name: String,
    pub description: Option<String>,
    pub job_type_id: i64,

    // Template type
    /// If true, this is a composite job using job_template_steps
    pub is_composite: bool,

    // Simple job (single command) - only used if is_composite = false
    pub command_template_id: Option<i64>,
    /// JSON object for variable substitution
    pub variables: Option<String>,

    // Execution defaults
    pub timeout_seconds: i32,
    pub retry_count: i32,
    pub retry_delay_seconds: i32,

    // Notification defaults
    pub notify_on_success: bool,
    pub notify_on_failure: bool,
    pub notification_policy_id: Option<i64>,

    pub metadata: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl JobTemplate {
    /// Get variables as a map
    pub fn get_variables(&self) -> std::collections::HashMap<String, String> {
        self.variables
            .as_ref()
            .and_then(|v| serde_json::from_str(v).ok())
            .unwrap_or_default()
    }

    /// Get metadata as JSON value
    pub fn get_metadata(&self) -> JsonValue {
        self.metadata
            .as_ref()
            .and_then(|m| serde_json::from_str(m).ok())
            .unwrap_or(JsonValue::Object(serde_json::Map::new()))
    }

    /// Check if this is a simple job (single command)
    pub fn is_simple(&self) -> bool {
        !self.is_composite
    }

    /// Validate the job template structure
    pub fn validate(&self) -> Result<(), String> {
        if !self.is_composite && self.command_template_id.is_none() {
            return Err("Simple jobs must have a command_template_id".to_string());
        }

        if self.timeout_seconds < 1 {
            return Err("Timeout must be at least 1 second".to_string());
        }

        if self.retry_count < 0 {
            return Err("Retry count cannot be negative".to_string());
        }

        if self.retry_delay_seconds < 0 {
            return Err("Retry delay cannot be negative".to_string());
        }

        Ok(())
    }
}

/// JobTemplateStep model - for composite jobs (multi-step workflows)
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct JobTemplateStep {
    pub id: i64,
    pub job_template_id: i64,
    /// Execution order (0, 1, 2, ...)
    pub step_order: i32,
    /// Step name for logging
    pub name: String,
    pub command_template_id: i64,
    /// JSON object for step-specific variables
    pub variables: Option<String>,

    // Step control
    /// If false, abort job on step failure
    pub continue_on_failure: bool,
    /// Override template timeout if set
    pub timeout_seconds: Option<i32>,

    pub metadata: Option<String>,
}

impl JobTemplateStep {
    /// Get variables as a map
    pub fn get_variables(&self) -> std::collections::HashMap<String, String> {
        self.variables
            .as_ref()
            .and_then(|v| serde_json::from_str(v).ok())
            .unwrap_or_default()
    }

    /// Get metadata as JSON value
    pub fn get_metadata(&self) -> JsonValue {
        self.metadata
            .as_ref()
            .and_then(|m| serde_json::from_str(m).ok())
            .unwrap_or(JsonValue::Object(serde_json::Map::new()))
    }

    /// Get effective timeout (step timeout or default)
    pub fn effective_timeout(&self, default_timeout: i32) -> i32 {
        self.timeout_seconds.unwrap_or(default_timeout)
    }
}

/// Input for creating a new job template
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateJobTemplate {
    pub name: String,
    pub display_name: String,
    pub description: Option<String>,
    pub job_type_id: i64,
    #[serde(default)]
    pub is_composite: bool,
    pub command_template_id: Option<i64>,
    pub variables: Option<std::collections::HashMap<String, String>>,
    #[serde(default = "default_timeout")]
    pub timeout_seconds: i32,
    #[serde(default)]
    pub retry_count: i32,
    #[serde(default = "default_retry_delay")]
    pub retry_delay_seconds: i32,
    #[serde(default)]
    pub notify_on_success: bool,
    #[serde(default = "default_notify_on_failure")]
    pub notify_on_failure: bool,
    pub notification_policy_id: Option<i64>,
    pub metadata: Option<JsonValue>,
}

impl CreateJobTemplate {
    /// Convert variables to JSON string
    pub fn variables_string(&self) -> Option<String> {
        self.variables
            .as_ref()
            .and_then(|v| serde_json::to_string(v).ok())
    }

    /// Convert metadata to JSON string
    pub fn metadata_string(&self) -> Option<String> {
        self.metadata
            .as_ref()
            .and_then(|m| serde_json::to_string(m).ok())
    }

    /// Validate the input
    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("Job template name cannot be empty".to_string());
        }

        if !self.is_composite && self.command_template_id.is_none() {
            return Err("Simple jobs must have a command_template_id".to_string());
        }

        if self.timeout_seconds < 1 {
            return Err("Timeout must be at least 1 second".to_string());
        }

        if self.retry_count < 0 {
            return Err("Retry count cannot be negative".to_string());
        }

        if self.retry_delay_seconds < 0 {
            return Err("Retry delay cannot be negative".to_string());
        }

        Ok(())
    }
}

/// Input for updating an existing job template
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateJobTemplate {
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub command_template_id: Option<i64>,
    pub variables: Option<std::collections::HashMap<String, String>>,
    pub timeout_seconds: Option<i32>,
    pub retry_count: Option<i32>,
    pub retry_delay_seconds: Option<i32>,
    pub notify_on_success: Option<bool>,
    pub notify_on_failure: Option<bool>,
    pub notification_policy_id: Option<i64>,
    pub metadata: Option<JsonValue>,
}

impl UpdateJobTemplate {
    /// Convert variables to JSON string
    pub fn variables_string(&self) -> Option<String> {
        self.variables
            .as_ref()
            .and_then(|v| serde_json::to_string(v).ok())
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
            || self.command_template_id.is_some()
            || self.variables.is_some()
            || self.timeout_seconds.is_some()
            || self.retry_count.is_some()
            || self.retry_delay_seconds.is_some()
            || self.notify_on_success.is_some()
            || self.notify_on_failure.is_some()
            || self.notification_policy_id.is_some()
            || self.metadata.is_some()
    }
}

/// Input for creating a new job template step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateJobTemplateStep {
    pub job_template_id: i64,
    pub step_order: i32,
    pub name: String,
    pub command_template_id: i64,
    pub variables: Option<std::collections::HashMap<String, String>>,
    #[serde(default)]
    pub continue_on_failure: bool,
    pub timeout_seconds: Option<i32>,
    pub metadata: Option<JsonValue>,
}

impl CreateJobTemplateStep {
    /// Convert variables to JSON string
    pub fn variables_string(&self) -> Option<String> {
        self.variables
            .as_ref()
            .and_then(|v| serde_json::to_string(v).ok())
    }

    /// Convert metadata to JSON string
    pub fn metadata_string(&self) -> Option<String> {
        self.metadata
            .as_ref()
            .and_then(|m| serde_json::to_string(m).ok())
    }

    /// Validate the input
    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("Step name cannot be empty".to_string());
        }

        if self.step_order < 0 {
            return Err("Step order cannot be negative".to_string());
        }

        if let Some(timeout) = self.timeout_seconds {
            if timeout < 1 {
                return Err("Timeout must be at least 1 second".to_string());
            }
        }

        Ok(())
    }
}

/// Input for updating an existing job template step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateJobTemplateStep {
    pub step_order: Option<i32>,
    pub name: Option<String>,
    pub command_template_id: Option<i64>,
    pub variables: Option<std::collections::HashMap<String, String>>,
    pub continue_on_failure: Option<bool>,
    pub timeout_seconds: Option<i32>,
    pub metadata: Option<JsonValue>,
}

impl UpdateJobTemplateStep {
    /// Convert variables to JSON string
    pub fn variables_string(&self) -> Option<String> {
        self.variables
            .as_ref()
            .and_then(|v| serde_json::to_string(v).ok())
    }

    /// Convert metadata to JSON string
    pub fn metadata_string(&self) -> Option<String> {
        self.metadata
            .as_ref()
            .and_then(|m| serde_json::to_string(m).ok())
    }

    /// Check if this update contains any changes
    pub fn has_changes(&self) -> bool {
        self.step_order.is_some()
            || self.name.is_some()
            || self.command_template_id.is_some()
            || self.variables.is_some()
            || self.continue_on_failure.is_some()
            || self.timeout_seconds.is_some()
            || self.metadata.is_some()
    }
}

fn default_timeout() -> i32 {
    300
}

fn default_retry_delay() -> i32 {
    60
}

fn default_notify_on_failure() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_template_validation() {
        let valid_simple = JobTemplate {
            id: 1,
            name: "test".to_string(),
            display_name: "Test".to_string(),
            description: None,
            job_type_id: 1,
            is_composite: false,
            command_template_id: Some(1),
            variables: None,
            timeout_seconds: 300,
            retry_count: 0,
            retry_delay_seconds: 60,
            notify_on_success: false,
            notify_on_failure: true,
            notification_policy_id: None,
            metadata: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        assert!(valid_simple.validate().is_ok());
        assert!(valid_simple.is_simple());
        assert!(!valid_simple.is_composite);

        let invalid_simple = JobTemplate {
            id: 2,
            name: "invalid".to_string(),
            display_name: "Invalid".to_string(),
            description: None,
            job_type_id: 1,
            is_composite: false,
            command_template_id: None, // Missing command_template_id
            variables: None,
            timeout_seconds: 300,
            retry_count: 0,
            retry_delay_seconds: 60,
            notify_on_success: false,
            notify_on_failure: true,
            notification_policy_id: None,
            metadata: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        assert!(invalid_simple.validate().is_err());
    }

    #[test]
    fn test_job_template_step_effective_timeout() {
        let step_with_timeout = JobTemplateStep {
            id: 1,
            job_template_id: 1,
            step_order: 0,
            name: "Test".to_string(),
            command_template_id: 1,
            variables: None,
            continue_on_failure: false,
            timeout_seconds: Some(600),
            metadata: None,
        };
        assert_eq!(step_with_timeout.effective_timeout(300), 600);

        let step_without_timeout = JobTemplateStep {
            id: 2,
            job_template_id: 1,
            step_order: 1,
            name: "Test 2".to_string(),
            command_template_id: 2,
            variables: None,
            continue_on_failure: false,
            timeout_seconds: None,
            metadata: None,
        };
        assert_eq!(step_without_timeout.effective_timeout(300), 300);
    }

    #[test]
    fn test_create_job_template_validation() {
        let valid = CreateJobTemplate {
            name: "test".to_string(),
            display_name: "Test".to_string(),
            description: None,
            job_type_id: 1,
            is_composite: false,
            command_template_id: Some(1),
            variables: None,
            timeout_seconds: 300,
            retry_count: 0,
            retry_delay_seconds: 60,
            notify_on_success: false,
            notify_on_failure: true,
            notification_policy_id: None,
            metadata: None,
        };
        assert!(valid.validate().is_ok());

        let invalid_timeout = CreateJobTemplate {
            name: "test".to_string(),
            display_name: "Test".to_string(),
            description: None,
            job_type_id: 1,
            is_composite: false,
            command_template_id: Some(1),
            variables: None,
            timeout_seconds: 0, // Invalid
            retry_count: 0,
            retry_delay_seconds: 60,
            notify_on_success: false,
            notify_on_failure: true,
            notification_policy_id: None,
            metadata: None,
        };
        assert!(invalid_timeout.validate().is_err());
    }
}
