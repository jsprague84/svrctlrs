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
use tokio::sync::{broadcast, mpsc};
use tracing::{debug, error, info, warn};

use crate::api_tokens::TokenAuth;
use crate::routes::ui::auth::CurrentUser;
use crate::sessions::{Followed, LiveSession, OutputFrame, PtyCommand};
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
    /// On `connected`: whether the session's output is being recorded
    #[serde(skip_serializing_if = "Option::is_none")]
    recording: Option<bool>,
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
            recording: None,
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
    token: Option<axum::Extension<TokenAuth>>,
) -> Response {
    // Cross-site WebSocket hijacking guard; see auth::websocket_origin_allowed.
    if !crate::routes::ui::auth::websocket_origin_allowed(&headers, &state.allowed_origins) {
        warn!("PTY WebSocket refused: Origin not allowed");
        return (StatusCode::FORBIDDEN, "origin not allowed").into_response();
    }
    info!("Interactive PTY WebSocket connection request");
    let ip = crate::routes::ui::auth::client_ip(&headers);
    let token = token.map(|axum::Extension(t)| t);
    // The socket outlives the request's task, so it takes the token along: for the audit
    // rows it writes, and to end when the token is revoked.
    ws.on_upgrade(move |socket| {
        let token_id = token.as_ref().map(|t| t.id);
        crate::audit::with_token(
            token,
            handle_pty_socket(socket, state, user_id, ip, token_id),
        )
    })
    .into_response()
}

/// What woke the socket loop
enum Wake {
    Message(Result<Message, axum::Error>),
    Ended,
    TokenRevoked,
}

/// Resolves when the API token `token_id` is revoked or removed; never without a token.
async fn token_revoked(
    state: &AppState,
    revocations: &mut Option<broadcast::Receiver<i64>>,
    token_id: Option<i64>,
) {
    let (Some(rx), Some(id)) = (revocations.as_mut(), token_id) else {
        return std::future::pending().await;
    };
    loop {
        match rx.recv().await {
            Ok(revoked) if revoked == id => return,
            Ok(_) => {}
            // Missed some: ask the database instead
            Err(broadcast::error::RecvError::Lagged(_)) => {
                if !crate::api_tokens::still_valid(&state.pool, id).await {
                    return;
                }
            }
            Err(broadcast::error::RecvError::Closed) => return std::future::pending().await,
        }
    }
}

