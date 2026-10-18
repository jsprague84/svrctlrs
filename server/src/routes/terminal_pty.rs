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
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use crate::routes::ui::auth::CurrentUser;
use crate::sessions::{LiveSession, OutputFrame, PtyCommand, SessionEvent};
use crate::ssh::ConnectError;
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
    /// `shell`/`attach`: send output as binary frames (8-byte big-endian `seq`, then the raw
    /// PTY bytes) instead of JSON `output` text. Clients that do not ask get JSON.
    #[serde(default)]
    binary: bool,
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
    /// On `error`: a machine-readable reason (`session_gone`, `quota`, `host_key_unconfirmed`,
    /// ...)
    #[serde(skip_serializing_if = "Option::is_none")]
    code: Option<String>,
    /// On `error`: what the client needs to act on the reason (`host_key_unconfirmed`:
    /// `key_type`, `fingerprint`)
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<serde_json::Value>,
}

impl PtyResponse {
    fn plain(kind: &str, data: impl Into<String>) -> Self {
        Self {
            response_type: kind.to_string(),
            data: data.into(),
            seq: None,
            session_id: None,
            code: None,
            details: None,
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
                                forwarder = Some(spawn_forwarder(
                                    live.clone(),
                                    0,
                                    ws_sender.clone(),
                                    req.binary,
                                ));
                                send(&ws_sender, PtyResponse {
                                    session_id: Some(live.id.clone()),
                                    ..PtyResponse::plain("connected", "\x1b[32m✓ Interactive shell session started\x1b[0m\r\n")
                                }).await;
                                session = Some(live);
                            }
                            Err(e) => {
                                let (code, details) = match &e {
                                    ConnectError::HostKeyUnconfirmed {
                                        server_id,
                                        server_name,
                                        key_type,
                                        fingerprint,
                                    } => (
                                        Some("host_key_unconfirmed".to_string()),
                                        Some(serde_json::json!({
                                            "server_id": server_id,
                                            "server_name": server_name,
                                            "key_type": key_type,
                                            "fingerprint": fingerprint,
                                        })),
                                    ),
                                    ConnectError::HostKeyMismatch(_) => {
                                        (Some("host_key_mismatch".to_string()), None)
                                    }
                                    ConnectError::Failed(_) => (None, None),
                                };
                                send(
                                    &ws_sender,
                                    PtyResponse {
                                        code,
                                        details,
                                        ..PtyResponse::plain(
                                            "error",
                                            format!(
                                                "\x1b[31m✗ Failed to start shell: {e}\x1b[0m\r\n"
                                            ),
                                        )
                                    },
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
                                    req.binary,
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

/// Encode an output frame for the wire: `seq` as 8 big-endian bytes, then the PTY bytes.
fn binary_frame(frame: &OutputFrame) -> Vec<u8> {
    let mut out = Vec::with_capacity(8 + frame.data.len());
    out.extend_from_slice(&frame.seq.to_be_bytes());
    out.extend_from_slice(&frame.data);
    out
}

/// Send one output frame: binary when the client asked for it, otherwise the legacy JSON
/// `output` text (lossy for a multibyte character split across frames — old clients only).
async fn send_output(sender: &WsSender, frame: &OutputFrame, binary: bool) -> bool {
    if binary {
        return sender
            .lock()
            .await
            .send(Message::Binary(binary_frame(frame).into()))
            .await
            .is_ok();
    }
    send(
        sender,
        PtyResponse {
            seq: Some(frame.seq),
            ..PtyResponse::plain("output", String::from_utf8_lossy(&frame.data).into_owned())
        },
    )
    .await
}

/// Stream a session's output to one socket: first everything after `last_seq` from the replay
/// ring, then live frames. A lagging subscriber is caught up from the ring instead of silently
/// losing frames; a client too far behind the ring is told to redraw from scratch.
fn spawn_forwarder(
    live: Arc<LiveSession>,
    last_seq: u64,
    sender: WsSender,
    binary: bool,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut events = live.subscribe();
        let mut sent = last_seq;

        if last_seq > 0 || live.latest_seq().await > 0 {
            send(&sender, PtyResponse::plain("replay_start", "")).await;
            match live.replay_since(last_seq).await {
                Ok(frames) => {
                    for f in frames {
                        if !send_output(&sender, &f, binary).await {
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
                            if !send_output(&sender, &f, binary).await {
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
                    if !send_output(&sender, &f, binary).await {
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
                                if !send_output(&sender, &f, binary).await {
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

/// Start an interactive PTY session: connect through the shared SSH client, open a shell
/// with a PTY, and hand the channel to a driver task that owns it for the session's life.
async fn start_pty_session(
    state: &AppState,
    user_id: i64,
    client_ip: &str,
    server_id: i64,
    cols: u32,
    rows: u32,
) -> Result<Arc<LiveSession>, ConnectError> {
    use svrctlrs_database::queries;

    let server = queries::servers::get_server(&state.pool, server_id)
        .await
        .map_err(|e| format!("Server not found: {}", e))?;
    let credential = match server.credential_id {
        Some(cred_id) => queries::credentials::get_credential(&state.pool, cred_id)
            .await
            .ok(),
        None => None,
    };

    let conn = crate::ssh::connect(&state.pool, &server, credential.as_ref()).await?;
    crate::audit::record(
        &state.pool,
        NewAuditEntry::new("session.pty_opened")
            .user(user_id)
            .client_ip(Some(client_ip.to_string()))
            .target("server", server.id, &server.name)
            .details(serde_json::json!({
                "login": conn.target.login(),
                "auth": conn.auth,
            })),
    );
    let session = conn.handle;

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

    // The server's startup command, typed as the first line of input. It echoes in the
    // terminal like anything typed, so the user sees what ran.
    if let Some(cmd) = server
        .startup_command
        .as_deref()
        .map(str::trim)
        .filter(|c| !c.is_empty())
    {
        let line = format!("{cmd}\n");
        channel
            .data(line.as_bytes())
            .await
            .map_err(|e| format!("Failed to send the startup command: {e}"))?;
    }

    // The session owns the PTY from here on; sockets attach to it.
    let (command_tx, mut command_rx) = mpsc::channel::<PtyCommand>(256);
    let live = LiveSession::new(user_id, server.id, server.name.clone(), command_tx);
    let session_for_driver = live.clone();
    let registry = state.sessions.clone();

    // Spawn task to handle channel I/O - owns the channel and the SSH session
    tokio::spawn(async move {
        let live = session_for_driver;
        let session = session;
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
                            live.push_output(data.to_vec()).await;
                        }
                        Some(russh::ChannelMsg::ExtendedData { data, .. }) => {
                            // Extended data (usually stderr)
                            live.push_output(data.to_vec()).await;
                        }
                        Some(russh::ChannelMsg::Eof) => {
                            live.push_text("\r\n\x1b[33m[Session ended]\x1b[0m\r\n").await;
                            break;
                        }
                        Some(russh::ChannelMsg::Close) => {
                            break;
                        }
                        Some(russh::ChannelMsg::ExitStatus { exit_status }) => {
                            reason = format!("Shell exited with status {exit_status}");
                            live.push_text(&format!("\r\n\x1b[90m[Exit status: {}]\x1b[0m\r\n", exit_status)).await;
                        }
                        None => break,
                        _ => {}
                    }
                }
            }
        }
        live.mark_closed(&reason);
        registry.remove(&live.id).await;
        let _ = session
            .disconnect(russh::Disconnect::ByApplication, "", "English")
            .await;
    });

    Ok(live)
}
