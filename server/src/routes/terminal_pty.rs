//! Interactive PTY terminal handler using russh
//!
//! Provides true interactive SSH sessions with PTY allocation,
//! enabling commands like sudo, vim, top, etc.

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::any,
    Router,
};
use futures_util::{SinkExt, StreamExt};
use russh::client;
use russh::keys::key::PrivateKeyWithHashAlg;
use russh::keys::{Algorithm, HashAlg};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use svrctlrs_database::sqlx::{Pool, Sqlite};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use crate::routes::ui::auth::CurrentUser;
use crate::state::AppState;
use svrctlrs_database::models::NewAuditEntry;

/// Interactive terminal request from frontend
#[derive(Debug, Deserialize)]
struct PtyRequest {
    #[serde(rename = "type")]
    request_type: String,
    server_id: Option<i64>,
    /// Input data to send to PTY
    data: Option<String>,
    /// Terminal dimensions for resize
    cols: Option<u32>,
    rows: Option<u32>,
}

/// Interactive terminal response to frontend
#[derive(Debug, Serialize)]
struct PtyResponse {
    #[serde(rename = "type")]
    response_type: String,
    data: String,
}

/// Create PTY terminal routes
pub fn routes() -> Router<AppState> {
    Router::new().route("/ws/terminal/pty", any(pty_ws_handler))
}

/// WebSocket upgrade handler for interactive PTY
async fn pty_ws_handler(
    ws: WebSocketUpgrade,
    headers: HeaderMap,
    State(state): State<AppState>,
    CurrentUser(user_id): CurrentUser,
) -> Response {
    // Cross-site WebSocket hijacking guard; see auth::websocket_origin_allowed.
    if !crate::routes::ui::auth::websocket_origin_allowed(&headers, &state.allowed_origins) {
        warn!("PTY WebSocket refused: Origin not allowed");
        return (StatusCode::FORBIDDEN, "origin not allowed").into_response();
    }
    info!("Interactive PTY WebSocket connection request");
    let ip = crate::routes::ui::auth::client_ip(&headers);
    ws.on_upgrade(move |socket| handle_pty_socket(socket, state, user_id, ip))
        .into_response()
}

/// SSH client handler for russh with host key verification
struct SshClientHandler {
    pool: Pool<Sqlite>,
    server_id: i64,
}

impl client::Handler for SshClientHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &russh::keys::PublicKeyOrCertificate,
    ) -> Result<bool, Self::Error> {
        use crate::host_keys::{self, Verdict};

        let openssh_key = crate::host_keys::presented_key_openssh(server_public_key)?;
        let key_type = openssh_key
            .split_whitespace()
            .next()
            .unwrap_or("unknown")
            .to_string();

        // Fail closed: a key we cannot verify or record is a key we do not trust.
        match host_keys::verify_and_record(&self.pool, self.server_id, &key_type, &openssh_key)
            .await
        {
            Ok(Verdict::Trusted) | Ok(Verdict::FirstSeen) => Ok(true),
            Ok(Verdict::Mismatch { .. }) => Ok(false),
            Err(e) => {
                error!(
                    server_id = self.server_id,
                    error = %e,
                    "PTY host key check failed; refusing connection"
                );
                Ok(false)
            }
        }
    }
}

/// Commands to send to the PTY session
enum PtyCommand {
    Data(Vec<u8>),
    Resize { cols: u32, rows: u32 },
    Close,
}

/// Active PTY session state
struct PtySession {
    command_tx: mpsc::Sender<PtyCommand>,
}

impl PtySession {
    /// Send data to the PTY
    async fn send_data(&self, data: &[u8]) -> Result<(), String> {
        self.command_tx
            .send(PtyCommand::Data(data.to_vec()))
            .await
            .map_err(|_| "Channel closed".to_string())
    }

    /// Resize the PTY window
    async fn resize(&self, cols: u32, rows: u32) -> Result<(), String> {
        self.command_tx
            .send(PtyCommand::Resize { cols, rows })
            .await
            .map_err(|_| "Channel closed".to_string())
    }

    /// Close the session
    async fn close(self) {
        let _ = self.command_tx.send(PtyCommand::Close).await;
    }
}

