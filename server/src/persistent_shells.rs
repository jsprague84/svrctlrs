//! Shells that outlive SvrCtlRS (ROADMAP 23).
//!
//! On a server with `persistent_shell` set, a new shell runs inside a tmux session on that
//! server, named `svrctlrs-<session id>`, and a `persistent_shells` row remembers it. When
//! SvrCtlRS stops, the session is released, not ended (`SessionRegistry::close_all`): the SSH
//! connection closes, which only detaches tmux. The same happens when the connection to the
//! server drops. A client reattaching with the session id then finds no live session, and
//! [`PersistentShells::revive`] opens a new connection attached to the same tmux session.
//!
//! A kept shell nobody reattaches to is ended after the user's keep time, counted from the
//! restart (or the lost connection): [`PersistentShells::wait`] arms that, [`discard`] does it
//! (a new connection runs `tmux kill-session`, and the row goes). A shell that ends for good
//! while connected (exit, closed by the user, left detached too long) ends its tmux session
//! on the same connection (`terminal_pty::spawn_pty_driver`).
//!
//! A server without tmux gets a plain shell and a note saying it will not survive a restart.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use svrctlrs_database::queries;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tracing::{info, warn};

use crate::sessions::LiveSession;
use crate::ssh::ConnectError;
use crate::state::AppState;

/// The tmux session holding the shell of SvrCtlRS session `session_id`. Session ids are
/// URL-safe base64, which tmux and every shell take as a plain word.
pub fn tmux_name(session_id: &str) -> String {
    format!("svrctlrs-{session_id}")
}

