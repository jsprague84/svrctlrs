use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::FromRow;

/// Job execution status enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JobExecutionStatus {
    Running,
    Success,
    Failure,
    Timeout,
    Cancelled,
}

impl JobExecutionStatus {
    /// Parse status from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "running" => Some(Self::Running),
            "success" => Some(Self::Success),
            "failure" => Some(Self::Failure),
            "timeout" => Some(Self::Timeout),
            "cancelled" => Some(Self::Cancelled),
            _ => None,
        }
    }

    /// Convert status to string
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Success => "success",
            Self::Failure => "failure",
            Self::Timeout => "timeout",
            Self::Cancelled => "cancelled",
        }
    }

    /// Check if status indicates completion
    pub fn is_complete(&self) -> bool {
        !matches!(self, Self::Running)
    }

    /// Check if status indicates success
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Success)
    }

    /// Check if status indicates failure
    pub fn is_failure(&self) -> bool {
        matches!(self, Self::Failure | Self::Timeout | Self::Cancelled)
    }
}

/// JobRun model - execution history for job schedules (replaces task_history)
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct JobRun {
    pub id: i64,
    pub job_schedule_id: i64,
    pub job_template_id: i64,
    pub server_id: i64,

    // Execution details
    #[sqlx(rename = "status")]
    pub status_str: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub duration_ms: Option<i64>,

    // Results (for simple jobs)
    pub exit_code: Option<i32>,
    pub output: Option<String>,
    pub error: Option<String>,

    // Retry tracking
    pub retry_attempt: i32,
    pub is_retry: bool,

    // Notification tracking
    pub notification_sent: bool,
    pub notification_error: Option<String>,

    pub metadata: Option<String>,
}

impl JobRun {
    /// Get status as enum
    pub fn status(&self) -> Option<JobExecutionStatus> {
        JobExecutionStatus::from_str(&self.status_str)
    }

    /// Get metadata as JSON value
    pub fn get_metadata(&self) -> JsonValue {
        self.metadata
            .as_ref()
            .and_then(|m| serde_json::from_str(m).ok())
            .unwrap_or(JsonValue::Object(serde_json::Map::new()))
    }

    /// Calculate duration from timestamps if not set
    pub fn calculate_duration_ms(&self) -> Option<i64> {
        if let Some(duration) = self.duration_ms {
            return Some(duration);
        }

        if let Some(finished) = self.finished_at {
            let duration = finished.signed_duration_since(self.started_at);
            Some(duration.num_milliseconds())
        } else {
            None
        }
    }

    /// Get duration in seconds
    pub fn duration_seconds(&self) -> Option<f64> {
        self.calculate_duration_ms().map(|ms| ms as f64 / 1000.0)
    }

    /// Check if job run succeeded
    pub fn succeeded(&self) -> bool {
        self.status() == Some(JobExecutionStatus::Success)
    }

    /// Check if job run failed
    pub fn failed(&self) -> bool {
        matches!(
            self.status(),
            Some(JobExecutionStatus::Failure)
                | Some(JobExecutionStatus::Timeout)
                | Some(JobExecutionStatus::Cancelled)
        )
    }

    /// Check if job run is still running
    pub fn is_running(&self) -> bool {
        self.status() == Some(JobExecutionStatus::Running)
    }
}

/// ServerJobResult model - per-server execution results (for multi-server jobs)
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ServerJobResult {
    pub id: i64,
    pub job_run_id: i64,
    pub server_id: i64,

    #[sqlx(rename = "status")]
    pub status_str: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub duration_ms: Option<i64>,

    pub exit_code: Option<i32>,
    pub output: Option<String>,
    pub error: Option<String>,

    pub metadata: Option<String>,
}

impl ServerJobResult {
    /// Get status as enum
    pub fn status(&self) -> Option<JobExecutionStatus> {
        JobExecutionStatus::from_str(&self.status_str)
    }

    /// Get metadata as JSON value
    pub fn get_metadata(&self) -> JsonValue {
        self.metadata
            .as_ref()
            .and_then(|m| serde_json::from_str(m).ok())
            .unwrap_or(JsonValue::Object(serde_json::Map::new()))
    }

