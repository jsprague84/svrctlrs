use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::FromRow;

/// Job run status enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JobRunStatus {
    Pending,
    Running,
    Success,
    Failed,  // Alias for Failure
    Failure, // Original name
    PartialSuccess,
    Timeout,
    Skipped,
}

impl JobRunStatus {
    /// Parse status from string
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(Self::Pending),
            "running" => Some(Self::Running),
            "success" => Some(Self::Success),
            "failed" | "failure" => Some(Self::Failed),
            "partial_success" | "partialsuccess" => Some(Self::PartialSuccess),
            "timeout" => Some(Self::Timeout),
            "skipped" => Some(Self::Skipped),
            _ => None,
        }
    }

    /// Convert status to string
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Success => "success",
            Self::Failed | Self::Failure => "failed",
            Self::PartialSuccess => "partial_success",
            Self::Timeout => "timeout",
            Self::Skipped => "skipped",
        }
    }

    /// Check if status indicates success
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Success | Self::PartialSuccess)
    }

    /// Check if status indicates failure
    pub fn is_failure(&self) -> bool {
        matches!(self, Self::Failed | Self::Failure | Self::Timeout)
    }

    /// Check if status is terminal (not pending or running)
    pub fn is_terminal(&self) -> bool {
        !matches!(self, Self::Pending | Self::Running)
    }
}

/// JobSchedule model - scheduled job instances (replaces tasks table)
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct JobSchedule {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,

    // Job definition
    pub job_template_id: i64,
    pub server_id: i64,

    // Schedule
    /// Cron expression
    pub schedule: String,
    pub enabled: bool,

    // Overrides for this specific schedule
    pub timeout_seconds: Option<i32>,
    pub retry_count: Option<i32>,
    pub notify_on_success: Option<bool>,
    pub notify_on_failure: Option<bool>,
    pub notification_policy_id: Option<i64>,

    // Tracking
    pub last_run_at: Option<DateTime<Utc>>,
    #[sqlx(rename = "last_run_status")]
    pub last_run_status_str: Option<String>,
    pub next_run_at: Option<DateTime<Utc>>,
    pub success_count: i64,
    pub failure_count: i64,

    pub metadata: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl JobSchedule {
    /// Get last run status as enum
    pub fn last_run_status(&self) -> Option<JobRunStatus> {
        self.last_run_status_str
            .as_ref()
            .and_then(|s| JobRunStatus::from_str(s))
    }

    /// Get metadata as JSON value
    pub fn get_metadata(&self) -> JsonValue {
        self.metadata
            .as_ref()
            .and_then(|m| serde_json::from_str(m).ok())
            .unwrap_or(JsonValue::Object(serde_json::Map::new()))
    }

    /// Calculate success rate (0.0 to 1.0)
    pub fn success_rate(&self) -> f64 {
        let total = self.success_count + self.failure_count;
        if total == 0 {
            0.0
        } else {
            self.success_count as f64 / total as f64
        }
    }

    /// Check if schedule is overdue (next_run_at is in the past)
    pub fn is_overdue(&self) -> bool {
        if let Some(next_run) = self.next_run_at {
            next_run < Utc::now()
        } else {
            false
        }
    }

    /// Parse cron expression (basic validation)
    pub fn validate_cron(&self) -> Result<(), String> {
        // Basic cron validation - should be 6 fields for cron with seconds
        // or 5 fields for traditional cron
        let fields: Vec<&str> = self.schedule.split_whitespace().collect();
        if fields.len() != 5 && fields.len() != 6 {
            return Err(format!(
                "Invalid cron expression: expected 5 or 6 fields, got {}",
                fields.len()
            ));
        }
        Ok(())
    }
}

/// Input for creating a new job schedule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateJobSchedule {
    pub name: String,
    pub description: Option<String>,
    pub job_template_id: i64,
    pub server_id: i64,
    pub schedule: String,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    pub timeout_seconds: Option<i32>,
    pub retry_count: Option<i32>,
    pub notify_on_success: Option<bool>,
    pub notify_on_failure: Option<bool>,
    pub notification_policy_id: Option<i64>,
    pub metadata: Option<JsonValue>,
}

impl CreateJobSchedule {
    /// Convert metadata to JSON string
    pub fn metadata_string(&self) -> Option<String> {
        self.metadata
            .as_ref()
            .and_then(|m| serde_json::to_string(m).ok())
    }

    /// Validate the input
    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("Job schedule name cannot be empty".to_string());
        }

        // Basic cron validation
        let fields: Vec<&str> = self.schedule.split_whitespace().collect();
        if fields.len() != 5 && fields.len() != 6 {
            return Err(format!(
                "Invalid cron expression: expected 5 or 6 fields, got {}",
                fields.len()
            ));
        }

        if let Some(timeout) = self.timeout_seconds {
            if timeout < 1 {
                return Err("Timeout must be at least 1 second".to_string());
            }
        }

        if let Some(retry) = self.retry_count {
            if retry < 0 {
                return Err("Retry count cannot be negative".to_string());
            }
        }

        Ok(())
    }
}

/// Input for updating an existing job schedule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateJobSchedule {
    pub name: Option<String>,
    pub description: Option<String>,
    pub schedule: Option<String>,
    pub enabled: Option<bool>,
    pub timeout_seconds: Option<i32>,
    pub retry_count: Option<i32>,
    pub notify_on_success: Option<bool>,
    pub notify_on_failure: Option<bool>,
    pub notification_policy_id: Option<i64>,
    pub next_run_at: Option<DateTime<Utc>>,
    pub metadata: Option<JsonValue>,
}

