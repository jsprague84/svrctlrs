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
    let client = match tokio::time::timeout(config.timeout, connect_ssh(config)).await {
        Err(_) => {
            return Err(anyhow::anyhow!(
                "Connection timeout after {} seconds",
                config.timeout.as_secs()
            ));
        }
        Ok(Err(e)) => {
            // Preserve the detailed error from connect_ssh
            return Err(e);
        }
        Ok(Ok(client)) => client,
    };

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

/// Check what SSH keys are available
pub fn list_available_keys() -> Vec<String> {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| "/home/svrctlrs".to_string());
    
    let possible_keys = vec![
        format!("{}/.ssh/id_ed25519", home),
        format!("{}/.ssh/id_rsa", home),
        format!("{}/.ssh/id_ecdsa", home),
        format!("{}/.ssh/id_dsa", home),
    ];
    
    possible_keys
        .into_iter()
        .filter(|path| std::path::Path::new(path).exists())
        .collect()
}

/// Connect to SSH server
pub async fn connect_ssh(config: &SshConfig) -> Result<Client> {
    // Determine authentication method
    if let Some(key_path) = &config.key_path {
        // Use SSH key authentication
        info!("Using SSH key authentication: {}", key_path);
        if !std::path::Path::new(key_path).exists() {
            return Err(anyhow::anyhow!("SSH key not found: {}", key_path));
        }
        let auth_method = AuthMethod::with_key_file(key_path, None);
        return connect_with_auth(config, auth_method).await;
    }
    
    // Try default SSH keys - check multiple possible locations
    info!("Searching for SSH keys in default locations");
    
    // Get home directory - try multiple methods
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| {
            // Fallback to /home/svrctlrs for Docker container
            "/home/svrctlrs".to_string()
        });
    
    info!("Using home directory: {}", home);
    
    let default_keys = vec![
        format!("{}/.ssh/id_ed25519", home),
        format!("{}/.ssh/id_rsa", home),
        format!("{}/.ssh/id_ecdsa", home),
        format!("{}/.ssh/id_dsa", home),
    ];

    let mut tried_keys = Vec::new();
    let mut last_error = None;
    
    // Try each key until one works
    for key_path in default_keys {
        if std::path::Path::new(&key_path).exists() {
            info!("Found SSH key: {}", key_path);
            tried_keys.push(key_path.clone());
            
            let auth_method = AuthMethod::with_key_file(&key_path, None);
            
            // Try to connect with this key
            match connect_with_auth(config, auth_method).await {
                Ok(client) => {
                    info!("Successfully connected with key: {}", key_path);
                    return Ok(client);
                }
                Err(e) => {
                    info!("Failed to connect with key {}: {}", key_path, e);
                    last_error = Some(e);
                }
            }
        } else {
            debug!("SSH key not found: {}", key_path);
        }
    }

    // If we tried keys but all failed
    if !tried_keys.is_empty() {
        Err(anyhow::anyhow!(
            "Connection failed with all available SSH keys. Tried: {}. Last error: {}",
            tried_keys.join(", "),
            last_error.map(|e| e.to_string()).unwrap_or_else(|| "Unknown".to_string())
        ))
    } else {
        Err(anyhow::anyhow!(
            "No SSH keys found in {}/.ssh/. Please ensure SSH keys are mounted to the container.",
            home
        ))
    }
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

