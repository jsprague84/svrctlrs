//! Terminal WebSocket route handler
//!
//! Provides real-time terminal functionality for testing command templates
//! and interactive server debugging via WebSocket connections.

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
    routing::any,
    Router,
};
use futures_util::{sink::SinkExt, stream::StreamExt};
use serde::{Deserialize, Serialize};
use tracing::{error, info, warn};

use crate::state::AppState;

/// Terminal command message from frontend
#[derive(Debug, Deserialize)]
struct TerminalRequest {
    #[serde(rename = "type")]
    request_type: String,
    server_id: Option<i64>,
    command: Option<String>,
    /// Environment variables to set before executing the command
    env: Option<std::collections::HashMap<String, String>>,
    #[allow(dead_code)]
    cols: Option<u16>,
    #[allow(dead_code)]
    rows: Option<u16>,
}

/// Terminal response message to frontend
#[derive(Debug, Serialize)]
struct TerminalResponse {
    #[serde(rename = "type")]
    response_type: String,
    data: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    exit_code: Option<i32>,
}

/// Create terminal routes
pub fn routes() -> Router<AppState> {
    Router::new().route("/ws/terminal", any(terminal_ws_handler))
}

/// WebSocket upgrade handler
async fn terminal_ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    info!("Terminal WebSocket connection request");
    ws.on_upgrade(move |socket| handle_terminal_socket(socket, state))
}

/// Handle the WebSocket connection
async fn handle_terminal_socket(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();

    info!("Terminal WebSocket connected");

    // Send welcome message
    let welcome = TerminalResponse {
        response_type: "output".to_string(),
        data: "\x1b[1;36m╔════════════════════════════════════════════════════╗\x1b[0m\r\n\
               \x1b[1;36m║\x1b[0m         \x1b[1mSvrCtlRS Terminal v1.0\x1b[0m                   \x1b[1;36m║\x1b[0m\r\n\
               \x1b[1;36m╚════════════════════════════════════════════════════╝\x1b[0m\r\n\r\n\
               \x1b[90mReady. Select a server and enter a command.\x1b[0m\r\n\r\n"
            .to_string(),
        exit_code: None,
    };

    if let Ok(json) = serde_json::to_string(&welcome) {
        if sender.send(Message::Text(json.into())).await.is_err() {
            return;
        }
    }

    while let Some(msg) = receiver.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                match serde_json::from_str::<TerminalRequest>(&text) {
                    Ok(req) => match req.request_type.as_str() {
                        "execute" => {
                            if let (Some(server_id), Some(command)) =
                                (req.server_id, req.command.as_ref())
                            {
                                execute_command(
                                    &state,
                                    server_id,
                                    command,
                                    req.env.as_ref(),
                                    &mut sender,
                                )
                                .await;
                            } else {
                                send_error(&mut sender, "Missing server_id or command").await;
                            }
                        }
                        "resize" => {
                            // Terminal resize - currently a no-op since we don't have PTY
                            // This could be enhanced later for interactive terminal sessions
                        }
                        "ping" => {
                            // Keep-alive ping
                            let pong = TerminalResponse {
                                response_type: "pong".to_string(),
                                data: String::new(),
                                exit_code: None,
                            };
                            if let Ok(json) = serde_json::to_string(&pong) {
                                sender.send(Message::Text(json.into())).await.ok();
                            }
                        }
                        _ => {
                            warn!("Unknown terminal request type: {}", req.request_type);
                            send_error(
                                &mut sender,
                                &format!("Unknown request type: {}", req.request_type),
                            )
                            .await;
                        }
                    },
                    Err(e) => {
                        error!("Failed to parse terminal request: {}", e);
                        send_error(&mut sender, &format!("Invalid request format: {}", e)).await;
                    }
                }
            }
            Ok(Message::Close(_)) => {
                info!("Terminal WebSocket closed by client");
                break;
            }
            Ok(Message::Ping(data)) => {
                // Respond to ping with pong
                sender.send(Message::Pong(data)).await.ok();
            }
            Err(e) => {
                error!("Terminal WebSocket error: {}", e);
                break;
            }
            _ => {}
        }
    }

    info!("Terminal WebSocket connection ended");
}