/// Commands to send to the PTY session
async fn handle_pty_socket(
    socket: WebSocket,
    state: AppState,
    user_id: i64,
    client_ip: String,
    token_id: Option<i64>,
) {
    let (ws_sender, mut ws_receiver) = socket.split();
    let ws_sender = Arc::new(tokio::sync::Mutex::new(ws_sender));

    info!("Interactive PTY WebSocket connected");

    // The session this socket is attached to, and the task streaming its output here.
    let mut session: Option<Arc<LiveSession>> = None;
    let mut forwarder: Option<tokio::task::JoinHandle<()>> = None;
    // `close` means "kill the shell"; anything else leaving a session behind only detaches it.
    let mut close_on_exit = false;
    // A shell this socket started (not one it attached to) ends with a revoked token.
    let mut started_here = false;
    let mut revoked = false;
    let mut revocations = token_id.map(|_| state.token_revocations.subscribe());

    loop {
        let wake = tokio::select! {
            msg = ws_receiver.next() => match msg {
                Some(msg) => Wake::Message(msg),
                None => Wake::Ended,
            },
            () = token_revoked(&state, &mut revocations, token_id) => Wake::TokenRevoked,
        };
        let msg = match wake {
            Wake::Message(msg) => msg,
            Wake::Ended => break,
            Wake::TokenRevoked => {
                revoked = true;
                break;
            }
        };
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
                        match start_pty_session(
                            &state,
                            user_id,
                            &client_ip,
                            server_id,
                            (cols, rows),
                            Launch::New,
                        )
                        .await
                        {
                            Ok((live, record)) => {
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
                                let note = if record {
                                    recording_note(&state, &live, cols, rows).await
                                } else {
                                    String::new()
                                };
                                forwarder = Some(spawn_forwarder(
                                    live.clone(),
                                    0,
                                    ws_sender.clone(),
                                    req.binary,
                                ));
                                send(&ws_sender, PtyResponse {
                                    session_id: Some(live.id.clone()),
                                    recording: Some(live.is_recorded()),
                                    ..PtyResponse::plain("connected", format!("\x1b[32m✓ Interactive shell session started\x1b[0m\r\n{note}"))
                                }).await;
                                session = Some(live);
                                started_here = true;
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
                        let attached = match state.sessions.attach(id, user_id).await {
                            Some(live) => Some(live),
                            // Not live: a kept shell (tmux) from before a restart or a lost
                            // connection is reattached over a new connection
                            None => {
                                let size = (req.cols.unwrap_or(80), req.rows.unwrap_or(24));
                                match state
                                    .persistent
                                    .revive(&state, user_id, id, size, req.last_seq, &client_ip)
                                    .await
                                {
                                    Ok(Some((live, record))) => {
                                        if record {
                                            let (cols, rows) = size;
                                            recording_note(&state, &live, cols, rows).await;
                                        }
                                        Some(live)
                                    }
                                    Ok(None) => None,
                                    Err(e) => {
                                        send(
                                            &ws_sender,
                                            PtyResponse {
                                                code: Some("resume_failed".into()),
                                                ..PtyResponse::plain(
                                                    "error",
                                                    format!("\r\n\x1b[31m✗ Could not reconnect the kept shell: {e}\x1b[0m\r\n"),
                                                )
                                            },
                                        )
                                        .await;
                                        continue;
                                    }
                                }
                            }
                        };
                        match attached {
                            Some(live) => {
                                info!(session_id = %live.id, server = %live.server_name, "Client reattached to session");
                                if let (Some(cols), Some(rows)) = (req.cols, req.rows) {
                                    let _ = live.resize(cols, rows).await;
                                }
                                send(
                                    &ws_sender,
                                    PtyResponse {
                                        session_id: Some(live.id.clone()),
                                        recording: Some(live.is_recorded()),
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
    if revoked {
        warn!(token_id, "API token revoked; ending its terminal");
        send(
            &ws_sender,
            PtyResponse {
                code: Some("token_revoked".into()),
                ..PtyResponse::plain(
                    "error",
                    "\r\n\x1b[31m✗ The API token this terminal was opened with was revoked\x1b[0m\r\n",
                )
            },
        )
        .await;
        ws_sender.lock().await.close().await.ok();
        close_on_exit = started_here;
    }
    if let Some(live) = session {
        if close_on_exit {
            live.close().await;
            live.mark_closed("Session closed");
            state.sessions.remove(&live.id).await;
        } else {
            // Reload, lost link, backgrounded tab: keep the shell for the user's keep time.
            let (ttl, _) = crate::routes::api::sessions::keep_time(&state, user_id).await;
            state.sessions.detach(&live, ttl).await;
            info!(session_id = %live.id, ttl_secs = ttl.as_secs(),
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

/// Stream a session's output to one socket: everything after `last_seq` from the replay ring
/// (between `replay_start` and `replay_end`), then live frames. A socket that falls behind is
/// caught up from the ring; one too far behind for the ring gets `replay_window_lost` and a
/// redraw from what the ring still holds, whether that happens on attach or later.
fn spawn_forwarder(
    live: Arc<LiveSession>,
    last_seq: u64,
    sender: WsSender,
    binary: bool,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut follow = live.follow(last_seq);
        if last_seq > 0 || live.latest_seq().await > 0 {
            send(&sender, PtyResponse::plain("replay_start", "")).await;
            // A client with nothing on screen wants the screen; one that has seen frames wants
            // exactly the rest, or to hear that some are gone.
            let items = if last_seq == 0 {
                follow.redraw().await
            } else {
                follow.catch_up().await
            };
            for item in items {
                if !forward(&sender, item, binary).await {
                    return;
                }
            }
            send(&sender, PtyResponse::plain("replay_end", "")).await;
        }
        while let Some(item) = follow.next().await {
            if !forward(&sender, item, binary).await {
                return;
            }
        }
    })
}

/// Hand one followed item to the socket. `false` when the socket is gone or the session ended.
async fn forward(sender: &WsSender, item: Followed, binary: bool) -> bool {
    match item {
        Followed::Output(frame) => send_output(sender, &frame, binary).await,
        Followed::WindowLost => send(sender, PtyResponse::plain("replay_window_lost", "")).await,
        Followed::Resized { .. } => true,
        Followed::Closed(reason) => {
            send(sender, PtyResponse::plain("closed", reason.to_string())).await;
            false
        }
        // A kept shell let go for a restart: closing the socket makes the client reconnect,
        // and its reattach reaches the restarted server
        Followed::Released => {
            sender.lock().await.close().await.ok();
            false
        }
    }
}

/// Start recording a session that was just registered, and say so in the terminal: a grey
/// line when it is recorded, a yellow one when recording was wanted and failed (the shell
/// goes ahead either way).
async fn recording_note(state: &AppState, live: &Arc<LiveSession>, cols: u32, rows: u32) -> String {
    match crate::recordings::start(&state.pool, &state.recordings, live, cols, rows).await {
        Ok(_) => "\x1b[90m● This session is recorded (what the terminal shows, not what you type)\x1b[0m\r\n".to_string(),
        Err(e) => {
            warn!(session_id = %live.id, error = %e, "Could not start recording the session");
            format!("\x1b[33m⚠ This session is not recorded: {e}\x1b[0m\r\n")
        }
    }
}

/// What [`start_pty_session`] starts.
pub(crate) enum Launch<'a> {
    /// A new shell: in tmux when the server keeps its shells, else the login shell
    New,
    /// A kept shell again, after a restart or a lost connection; output is numbered after
    /// `last_seq`, the last frame the reattaching client saw
    Revive {
        shell: &'a svrctlrs_database::models::PersistentShell,
        last_seq: u64,
    },
}

/// Start an interactive PTY session: connect through the shared SSH client, open a shell
/// with a PTY, and hand the channel to a driver task that owns it for the session's life.
/// Also returns whether the server wants its sessions recorded.
pub(crate) async fn start_pty_session(
    state: &AppState,
    user_id: i64,
    client_ip: &str,
    server_id: i64,
    (cols, rows): (u32, u32),
    launch: Launch<'_>,
) -> Result<(Arc<LiveSession>, bool), ConnectError> {
    use crate::persistent_shells::{shell_command, tmux_name};
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

    // A kept shell: its id (which names the tmux session), the tmux name, the command run
    // instead of the login shell, and whether it is a reattach
    let kept = match &launch {
        Launch::Revive { shell, .. } => Some((shell.id.clone(), shell.tmux_name.clone(), true)),
        Launch::New if server.persistent_shell => {
            let id = crate::sessions::new_session_id();
            let name = tmux_name(&id);
            Some((id, name, false))
        }
        Launch::New => None,
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
                "recorded": server.record_sessions,
                "persistent": kept.is_some(),
                "reattached": matches!(launch, Launch::Revive { .. }),
            })),
    );
    let program = kept
        .as_ref()
        .map(|(_, name, revived)| shell_command(name, *revived));
    // The startup command belongs to a new shell; a reattached one already ran it
    let startup = match launch {
        Launch::New => server.startup_command.as_deref(),
        Launch::Revive { .. } => None,
    };
    let channel = open_shell(&conn, program.as_deref(), startup, cols, rows).await?;

    // The session owns the PTY from here on; sockets attach to it.
    let (command_tx, command_rx) = mpsc::channel::<PtyCommand>(256);
    let (live, keep) = match kept {
        None => (
            LiveSession::new(user_id, server.id, server.name.clone(), command_tx),
            None,
        ),
        Some((id, name, revived)) => {
            let (created_at, seq_base) = match launch {
                Launch::Revive { shell, last_seq } => (shell.created_at, last_seq),
                Launch::New => (chrono::Utc::now(), 0),
            };
            if !revived {
                if let Err(e) =
                    queries::persistent_shells::insert(&state.pool, &id, user_id, server.id, &name)
                        .await
                {
                    let _ = channel.close().await;
                    return Err(ConnectError::Failed(format!(
                        "Could not keep the shell: {e}"
                    )));
                }
            }
            let live = LiveSession::persistent(
                id,
                user_id,
                server.id,
                server.name.clone(),
                command_tx,
                created_at,
                seq_base,
            );
            (
                live,
                Some(Keep {
                    state: state.clone(),
                    tmux_name: name,
                }),
            )
        }
    };
    spawn_pty_driver(
        live.clone(),
        conn,
        channel,
        command_rx,
        state.sessions.clone(),
        keep,
    );
    Ok((live, server.record_sessions))
}

/// What the driver of a kept shell needs when it ends.
pub(crate) struct Keep {
    pub state: AppState,
    pub tmux_name: String,
}

/// Open an interactive shell on an authenticated connection: a session channel, a PTY of the
/// given size, the login shell (or `program` in its place, for a kept shell), and the
/// server's startup command typed as the first line (it echoes like anything typed, so the
/// user sees what ran).
pub(crate) async fn open_shell(
    conn: &crate::ssh::Connection,
    program: Option<&str>,
    startup_command: Option<&str>,
    cols: u32,
    rows: u32,
) -> Result<russh::Channel<russh::client::Msg>, ConnectError> {
    let channel = conn
        .handle
        .channel_open_session()
        .await
        .map_err(|e| format!("Failed to open channel: {}", e))?;

    // Advertise 24-bit colour. sshd only passes variables its `AcceptEnv` lists, so this is
    // best effort (no reply wanted); the startup command is the sure way.
    let _ = channel.set_env(false, "COLORTERM", "truecolor").await;

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

    match program {
        None => channel.request_shell(false).await,
        Some(program) => channel.exec(false, program).await,
    }
    .map_err(|e| format!("Failed to start shell: {}", e))?;

    if let Some(cmd) = startup_command.map(str::trim).filter(|c| !c.is_empty()) {
        let line = format!("{cmd}\n");
        channel
            .data(line.as_bytes())
            .await
            .map_err(|e| format!("Failed to send the startup command: {e}"))?;
    }
    Ok(channel)
}

/// The PTY driver: one task that owns the shell's channel and its connection (jump hosts
/// included) for the session's life. Input, resizes and close come from the session's command
/// queue; output goes into the session's ring and out to attached sockets. When the shell
/// ends the session is marked closed with the reason, removed from the registry, and the
/// connection is closed.
///
/// A kept shell (`keep`) ends differently. Released for a restart: tmux keeps it, and so does
/// the row. Connection lost: released too, so clients reconnect and reattach, and it ends
/// after the keep time if none does. Ended for good (closed, expired, exited): its tmux
/// session is ended on this connection and the row goes.
pub(crate) fn spawn_pty_driver(
    live: Arc<LiveSession>,
    conn: crate::ssh::Connection,
    mut channel: russh::Channel<russh::client::Msg>,
    mut command_rx: mpsc::Receiver<PtyCommand>,
    registry: Arc<crate::sessions::SessionRegistry>,
    keep: Option<Keep>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut reason = "Session ended".to_string();
        // How it ended: closed by a command, or the shell ended; neither means the connection
        // was lost
        let mut ended_here = false;
        loop {
            tokio::select! {
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
                            ended_here = true;
                            let _ = channel.eof().await;
                            let _ = channel.close().await;
                            break;
                        }
                    }
                }
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
                            ended_here = true;
                            live.push_text("\r\n\x1b[33m[Session ended]\x1b[0m\r\n").await;
                            break;
                        }
                        Some(russh::ChannelMsg::Close) => {
                            break;
                        }
                        Some(russh::ChannelMsg::ExitStatus { exit_status }) => {
                            ended_here = true;
                            reason = format!("Shell exited with status {exit_status}");
                            live.push_text(&format!("\r\n\x1b[90m[Exit status: {}]\x1b[0m\r\n", exit_status)).await;
                        }
                        None => break,
                        _ => {}
                    }
                }
            }
        }
        if let Some(keep) = keep {
            let restart = live.is_released();
            if restart || !ended_here {
                // A restart, or the connection dropped: tmux keeps the shell for a reattach
                live.release();
                registry.remove(&live.id).await;
                let _ = conn
                    .handle
                    .disconnect(russh::Disconnect::ByApplication, "", "English")
                    .await;
                // After a restart the next start-up arms the wait
                if !restart {
                    let (ttl, _) =
                        crate::routes::api::sessions::keep_time(&keep.state, live.user_id).await;
                    info!(session_id = %live.id, ttl_secs = ttl.as_secs(),
                        "Kept shell lost its connection; waiting for a reattach");
                    keep.state
                        .persistent
                        .wait(keep.state.clone(), live.id.clone(), ttl)
                        .await;
                }
                return;
            }
            // Ended for good: end the tmux session too (it may have ended already)
            end_tmux(&conn, &keep.tmux_name).await;
            if let Err(e) =
                svrctlrs_database::queries::persistent_shells::delete(&keep.state.pool, &live.id)
                    .await
            {
                warn!(session_id = %live.id, error = %e, "Could not forget a kept shell");
            }
        }
        live.mark_closed(&reason);
        registry.remove(&live.id).await;
        let _ = conn
            .handle
            .disconnect(russh::Disconnect::ByApplication, "", "English")
            .await;
    })
}