/// The command run instead of the login shell. It goes through the user's login shell (`exec`
/// request), which may be bash, zsh, fish or csh, so it is one `sh -c '…'` line with no `!`,
/// no `\\` and no `'` inside. tmux starts the user's shell as a login shell in its window; a
/// server without tmux gets a note and the login shell itself. `revived`: the client had this
/// shell before a restart, and is told if tmux no longer has it (the server rebooted).
pub fn shell_command(tmux_name: &str, revived: bool) -> String {
    let note = |text: &str| format!(r#"printf "\033[33m%s\033[0m\r\n" "{text}""#);
    let gone = if revived {
        format!(
            "tmux has-session -t ={tmux_name} 2>/dev/null || {}; ",
            note("The kept shell had ended (the server restarted?); this is a new one.")
        )
    } else {
        String::new()
    };
    format!(
        "sh -c 'if command -v tmux >/dev/null 2>&1; then {gone}exec tmux new-session -A -s {tmux_name}; fi; {}; exec \"${{SHELL:-/bin/sh}}\" -l'",
        note("tmux is not installed on this server: a plain shell, which ends when SvrCtlRS restarts.")
    )
}

/// Ends the tmux session; its exit status is of no interest (it may be gone already).
pub fn kill_command(tmux_name: &str) -> String {
    format!("tmux kill-session -t ={tmux_name}")
}

/// Kept shells nobody is connected to, and revivals in progress.
#[derive(Default)]
pub struct PersistentShells {
    /// Expiry timers of kept shells waiting for a client (after a restart or a lost connection)
    waiting: Mutex<HashMap<String, JoinHandle<()>>>,
    /// One revival at a time: two clients reattaching at once share one connection
    reviving: Mutex<()>,
}

impl PersistentShells {
    /// End the kept shell `id` after `ttl` unless a client reattaches first.
    pub async fn wait(self: &Arc<Self>, state: AppState, id: String, ttl: Duration) {
        let shells = Arc::clone(self);
        let key = id.clone();
        let handle = tokio::spawn(async move {
            tokio::time::sleep(ttl).await;
            if state.sessions.contains(&id).await {
                return;
            }
            shells.waiting.lock().await.remove(&id);
            info!(session_id = %id, "Kept shell not reattached in time; ending it");
            discard(&state, &id).await;
        });
        if let Some(old) = self.waiting.lock().await.insert(key, handle) {
            old.abort();
        }
    }

    async fn stop_waiting(&self, id: &str) {
        if let Some(timer) = self.waiting.lock().await.remove(id) {
            timer.abort();
        }
    }

    /// Reattach the caller to their kept shell `id`, which has no live session (SvrCtlRS
    /// restarted, or the connection dropped): a new connection attached to its tmux session.
    /// The session numbers its output after `last_seq`, the last frame the client saw. Returns
    /// the session, attached once, and whether the server records its shells; `None` when the
    /// caller has no kept shell by that id.
    pub async fn revive(
        self: &Arc<Self>,
        state: &AppState,
        user_id: i64,
        id: &str,
        size: (u32, u32),
        last_seq: u64,
        client_ip: &str,
    ) -> Result<Option<(Arc<LiveSession>, bool)>, ConnectError> {
        let _one_at_a_time = self.reviving.lock().await;
        // Revived by another client while this one waited
        if let Some(live) = state.sessions.attach(id, user_id).await {
            return Ok(Some((live, false)));
        }
        let Ok(Some(row)) =
            queries::persistent_shells::get_for_user(&state.pool, id, user_id).await
        else {
            return Ok(None);
        };
        self.stop_waiting(id).await;
        let started = crate::routes::terminal_pty::start_pty_session(
            state,
            user_id,
            client_ip,
            row.server_id,
            size,
            crate::routes::terminal_pty::Launch::Revive {
                shell: &row,
                last_seq,
            },
        )
        .await;
        let (live, record) = match started {
            Ok(started) => started,
            Err(e) => {
                // Try again later; it still ends after the keep time
                let (ttl, _) = crate::routes::api::sessions::keep_time(state, user_id).await;
                self.wait(state.clone(), id.to_string(), ttl).await;
                return Err(e);
            }
        };
        if let Err(e) = state.sessions.insert(live.clone()).await {
            // Over the quota: let go of the connection again, keep the shell
            live.release();
            live.close().await;
            return Err(ConnectError::Failed(e));
        }
        state.sessions.attach_new(&live);
        info!(session_id = %live.id, server = %live.server_name, "Kept shell reattached");
        Ok(Some((live, record)))
    }
}

/// End the kept shell `id` for good from outside a session: connect, end its tmux session,
/// forget the row. The row goes even when the server cannot be reached, so an unreachable
/// server does not keep a dead entry in the list (its tmux session is then left behind).
pub async fn discard(state: &AppState, id: &str) {
    let Ok(Some(row)) = queries::persistent_shells::get(&state.pool, id).await else {
        return;
    };
    let ended = async {
        let server = queries::servers::get_server(&state.pool, row.server_id)
            .await
            .map_err(|e| e.to_string())?;
        let credential = match server.credential_id {
            Some(c) => queries::credentials::get_credential(&state.pool, c)
                .await
                .ok(),
            None => None,
        };
        crate::ssh::run_bounded(
            &state.pool,
            &server,
            credential.as_ref(),
            &kill_command(&row.tmux_name),
            Duration::from_secs(15),
            4096,
        )
        .await
        .map_err(|e| e.to_string())
    }
    .await;
    if let Err(e) = ended {
        warn!(session_id = %id, error = %e, "Could not end the kept shell's tmux session");
    }
    if let Err(e) = queries::persistent_shells::delete(&state.pool, id).await {
        warn!(session_id = %id, error = %e, "Could not forget a kept shell");
    }
}

/// At start-up every kept shell is waiting for its client: each ends after its owner's keep
/// time unless reattached first.
pub fn start_housekeeping(state: AppState) {
    tokio::spawn(async move {
        let rows = match queries::persistent_shells::list_all(&state.pool).await {
            Ok(rows) => rows,
            Err(e) => {
                warn!(error = %e, "Could not list kept shells");
                return;
            }
        };
        if !rows.is_empty() {
            info!(count = rows.len(), "Kept shells waiting to be reattached");
        }
        for row in rows {
            let (ttl, _) = crate::routes::api::sessions::keep_time(&state, row.user_id).await;
            state.persistent.wait(state.clone(), row.id, ttl).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_command_survives_any_login_shell() {
        for revived in [false, true] {
            let cmd = shell_command("svrctlrs-AbC_d-9", revived);
            assert!(cmd.starts_with("sh -c '") && cmd.ends_with('\''), "{cmd}");
            let inner = &cmd["sh -c '".len()..cmd.len() - 1];
            // Nothing a login shell would expand or end the quotes with
            assert!(!inner.contains('\''), "{inner}");
            assert!(!inner.contains('!'), "csh history: {inner}");
            assert!(!inner.contains("\\\\"), "fish escapes: {inner}");
            assert!(!inner.contains('\n'), "csh: one line");
            assert!(inner.contains("exec tmux new-session -A -s svrctlrs-AbC_d-9"));
            assert!(inner.contains(r#"exec "${SHELL:-/bin/sh}" -l"#));
            assert_eq!(inner.contains("has-session -t =svrctlrs-AbC_d-9"), revived);
        }
    }

    #[test]
    fn the_command_runs_under_sh() {
        // With tmux absent the note is printed and the login shell started; here `SHELL` is
        // `echo`, so the "shell" prints its argument and exits.
        let cmd = shell_command("svrctlrs-x", true);
        let out = std::process::Command::new("sh")
            .arg("-c")
            .arg(&cmd)
            .env("PATH", "/nonexistent")
            .env("SHELL", "/bin/echo")
            .output();
        let Ok(out) = out else { return }; // no sh here
        let text = String::from_utf8_lossy(&out.stdout);
        if out.status.success() || !text.is_empty() {
            assert!(text.contains("tmux is not installed"), "{text}");
            assert!(text.contains("-l"), "the login shell ran: {text}");
        }
    }

    /// The whole path through the app and a loopback SSH server that acts like tmux: a kept
    /// shell starts in tmux, survives a "restart" (the registry lets go of it), is listed as
    /// waiting, reattaches over a new connection with its output numbered after what the client
    /// saw, and closing it ends the tmux session and the row.
    #[tokio::test]
    async fn a_kept_shell_outlives_a_restart_and_ends_with_its_tmux_session() {
        use crate::test_support::app::{call, AppHarness};
        use crate::test_support::ssh_server::{Event, Options, TestSshServer};
        use futures_util::{SinkExt, StreamExt};
        use serde_json::{json, Value};
        use svrctlrs_database::models::{CredentialType, UpdateServer};
        use tokio_tungstenite::tungstenite::Message as WsMessage;

        let h = AppHarness::new().await;
        let ssh = TestSshServer::start(Options::password("alice", "pw")).await;
        let pool = &h.state.pool;
        let cred =
            crate::test_support::add_credential(pool, "pw", CredentialType::Password, "pw").await;
        let server_id =
            crate::test_support::add_server(pool, "loop", ssh.port, "alice", Some(cred), None)
                .await;
        queries::servers::update_server(
            pool,
            server_id,
            &UpdateServer {
                persistent_shell: Some(true),
                startup_command: Some("cd /srv".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        // Trust the loopback host key up front
        let server = queries::servers::get_server(pool, server_id).await.unwrap();
        let credential = queries::credentials::get_credential(pool, cred)
            .await
            .unwrap();
        if let Err(ConnectError::HostKeyUnconfirmed { .. }) =
            crate::ssh::connect(pool, &server, Some(&credential)).await
        {
            crate::test_support::trust_recorded_keys(pool, server_id).await;
        }

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let served = h.app.clone();
        tokio::spawn(async move { axum::serve(listener, served.into_make_service()).await });
        let socket = || async {
            let (status, body) = call(
                &h.app,
                "POST",
                "/api/v1/auth/ws-ticket",
                Some(&h.session),
                None,
            )
            .await;
            assert!(status.is_success(), "{body}");
            let ticket = body["ticket"].as_str().unwrap().to_string();
            tokio_tungstenite::connect_async(format!("ws://{addr}/ws/terminal/pty?ticket={ticket}"))
                .await
                .expect("upgrade")
                .0
        };
        async fn next<S>(ws: &mut S, want: impl Fn(&Value) -> bool) -> Option<Value>
        where
            S: futures_util::Stream<
                    Item = Result<WsMessage, tokio_tungstenite::tungstenite::Error>,
                > + Unpin,
        {
            let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
            loop {
                let msg = tokio::time::timeout_at(deadline, ws.next())
                    .await
                    .ok()??
                    .ok()?;
                if let WsMessage::Text(text) = msg {
                    let v: Value = serde_json::from_str(&text).ok()?;
                    if want(&v) {
                        return Some(v);
                    }
                }
            }
        }

        // A new shell runs the tmux command, and the startup command is typed into it
        let mut ws = socket().await;
        ws.send(WsMessage::text(
            json!({ "type": "shell", "server_id": server_id, "cols": 80, "rows": 24 }).to_string(),
        ))
        .await
        .unwrap();
        let connected = next(&mut ws, |v| v["type"] == "connected")
            .await
            .expect("a shell");
        let id = connected["session_id"].as_str().unwrap().to_string();
        let name = tmux_name(&id);
        assert!(
            ssh.wait_for(
                |e| matches!(e, Event::Exec(c) if c.contains(&format!("new-session -A -s {name}")))
            )
            .await
        );
        assert!(
            ssh.wait_for(|e| *e == Event::Input(b"cd /srv\n".to_vec()))
                .await
        );
        assert_eq!(ssh.tmux_sessions().await, vec![name.clone()]);
        assert!(queries::persistent_shells::get(pool, &id)
            .await
            .unwrap()
            .is_some());

        // "Restart": the session is released, the socket closed so the client reconnects, and
        // tmux keeps the shell
        h.state.sessions.close_all().await;
        let closed = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                match ws.next().await {
                    None | Some(Err(_)) | Some(Ok(WsMessage::Close(_))) => return true,
                    Some(Ok(WsMessage::Text(t))) if t.contains("\"closed\"") => return false,
                    _ => {}
                }
            }
        })
        .await
        .expect("the socket ends");
        assert!(closed, "the client is not told the shell ended");
        assert!({
            let sessions = h.state.sessions.clone();
            crate::test_support::app::eventually(|| async {
                sessions.list_for_user(1).await.is_empty()
            })
            .await
        });
        assert_eq!(ssh.tmux_sessions().await, vec![name.clone()]);
        let (_, list) = call(&h.app, "GET", "/api/v1/sessions", Some(&h.session), None).await;
        assert_eq!(list["sessions"][0]["id"], id.as_str());
        assert_eq!(list["sessions"][0]["waiting"], true);

        // The client reattaches: a new connection attached to the same tmux session, output
        // numbered after the 7 frames it had seen, and no startup command typed again
        let execs_before = ssh
            .events()
            .await
            .iter()
            .filter(|e| matches!(e, Event::Exec(_)))
            .count();
        let mut ws = socket().await;
        ws.send(WsMessage::text(
            json!({ "type": "attach", "session_id": id, "last_seq": 7, "cols": 80, "rows": 24 })
                .to_string(),
        ))
        .await
        .unwrap();
        let back = next(&mut ws, |v| v["type"] == "connected")
            .await
            .expect("reattached");
        assert_eq!(back["session_id"], id.as_str());
        let live = h.state.sessions.get(&id, 1).await.expect("live again");
        assert!(live.persistent);
        assert!(
            crate::test_support::app::eventually(|| async {
                ssh.events()
                    .await
                    .iter()
                    .filter(|e| matches!(e, Event::Exec(_)))
                    .count()
                    > execs_before
            })
            .await
        );
        let events = ssh.events().await;
        let execs: Vec<&String> = events
            .iter()
            .filter_map(|e| match e {
                Event::Exec(c) => Some(c),
                _ => None,
            })
            .collect();
        assert_eq!(execs.len(), execs_before + 1);
        assert!(
            execs.last().unwrap().contains("has-session"),
            "a reattach checks tmux"
        );
        assert_eq!(
            events
                .iter()
                .filter(|e| **e == Event::Input(b"cd /srv\n".to_vec()))
                .count(),
            1,
            "the startup command ran once"
        );
        assert!(
            crate::test_support::app::eventually(|| async { live.latest_seq().await > 7 }).await
        );
        assert_eq!(
            ssh.tmux_sessions().await,
            vec![name.clone()],
            "the same tmux session"
        );

        // Closing it ends the tmux session and forgets the row
        ws.send(WsMessage::text(json!({ "type": "close" }).to_string()))
            .await
            .unwrap();
        assert!(
            crate::test_support::app::eventually(|| async { ssh.tmux_sessions().await.is_empty() })
                .await
        );
        assert!(
            crate::test_support::app::eventually(|| async {
                queries::persistent_shells::get(pool, &id)
                    .await
                    .unwrap()
                    .is_none()
            })
            .await
        );

        // A kept shell nobody comes back for ends after the keep time: the expiry connects,
        // ends the tmux session and forgets the row
        let mut ws = socket().await;
        ws.send(WsMessage::text(
            json!({ "type": "shell", "server_id": server_id, "cols": 80, "rows": 24 }).to_string(),
        ))
        .await
        .unwrap();
        let connected = next(&mut ws, |v| v["type"] == "connected")
            .await
            .expect("a second shell");
        let second = connected["session_id"].as_str().unwrap().to_string();
        assert!(
            crate::test_support::app::eventually(|| async {
                ssh.tmux_sessions().await == vec![tmux_name(&second)]
            })
            .await
        );
        h.state.sessions.close_all().await;
        {
            let sessions = h.state.sessions.clone();
            assert!(
                crate::test_support::app::eventually(|| async {
                    sessions.list_for_user(1).await.is_empty()
                })
                .await
            );
        }
        h.state
            .persistent
            .wait(h.state.clone(), second.clone(), Duration::from_millis(50))
            .await;
        assert!(
            crate::test_support::app::eventually(|| async { ssh.tmux_sessions().await.is_empty() })
                .await
        );
        assert!(
            crate::test_support::app::eventually(|| async {
                queries::persistent_shells::get(pool, &second)
                    .await
                    .unwrap()
                    .is_none()
            })
            .await
        );
    }

    #[test]
    fn names_and_kill() {
        assert_eq!(tmux_name("abc"), "svrctlrs-abc");
        assert_eq!(
            kill_command("svrctlrs-abc"),
            "tmux kill-session -t =svrctlrs-abc"
        );
    }
}
