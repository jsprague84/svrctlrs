//! Task execution engine
//!
//! Handles execution of scheduled tasks, including:
//! - Plugin task execution
//! - SSH command execution on remote servers
//! - Task history tracking
//! - Error handling and retries

use anyhow::{Context, Result};
use serde_json::Value as JsonValue;
use std::time::Instant;
use tracing::{debug, error, info, warn};

use svrctlrs_database::{
    models::task::{Task, TaskHistoryEntry},
    queries,
};

use crate::{
    features::{self, FeatureResult},
    ssh::{self, SshConfig},
    state::AppState,
};

/// Execute a task by ID
pub async fn execute_task(state: &AppState, task_id: i64) -> Result<TaskExecutionResult> {
    let start_time = Instant::now();

    info!("Starting execution of task {}", task_id);

    // Load task from database
    let db = state.db().await;
    let task = queries::tasks::get_task(db.pool(), task_id)
        .await
        .context("Failed to load task")?;

    if !task.enabled {
        warn!("Task {} is disabled, skipping execution", task_id);
        return Ok(TaskExecutionResult {
            task_id,
            success: false,
            output: "Task is disabled".to_string(),
            error: None,
            duration_ms: start_time.elapsed().as_millis() as u64,
        });
    }

    // Execute based on task type
    // New architecture: Use features with RemoteExecutor for both local and remote
    let result = if task.feature_id == "ssh" || task.feature_id.starts_with("ssh:") {
        // Legacy SSH command task - execute directly
        if let Some(server_id) = task.server_id {
            execute_remote_task(state, &task, server_id).await
        } else {
            anyhow::bail!("SSH tasks require a server_id")
        }
    } else if task.feature_id == "docker" {
        // New feature-based execution: Docker monitoring
        execute_docker_feature(state, &task).await
    } else if task.feature_id == "updates" {
        // New feature-based execution: Updates monitoring
        execute_updates_feature(state, &task).await
    } else if task.feature_id == "health" {
        // New feature-based execution: Health monitoring
        execute_health_feature(state, &task).await
    } else {
        // Unknown feature - return error
        anyhow::bail!("Unknown feature: {}. Supported features: ssh, docker, updates, health", task.feature_id)
    };

    let duration_ms = start_time.elapsed().as_millis() as u64;

    // Record execution in task history and update stats atomically
    let history_entry = TaskHistoryEntry {
        task_id,
        feature_id: task.feature_id.clone(),
        server_id: task.server_id,
        success: result.is_ok(),
        output: result.as_ref().map(|s| s.clone()).unwrap_or_default(),
        error: result.as_ref().err().map(|e| e.to_string()),
        duration_ms,
        executed_at: chrono::Utc::now(),
    };

    // Use transaction to ensure atomicity of history recording and stats update
    if let Err(e) =
        queries::tasks::record_task_execution_with_stats(db.pool(), &history_entry).await
    {
        error!("Failed to record task execution and update stats: {}", e);
    }

    // Calculate and update next run time after execution
    match queries::tasks::calculate_next_run(&task.schedule) {
        Ok(next_run) => {
            if let Err(e) = queries::tasks::update_task_next_run(db.pool(), task_id, next_run).await
            {
                warn!("Failed to update next_run_at for task {}: {}", task_id, e);
            } else {
                debug!("Updated next_run_at for task {}: {:?}", task_id, next_run);
            }
        }
        Err(e) => {
            warn!(
                "Failed to calculate next_run_at for task {}: {}",
                task_id, e
            );
        }
    }

    match result {
        Ok(output) => {
            info!(
                "Task {} completed successfully in {}ms",
                task_id, duration_ms
            );
            Ok(TaskExecutionResult {
                task_id,
                success: true,
                output,
                error: None,
                duration_ms,
            })
        }
        Err(e) => {
            error!("Task {} failed after {}ms: {}", task_id, duration_ms, e);
            Ok(TaskExecutionResult {
                task_id,
                success: false,
                output: String::new(),
                error: Some(e.to_string()),
                duration_ms,
            })
        }
    }
}

/// Execute a task on a remote server via SSH
async fn execute_remote_task(state: &AppState, task: &Task, server_id: i64) -> Result<String> {
    debug!("Executing remote task {} on server {}", task.id, server_id);

    // Load server configuration
    let db = state.db().await;
    let server = queries::servers::get_server(db.pool(), server_id)
        .await
        .context("Failed to load server")?;

    if !server.enabled {
        anyhow::bail!("Server {} is disabled", server_id);
    }

    // Build SSH configuration
    let host = server
        .host
        .ok_or_else(|| anyhow::anyhow!("Server {} has no host configured", server.name))?;
    let ssh_config = SshConfig {
        host,
        port: server.port as u16,
        username: server.username,
        key_path: server.ssh_key_path,
        timeout: std::time::Duration::from_secs(task.timeout as u64),
    };

    // Build command with args
    let command = if let Some(args_str) = &task.args {
        // Parse args as JSON and append to command
        match serde_json::from_str::<JsonValue>(args_str) {
            Ok(JsonValue::Array(args)) => {
                let arg_strings: Vec<String> = args
                    .iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect();
                format!("{} {}", task.command, arg_strings.join(" "))
            }
            Ok(JsonValue::Object(map)) => {
                // Convert object to command-line flags
                let flags: Vec<String> = map
                    .iter()
                    .map(|(k, v)| {
                        if let Some(s) = v.as_str() {
                            format!("--{} {}", k, s)
                        } else {
                            format!("--{} {}", k, v)
                        }
                    })
                    .collect();
                format!("{} {}", task.command, flags.join(" "))
            }
            _ => task.command.clone(),
        }
    } else {
        task.command.clone()
    };

    info!("Executing command on {}: {}", server.name, command);

    // Execute command
    let output = ssh::execute_command(&ssh_config, &command)
        .await
        .context("Failed to execute command")?;

    if !output.success {
        anyhow::bail!(
            "Command failed with exit code {}: {}",
            output.exit_code,
            output.stderr
        );
    }

    Ok(output.stdout)
}

