//! Remote execution via SSH

use crate::{Error, Result, Server};
use tracing::{debug, instrument};

/// Remote command executor
pub struct RemoteExecutor {
    ssh_key_path: Option<String>,
}

impl RemoteExecutor {
    /// Create a new remote executor
    pub fn new(ssh_key_path: Option<String>) -> Self {
        Self { ssh_key_path }
    }

    /// Execute a command on a server
    ///
    /// # Arguments
    ///
    /// * `server` - Target server
    /// * `command` - Command to execute
    ///
    /// # Returns
    ///
    /// Command output (stdout)
    #[instrument(skip(self), fields(server = %server.name))]
    pub async fn execute(&self, server: &Server, command: &str) -> Result<String> {
        if server.is_local() {
            self.execute_local(command).await
        } else {
            self.execute_remote(server, command).await
        }
    }

    /// Execute locally
    async fn execute_local(&self, command: &str) -> Result<String> {
        debug!(command = %command, "Executing locally");

        let output = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(command)
            .output()
            .await
            .map_err(|e| Error::RemoteExecutionError(format!("Failed to execute: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::RemoteExecutionError(format!(
                "Command failed: {}",
                stderr
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        Ok(stdout)
    }

    /// Execute remotely via SSH
    async fn execute_remote(&self, server: &Server, command: &str) -> Result<String> {
        let ssh_host = server
            .ssh_host
            .as_ref()
            .ok_or_else(|| Error::RemoteExecutionError("No SSH host specified".into()))?;

        debug!(
            ssh_host = %ssh_host,
            command = %command,
            "Executing remotely via SSH"
        );

        // Build SSH command
        let mut ssh_cmd = tokio::process::Command::new("ssh");
        ssh_cmd.arg("-o").arg("StrictHostKeyChecking=accept-new");

        if let Some(key_path) = &self.ssh_key_path {
            ssh_cmd.arg("-i").arg(key_path);
        }

        ssh_cmd.arg(ssh_host).arg(command);

        let output = ssh_cmd
            .output()
            .await
            .map_err(|e| Error::RemoteExecutionError(format!("SSH failed: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::RemoteExecutionError(format!(
                "Remote command failed: {}",
                stderr
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        Ok(stdout)
    }
}

impl Default for RemoteExecutor {
    fn default() -> Self {
        Self::new(None)
    }
}