/// Execute a command on a server via SSH
async fn execute_command(
    state: &AppState,
    server_id: i64,
    command: &str,
    env: Option<&std::collections::HashMap<String, String>>,
    sender: &mut futures_util::stream::SplitSink<WebSocket, Message>,
) {
    use svrctlrs_database::queries;

    // Get server from database
    let server = match queries::servers::get_server(&state.pool, server_id).await {
        Ok(s) => s,
        Err(e) => {
            send_error(
                sender,
                &format!("Server not found or database error: {}", e),
            )
            .await;
            return;
        }
    };

    // Send "executing" message
    let executing_msg = format!(
        "\x1b[33m▶ Executing on {}\x1b[0m\r\n\x1b[90m$ {}\x1b[0m\r\n\r\n",
        server.name, command
    );
    send_output(sender, &executing_msg).await;

    // Show environment variables if any
    if let Some(env_vars) = env {
        if !env_vars.is_empty() {
            let env_display: Vec<String> = env_vars.keys().cloned().collect();
            let env_msg = format!("\x1b[90m  env: {}\x1b[0m\r\n", env_display.join(", "));
            send_output(sender, &env_msg).await;
        }
    }

    // Get credential if assigned
    let credential = if let Some(cred_id) = server.credential_id {
        match queries::credentials::get_credential(&state.pool, cred_id).await {
            Ok(c) => Some(c),
            Err(e) => {
                warn!(
                    "Failed to get credential {} for server {}: {}",
                    cred_id, server.name, e
                );
                None
            }
        }
    } else {
        None
    };

    // Execute command via SSH or locally
    let result = if server.is_local {
        // Local execution
        execute_local_command(command, env).await
    } else {
        // Remote execution via SSH
        execute_ssh_command(&server, credential.as_ref(), command, env).await
    };

    match result {
        Ok((stdout, stderr, exit_code)) => {
            // Send stdout
            if !stdout.is_empty() {
                // Convert \n to \r\n for proper terminal display
                let stdout_formatted = stdout.replace('\n', "\r\n");
                send_output(sender, &stdout_formatted).await;
            }

            // Send stderr in red
            if !stderr.is_empty() {
                let stderr_formatted = stderr.replace('\n', "\r\n");
                let stderr_colored = format!("\x1b[31m{}\x1b[0m", stderr_formatted);
                send_output(sender, &stderr_colored).await;
            }

            // Send exit code with color
            send_exit_code(sender, exit_code).await;
        }
        Err(e) => {
            send_error(sender, &format!("Execution failed: {}", e)).await;
        }
    }
}