/// Helper: Load server from database or create localhost server
async fn load_server_for_task(state: &AppState, task: &Task) -> Result<svrctlrs_core::Server> {
    use svrctlrs_core::Server as CoreServer;

    let db = state.db().await;
    if let Some(server_id) = task.server_id {
        let db_server = queries::servers::get_server(db.pool(), server_id)
            .await
            .context("Failed to load server")?;

        if !db_server.enabled {
            anyhow::bail!("Server {} is disabled", db_server.name);
        }

        // Build SSH host string
        let ssh_host = db_server.host.map(|host| {
            if db_server.port != 22 {
                format!("{}@{}:{}", db_server.username, host, db_server.port)
            } else {
                format!("{}@{}", db_server.username, host)
            }
        });

        Ok(CoreServer {
            name: db_server.name,
            ssh_host,
        })
    } else {
        // Localhost execution
        Ok(CoreServer {
            name: "localhost".to_string(),
            ssh_host: None,
        })
    }
}

/// Execute a Docker feature task
async fn execute_docker_feature(state: &AppState, task: &Task) -> Result<String> {
    use svrctlrs_core::RemoteExecutor;

    debug!(
        "Executing Docker feature task {} ({})",
        task.command, task.name
    );

    let server = load_server_for_task(state, task).await?;
    let ssh_key_path = std::env::var("SSH_KEY_PATH").ok();
    let executor = RemoteExecutor::new(ssh_key_path);
    let notify = state.notification_manager().await;
    let config = features::docker::DockerConfig::default();

    let result: FeatureResult = match task.command.as_str() {
        "docker_health" => {
            features::docker::check_health(&server, &executor, &notify, &config).await?
        }
        "docker_images_report" => {
            features::docker::check_image_updates(&server, &executor, &notify).await?
        }
        _ => {
            anyhow::bail!("Unknown Docker feature task: {}", task.command);
        }
    };

    if result.success {
        Ok(result.message)
    } else {
        anyhow::bail!("Docker feature failed: {}", result.message);
    }
}

/// Execute an Updates feature task
async fn execute_updates_feature(state: &AppState, task: &Task) -> Result<String> {
    use svrctlrs_core::RemoteExecutor;

    debug!(
        "Executing Updates feature task {} ({})",
        task.command, task.name
    );

    let server = load_server_for_task(state, task).await?;
    let ssh_key_path = std::env::var("SSH_KEY_PATH").ok();
    let executor = RemoteExecutor::new(ssh_key_path);
    let notify = state.notification_manager().await;
    let config = features::updates::UpdatesConfig::default();

    let result: FeatureResult = match task.command.as_str() {
        "updates_check" => {
            features::updates::check_updates(&server, &executor, &notify, &config).await?
        }
        "updates_report" => {
            // Load all enabled servers for multi-server report
            let db = state.db().await;
            let db_servers = queries::servers::list_servers(db.pool())
                .await
                .context("Failed to load servers")?;

            let servers: Vec<svrctlrs_core::Server> = db_servers
                .into_iter()
                .filter(|s| s.enabled)
                .map(|s| {
                    let ssh_host = s.host.map(|host| {
                        if s.port != 22 {
                            format!("{}@{}:{}", s.username, host, s.port)
                        } else {
                            format!("{}@{}", s.username, host)
                        }
                    });
                    svrctlrs_core::Server {
                        name: s.name,
                        ssh_host,
                    }
                })
                .collect();

            features::updates::generate_updates_report(&servers, &executor, &notify).await?
        }
        _ => {
            anyhow::bail!("Unknown Updates feature task: {}", task.command);
        }
    };

    if result.success {
        Ok(result.message)
    } else {
        anyhow::bail!("Updates feature failed: {}", result.message);
    }
}

/// Execute a Health feature task
async fn execute_health_feature(state: &AppState, task: &Task) -> Result<String> {
    use svrctlrs_core::RemoteExecutor;

    debug!(
        "Executing Health feature task {} ({})",
        task.command, task.name
    );

    let server = load_server_for_task(state, task).await?;
    let ssh_key_path = std::env::var("SSH_KEY_PATH").ok();
    let executor = RemoteExecutor::new(ssh_key_path);
    let notify = state.notification_manager().await;
    let config = features::health::HealthConfig::default();

    let result: FeatureResult = match task.command.as_str() {
        "system_metrics" => {
            features::health::collect_metrics(&server, &executor, &notify, &config).await?
        }
        _ => {
            anyhow::bail!("Unknown Health feature task: {}", task.command);
        }
    };

    if result.success {
        Ok(result.message)
    } else {
        anyhow::bail!("Health feature failed: {}", result.message);
    }
}

/// Result of a task execution
#[derive(Debug, Clone)]
pub struct TaskExecutionResult {
    #[allow(dead_code)]
    pub task_id: i64,
    pub success: bool,
    pub output: String,
    pub error: Option<String>,
    pub duration_ms: u64,
}