/// End a kept shell's tmux session on a connection that is still up, within a few seconds.
async fn end_tmux(conn: &crate::ssh::Connection, tmux_name: &str) {
    let ended = async {
        let mut channel = conn.handle.channel_open_session().await.ok()?;
        channel
            .exec(true, crate::persistent_shells::kill_command(tmux_name))
            .await
            .ok()?;
        while let Some(msg) = channel.wait().await {
            if matches!(msg, russh::ChannelMsg::Close) {
                break;
            }
        }
        Some(())
    };
    if tokio::time::timeout(std::time::Duration::from_secs(5), ended)
        .await
        .ok()
        .flatten()
        .is_none()
    {
        warn!(tmux = %tmux_name, "Could not end the kept shell's tmux session");
    }
}

/// The PTY driver against the in-process SSH server: what the shell is asked for, input and
/// resizes going out, output and the exit status coming back, and the session's end.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::sessions::{SessionEvent, SessionRegistry};
    use crate::test_support::ssh_server::{Event, Options, TestSshServer};
    use crate::test_support::{add_credential, add_server, database, trust_recorded_keys};
    use std::time::Duration;
    use svrctlrs_database::models::CredentialType;
    use svrctlrs_database::queries;
    use svrctlrs_database::sqlx::{Pool, Sqlite};
    use tokio::sync::broadcast;

    /// A trusted, authenticated connection to the loopback server (through `via` if given)
    async fn connected(
        ssh: &TestSshServer,
        pool: &Pool<Sqlite>,
        via_loop: bool,
    ) -> crate::ssh::Connection {
        let cred = add_credential(pool, "pw", CredentialType::Password, "pw").await;
        let mut id = add_server(pool, "loop", ssh.port, "alice", Some(cred), None).await;
        if via_loop {
            id = add_server(pool, "inner", ssh.port, "alice", Some(cred), Some(id)).await;
        }
        for _ in 0..3 {
            let server = queries::servers::get_server(pool, id).await.unwrap();
            let credential = queries::credentials::get_credential(pool, cred)
                .await
                .unwrap();
            match crate::ssh::connect(pool, &server, Some(&credential)).await {
                Ok(conn) => return conn,
                Err(ConnectError::HostKeyUnconfirmed { server_id, .. }) => {
                    trust_recorded_keys(pool, server_id).await
                }
                Err(e) => panic!("connect: {e}"),
            }
        }
        panic!("still unconfirmed after trusting every hop");
    }

    /// Everything the session printed so far, as text
    async fn screen(live: &LiveSession) -> String {
        let frames = live.held_frames().await;
        frames
            .iter()
            .map(|f| String::from_utf8_lossy(&f.data).into_owned())
            .collect()
    }

    async fn wait_for_screen(live: &LiveSession, needle: &str) -> bool {
        for _ in 0..500 {
            if screen(live).await.contains(needle) {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        false
    }

    async fn closed_reason(rx: &mut broadcast::Receiver<SessionEvent>) -> String {
        let wait = async {
            loop {
                match rx.recv().await {
                    Ok(SessionEvent::Closed(reason)) => return reason.to_string(),
                    Ok(_) => continue,
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(e) => panic!("events ended: {e}"),
                }
            }
        };
        tokio::time::timeout(Duration::from_secs(5), wait)
            .await
            .expect("session closed in time")
    }

    struct Running {
        live: Arc<LiveSession>,
        registry: Arc<SessionRegistry>,
        events: broadcast::Receiver<SessionEvent>,
    }

    async fn start(conn: crate::ssh::Connection, startup: Option<&str>) -> Running {
        let channel = open_shell(&conn, None, startup, 100, 30)
            .await
            .expect("shell");
        let registry = SessionRegistry::new(Duration::from_secs(60), Duration::from_secs(3600), 16);
        let (command_tx, command_rx) = mpsc::channel(256);
        let live = LiveSession::new(1, 1, "loop".into(), command_tx);
        registry.insert(live.clone()).await.unwrap();
        let events = live.subscribe();
        spawn_pty_driver(
            live.clone(),
            conn,
            channel,
            command_rx,
            registry.clone(),
            None,
        );
        Running {
            live,
            registry,
            events,
        }
    }

    #[tokio::test]
    async fn the_shell_gets_a_pty_the_startup_command_input_and_resizes() {
        let ssh = TestSshServer::start(Options::password("alice", "pw")).await;
        let pool = database().await;
        let conn = connected(&ssh, &pool, false).await;
        let mut run = start(conn, Some("  cd /srv && ls  ")).await;

        assert!(ssh.wait_for(|e| *e == Event::Shell).await);
        let events = ssh.events().await;
        assert!(events.contains(&Event::Pty {
            term: "xterm-256color".into(),
            cols: 100,
            rows: 30
        }));
        assert!(events.contains(&Event::Env {
            name: "COLORTERM".into(),
            value: "truecolor".into()
        }));
        // The startup command is the first input, trimmed, with its newline
        assert_eq!(
            events.iter().find(|e| matches!(e, Event::Input(_))),
            Some(&Event::Input(b"cd /srv && ls\n".to_vec()))
        );

        run.live.send_input(b"uptime\n").await.unwrap();
        assert!(
            wait_for_screen(&run.live, "ran: uptime").await,
            "{}",
            screen(&run.live).await
        );
        assert!(screen(&run.live).await.contains("ran: cd /srv && ls"));

        run.live.resize(120, 40).await.unwrap();
        assert!(
            ssh.wait_for(|e| *e
                == Event::Resize {
                    cols: 120,
                    rows: 40
                })
            .await
        );

        // Output is numbered in order, so a reconnect can ask for what it missed
        let frames = run.live.held_frames().await;
        assert!(frames.windows(2).all(|w| w[1].seq == w[0].seq + 1));

        run.live.send_input(b"exit 3\n").await.unwrap();
        assert_eq!(
            closed_reason(&mut run.events).await,
            "Shell exited with status 3"
        );
        assert!(run.live.is_closed());
        assert!(screen(&run.live).await.contains("[Exit status: 3]"));
        for _ in 0..100 {
            if run.registry.get(&run.live.id, 1).await.is_none() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(
            run.registry.get(&run.live.id, 1).await.is_none(),
            "an ended shell leaves the registry"
        );
    }

    #[tokio::test]
    async fn closing_the_session_ends_the_shell() {
        let ssh = TestSshServer::start(Options::password("alice", "pw")).await;
        let pool = database().await;
        let conn = connected(&ssh, &pool, false).await;
        let mut run = start(conn, None).await;
        assert!(wait_for_screen(&run.live, "$ ").await);

        run.live.close().await;
        assert_eq!(closed_reason(&mut run.events).await, "Session ended");
        assert!(
            run.live.send_input(b"x").await.is_err(),
            "the driver is gone"
        );
    }

    #[tokio::test]
    async fn a_shell_through_a_jump_host_stays_up() {
        let ssh = TestSshServer::start(Options::password("alice", "pw")).await;
        let pool = database().await;
        let conn = connected(&ssh, &pool, true).await;
        let run = start(conn, None).await;

        // The jump host's connection must outlive open_shell: the driver holds it
        tokio::time::sleep(Duration::from_millis(200)).await;
        run.live.send_input(b"still here\n").await.unwrap();
        assert!(
            wait_for_screen(&run.live, "ran: still here").await,
            "{}",
            screen(&run.live).await
        );

        // A large burst arrives whole and in order
        run.live.send_input(b"big 300000\n").await.unwrap();
        assert!(wait_for_screen(&run.live, "big 300000").await);
        for _ in 0..500 {
            if screen(&run.live).await.matches('x').count() >= 300_000 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(screen(&run.live).await.matches('x').count() >= 300_000);
    }
}