/// Execute a command locally
async fn execute_local_command(
    command: &str,
    env: Option<&std::collections::HashMap<String, String>>,
) -> Result<(String, String, i32), String> {
    use tokio::process::Command;

    let mut cmd = Command::new("sh");
    cmd.arg("-c").arg(command);

    // Add environment variables if provided
    if let Some(env_vars) = env {
        for (key, value) in env_vars {
            cmd.env(key, value);
        }
    }

    let output = cmd
        .output()
        .await
        .map_err(|e| format!("Failed to execute command: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let exit_code = output.status.code().unwrap_or(-1);

    Ok((stdout, stderr, exit_code))
}

/// Execute a command via SSH
async fn execute_ssh_command(
    server: &svrctlrs_database::models::Server,
    credential: Option<&svrctlrs_database::models::Credential>,
    command: &str,
    env: Option<&std::collections::HashMap<String, String>>,
) -> Result<(String, String, i32), String> {
    use async_ssh2_tokio::client::{AuthMethod, Client, ServerCheckMethod};
    use svrctlrs_database::models::CredentialType;

    // Get hostname - required for SSH
    let hostname = server
        .hostname
        .as_deref()
        .ok_or_else(|| "Server has no hostname configured".to_string())?;

    let port = server.port as u16;
    let username = server.username.as_deref().unwrap_or("root");

    // Determine authentication method
    let auth_method = if let Some(cred) = credential {
        match cred.credential_type() {
            Some(CredentialType::SshKey) => {
                // value contains the path to the SSH key file
                let key_path = &cred.value;

                // Read the key file
                let key_content = tokio::fs::read_to_string(key_path)
                    .await
                    .map_err(|e| format!("Failed to read SSH key at {}: {}", key_path, e))?;

                // Check metadata for passphrase
                let metadata = cred.get_metadata();
                let passphrase = metadata
                    .get("passphrase")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string());

                AuthMethod::with_key(&key_content, passphrase.as_deref())
            }
            Some(CredentialType::Password) => {
                // value contains the password
                AuthMethod::with_password(&cred.value)
            }
            Some(other) => {
                return Err(format!("Unsupported credential type for SSH: {:?}", other));
            }
            None => {
                return Err(format!(
                    "Invalid credential type: {}",
                    cred.credential_type_str
                ));
            }
        }
    } else {
        // Try to use default SSH key from home directory
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let default_key_path = format!("{}/.ssh/id_rsa", home);

        // Try to read the default key file
        match tokio::fs::read_to_string(&default_key_path).await {
            Ok(key_content) => AuthMethod::with_key(&key_content, None),
            Err(_) => {
                // Try ed25519 key as fallback
                let ed25519_path = format!("{}/.ssh/id_ed25519", home);
                match tokio::fs::read_to_string(&ed25519_path).await {
                    Ok(key_content) => AuthMethod::with_key(&key_content, None),
                    Err(_) => {
                        return Err(
                            "No credential assigned and no default SSH key found".to_string()
                        );
                    }
                }
            }
        }
    };

    // Connect to SSH server
    let client = Client::connect(
        (hostname, port),
        username,
        auth_method,
        ServerCheckMethod::NoCheck,
    )
    .await
    .map_err(|e| format!("SSH connection failed: {}", e))?;

    // Build command with environment variables if provided
    // SSH doesn't directly support env vars, so we prepend them to the command
    let full_command = if let Some(env_vars) = env {
        if !env_vars.is_empty() {
            let env_prefix: Vec<String> = env_vars
                .iter()
                .map(|(k, v)| format!("{}={}", k, shell_escape(v)))
                .collect();
            format!("{} {}", env_prefix.join(" "), command)
        } else {
            command.to_string()
        }
    } else {
        command.to_string()
    };

    // Execute command
    let result = client
        .execute(&full_command)
        .await
        .map_err(|e| format!("SSH command execution failed: {}", e))?;

    Ok((result.stdout, result.stderr, result.exit_status as i32))
}

/// Escape a string for safe use in shell commands
fn shell_escape(s: &str) -> String {
    // If the string contains special characters, wrap in single quotes
    // and escape any single quotes within
    if s.chars().any(|c| {
        matches!(
            c,
            ' ' | '"'
                | '\''
                | '\\'
                | '$'
                | '`'
                | '!'
                | '*'
                | '?'
                | '['
                | ']'
                | '('
                | ')'
                | '{'
                | '}'
                | '<'
                | '>'
                | '|'
                | '&'
                | ';'
                | '\n'
                | '\t'
        )
    }) {
        format!("'{}'", s.replace('\'', "'\\''"))
    } else {
        s.to_string()
    }
}

/// Send output to the terminal
async fn send_output(sender: &mut futures_util::stream::SplitSink<WebSocket, Message>, data: &str) {
    let response = TerminalResponse {
        response_type: "output".to_string(),
        data: data.to_string(),
        exit_code: None,
    };

    if let Ok(json) = serde_json::to_string(&response) {
        sender.send(Message::Text(json.into())).await.ok();
    }
}

/// Send exit code to the terminal
async fn send_exit_code(
    sender: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    exit_code: i32,
) {
    let status_msg = if exit_code == 0 {
        format!(
            "\r\n\x1b[32m✓ Process exited with code: {}\x1b[0m\r\n",
            exit_code
        )
    } else {
        format!(
            "\r\n\x1b[31m✗ Process exited with code: {}\x1b[0m\r\n",
            exit_code
        )
    };

    let response = TerminalResponse {
        response_type: "exit".to_string(),
        data: status_msg,
        exit_code: Some(exit_code),
    };

    if let Ok(json) = serde_json::to_string(&response) {
        sender.send(Message::Text(json.into())).await.ok();
    }
}

/// Send error message to the terminal
async fn send_error(sender: &mut futures_util::stream::SplitSink<WebSocket, Message>, error: &str) {
    let error_msg = format!("\r\n\x1b[31m✗ Error: {}\x1b[0m\r\n", error);
    let response = TerminalResponse {
        response_type: "error".to_string(),
        data: error_msg,
        exit_code: Some(-1),
    };

    if let Ok(json) = serde_json::to_string(&response) {
        sender.send(Message::Text(json.into())).await.ok();
    }
}
