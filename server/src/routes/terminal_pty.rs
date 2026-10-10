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
use crate::sessions::{LiveSession, PtyCommand, SessionEvent};
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
    /// `attach`: the session to resume and the last output sequence the client has shown
    session_id: Option<String>,
    #[serde(default)]
    last_seq: u64,
}

/// Interactive terminal response to frontend
#[derive(Debug, Serialize)]
struct PtyResponse {
    #[serde(rename = "type")]
    response_type: String,
    data: String,
    /// Sequence number of an `output` frame, so a reconnecting client can ask for the rest
    #[serde(skip_serializing_if = "Option::is_none")]
    seq: Option<u64>,
    /// On `connected`: the session to reattach to later
    #[serde(skip_serializing_if = "Option::is_none")]
    session_id: Option<String>,
    /// On `error`: a machine-readable reason (`session_gone`, `quota`, ...)
    #[serde(skip_serializing_if = "Option::is_none")]
    code: Option<String>,
}

impl PtyResponse {
    fn plain(kind: &str, data: impl Into<String>) -> Self {
        Self {
            response_type: kind.to_string(),
            data: data.into(),
            seq: None,
            session_id: None,
            code: None,
        }
    }
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
async fn handle_pty_socket(socket: WebSocket, state: AppState, user_id: i64, client_ip: String) {
    let (ws_sender, mut ws_receiver) = socket.split();
    let ws_sender = Arc::new(tokio::sync::Mutex::new(ws_sender));

    info!("Interactive PTY WebSocket connected");

    // The session this socket is attached to, and the task streaming its output here.
    let mut session: Option<Arc<LiveSession>> = None;
    let mut forwarder: Option<tokio::task::JoinHandle<()>> = None;
    // `close` means "kill the shell"; anything else leaving a session behind only detaches it.
    let mut close_on_exit = false;

    while let Some(msg) = ws_receiver.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                let req = match serde_json::from_str::<PtyRequest>(&text) {
                    Ok(r) => r,
                    Err(e) => {
                        error!("Failed to parse PTY request: {}", e);
                        continue;
                    }
                };
                match req.request_type.as_str() {
                    "shell" => {
                        if session.is_some() {
                            send(
                                &ws_sender,
                                PtyResponse::plain(
                                    "error",
                                    "A session is already attached on this connection",
                                ),
                            )
                            .await;
                            continue;
                        }
                        let Some(server_id) = req.server_id else {
                            send(
                                &ws_sender,
                                PtyResponse::plain("error", "shell needs a server_id"),
                            )
                            .await;
                            continue;
                        };
                        // The banner belongs to a new shell only; a reattach must not paint
                        // it into the middle of the restored screen.
                        send(&ws_sender, PtyResponse::plain("output",
                            "\x1b[1;36m╔════════════════════════════════════════════════════╗\x1b[0m\r\n\
                             \x1b[1;36m║\x1b[0m    \x1b[1mSvrCtlRS Interactive Terminal (PTY)\x1b[0m             \x1b[1;36m║\x1b[0m\r\n\
                             \x1b[1;36m╚════════════════════════════════════════════════════╝\x1b[0m\r\n\r\n")).await;
                        let cols = req.cols.unwrap_or(80);
                        let rows = req.rows.unwrap_or(24);
                        match start_pty_session(&state, user_id, &client_ip, server_id, cols, rows)
                            .await
                        {
                            Ok(live) => {
                                if let Err(e) = state.sessions.insert(live.clone()).await {
                                    live.close().await;
                                    live.mark_closed(&e);
                                    send(
                                        &ws_sender,
                                        PtyResponse {
                                            code: Some("quota".into()),
                                            ..PtyResponse::plain(
                                                "error",
                                                format!("\x1b[31m✗ {e}\x1b[0m\r\n"),
                                            )
                                        },
                                    )
                                    .await;
                                    continue;
                                }
                                state.sessions.attach_new(&live);
                                forwarder =
                                    Some(spawn_forwarder(live.clone(), 0, ws_sender.clone()));
                                send(&ws_sender, PtyResponse {
                                    session_id: Some(live.id.clone()),
                                    ..PtyResponse::plain("connected", "\x1b[32m✓ Interactive shell session started\x1b[0m\r\n")
                                }).await;
                                session = Some(live);
                            }
                            Err(e) => {
                                send(
                                    &ws_sender,
                                    PtyResponse::plain(
                                        "error",
                                        format!(
                                            "\x1b[31m✗ Failed to start shell: {}\x1b[0m\r\n",
                                            e
                                        ),
                                    ),
                                )
                                .await;
                            }
                        }
                    }
                    "attach" => {
                        if session.is_some() {
                            send(
                                &ws_sender,
                                PtyResponse::plain(
                                    "error",
                                    "A session is already attached on this connection",
                                ),
                            )
                            .await;
                            continue;
                        }
                        let Some(id) = req.session_id.as_deref() else {
                            send(
                                &ws_sender,
                                PtyResponse::plain("error", "attach needs a session_id"),
                            )
                            .await;
                            continue;
                        };
                        match state.sessions.attach(id, user_id).await {
                            Some(live) => {
                                info!(session_id = %live.id, server = %live.server_name, "Client reattached to session");
                                if let (Some(cols), Some(rows)) = (req.cols, req.rows) {
                                    let _ = live.resize(cols, rows).await;
                                }
                                send(
                                    &ws_sender,
                                    PtyResponse {
                                        session_id: Some(live.id.clone()),
                                        ..PtyResponse::plain("connected", "")
                                    },
                                )
                                .await;
                                forwarder = Some(spawn_forwarder(
                                    live.clone(),
                                    req.last_seq,
                                    ws_sender.clone(),
                                ));
                                session = Some(live);
                            }
                            None => {
                                send(
                                    &ws_sender,
                                    PtyResponse {
                                        code: Some("session_gone".into()),
                                        ..PtyResponse::plain("error", "That session has ended")
                                    },
                                )
                                .await;
                            }
                        }
                    }
                    "input" => {
                        if let (Some(live), Some(data)) = (&session, req.data) {
                            if let Err(e) = live.send_input(data.as_bytes()).await {
                                warn!("Failed to send data to PTY: {}", e);
                            }
                        }
                    }
                    "resize" => {
                        if let Some(live) = &session {
                            let cols = req.cols.unwrap_or(80);
                            let rows = req.rows.unwrap_or(24);
                            if let Err(e) = live.resize(cols, rows).await {
                                warn!("Failed to resize PTY: {}", e);
                            } else {
                                debug!("PTY resized to {}x{}", cols, rows);
                            }
                        }
                    }
                    "ping" => {
                        send(&ws_sender, PtyResponse::plain("pong", "")).await;
                    }
                    "close" => {
                        close_on_exit = true;
                        break;
                    }
                    "detach" => break,
                    other => warn!("Unknown PTY request type: {}", other),
                }
            }
            Ok(Message::Close(_)) => {
                info!("PTY WebSocket closed by client");
                break;
            }
            Ok(Message::Ping(data)) => {
                ws_sender.lock().await.send(Message::Pong(data)).await.ok();
            }
            Err(e) => {
                error!("PTY WebSocket error: {}", e);
                break;
            }
            _ => {}
        }
    }

    if let Some(task) = forwarder {
        task.abort();
    }
    if let Some(live) = session {
        if close_on_exit {
            live.close().await;
            live.mark_closed("Session closed");
            state.sessions.remove(&live.id).await;
        } else {
            // Reload, lost link, backgrounded tab: keep the shell for a while.
            state.sessions.detach(&live).await;
            info!(session_id = %live.id, ttl_secs = state.sessions.detach_ttl().as_secs(),
                "Client detached; session kept for reattach");
        }
    }
    info!("Interactive PTY WebSocket connection ended");
}

