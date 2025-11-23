//! Remote execution via SSH

use crate::{Error, Result, Server};
use tokio::process::Command;
use tokio::time::{timeout, Duration};
use tracing::{debug, info, instrument};

/// Default command timeout in seconds
pub const DEFAULT_TIMEOUT_SECS: u64 = 120;

/// Remote command executor
///
/// Handles executing commands either locally or via SSH.
/// Uses the `ssh` command for remote execution, not SSH libraries.
pub struct RemoteExecutor {
    server: Server,
    ssh_key_path: Option<String>,
    timeout_secs: u64,
}

impl RemoteExecutor {
    /// Create a new remote executor
    ///
    /// # Arguments
    ///
    /// * `ssh_key_path` - Optional path to SSH private key
    pub fn new(ssh_key_path: Option<String>) -> Self {
        Self {
            server: Server::local("localhost"),
            ssh_key_path,
            timeout_secs: DEFAULT_TIMEOUT_SECS,
        }
    }

    /// Create a remote executor for a specific server
    pub fn for_server(server: Server, ssh_key_path: Option<String>) -> Self {
        Self {
            server,
            ssh_key_path,
            timeout_secs: DEFAULT_TIMEOUT_SECS,
        }
    }

    /// Set command timeout in seconds
    pub fn with_timeout(mut self, timeout_secs: u64) -> Self {
        self.timeout_secs = timeout_secs;
        self
    }

    /// Execute a command on the configured server
    ///
    /// # Arguments
    ///
    /// * `cmd` - Command to execute
    /// * `args` - Command arguments
    ///
    /// # Returns
    ///
    /// Command output (stdout)
    #[instrument(skip(self), fields(server = %self.server.name, cmd = %cmd))]
    pub async fn execute_command(&self, cmd: &str, args: &[&str]) -> Result<String> {
        if self.server.is_local() {
            self.execute_local(cmd, args).await
        } else {
            self.execute_ssh(cmd, args).await
        }
    }

    /// Execute a command on a specific server (overrides configured server)
    ///
    /// # Arguments
    ///
    /// * `server` - Target server
    /// * `cmd` - Command to execute
    /// * `args` - Command arguments
    ///
    /// # Returns
    ///
    /// Command output (stdout)
    #[instrument(skip(self), fields(server = %server.name, cmd = %cmd))]
    pub async fn execute(&self, server: &Server, cmd: &str, args: &[&str]) -> Result<String> {
        if server.is_local() {
            self.execute_local(cmd, args).await
        } else {
            self.execute_ssh_on_server(server, cmd, args).await
        }
    }

    /// Execute command locally
    async fn execute_local(&self, cmd: &str, args: &[&str]) -> Result<String> {
        info!(cmd = %cmd, args = ?args, "Executing command locally");

        let output = timeout(
            Duration::from_secs(self.timeout_secs),
            Command::new(cmd).args(args).output(),
        )
        .await
        .map_err(|_| {
            Error::RemoteExecutionError(format!(
                "Command timed out after {}s: {} {}",
                self.timeout_secs,
                cmd,
                args.join(" ")
            ))
        })?
        .map_err(|e| Error::RemoteExecutionError(format!("Failed to execute {}: {}", cmd, e)))?;

        // Note: Some commands use non-zero exit codes to indicate status
        // (e.g., dnf check-update returns 100 if updates exist)
        // So we don't fail on non-zero exit here

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if !stderr.is_empty() {
            debug!(cmd = %cmd, stderr = %stderr, "Command produced stderr output");
        }

        Ok(stdout)
    }

    /// Execute command via SSH on configured server
    async fn execute_ssh(&self, cmd: &str, args: &[&str]) -> Result<String> {
        self.execute_ssh_on_server(&self.server, cmd, args).await
    }