    /// Calculate duration from timestamps if not set
    pub fn calculate_duration_ms(&self) -> Option<i64> {
        if let Some(duration) = self.duration_ms {
            return Some(duration);
        }

        if let Some(finished) = self.finished_at {
            let duration = finished.signed_duration_since(self.started_at);
            Some(duration.num_milliseconds())
        } else {
            None
        }
    }

    /// Get duration in seconds
    pub fn duration_seconds(&self) -> Option<f64> {
        self.calculate_duration_ms().map(|ms| ms as f64 / 1000.0)
    }
}

/// StepExecutionResult model - for composite jobs, tracks each step's execution
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct StepExecutionResult {
    pub id: i64,
    pub job_run_id: i64,
    pub step_order: i32,
    pub step_name: String,
    pub command_template_id: i64,

    #[sqlx(rename = "status")]
    pub status_str: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub duration_ms: Option<i64>,

    pub exit_code: Option<i32>,
    pub output: Option<String>,
    pub error: Option<String>,

    pub metadata: Option<String>,
}

impl StepExecutionResult {
    /// Get status as enum
    pub fn status(&self) -> Option<JobExecutionStatus> {
        JobExecutionStatus::from_str(&self.status_str)
    }

    /// Get metadata as JSON value
    pub fn get_metadata(&self) -> JsonValue {
        self.metadata
            .as_ref()
            .and_then(|m| serde_json::from_str(m).ok())
            .unwrap_or(JsonValue::Object(serde_json::Map::new()))
    }

    /// Calculate duration from timestamps if not set
    pub fn calculate_duration_ms(&self) -> Option<i64> {
        if let Some(duration) = self.duration_ms {
            return Some(duration);
        }

        if let Some(finished) = self.finished_at {
            let duration = finished.signed_duration_since(self.started_at);
            Some(duration.num_milliseconds())
        } else {
            None
        }
    }

    /// Get duration in seconds
    pub fn duration_seconds(&self) -> Option<f64> {
        self.calculate_duration_ms().map(|ms| ms as f64 / 1000.0)
    }

    /// Check if step succeeded
    pub fn succeeded(&self) -> bool {
        self.status() == Some(JobExecutionStatus::Success)
    }

    /// Check if step failed
    pub fn failed(&self) -> bool {
        matches!(
            self.status(),
            Some(JobExecutionStatus::Failure)
                | Some(JobExecutionStatus::Timeout)
                | Some(JobExecutionStatus::Cancelled)
        )
    }

    /// Check if step is still running
    pub fn is_running(&self) -> bool {
        self.status() == Some(JobExecutionStatus::Running)
    }

    /// Check if step was skipped
    pub fn is_skipped(&self) -> bool {
        self.status_str == "skipped"
    }
}

/// JobRun with steps (for composite job display)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobRunWithSteps {
    #[serde(flatten)]
    pub job_run: JobRun,
    pub steps: Vec<StepExecutionResult>,
}

impl JobRunWithSteps {
    /// Get overall status based on steps
    pub fn aggregate_status(&self) -> JobExecutionStatus {
        if self.steps.is_empty() {
            return self.job_run.status().unwrap_or(JobExecutionStatus::Failure);
        }

        // If any step is running, overall is running
        if self.steps.iter().any(|s| s.is_running()) {
            return JobExecutionStatus::Running;
        }

        // If any step failed, overall is failure
        if self.steps.iter().any(|s| s.failed()) {
            return JobExecutionStatus::Failure;
        }

        // If all steps succeeded, overall is success
        if self.steps.iter().all(|s| s.succeeded()) {
            return JobExecutionStatus::Success;
        }

        // Otherwise, check the job run status
        self.job_run.status().unwrap_or(JobExecutionStatus::Failure)
    }

    /// Get total duration across all steps
    pub fn total_duration_ms(&self) -> i64 {
        self.steps
            .iter()
            .filter_map(|s| s.calculate_duration_ms())
            .sum()
    }