impl UpdateJobSchedule {
    /// Convert metadata to JSON string
    pub fn metadata_string(&self) -> Option<String> {
        self.metadata
            .as_ref()
            .and_then(|m| serde_json::to_string(m).ok())
    }

    /// Check if this update contains any changes
    pub fn has_changes(&self) -> bool {
        self.name.is_some()
            || self.description.is_some()
            || self.schedule.is_some()
            || self.enabled.is_some()
            || self.timeout_seconds.is_some()
            || self.retry_count.is_some()
            || self.notify_on_success.is_some()
            || self.notify_on_failure.is_some()
            || self.notification_policy_id.is_some()
            || self.next_run_at.is_some()
            || self.metadata.is_some()
    }
}

fn default_enabled() -> bool {
    true
}

/// JobSchedule with related data (for UI display)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobScheduleWithDetails {
    #[serde(flatten)]
    pub schedule: JobSchedule,
    pub job_template_name: String,
    pub server_name: String,
    pub job_type_name: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_run_status() {
        assert_eq!(
            JobRunStatus::from_str("success"),
            Some(JobRunStatus::Success)
        );
        assert_eq!(
            JobRunStatus::from_str("failure"),
            Some(JobRunStatus::Failure)
        );
        assert_eq!(
            JobRunStatus::from_str("timeout"),
            Some(JobRunStatus::Timeout)
        );
        assert_eq!(
            JobRunStatus::from_str("skipped"),
            Some(JobRunStatus::Skipped)
        );
        assert_eq!(JobRunStatus::from_str("invalid"), None);

        assert_eq!(JobRunStatus::Success.as_str(), "success");
        assert!(JobRunStatus::Success.is_success());
        assert!(!JobRunStatus::Success.is_failure());

        assert!(JobRunStatus::Failure.is_failure());
        assert!(JobRunStatus::Timeout.is_failure());
        assert!(!JobRunStatus::Skipped.is_failure());
    }

    #[test]
    fn test_job_schedule_success_rate() {
        let schedule = JobSchedule {
            id: 1,
            name: "test".to_string(),
            description: None,
            job_template_id: 1,
            server_id: 1,
            schedule: "0 0 * * *".to_string(),
            enabled: true,
            timeout_seconds: None,
            retry_count: None,
            notify_on_success: None,
            notify_on_failure: None,
            notification_policy_id: None,
            last_run_at: None,
            last_run_status_str: None,
            next_run_at: None,
            success_count: 8,
            failure_count: 2,
            metadata: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        assert_eq!(schedule.success_rate(), 0.8);
    }

    #[test]
    fn test_cron_validation() {
        let valid_5_field = JobSchedule {
            id: 1,
            name: "test".to_string(),
            description: None,
            job_template_id: 1,
            server_id: 1,
            schedule: "0 0 * * *".to_string(), // 5 fields
            enabled: true,
            timeout_seconds: None,
            retry_count: None,
            notify_on_success: None,
            notify_on_failure: None,
            notification_policy_id: None,
            last_run_at: None,
            last_run_status_str: None,
            next_run_at: None,
            success_count: 0,
            failure_count: 0,
            metadata: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        assert!(valid_5_field.validate_cron().is_ok());

        let valid_6_field = JobSchedule {
            id: 2,
            name: "test".to_string(),
            description: None,
            job_template_id: 1,
            server_id: 1,
            schedule: "0 0 0 * * *".to_string(), // 6 fields
            enabled: true,
            timeout_seconds: None,
            retry_count: None,
            notify_on_success: None,
            notify_on_failure: None,
            notification_policy_id: None,
            last_run_at: None,
            last_run_status_str: None,
            next_run_at: None,
            success_count: 0,
            failure_count: 0,
            metadata: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        assert!(valid_6_field.validate_cron().is_ok());

        let invalid = JobSchedule {
            id: 3,
            name: "test".to_string(),
            description: None,
            job_template_id: 1,
            server_id: 1,
            schedule: "0 0 *".to_string(), // Only 3 fields
            enabled: true,
            timeout_seconds: None,
            retry_count: None,
            notify_on_success: None,
            notify_on_failure: None,
            notification_policy_id: None,
            last_run_at: None,
            last_run_status_str: None,
            next_run_at: None,
            success_count: 0,
            failure_count: 0,
            metadata: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        assert!(invalid.validate_cron().is_err());
    }

    #[test]
    fn test_create_job_schedule_validation() {
        let valid = CreateJobSchedule {
            name: "test".to_string(),
            description: None,
            job_template_id: 1,
            server_id: 1,
            schedule: "0 0 * * *".to_string(),
            enabled: true,
            timeout_seconds: None,
            retry_count: None,
            notify_on_success: None,
            notify_on_failure: None,
            notification_policy_id: None,
            metadata: None,
        };
        assert!(valid.validate().is_ok());

        let invalid_cron = CreateJobSchedule {
            name: "test".to_string(),
            description: None,
            job_template_id: 1,
            server_id: 1,
            schedule: "invalid".to_string(),
            enabled: true,
            timeout_seconds: None,
            retry_count: None,
            notify_on_success: None,
            notify_on_failure: None,
            notification_policy_id: None,
            metadata: None,
        };
        assert!(invalid_cron.validate().is_err());
    }
}
