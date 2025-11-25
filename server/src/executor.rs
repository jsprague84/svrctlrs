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
    let result = match task.server_id {
        Some(server_id) => {
            // Task requires SSH execution on a remote server
            execute_remote_task(state, &task, server_id).await
        }
        None => {
            // Task is a local plugin execution
            execute_plugin_task(state, &task).await
        }
    };
    
    let duration_ms = start_time.elapsed().as_millis() as u64;
    
    // Record execution in task history
    let history_entry = TaskHistoryEntry {
        task_id,
        plugin_id: task.plugin_id.clone(),
        server_id: task.server_id,
        success: result.is_ok(),
        output: result.as_ref().map(|s| s.clone()).unwrap_or_default(),
        error: result.as_ref().err().map(|e| e.to_string()),
        duration_ms,
        executed_at: chrono::Utc::now(),
    };
    
    if let Err(e) = queries::tasks::record_task_execution(db.pool(), &history_entry).await {
        error!("Failed to record task execution in history: {}", e);
    }
    
    // Update task's last_run_at and run_count
    if let Err(e) = queries::tasks::update_task_stats(db.pool(), task_id).await {
        error!("Failed to update task stats: {}", e);
    }
    
    match result {
        Ok(output) => {
            info!("Task {} completed successfully in {}ms", task_id, duration_ms);
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
    let host = server.host.ok_or_else(|| anyhow::anyhow!("Server has no host configured"))?;
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

/// Execute a plugin task locally
async fn execute_plugin_task(state: &AppState, task: &Task) -> Result<String> {
    use svrctlrs_core::{PluginContext, Server as CoreServer};
    use std::collections::HashMap;

    debug!("Executing plugin task {} for plugin {}", task.id, task.plugin_id);
    
    // Get plugin from registry
    let plugins = state.plugins.read().await;
    let plugin = plugins
        .get(&task.plugin_id)
        .ok_or_else(|| anyhow::anyhow!("Plugin '{}' not found in registry", task.plugin_id))?;

    // Build plugin context
    let db = state.db().await;
    
    // Load servers from database (all enabled servers)
    let db_servers = queries::servers::list_servers(db.pool())
        .await
        .context("Failed to load servers for plugin execution")?;
    
    let servers: Vec<CoreServer> = db_servers
        .into_iter()
        .filter(|s| s.enabled)
        .map(|s| {
            // Build SSH host string (username@host:port)
            let ssh_host = s.host.map(|host| {
                if s.port != 22 {
                    format!("{}@{}:{}", s.username, host, s.port)
                } else {
                    format!("{}@{}", s.username, host)
                }
            });
            
            CoreServer {
                name: s.name,
                ssh_host,
            }
        })
        .collect();

    // Parse task config from args
    let config: HashMap<String, String> = if let Some(args_str) = &task.args {
        match serde_json::from_str::<JsonValue>(args_str) {
            Ok(JsonValue::Object(obj)) => obj
                .iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                .collect(),
            _ => HashMap::new(),
        }
    } else {
        HashMap::new()
    };

    // Get notification manager
    let notification_manager = state.notification_manager().await;

    let context = PluginContext {
        servers,
        config,
        notification_manager,
    };

    // Execute plugin
    info!("Executing plugin {} for task '{}'", task.plugin_id, task.name);
    let result = plugin
        .execute(&task.name, &context)
        .await
        .context(format!("Plugin {} execution failed", task.plugin_id))?;

    if result.success {
        Ok(format!(
            "Plugin {} executed successfully: {}",
            task.plugin_id, result.message
        ))
    } else {
        anyhow::bail!(
            "Plugin {} execution failed: {}",
            task.plugin_id,
            result.message
        )
    }
}

/// Result of a task execution
#[derive(Debug, Clone)]
pub struct TaskExecutionResult {
    pub task_id: i64,
    pub success: bool,
    pub output: String,
    pub error: Option<String>,
    pub duration_ms: u64,
}