/// Handle the interactive PTY WebSocket connection
async fn handle_pty_socket(socket: WebSocket, state: AppState, user_id: i64, client_ip: String) {
    let (mut ws_sender, mut ws_receiver) = socket.split();

    info!("Interactive PTY WebSocket connected");

    // Send welcome message
    let welcome = PtyResponse {
        response_type: "output".to_string(),
        data: "\x1b[1;36m╔════════════════════════════════════════════════════╗\x1b[0m\r\n\
               \x1b[1;36m║\x1b[0m    \x1b[1mSvrCtlRS Interactive Terminal (PTY)\x1b[0m             \x1b[1;36m║\x1b[0m\r\n\
               \x1b[1;36m╚════════════════════════════════════════════════════╝\x1b[0m\r\n\r\n\
               \x1b[90mSend 'shell' message with server_id to start interactive session.\x1b[0m\r\n\r\n"
            .to_string(),
    };

    if let Ok(json) = serde_json::to_string(&welcome) {
        if ws_sender.send(Message::Text(json.into())).await.is_err() {
            return;
        }
    }

    // Channel for SSH -> WebSocket output
    let (ssh_tx, mut ssh_rx) = mpsc::channel::<String>(256);

    // State for active SSH session
    let mut active_session: Option<PtySession> = None;

    // Spawn task to forward SSH output to WebSocket
    let ws_sender_clone = Arc::new(tokio::sync::Mutex::new(ws_sender));
    let ws_sender_for_output = ws_sender_clone.clone();

    let output_task = tokio::spawn(async move {
        while let Some(data) = ssh_rx.recv().await {
            let response = PtyResponse {
                response_type: "output".to_string(),
                data,
            };
            if let Ok(json) = serde_json::to_string(&response) {
                let mut sender = ws_sender_for_output.lock().await;
                if let Err(e) = sender.send(Message::Text(json.into())).await {
                    tracing::debug!(
                        "PTY output WebSocket send failed (client likely disconnected): {e}"
                    );
                    break;
                }
            }
        }
    });

    // Handle WebSocket messages
    while let Some(msg) = ws_receiver.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                match serde_json::from_str::<PtyRequest>(&text) {
                    Ok(req) => {
                        match req.request_type.as_str() {
                            "shell" => {
                                // Start interactive shell session
                                if let Some(server_id) = req.server_id {
                                    let cols = req.cols.unwrap_or(80);
                                    let rows = req.rows.unwrap_or(24);

                                    match start_pty_session(
                                        &state,
                                        user_id,
                                        &client_ip,
                                        server_id,
                                        cols,
                                        rows,
                                        ssh_tx.clone(),
                                    )
                                    .await
                                    {
                                        Ok(session) => {
                                            active_session = Some(session);
                                            let connected = PtyResponse {
                                                response_type: "connected".to_string(),
                                                data: "\x1b[32m✓ Interactive shell session started\x1b[0m\r\n".to_string(),
                                            };
                                            if let Ok(json) = serde_json::to_string(&connected) {
                                                let mut sender = ws_sender_clone.lock().await;
                                                sender.send(Message::Text(json.into())).await.ok();
                                            }
                                        }
                                        Err(e) => {
                                            let error_msg = PtyResponse {
                                                response_type: "error".to_string(),
                                                data: format!("\x1b[31m✗ Failed to start shell: {}\x1b[0m\r\n", e),
                                            };
                                            if let Ok(json) = serde_json::to_string(&error_msg) {
                                                let mut sender = ws_sender_clone.lock().await;
                                                sender.send(Message::Text(json.into())).await.ok();
                                            }
                                        }
                                    }
                                }
                            }
                            "input" => {
                                // Send input to PTY
                                if let (Some(ref mut session), Some(data)) =
                                    (&mut active_session, req.data)
                                {
                                    if let Err(e) = session.send_data(data.as_bytes()).await {
                                        warn!("Failed to send data to PTY: {}", e);
                                    }
                                }
                            }
                            "resize" => {
                                // Resize PTY
                                if let Some(ref mut session) = active_session {
                                    let cols = req.cols.unwrap_or(80);
                                    let rows = req.rows.unwrap_or(24);
                                    if let Err(e) = session.resize(cols, rows).await {
                                        warn!("Failed to resize PTY: {}", e);
                                    } else {
                                        debug!("PTY resized to {}x{}", cols, rows);
                                    }
                                }
                            }
                            "ping" => {
                                let pong = PtyResponse {
                                    response_type: "pong".to_string(),
                                    data: String::new(),
                                };
                                if let Ok(json) = serde_json::to_string(&pong) {
                                    let mut sender = ws_sender_clone.lock().await;
                                    sender.send(Message::Text(json.into())).await.ok();
                                }
                            }
                            "close" => {
                                // Close the session
                                if let Some(session) = active_session.take() {
                                    session.close().await;
                                }
                                break;
                            }
                            _ => {
                                warn!("Unknown PTY request type: {}", req.request_type);
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to parse PTY request: {}", e);
                    }
                }
            }
            Ok(Message::Close(_)) => {
                info!("PTY WebSocket closed by client");
                break;
            }
            Ok(Message::Ping(data)) => {
                let mut sender = ws_sender_clone.lock().await;
                sender.send(Message::Pong(data)).await.ok();
            }
            Err(e) => {
                error!("PTY WebSocket error: {}", e);
                break;
            }
            _ => {}
        }
    }

    // Clean up
    if let Some(session) = active_session {
        session.close().await;
    }

    output_task.abort();
    info!("Interactive PTY WebSocket connection ended");
}

