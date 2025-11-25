//! SSH connection utilities for remote server management

use anyhow::{Context, Result};
use async_ssh2_tokio::{client::Client, AuthMethod, ServerCheckMethod};
use std::time::Duration;
use tracing::{debug, info};

/// SSH connection configuration
#[derive(Debug, Clone)]
pub struct SshConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub key_path: Option<String>,
    pub timeout: Duration,
}

impl Default for SshConfig {
    fn default() -> Self {
        Self {
            host: String::new(),
            port: 22,
            username: "root".to_string(),
            key_path: None,
            timeout: Duration::from_secs(30),
        }
    }
}

/// Test SSH connection to a server
pub async fn test_connection(config: &SshConfig) -> Result<String> {
    info!(
        "Testing SSH connection to {}@{}:{}",
        config.username, config.host, config.port
    );

    // Create client with timeout
    let client = tokio::time::timeout(
        config.timeout,
        connect_ssh(config)
    )
    .await
    .context("Connection timeout")?
    .context("Failed to establish SSH connection")?;

    // Try to execute a simple command to verify connection
    let result = client
        .execute("echo 'Connection successful'")
        .await
        .context("Failed to execute test command")?;

    let output = result.stdout.trim().to_string();
    
    info!("SSH connection test successful: {}", output);
    Ok(output)
}

/// Execute a command on a remote server via SSH
pub async fn execute_command(config: &SshConfig, command: &str) -> Result<CommandResult> {
    debug!(
        "Executing command on {}@{}:{}: {}",
        config.username, config.host, config.port, command
    );

    let client = tokio::time::timeout(
        config.timeout,
        connect_ssh(config)
    )
    .await
    .context("Connection timeout")?
    .context("Failed to establish SSH connection")?;

    let result = client
        .execute(command)
        .await
        .context("Failed to execute command")?;

    let stdout = result.stdout;
    let stderr = result.stderr;
    let exit_code = result.exit_status as i32;

    debug!(
        "Command execution completed with exit code {}: stdout={}, stderr={}",
        exit_code, stdout, stderr
    );

    Ok(CommandResult {
        stdout,
        stderr,
        exit_code,
        success: exit_code == 0,
    })
}

/// Result of a command execution
#[derive(Debug, Clone)]
pub struct CommandResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub success: bool,
}

/// Connect to SSH server
async fn connect_ssh(config: &SshConfig) -> Result<Client> {
    // Determine authentication method
    if let Some(key_path) = &config.key_path {
        // Use SSH key authentication
        debug!("Using SSH key authentication: {}", key_path);
        let auth_method = AuthMethod::with_key_file(key_path, None)
            .context("Failed to load SSH key")?;
        return connect_with_auth(config, auth_method).await;
    }
    
    // Try default SSH keys
    debug!("Using default SSH key locations");
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    let default_keys = vec![
        format!("{}/.ssh/id_rsa", home),
        format!("{}/.ssh/id_ed25519", home),
        format!("{}/.ssh/id_ecdsa", home),
    ];

    // Try each key until one works
    let mut last_error = None;
    for key_path in default_keys {
        if std::path::Path::new(&key_path).exists() {
            debug!("Trying SSH key: {}", key_path);
            match AuthMethod::with_key_file(&key_path, None) {
                Ok(auth) => {
                    return connect_with_auth(config, auth).await;
                }
                Err(e) => {
                    debug!("Failed to load key {}: {}", key_path, e);
                    last_error = Some(e);
                }
            }
        }
    }

    Err(anyhow::anyhow!(
        "No valid SSH keys found. Last error: {:?}",
        last_error
    ))
}

/// Connect with specific auth method
async fn connect_with_auth(config: &SshConfig, auth_method: AuthMethod) -> Result<Client> {
    let client = Client::connect(
        (config.host.clone(), config.port),
        &config.username,
        auth_method,
        ServerCheckMethod::NoCheck, // TODO: Implement proper host key checking
    )
    .await
    .context("Failed to connect to SSH server")?;

    Ok(client)
}