type WsSender = Arc<tokio::sync::Mutex<futures_util::stream::SplitSink<WebSocket, Message>>>;

async fn send(sender: &WsSender, response: PtyResponse) -> bool {
    match serde_json::to_string(&response) {
        Ok(json) => sender
            .lock()
            .await
            .send(Message::Text(json.into()))
            .await
            .is_ok(),
        Err(_) => false,
    }
}

/// Stream a session's output to one socket: first everything after `last_seq` from the replay
/// ring, then live frames. A lagging subscriber is caught up from the ring instead of silently
/// losing frames; a client too far behind the ring is told to redraw from scratch.
fn spawn_forwarder(
    live: Arc<LiveSession>,
    last_seq: u64,
    sender: WsSender,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut events = live.subscribe();
        let mut sent = last_seq;

        if last_seq > 0 || live.latest_seq().await > 0 {
            send(&sender, PtyResponse::plain("replay_start", "")).await;
            match live.replay_since(last_seq).await {
                Ok(frames) => {
                    for f in frames {
                        if !send(
                            &sender,
                            PtyResponse {
                                seq: Some(f.seq),
                                ..PtyResponse::plain("output", f.data.to_string())
                            },
                        )
                        .await
                        {
                            return;
                        }
                        sent = f.seq;
                    }
                }
                Err(_) => {
                    // Redraw from what the ring still has.
                    send(&sender, PtyResponse::plain("replay_window_lost", "")).await;
                    if let Ok(frames) = live.replay_since(0).await {
                        for f in frames {
                            if !send(
                                &sender,
                                PtyResponse {
                                    seq: Some(f.seq),
                                    ..PtyResponse::plain("output", f.data.to_string())
                                },
                            )
                            .await
                            {
                                return;
                            }
                            sent = f.seq;
                        }
                    }
                }
            }
            send(&sender, PtyResponse::plain("replay_end", "")).await;
        }
        if live.is_closed() {
            send(&sender, PtyResponse::plain("closed", "Session ended")).await;
            return;
        }

        loop {
            match events.recv().await {
                Ok(SessionEvent::Output(f)) => {
                    if f.seq <= sent {
                        continue; // already replayed
                    }
                    if !send(
                        &sender,
                        PtyResponse {
                            seq: Some(f.seq),
                            ..PtyResponse::plain("output", f.data.to_string())
                        },
                    )
                    .await
                    {
                        return;
                    }
                    sent = f.seq;
                }
                Ok(SessionEvent::Closed(reason)) => {
                    send(&sender, PtyResponse::plain("closed", reason.to_string())).await;
                    return;
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    debug!(
                        lagged = n,
                        "Slow PTY client; catching up from the replay ring"
                    );
                    match live.replay_since(sent).await {
                        Ok(frames) => {
                            for f in frames {
                                if !send(
                                    &sender,
                                    PtyResponse {
                                        seq: Some(f.seq),
                                        ..PtyResponse::plain("output", f.data.to_string())
                                    },
                                )
                                .await
                                {
                                    return;
                                }
                                sent = f.seq;
                            }
                        }
                        Err(_) => {
                            send(&sender, PtyResponse::plain("replay_window_lost", "")).await;
                        }
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => return,
            }
        }
    })
}