/// Start an interactive PTY session
async fn start_pty_session(
    state: &AppState,
    user_id: i64,
    client_ip: &str,
    server_id: i64,
    cols: u32,
    rows: u32,
    output_tx: mpsc::Sender<String>,
) -> Result<PtySession, String> {
    use svrctlrs_database::queries;

    // Get server from database
    let server = queries::servers::get_server(&state.pool, server_id)
        .await
        .map_err(|e| format!("Server not found: {}", e))?;

    let hostname = server
        .hostname
        .as_deref()
        .ok_or_else(|| "Server has no hostname configured".to_string())?;

    let port = server.port as u16;
    let username = server.username.as_deref().unwrap_or("root").to_string();

    // Get credential if assigned
    let credential = if let Some(cred_id) = server.credential_id {
        queries::credentials::get_credential(&state.pool, cred_id)
            .await
            .ok()
    } else {
        None
    };

    // Build list of SSH keys to try
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| "/home/svrctlrs".to_string());

    // Keys to try: the credential's (pasted material or a file, with its passphrase) first,
    // then the server's own default keys.
    let mut candidates: Vec<crate::ssh_keys::KeyCandidate> = Vec::new();
    if let Some(ref cred) = credential {
        if cred.credential_type_str == "ssh_key" {
            candidates.push(crate::ssh_keys::credential_key(cred).await?);
        }
        // A password credential cannot be used here: russh has no simple
        // keyboard-interactive flow, so fall through to the default keys.
    }
    for key in crate::ssh_keys::default_keys().await {
        if !candidates.iter().any(|c| c.label == key.label) {
            candidates.push(key);
        }
    }

    if candidates.is_empty() {
        return Err(format!(
            "No SSH keys found in {}/.ssh/. Please configure an SSH key credential or ensure keys are available.",
            home
        ));
    }

    info!(
        "Attempting PTY connection to {}@{}:{} with {} potential keys",
        username,
        hostname,
        port,
        candidates.len()
    );

    // Try each key until one works
    let mut last_error = String::new();
    let mut tried_keys: Vec<String> = Vec::new();

    for key in &candidates {
        let key_path = &key.label;
        tried_keys.push(key_path.clone());
        info!("Trying SSH key: {}", key_path);

        let key_pair =
            match russh::keys::decode_secret_key(&key.material, key.passphrase.as_deref()) {
                Ok(kp) => kp,
                Err(e) => {
                    debug!("Failed to decode SSH key {}: {}", key_path, e);
                    last_error = format!("Failed to decode key {}: {}", key_path, e);
                    continue;
                }
            };

        // Connect to SSH server (new connection for each attempt)
        let config = Arc::new(russh::client::Config::default());
        let handler = SshClientHandler {
            pool: state.pool.clone(),
            server_id,
        };

        let mut session = match russh::client::connect(config, (hostname, port), handler).await {
            Ok(s) => s,
            Err(e) => {
                last_error = format!("SSH connection failed: {}", e);
                // Connection failure means we can't try other keys on this server
                return Err(last_error);
            }
        };

        // Determine the hash algorithm based on key type
        // RSA keys on modern SSH servers require rsa-sha2-256 or rsa-sha2-512
        // instead of the legacy ssh-rsa (SHA-1)
        let hash_alg = match key_pair.algorithm() {
            Algorithm::Rsa { .. } => {
                // RSA key - use SHA-256 signature algorithm
                debug!("Detected RSA key, using SHA-256 signature algorithm");
                Some(HashAlg::Sha256)
            }
            _ => {
                // Ed25519, ECDSA, and other keys have built-in signature algorithms
                debug!("Non-RSA key detected (Ed25519/ECDSA), using default signature algorithm");
                None
            }
        };

        // Authenticate with public key
        let auth_result = match session
            .authenticate_publickey(
                &username,
                PrivateKeyWithHashAlg::new(Arc::new(key_pair), hash_alg),
            )
            .await
        {
            Ok(result) => result,
            Err(e) => {
                debug!("Authentication error with key {}: {}", key_path, e);
                last_error = format!("Authentication error with {}: {}", key_path, e);
                continue;
            }
        };

        match auth_result {
            russh::client::AuthResult::Success => {
                info!("Successfully authenticated with key: {}", key_path);
                crate::audit::record(
                    &state.pool,
                    NewAuditEntry::new("session.pty_opened")
                        .user(user_id)
                        .client_ip(Some(client_ip.to_string()))
                        .target("server", server.id, &server.name)
                        .details(serde_json::json!({
                            "login": format!("{username}@{hostname}:{port}"),
                            "key": key_path,
                        })),
                );

                // Open a channel
                let channel = session
                    .channel_open_session()
                    .await
                    .map_err(|e| format!("Failed to open channel: {}", e))?;

                // Request PTY
                channel
                    .request_pty(
                        false, // want_reply
                        "xterm-256color",
                        cols,
                        rows,
                        0,   // pix width
                        0,   // pix height
                        &[], // terminal modes
                    )
                    .await
                    .map_err(|e| format!("Failed to request PTY: {}", e))?;

                // Start shell
                channel
                    .request_shell(false)
                    .await
                    .map_err(|e| format!("Failed to start shell: {}", e))?;

                // Create command channel for sending data/commands to the PTY
                let (command_tx, mut command_rx) = mpsc::channel::<PtyCommand>(256);

                // Spawn task to handle channel I/O - owns the channel
                tokio::spawn(async move {
                    let mut channel = channel;
                    loop {
                        tokio::select! {
                            // Handle commands from WebSocket
                            cmd = command_rx.recv() => {
                                match cmd {
                                    Some(PtyCommand::Data(data)) => {
                                        if channel.data(&data[..]).await.is_err() {
                                            break;
                                        }
                                    }
                                    Some(PtyCommand::Resize { cols, rows }) => {
                                        if channel.window_change(cols, rows, 0, 0).await.is_err() {
                                            break;
                                        }
                                    }
                                    Some(PtyCommand::Close) | None => {
                                        let _ = channel.eof().await;
                                        let _ = channel.close().await;
                                        break;
                                    }
                                }
                            }
                            // Handle output from SSH
                            msg = channel.wait() => {
                                match msg {
                                    Some(russh::ChannelMsg::Data { data }) => {
                                        let text = String::from_utf8_lossy(&data).to_string();
                                        if output_tx.send(text).await.is_err() {
                                            break;
                                        }
                                    }
                                    Some(russh::ChannelMsg::ExtendedData { data, .. }) => {
                                        // Extended data (usually stderr)
                                        let text = String::from_utf8_lossy(&data).to_string();
                                        if output_tx.send(text).await.is_err() {
                                            break;
                                        }
                                    }
                                    Some(russh::ChannelMsg::Eof) => {
                                        output_tx.send("\r\n\x1b[33m[Session ended]\x1b[0m\r\n".to_string()).await.ok();
                                        break;
                                    }
                                    Some(russh::ChannelMsg::Close) => {
                                        break;
                                    }
                                    Some(russh::ChannelMsg::ExitStatus { exit_status }) => {
                                        let msg = format!("\r\n\x1b[90m[Exit status: {}]\x1b[0m\r\n", exit_status);
                                        output_tx.send(msg).await.ok();
                                    }
                                    None => break,
                                    _ => {}
                                }
                            }
                        }
                    }
                });

                return Ok(PtySession { command_tx });
            }
            _ => {
                debug!("Key rejected by server: {}", key_path);
                last_error = format!("Key rejected: {}", key_path);
                continue;
            }
        }
    }

    // All keys failed
    Err(format!(
        "SSH authentication failed. Tried keys: {}. Last error: {}",
        tried_keys.join(", "),
        last_error
    ))
}