    /// Execute command via SSH on a specific server
    async fn execute_ssh_on_server(
        &self,
        server: &Server,
        cmd: &str,
        args: &[&str],
    ) -> Result<String> {
        let ssh_host = server.ssh_host.as_ref().ok_or_else(|| {
            Error::RemoteExecutionError("No SSH host configured".to_string())
        })?;

        // Build the remote command string with proper shell escaping
        let remote_cmd = self.build_remote_command(cmd, args);

        info!(
            ssh_host = %ssh_host,
            remote_cmd = %remote_cmd,
            "Executing command via SSH"
        );

        // Build SSH command
        let mut ssh_cmd = Command::new("ssh");
        ssh_cmd
            .arg("-o")
            .arg("BatchMode=yes") // No interactive prompts
            .arg("-o")
            .arg("StrictHostKeyChecking=accept-new") // Auto-accept new host keys
            .arg("-o")
            .arg("UserKnownHostsFile=/dev/null"); // Don't save host keys (read-only mount)

        // Add SSH key if specified
        if let Some(key_path) = &self.ssh_key_path {
            ssh_cmd.arg("-i").arg(key_path);
        }

        ssh_cmd.arg(ssh_host).arg(&remote_cmd);

        // Execute with timeout
        let output = timeout(Duration::from_secs(self.timeout_secs), ssh_cmd.output())
            .await
            .map_err(|_| {
                Error::RemoteExecutionError(format!(
                    "SSH command timed out after {}s to {}",
                    self.timeout_secs, ssh_host
                ))
            })?
            .map_err(|e| {
                Error::RemoteExecutionError(format!("Failed to SSH to {}: {}", ssh_host, e))
            })?;

        // Check for SSH-specific errors
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("Permission denied") || stderr.contains("Connection refused") {
                return Err(Error::RemoteExecutionError(format!(
                    "SSH failed to {}: {}",
                    ssh_host, stderr
                )));
            }
        }

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        Ok(stdout)
    }

    /// Build remote command string with proper quoting
    fn build_remote_command(&self, cmd: &str, args: &[&str]) -> String {
        if args.is_empty() {
            cmd.to_string()
        } else {
            // Quote each argument that might contain spaces or special chars
            let quoted_args: Vec<String> = args
                .iter()
                .map(|arg| {
                    // If arg contains spaces or special chars, quote it
                    if arg.contains(' ')
                        || arg.contains('*')
                        || arg.contains('$')
                        || arg.contains('&')
                        || arg.contains('|')
                        || arg.contains(';')
                    {
                        // Escape single quotes within the argument
                        format!("'{}'", arg.replace('\'', "'\\''"))
                    } else {
                        arg.to_string()
                    }
                })
                .collect();
            format!("{} {}", cmd, quoted_args.join(" "))
        }
    }

    /// Get reference to the configured server
    pub fn server(&self) -> &Server {
        &self.server
    }

    /// Get the configured timeout in seconds
    pub fn timeout_secs(&self) -> u64 {
        self.timeout_secs
    }
}

impl Default for RemoteExecutor {
    fn default() -> Self {
        Self::new(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_remote_command_simple() {
        let executor = RemoteExecutor::new(None);
        let cmd = executor.build_remote_command("ls", &["-la"]);
        assert_eq!(cmd, "ls -la");
    }

    #[test]
    fn test_build_remote_command_with_spaces() {
        let executor = RemoteExecutor::new(None);
        let cmd = executor.build_remote_command("echo", &["hello world"]);
        assert_eq!(cmd, "echo 'hello world'");
    }

    #[test]
    fn test_build_remote_command_with_special_chars() {
        let executor = RemoteExecutor::new(None);
        let cmd = executor.build_remote_command("grep", &["test*"]);
        assert_eq!(cmd, "grep 'test*'");
    }

    #[test]
    fn test_build_remote_command_with_quotes() {
        let executor = RemoteExecutor::new(None);
        let cmd = executor.build_remote_command("echo", &["it's working"]);
        assert_eq!(cmd, "echo 'it'\\''s working'");
    }

    #[tokio::test]
    async fn test_execute_local_simple() {
        let executor = RemoteExecutor::new(None);
        let result = executor.execute_command("echo", &["hello"]).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().trim(), "hello");
    }

    #[tokio::test]
    async fn test_timeout_configuration() {
        let executor = RemoteExecutor::new(None).with_timeout(5);
        assert_eq!(executor.timeout_secs(), 5);
    }
}
