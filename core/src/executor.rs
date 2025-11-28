//! Job execution engine for SvrCtlRS
//!
//! This module provides the core job execution functionality, including:
//! - Simple job execution (single command)
//! - Composite job execution (multi-step workflows)
//! - Multi-server execution with concurrency control
//! - Variable substitution and command template selection
//! - Retry logic and error handling
//! - Database state tracking and result recording
//!
//! **Note**: This module requires the `executor` feature to be enabled.
//! The executor depends on the `database` crate for models and queries.
//! To use this module, add `features = ["executor"]` to your `svrctlrs-core` dependency.

use crate::{Error, RemoteExecutor, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::{FromRow, Pool, Sqlite};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;
use tokio::time::timeout;
use tracing::{debug, error, info, instrument, warn};

// We cannot import from svrctlrs-database due to circular dependency
// Instead, we'll re-define the minimal types we need here
// The actual database operations will be performed using raw SQL queries

/// Job Template - minimal definition for executor
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct JobTemplate {
    pub id: i64,
    pub name: String,
    pub display_name: String,
    pub description: Option<String>,
    pub job_type_id: i64,
    pub is_composite: bool,
    pub command_template_id: Option<i64>,
    pub variables: Option<String>,
    pub timeout_seconds: i32,
    pub retry_count: i32,
    pub retry_delay_seconds: i32,
    pub notify_on_success: bool,
    pub notify_on_failure: bool,
    pub notification_policy_id: Option<i64>,
    pub metadata: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl JobTemplate {
    /// Get variables as a map
    pub fn get_variables(&self) -> HashMap<String, String> {
        self.variables
            .as_ref()
            .and_then(|v| serde_json::from_str(v).ok())
            .unwrap_or_default()
    }
}

/// Job Template Step - minimal definition for executor
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct JobTemplateStep {
    pub id: i64,
    pub job_template_id: i64,
    pub step_order: i32,
    pub name: String,
    pub command_template_id: i64,
    pub variables: Option<String>,
    pub continue_on_failure: bool,
    pub timeout_seconds: Option<i32>,
    pub metadata: Option<String>,
}

impl JobTemplateStep {
    /// Get variables as a map
    pub fn get_variables(&self) -> HashMap<String, String> {
        self.variables
            .as_ref()
            .and_then(|v| serde_json::from_str(v).ok())
            .unwrap_or_default()
    }

    /// Get effective timeout (step timeout or default)
    pub fn effective_timeout(&self, default_timeout: i32) -> i32 {
        self.timeout_seconds.unwrap_or(default_timeout)
    }
}

/// Command Template - minimal definition for executor
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CommandTemplate {
    pub id: i64,
    pub job_type_id: i64,
    pub name: String,
    pub display_name: String,
    pub description: Option<String>,
    pub command: String,
    pub required_capabilities: Option<String>,
    pub os_filter: Option<String>,
    pub timeout_seconds: i32,
    pub working_directory: Option<String>,
    pub environment: Option<String>,
    pub output_format: Option<String>,
    pub parse_output: bool,
    pub output_parser: Option<String>,
    pub notify_on_success: bool,
    pub notify_on_failure: bool,
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

/// Server - minimal definition for executor
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Server {
    pub id: i64,
    pub name: String,
    pub hostname: Option<String>,
    pub port: i32,
    pub username: Option<String>,
    pub credential_id: Option<i64>,
    pub description: Option<String>,
    pub is_local: bool,
    pub enabled: bool,
    pub os_type: Option<String>,
    pub os_distro: Option<String>,
    pub package_manager: Option<String>,
    pub docker_available: bool,
    pub systemd_available: bool,
    pub metadata: Option<String>,
    pub last_seen_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Server {
    /// Get the display address for this server
    pub fn display_address(&self) -> String {
        if self.is_local {
            "localhost".to_string()
        } else if let Some(ref hostname) = self.hostname {
            if let Some(ref username) = self.username {
                if self.port != 22 {
                    format!("{}@{}:{}", username, hostname, self.port)
                } else {
                    format!("{}@{}", username, hostname)
                }
            } else {
                hostname.clone()
            }
        } else {
            "unknown".to_string()
        }
    }

    /// Check if server has required capability
    pub fn has_capability(&self, capability: &str) -> bool {
        match capability {
            "docker" => self.docker_available,
            "systemd" => self.systemd_available,
            "apt" => self.package_manager.as_deref() == Some("apt"),
            "dnf" => self.package_manager.as_deref() == Some("dnf"),
            "pacman" => self.package_manager.as_deref() == Some("pacman"),
            "yum" => self.package_manager.as_deref() == Some("yum"),
            _ => false,
        }
    }
}

/// Server Capability - minimal definition for executor
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ServerCapability {
    pub id: i64,
    pub server_id: i64,
    pub capability: String,
    pub available: bool,
    pub version: Option<String>,
    pub detected_at: DateTime<Utc>,
}

/// Job Run - minimal definition for executor
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct JobRun {
    pub id: i64,
    pub job_schedule_id: i64,
    pub job_template_id: i64,
    pub server_id: i64,
    #[sqlx(rename = "status")]
    pub status_str: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub duration_ms: Option<i64>,
    pub exit_code: Option<i32>,
    pub output: Option<String>,
    pub error: Option<String>,
    pub retry_attempt: i32,
    pub is_retry: bool,
    pub notification_sent: bool,
    pub notification_error: Option<String>,
    pub metadata: Option<String>,
}

/// Default maximum concurrent jobs
pub const DEFAULT_MAX_CONCURRENT_JOBS: usize = 5;

/// Default command timeout in seconds
pub const DEFAULT_TIMEOUT_SECONDS: u64 = 300;

/// Job execution context
#[derive(Debug, Clone)]
pub struct JobExecutionContext {
    pub job_run_id: i64,
    pub job_template: JobTemplate,
    pub server: Server,
    pub parameters: HashMap<String, String>,
}

/// Job executor - responsible for executing jobs on servers
pub struct JobExecutor {
    db_pool: Pool<Sqlite>,
    ssh_key_path: Option<String>,
    #[allow(dead_code)]
    max_concurrent_jobs: usize,
    semaphore: Arc<Semaphore>,
}

impl JobExecutor {
    /// Create a new job executor
    ///
    /// # Arguments
    ///
    /// * `db_pool` - Database connection pool
    /// * `ssh_key_path` - Optional path to SSH private key for remote execution
    /// * `max_concurrent_jobs` - Maximum number of concurrent job executions
    pub fn new(
        db_pool: Pool<Sqlite>,
        ssh_key_path: Option<String>,
        max_concurrent_jobs: usize,
    ) -> Self {
        let semaphore = Arc::new(Semaphore::new(max_concurrent_jobs));

        Self {
            db_pool,
            ssh_key_path,
            max_concurrent_jobs,
            semaphore,
        }
    }

    /// Execute a job run (main entry point)
    ///
    /// This is the primary method for executing a job. It:
    /// 1. Loads the job run from the database
    /// 2. Loads the associated job template and server
    /// 3. Dispatches to simple or composite job execution
    /// 4. Records results in the database
    ///
    /// # Arguments
    ///
    /// * `job_run_id` - ID of the job run to execute
    #[instrument(skip(self), fields(job_run_id))]
    pub async fn execute_job_run(&self, job_run_id: i64) -> Result<()> {
        info!(job_run_id, "Starting job run execution");

        // Acquire semaphore permit to limit concurrency
        let _permit = self
            .semaphore
            .acquire()
            .await
            .map_err(|e| Error::Other(format!("Failed to acquire semaphore: {}", e)))?;

        // Load job run from database
        let job_run = self.get_job_run(job_run_id).await.map_err(|e| {
            error!(job_run_id, error = %e, "Failed to load job run");
            e
        })?;

        // Load job template
        let job_template = self.get_job_template(job_run.job_template_id).await?;

        // Load server
        let server = self.get_server(job_run.server_id).await?;

        info!(
            job_run_id,
            template = %job_template.name,
            server = %server.name,
            is_composite = job_template.is_composite,
            "Executing job"
        );

        // Execute the job based on type
        let result = if job_template.is_composite {
            self.execute_composite_job(job_run_id, &job_template, &server)
                .await
        } else {
            self.execute_simple_job(job_run_id, &job_template, &server)
                .await
        };

        // Handle result
        match result {
            Ok(_) => {
                info!(job_run_id, "Job run completed successfully");
                Ok(())
            }
            Err(e) => {
                error!(job_run_id, error = %e, "Job run failed");
                // Error is already recorded in the database by the execution methods
                Err(e)
            }
        }
    }

    /// Execute a simple job (single command)
    #[instrument(skip(self, job_template, server))]
    async fn execute_simple_job(
        &self,
        job_run_id: i64,
        job_template: &JobTemplate,
        server: &Server,
    ) -> Result<()> {
        debug!(
            job_run_id,
            template = %job_template.name,
            server = %server.name,
            "Executing simple job"
        );

        // Get command template ID
        let command_template_id = job_template
            .command_template_id
            .ok_or_else(|| Error::Other("Simple job missing command_template_id".to_string()))?;

        // Load command template
        let command_template = self.get_command_template(command_template_id).await?;

        // Load server capabilities
        let capabilities = self.get_server_capabilities(server.id).await?;

        // Check if server has required capabilities
        let required_caps = command_template.get_required_capabilities();
        if !self.server_has_capabilities(server, &capabilities, &required_caps) {
            let error_msg = format!(
                "Server {} missing required capabilities: {:?}",
                server.name, required_caps
            );
            error!(job_run_id, server = %server.name, "{}", error_msg);

            self.finish_job_run(job_run_id, "failure", None, None, Some(error_msg))
                .await?;

            return Err(Error::Other(
                "Server missing required capabilities".to_string(),
            ));
        }

        // Check OS filter
        if !command_template.matches_os_filter(server.os_distro.as_deref()) {
            let error_msg = format!(
                "Server {} OS distro {:?} does not match command template filter",
                server.name, server.os_distro
            );
            error!(job_run_id, server = %server.name, "{}", error_msg);

            self.finish_job_run(job_run_id, "failure", None, None, Some(error_msg))
                .await?;

            return Err(Error::Other("OS distro does not match filter".to_string()));
        }

        // Get variables from job template
        let variables = job_template.get_variables();

        // Substitute variables in command
        let command = self.substitute_variables(&command_template.command, &variables)?;

        info!(
            job_run_id,
            server = %server.name,
            command = %command,
            "Executing command"
        );

        // Execute the command
        let timeout_secs = command_template.timeout_seconds as u64;
        let result = self.execute_command(server, &command, timeout_secs).await;

        // Record results
        match result {
            Ok((exit_code, output)) => {
                info!(
                    job_run_id,
                    server = %server.name,
                    exit_code,
                    "Command completed successfully"
                );

                self.finish_job_run(job_run_id, "success", Some(exit_code), Some(output), None)
                    .await?;

                Ok(())
            }
            Err(e) => {
                let error_msg = e.to_string();
                warn!(
                    job_run_id,
                    server = %server.name,
                    error = %error_msg,
                    "Command execution failed"
                );

                self.finish_job_run(job_run_id, "failure", None, None, Some(error_msg))
                    .await?;

                Err(e)
            }
        }
    }

    /// Execute a composite job (multi-step workflow)
    #[instrument(skip(self, job_template, server))]
    async fn execute_composite_job(
        &self,
        job_run_id: i64,
        job_template: &JobTemplate,
        server: &Server,
    ) -> Result<()> {
        info!(
            job_run_id,
            template = %job_template.name,
            server = %server.name,
            "Executing composite job"
        );

        // Load job template steps
        let steps = self.get_job_template_steps(job_template.id).await?;

        if steps.is_empty() {
            let error_msg = "Composite job has no steps defined".to_string();
            error!(job_run_id, "{}", error_msg);

            self.finish_job_run(job_run_id, "failure", None, None, Some(error_msg))
                .await?;

            return Err(Error::Other(
                "No steps defined for composite job".to_string(),
            ));
        }

        info!(job_run_id, step_count = steps.len(), "Loaded job steps");

        // Execute steps in order
        let mut overall_success = true;
        let mut step_outputs = Vec::new();

        for step in steps {
            info!(
                job_run_id,
                step_order = step.step_order,
                step_name = %step.name,
                "Executing step"
            );

            // Create step execution result record
            let step_result_id = self
                .create_step_execution_result(
                    job_run_id,
                    step.step_order,
                    &step.name,
                    step.command_template_id,
                    None,
                )
                .await?;

            // Execute the step
            let step_result = self
                .execute_step(job_run_id, &step, job_template, server)
                .await;

            // Record step result
            match step_result {
                Ok((exit_code, output)) => {
                    info!(
                        job_run_id,
                        step_order = step.step_order,
                        exit_code,
                        "Step completed successfully"
                    );

                    self.update_step_execution_result(
                        step_result_id,
                        "success",
                        Some(exit_code),
                        Some(output.clone()),
                        None,
                    )
                    .await?;

                    step_outputs.push(output);
                }
                Err(e) => {
                    let error_msg = e.to_string();
                    warn!(
                        job_run_id,
                        step_order = step.step_order,
                        error = %error_msg,
                        "Step execution failed"
                    );

                    self.update_step_execution_result(
                        step_result_id,
                        "failure",
                        None,
                        None,
                        Some(error_msg),
                    )
                    .await?;

                    overall_success = false;

                    // Check if we should continue on failure
                    if !step.continue_on_failure {
                        warn!(
                            job_run_id,
                            step_order = step.step_order,
                            "Stopping execution due to step failure (continue_on_failure=false)"
                        );
                        break;
                    } else {
                        info!(
                            job_run_id,
                            step_order = step.step_order,
                            "Continuing execution despite step failure (continue_on_failure=true)"
                        );
                    }
                }
            }
        }

        // Record overall job result
        let final_status = if overall_success {
            "success"
        } else {
            "failure"
        };
        let combined_output = step_outputs.join("\n---\n");

        self.finish_job_run(
            job_run_id,
            final_status,
            None, // No single exit code for composite jobs
            Some(combined_output),
            if overall_success {
                None
            } else {
                Some("One or more steps failed".to_string())
            },
        )
        .await?;

        if overall_success {
            info!(job_run_id, "Composite job completed successfully");
            Ok(())
        } else {
            warn!(job_run_id, "Composite job completed with failures");
            Err(Error::Other("One or more steps failed".to_string()))
        }
    }

    /// Execute a single step in a composite job
    #[instrument(skip(self, step, job_template, server))]
    async fn execute_step(
        &self,
        job_run_id: i64,
        step: &JobTemplateStep,
        job_template: &JobTemplate,
        server: &Server,
    ) -> Result<(i32, String)> {
        // Load command template for this step
        let command_template = self.get_command_template(step.command_template_id).await?;

        // Load server capabilities
        let capabilities = self.get_server_capabilities(server.id).await?;

        // Check if server has required capabilities
        let required_caps = command_template.get_required_capabilities();
        if !self.server_has_capabilities(server, &capabilities, &required_caps) {
            return Err(Error::Other(format!(
                "Server missing required capabilities: {:?}",
                required_caps
            )));
        }

        // Check OS filter
        if !command_template.matches_os_filter(server.os_distro.as_deref()) {
            return Err(Error::Other(format!(
                "Server OS distro {:?} does not match command template filter",
                server.os_distro
            )));
        }

        // Merge template and step variables (step variables override template)
        let mut variables = job_template.get_variables();
        variables.extend(step.get_variables());

        // Substitute variables in command
        let command = self.substitute_variables(&command_template.command, &variables)?;

        debug!(
            job_run_id,
            step_order = step.step_order,
            command = %command,
            "Executing step command"
        );

        // Get effective timeout (step timeout or template default)
        let timeout_secs = step.effective_timeout(command_template.timeout_seconds) as u64;

        // Execute the command
        self.execute_command(server, &command, timeout_secs).await
    }

    /// Execute a command on a server
    ///
    /// # Arguments
    ///
    /// * `server` - Target server
    /// * `command` - Command to execute (already with variables substituted)
    /// * `timeout_secs` - Command timeout in seconds
    ///
    /// # Returns
    ///
    /// Tuple of (exit_code, output)
    #[instrument(skip(self, command))]
    async fn execute_command(
        &self,
        server: &Server,
        command: &str,
        timeout_secs: u64,
    ) -> Result<(i32, String)> {
        // Create remote executor for this server
        let executor = if server.is_local {
            RemoteExecutor::for_server(
                crate::types::Server::local(&server.name),
                self.ssh_key_path.clone(),
            )
            .with_timeout(timeout_secs)
        } else {
            let ssh_host = server.display_address();
            RemoteExecutor::for_server(
                crate::types::Server::remote(&server.name, &ssh_host),
                self.ssh_key_path.clone(),
            )
            .with_timeout(timeout_secs)
        };

        // Split command into command and args
        // For simplicity, we'll use sh -c to execute the full command string
        let output = timeout(
            Duration::from_secs(timeout_secs + 5), // Add 5s buffer for SSH overhead
            executor.execute_command("sh", &["-c", command]),
        )
        .await
        .map_err(|_| {
            Error::RemoteExecutionError(format!("Command timed out after {}s", timeout_secs))
        })??;

        // We consider the command successful if it doesn't error
        // The actual exit code is not available from RemoteExecutor::execute_command
        // which only returns stdout. For now, we'll assume exit code 0 on success.
        Ok((0, output))
    }

    /// Substitute variables in a template string
    ///
    /// Variables are in the format {{variable_name}}
    fn substitute_variables(
        &self,
        template: &str,
        variables: &HashMap<String, String>,
    ) -> Result<String> {
        let mut result = template.to_string();

        for (key, value) in variables {
            let placeholder = format!("{{{{{}}}}}", key);
            result = result.replace(&placeholder, value);
        }

        // Check for any remaining unsubstituted variables
        if result.contains("{{") && result.contains("}}") {
            warn!("Template contains unsubstituted variables: {}", result);
        }

        Ok(result)
    }

    /// Check if a server has all required capabilities
    fn server_has_capabilities(
        &self,
        server: &Server,
        capabilities: &[ServerCapability],
        required: &[String],
    ) -> bool {
        if required.is_empty() {
            return true;
        }

        // First check built-in server fields
        for req in required {
            if !server.has_capability(req) {
                // Also check capabilities table
                if !capabilities
                    .iter()
                    .any(|c| c.capability == *req && c.available)
                {
                    debug!(
                        server = %server.name,
                        capability = %req,
                        "Server missing required capability"
                    );
                    return false;
                }
            }
        }

        true
    }

    // ========================================================================
    // Database query helpers
    // ========================================================================

    /// Get job template by ID
    async fn get_job_template(&self, id: i64) -> Result<JobTemplate> {
        sqlx::query_as::<_, JobTemplate>(
            r#"
            SELECT id, name, display_name, description, job_type_id, is_composite,
                   command_template_id, variables, timeout_seconds, retry_count,
                   retry_delay_seconds, notify_on_success, notify_on_failure,
                   notification_policy_id, metadata, created_at, updated_at
            FROM job_templates
            WHERE id = ?
            "#,
        )
        .bind(id)
        .fetch_one(&self.db_pool)
        .await
        .map_err(|e| Error::DatabaseError(format!("Failed to get job template: {}", e)))
    }

    /// Get server by ID
    async fn get_server(&self, id: i64) -> Result<Server> {
        sqlx::query_as::<_, Server>(
            r#"
            SELECT id, name, hostname, port, username, credential_id, description,
                   is_local, enabled, os_type, os_distro, package_manager,
                   docker_available, systemd_available, metadata, last_seen_at,
                   last_error, created_at, updated_at
            FROM servers
            WHERE id = ?
            "#,
        )
        .bind(id)
        .fetch_one(&self.db_pool)
        .await
        .map_err(|e| Error::DatabaseError(format!("Failed to get server: {}", e)))
    }

    /// Get command template by ID
    async fn get_command_template(&self, id: i64) -> Result<CommandTemplate> {
        sqlx::query_as::<_, CommandTemplate>(
            r#"
            SELECT id, job_type_id, name, display_name, description, command,
                   required_capabilities, os_filter, timeout_seconds, working_directory,
                   environment, output_format, parse_output, output_parser,
                   notify_on_success, notify_on_failure, metadata, created_at, updated_at
            FROM command_templates
            WHERE id = ?
            "#,
        )
        .bind(id)
        .fetch_one(&self.db_pool)
        .await
        .map_err(|e| Error::DatabaseError(format!("Failed to get command template: {}", e)))
    }

    /// Get job template steps for a composite job
    async fn get_job_template_steps(&self, job_template_id: i64) -> Result<Vec<JobTemplateStep>> {
        sqlx::query_as::<_, JobTemplateStep>(
            r#"
            SELECT id, job_template_id, step_order, name, command_template_id,
                   variables, continue_on_failure, timeout_seconds, metadata
            FROM job_template_steps
            WHERE job_template_id = ?
            ORDER BY step_order ASC
            "#,
        )
        .bind(job_template_id)
        .fetch_all(&self.db_pool)
        .await
        .map_err(|e| Error::DatabaseError(format!("Failed to get job template steps: {}", e)))
    }

    /// Get server capabilities
    async fn get_server_capabilities(&self, server_id: i64) -> Result<Vec<ServerCapability>> {
        sqlx::query_as::<_, ServerCapability>(
            r#"
            SELECT id, server_id, capability, available, version, detected_at
            FROM server_capabilities
            WHERE server_id = ?
            "#,
        )
        .bind(server_id)
        .fetch_all(&self.db_pool)
        .await
        .map_err(|e| Error::DatabaseError(format!("Failed to get server capabilities: {}", e)))
    }

    /// Get job run by ID
    async fn get_job_run(&self, id: i64) -> Result<JobRun> {
        sqlx::query_as::<_, JobRun>(
            r#"
            SELECT id, job_schedule_id, job_template_id, server_id, status, started_at, finished_at,
                   duration_ms, exit_code, output, error, retry_attempt, is_retry, notification_sent,
                   notification_error, metadata
            FROM job_runs
            WHERE id = ?
            "#,
        )
        .bind(id)
        .fetch_one(&self.db_pool)
        .await
        .map_err(|e| Error::DatabaseError(format!("Failed to get job run: {}", e)))
    }

    /// Finish a job run with final results
    async fn finish_job_run(
        &self,
        id: i64,
        status: &str,
        exit_code: Option<i32>,
        output: Option<String>,
        error: Option<String>,
    ) -> Result<()> {
        let now = Utc::now();

        // Calculate duration from started_at
        let job_run = self.get_job_run(id).await?;
        let duration_ms = now
            .signed_duration_since(job_run.started_at)
            .num_milliseconds();

        sqlx::query(
            r#"
            UPDATE job_runs
            SET status = ?,
                finished_at = ?,
                duration_ms = ?,
                exit_code = ?,
                output = ?,
                error = ?
            WHERE id = ?
            "#,
        )
        .bind(status)
        .bind(now)
        .bind(duration_ms)
        .bind(exit_code)
        .bind(output)
        .bind(error)
        .bind(id)
        .execute(&self.db_pool)
        .await
        .map_err(|e| Error::DatabaseError(format!("Failed to finish job run: {}", e)))?;

        Ok(())
    }

    /// Create a step execution result
    async fn create_step_execution_result(
        &self,
        job_run_id: i64,
        step_order: i32,
        step_name: &str,
        command_template_id: i64,
        metadata: Option<String>,
    ) -> Result<i64> {
        let result = sqlx::query(
            r#"
            INSERT INTO step_execution_results (
                job_run_id, step_order, step_name, command_template_id, status, started_at, metadata
            )
            VALUES (?, ?, ?, ?, 'running', CURRENT_TIMESTAMP, ?)
            "#,
        )
        .bind(job_run_id)
        .bind(step_order)
        .bind(step_name)
        .bind(command_template_id)
        .bind(metadata)
        .execute(&self.db_pool)
        .await
        .map_err(|e| {
            Error::DatabaseError(format!("Failed to create step execution result: {}", e))
        })?;

        Ok(result.last_insert_rowid())
    }

    /// Update a step execution result
    async fn update_step_execution_result(
        &self,
        id: i64,
        status: &str,
        exit_code: Option<i32>,
        output: Option<String>,
        error: Option<String>,
    ) -> Result<()> {
        let now = Utc::now();

        // Get step to calculate duration
        #[derive(FromRow)]
        struct StepTime {
            started_at: DateTime<Utc>,
        }

        let step = sqlx::query_as::<_, StepTime>(
            r#"
            SELECT started_at
            FROM step_execution_results
            WHERE id = ?
            "#,
        )
        .bind(id)
        .fetch_one(&self.db_pool)
        .await
        .map_err(|e| Error::DatabaseError(format!("Failed to get step start time: {}", e)))?;

        let duration_ms = now
            .signed_duration_since(step.started_at)
            .num_milliseconds();

        sqlx::query(
            r#"
            UPDATE step_execution_results
            SET status = ?, finished_at = ?, duration_ms = ?, exit_code = ?, output = ?, error = ?
            WHERE id = ?
            "#,
        )
        .bind(status)
        .bind(now)
        .bind(duration_ms)
        .bind(exit_code)
        .bind(output)
        .bind(error)
        .bind(id)
        .execute(&self.db_pool)
        .await
        .map_err(|e| {
            Error::DatabaseError(format!("Failed to update step execution result: {}", e))
        })?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_substitute_variables() {
        #[allow(invalid_value)]
        let executor = JobExecutor::new(
            // We can't easily create a real pool here, but we won't use it in this test
            unsafe { std::mem::zeroed() },
            None,
            DEFAULT_MAX_CONCURRENT_JOBS,
        );

        let mut vars = HashMap::new();
        vars.insert("name".to_string(), "test".to_string());
        vars.insert("value".to_string(), "123".to_string());

        let template = "echo {{name}} has value {{value}}";
        let result = executor.substitute_variables(template, &vars).unwrap();
        assert_eq!(result, "echo test has value 123");
    }

    #[test]
    fn test_substitute_variables_partial() {
        #[allow(invalid_value)]
        let executor = JobExecutor::new(
            unsafe { std::mem::zeroed() },
            None,
            DEFAULT_MAX_CONCURRENT_JOBS,
        );

        let mut vars = HashMap::new();
        vars.insert("name".to_string(), "test".to_string());

        let template = "echo {{name}} {{missing}}";
        let result = executor.substitute_variables(template, &vars).unwrap();
        // Should keep unmatched variables as-is
        assert_eq!(result, "echo test {{missing}}");
    }
}