    /// Get progress percentage (0-100)
    pub fn progress_percentage(&self) -> u8 {
        if self.steps.is_empty() {
            return 0;
        }

        let completed = self.steps.iter().filter(|s| !s.is_running()).count();
        ((completed as f64 / self.steps.len() as f64) * 100.0) as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_execution_status() {
        assert_eq!(
            JobExecutionStatus::from_str("running"),
            Some(JobExecutionStatus::Running)
        );
        assert_eq!(
            JobExecutionStatus::from_str("success"),
            Some(JobExecutionStatus::Success)
        );
        assert_eq!(
            JobExecutionStatus::from_str("failure"),
            Some(JobExecutionStatus::Failure)
        );
        assert_eq!(
            JobExecutionStatus::from_str("timeout"),
            Some(JobExecutionStatus::Timeout)
        );
        assert_eq!(
            JobExecutionStatus::from_str("cancelled"),
            Some(JobExecutionStatus::Cancelled)
        );
        assert_eq!(JobExecutionStatus::from_str("invalid"), None);

        assert!(!JobExecutionStatus::Running.is_complete());
        assert!(JobExecutionStatus::Success.is_complete());

        assert!(JobExecutionStatus::Success.is_success());
        assert!(!JobExecutionStatus::Failure.is_success());

        assert!(JobExecutionStatus::Failure.is_failure());
        assert!(JobExecutionStatus::Timeout.is_failure());
        assert!(JobExecutionStatus::Cancelled.is_failure());
        assert!(!JobExecutionStatus::Success.is_failure());
    }

    #[test]
    fn test_job_run_duration() {
        let started = Utc::now();
        let finished = started + chrono::Duration::seconds(30);

        let job_run = JobRun {
            id: 1,
            job_schedule_id: 1,
            job_template_id: 1,
            server_id: 1,
            status_str: "success".to_string(),
            started_at: started,
            finished_at: Some(finished),
            duration_ms: None,
            exit_code: Some(0),
            output: None,
            error: None,
            retry_attempt: 0,
            is_retry: false,
            notification_sent: false,
            notification_error: None,
            metadata: None,
        };

        let duration = job_run.calculate_duration_ms().unwrap();
        assert!(duration >= 30000 && duration < 31000); // ~30 seconds in ms

        let duration_sec = job_run.duration_seconds().unwrap();
        assert!(duration_sec >= 30.0 && duration_sec < 31.0);
    }

    #[test]
    fn test_job_run_with_steps_aggregation() {
        let job_run = JobRun {
            id: 1,
            job_schedule_id: 1,
            job_template_id: 1,
            server_id: 1,
            status_str: "running".to_string(),
            started_at: Utc::now(),
            finished_at: None,
            duration_ms: None,
            exit_code: None,
            output: None,
            error: None,
            retry_attempt: 0,
            is_retry: false,
            notification_sent: false,
            notification_error: None,
            metadata: None,
        };

        let step1 = StepExecutionResult {
            id: 1,
            job_run_id: 1,
            step_order: 0,
            step_name: "Step 1".to_string(),
            command_template_id: 1,
            status_str: "success".to_string(),
            started_at: Utc::now(),
            finished_at: Some(Utc::now()),
            duration_ms: Some(1000),
            exit_code: Some(0),
            output: None,
            error: None,
            metadata: None,
        };

        let step2 = StepExecutionResult {
            id: 2,
            job_run_id: 1,
            step_order: 1,
            step_name: "Step 2".to_string(),
            command_template_id: 2,
            status_str: "running".to_string(),
            started_at: Utc::now(),
            finished_at: None,
            duration_ms: None,
            exit_code: None,
            output: None,
            error: None,
            metadata: None,
        };

        let job_with_steps = JobRunWithSteps {
            job_run,
            steps: vec![step1, step2],
        };

        // One step still running, so overall is running
        assert_eq!(job_with_steps.aggregate_status(), JobExecutionStatus::Running);

        // Progress should be 50% (1 of 2 steps complete)
        assert_eq!(job_with_steps.progress_percentage(), 50);
    }
}