/// Start an interactive PTY session
async fn start_pty_session(
    state: &AppState,
    user_id: i64,
    client_ip: &str,
    server_id: i64,
    cols: u32,
    rows: u32,
) -> Result<Arc<LiveSession>, String> {
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

    // Get credential if assigned
    let credential = if let Some(cred_id) = server.credential_id {
        queries::credentials::get_credential(&state.pool, cred_id)
            .await
            .ok()
    } else {
        None
    };
    // The server's username, else the credential's default, else root
    let username = server
        .username
        .as_deref()
        .or_else(|| credential.as_ref().and_then(|c| c.username.as_deref()))
        .unwrap_or("root")
        .to_string();

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

                // The session owns the PTY from here on; sockets attach to it.
                let (command_tx, mut command_rx) = mpsc::channel::<PtyCommand>(256);
                let live = LiveSession::new(user_id, server.id, server.name.clone(), command_tx);
                let session_for_driver = live.clone();
                let registry = state.sessions.clone();

                // Spawn task to handle channel I/O - owns the channel
                tokio::spawn(async move {
                    let live = session_for_driver;
                    let mut channel = channel;
                    let mut reason = "Session ended".to_string();
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
                                        live.push_output(String::from_utf8_lossy(&data).to_string()).await;
                                    }
                                    Some(russh::ChannelMsg::ExtendedData { data, .. }) => {
                                        // Extended data (usually stderr)
                                        live.push_output(String::from_utf8_lossy(&data).to_string()).await;
                                    }
                                    Some(russh::ChannelMsg::Eof) => {
                                        live.push_output("\r\n\x1b[33m[Session ended]\x1b[0m\r\n".to_string()).await;
                                        break;
                                    }
                                    Some(russh::ChannelMsg::Close) => {
                                        break;
                                    }
                                    Some(russh::ChannelMsg::ExitStatus { exit_status }) => {
                                        reason = format!("Shell exited with status {exit_status}");
                                        live.push_output(format!("\r\n\x1b[90m[Exit status: {}]\x1b[0m\r\n", exit_status)).await;
                                    }
                                    None => break,
                                    _ => {}
                                }
                            }
                        }
                    }
                    live.mark_closed(&reason);
                    registry.remove(&live.id).await;
                });

                return Ok(live);
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
