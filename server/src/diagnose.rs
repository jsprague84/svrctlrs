//! "Why can't I reach server X?" — the connection a shell would make, one step at a time,
//! from where svrctlrs runs (its container, its DNS, its address as the target sees it, the
//! stored credentials), with a verdict and a fix for the first step that fails.
//!
//! Per hop: `dns` → `tcp` (or, behind a jump host, `tunnel`: the jump host opens the TCP
//! connection) → `handshake` → `host_key` → `login`. Jump hosts are walked first, in order; a
//! failed hop skips the rest. The checks are the real ones (`ssh.rs`'s handler and login), so a
//! diagnosis can never pass where a shell would fail. Like the connection test, a first
//! contact records the presented key as unconfirmed, which is what the fix then asks you to
//! confirm.

use std::net::{IpAddr, SocketAddr};
use std::time::{Duration, Instant};

use russh::client;
use svrctlrs_api::ops::{Diagnosis, DiagnosisHop, DiagnosisStep};
use svrctlrs_database::models::{Credential, Server};
use svrctlrs_database::queries;
use svrctlrs_database::sqlx::{Pool, Sqlite};
use tokio::io::{AsyncRead, AsyncWrite};

use crate::ssh::{self, AuthPlan, ConnectError, Connection, Handler};

const DNS_TIMEOUT: Duration = Duration::from_secs(4);
const TCP_TIMEOUT: Duration = Duration::from_secs(5);
const TUNNEL_TIMEOUT: Duration = Duration::from_secs(8);
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(8);
const LOGIN_TIMEOUT: Duration = Duration::from_secs(10);

/// Where SSH comes from, for the fixes that depend on it.
const ORIGIN: &str = "Connections come from the machine svrctlrs runs on (Docker NATs the \
                      container to that machine's address), or from the jump host when one is set";

/// Diagnose the connection to `server`.
pub async fn diagnose(pool: &Pool<Sqlite>, server: &Server) -> Diagnosis {
    let started = Instant::now();
    let mut hops = Vec::new();
    let conn = walk(pool, server, &mut Vec::new(), &mut hops).await;
    let ok = conn.is_some();
    if let Some(conn) = conn {
        let _ = conn
            .handle
            .disconnect(russh::Disconnect::ByApplication, "", "English")
            .await;
    }
    let verdict = verdict(server, &hops, ok);
    Diagnosis {
        server_id: server.id,
        server_name: server.name.clone(),
        ok,
        verdict,
        hops,
        duration_ms: started.elapsed().as_millis() as u64,
    }
}

/// The first failure in plain words, or that all is well.
fn verdict(server: &Server, hops: &[DiagnosisHop], ok: bool) -> String {
    if ok {
        let last = hops.last();
        let login = last
            .and_then(|h| h.steps.iter().find(|s| s.step == "login"))
            .map(|s| s.detail.clone())
            .unwrap_or_default();
        let through = if hops.len() > 1 {
            format!(
                " through {}",
                hops[..hops.len() - 1]
                    .iter()
                    .map(|h| h.server_name.as_str())
                    .collect::<Vec<_>>()
                    .join(" → ")
            )
        } else {
            String::new()
        };
        return format!("{} is reachable{through}: {login}", server.name);
    }
    for hop in hops {
        if let Some(step) = hop.steps.iter().find(|s| s.status == "failed") {
            let what = match step.step.as_str() {
                "dns" => "its name does not resolve",
                "tcp" => "nothing answers on its SSH port",
                "tunnel" => "the jump host cannot reach it",
                "handshake" => "the SSH handshake fails",
                "host_key" => "its host key is not trusted",
                "login" => "the login is refused",
                "jump_host" => "its jump host is not configured right",
                _ => "it is not configured right",
            };
            let whose = if hop.server_id == server.id {
                server.name.clone()
            } else {
                format!("{} (jump host of {})", hop.server_name, server.name)
            };
            return format!("{whose} cannot be reached: {what}. {}", step.detail);
        }
    }
    format!("{} cannot be reached", server.name)
}

fn step(
    name: &str,
    status: &str,
    detail: impl Into<String>,
    fix: Option<String>,
    started: Instant,
) -> DiagnosisStep {
    DiagnosisStep {
        step: name.to_string(),
        status: status.to_string(),
        detail: detail.into(),
        fix,
        duration_ms: started.elapsed().as_millis() as u64,
    }
}

fn skipped(name: &str, why: &str) -> DiagnosisStep {
    step(name, "skipped", why, None, Instant::now())
}

async fn credential_of(pool: &Pool<Sqlite>, server: &Server) -> Result<Option<Credential>, String> {
    match server.credential_id {
        None => Ok(None),
        Some(id) => queries::credentials::get_credential(pool, id)
            .await
            .map(Some)
            .map_err(|e| format!("Its credential {id} cannot be read: {e}")),
    }
}

/// Diagnose `server` (its jump hosts first) into `hops`; its authenticated connection when
/// every step passed.
fn walk<'a>(
    pool: &'a Pool<Sqlite>,
    server: &'a Server,
    visited: &'a mut Vec<i64>,
    hops: &'a mut Vec<DiagnosisHop>,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Option<Connection>> + Send + 'a>> {
    Box::pin(async move {
        let mut hop = DiagnosisHop {
            server_id: server.id,
            server_name: server.name.clone(),
            address: format!(
                "{}@{}:{}",
                server.username.as_deref().unwrap_or("root"),
                server.hostname,
                server.port
            ),
            via_server_id: server.via_server_id,
            steps: Vec::new(),
        };
        let started = Instant::now();
        let credential = match credential_of(pool, server).await {
            Ok(c) => c,
            Err(e) => {
                hop.steps.push(step(
                    "config",
                    "failed",
                    e,
                    Some("Assign a working credential to the server".into()),
                    started,
                ));
                hops.push(hop);
                return None;
            }
        };
        let target = match ssh::target(server, credential.as_ref()) {
            Ok(t) => t,
            Err(e) => {
                hop.steps.push(step(
                    "config",
                    "failed",
                    e,
                    Some("Fix the server's hostname and port".into()),
                    started,
                ));
                hops.push(hop);
                return None;
            }
        };
        hop.address = target.login();

        // Through a jump host: that hop first, then a tunnel instead of DNS and TCP
        let via = match server.via_server_id {
            None => None,
            Some(via_id) => {
                let started = Instant::now();
                if visited.contains(&via_id)
                    || via_id == server.id
                    || visited.len() >= ssh::MAX_JUMPS
                {
                    hop.steps.push(step(
                        "jump_host",
                        "failed",
                        format!(
                            "The jump hosts of {} form a loop or a chain longer than {}",
                            server.name,
                            ssh::MAX_JUMPS
                        ),
                        Some(
                            "Point the server at a jump host that does not lead back to it".into(),
                        ),
                        started,
                    ));
                    hops.push(hop);
                    return None;
                }
                let jump = match queries::servers::get_server(pool, via_id).await {
                    Ok(j) => j,
                    Err(e) => {
                        hop.steps.push(step(
                            "jump_host",
                            "failed",
                            format!("Its jump host {via_id} cannot be read: {e}"),
                            Some("Pick an existing jump host, or none".into()),
                            started,
                        ));
                        hops.push(hop);
                        return None;
                    }
                };
                visited.push(server.id);
                match walk(pool, &jump, visited, hops).await {
                    Some(conn) => Some((jump, conn)),
                    None => {
                        hop.steps.push(skipped(
                            "jump_host",
                            &format!("Its jump host {} failed above; fix that first", jump.name),
                        ));
                        hops.push(hop);
                        return None;
                    }
                }
            }
        };

        let (handle, notes, keep) = match via {
            None => {
                let Some(stream) = dns_and_tcp(&target, &mut hop.steps).await else {
                    for name in ["handshake", "host_key", "login"] {
                        hop.steps
                            .push(skipped(name, "No connection to shake hands over"));
                    }
                    hops.push(hop);
                    return None;
                };
                let (handler, notes) = Handler::new(pool, server);
                let handle = handshake(stream, handler, &mut hop.steps, &target).await;
                (handle, notes, None)
            }
            Some((jump, jump_conn)) => {
                let Some(channel) = tunnel(&jump, &jump_conn, &target, &mut hop.steps).await else {
                    for name in ["handshake", "host_key", "login"] {
                        hop.steps
                            .push(skipped(name, "No connection to shake hands over"));
                    }
                    hops.push(hop);
                    return None;
                };
                let (handler, notes) = Handler::new(pool, server);
                let handle =
                    handshake(channel.into_stream(), handler, &mut hop.steps, &target).await;
                // The jump host's connection must outlive the tunnel
                (handle, notes, Some(jump_conn))
            }
        };

        let result = match handle {
            Some(h) => {
                host_key_and_login(
                    server,
                    credential.as_ref(),
                    &target,
                    h,
                    &notes,
                    keep,
                    &mut hop.steps,
                )
                .await
            }
            None => {
                // A refused host key fails the handshake too; say which it was
                host_key_refused(server, &notes, &mut hop.steps).await;
                None
            }
        };
        hops.push(hop);
        result
    })
}

/// Ask the jump host to open a TCP connection to the target, recording the step.
async fn tunnel(
    jump: &Server,
    jump_conn: &Connection,
    target: &ssh::Target,
    steps: &mut Vec<DiagnosisStep>,
) -> Option<russh::Channel<client::Msg>> {
    let started = Instant::now();
    let opened = tokio::time::timeout(
        TUNNEL_TIMEOUT,
        jump_conn.handle.channel_open_direct_tcpip(
            target.hostname.as_str(),
            u32::from(target.port),
            "127.0.0.1",
            0,
        ),
    )
    .await;
    match opened {
        Ok(Ok(channel)) => {
            steps.push(step(
                "tunnel",
                "ok",
                format!(
                    "{} opened a connection to {}:{}",
                    jump.name, target.hostname, target.port
                ),
                None,
                started,
            ));
            Some(channel)
        }
        failure => {
            let why = match failure {
                Ok(Err(e)) => e.to_string(),
                _ => format!("no answer within {} s", TUNNEL_TIMEOUT.as_secs()),
            };
            steps.push(step(
                "tunnel",
                "failed",
                format!(
                    "{} could not open {}:{}: {why}",
                    jump.name, target.hostname, target.port
                ),
                Some(format!(
                    "Check from {} that the name resolves and the port answers (`nc -vz {} {}` \
                     there), and that its sshd allows TCP forwarding (AllowTcpForwarding)",
                    jump.name, target.hostname, target.port
                )),
                started,
            ));
            None
        }
    }
}

/// Resolve the name and open a TCP connection, recording both steps.
async fn dns_and_tcp(
    target: &ssh::Target,
    steps: &mut Vec<DiagnosisStep>,
) -> Option<tokio::net::TcpStream> {
    let started = Instant::now();
    let addrs: Vec<SocketAddr> = if let Ok(ip) = target.hostname.parse::<IpAddr>() {
        steps.push(step(
            "dns",
            "ok",
            format!("{ip} is an address; nothing to look up"),
            None,
            started,
        ));
        vec![SocketAddr::new(ip, target.port)]
    } else {
        let looked_up = tokio::time::timeout(
            DNS_TIMEOUT,
            tokio::net::lookup_host((target.hostname.as_str(), target.port)),
        )
        .await;
        match looked_up {
            Ok(Ok(found)) => {
                let found: Vec<SocketAddr> = found.collect();
                if found.is_empty() {
                    steps.push(dns_failure(target, "no addresses", started));
                    steps.push(skipped("tcp", "No address to connect to"));
                    return None;
                }
                steps.push(step(
                    "dns",
                    "ok",
                    format!(
                        "{} resolves to {}",
                        target.hostname,
                        found
                            .iter()
                            .map(|a| a.ip().to_string())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                    None,
                    started,
                ));
                found
            }
            Ok(Err(e)) => {
                steps.push(dns_failure(target, &e.to_string(), started));
                steps.push(skipped("tcp", "No address to connect to"));
                return None;
            }
            Err(_) => {
                steps.push(dns_failure(
                    target,
                    &format!("no answer within {} s", DNS_TIMEOUT.as_secs()),
                    started,
                ));
                steps.push(skipped("tcp", "No address to connect to"));
                return None;
            }
        }
    };

    let started = Instant::now();
    let mut last = String::new();
    let mut kind = std::io::ErrorKind::Other;
    for addr in addrs.iter().take(3) {
        match tokio::time::timeout(TCP_TIMEOUT, tokio::net::TcpStream::connect(addr)).await {
            Ok(Ok(stream)) => {
                let _ = stream.set_nodelay(true);
                steps.push(step(
                    "tcp",
                    "ok",
                    format!("Connected to {addr}"),
                    None,
                    started,
                ));
                return Some(stream);
            }
            Ok(Err(e)) => {
                kind = e.kind();
                last = format!("{addr}: {e}");
            }
            Err(_) => {
                kind = std::io::ErrorKind::TimedOut;
                last = format!("{addr}: no answer within {} s", TCP_TIMEOUT.as_secs());
            }
        }
    }
    let fix = match kind {
        std::io::ErrorKind::ConnectionRefused => format!(
            "The host answers but nothing listens on port {}: is sshd running there \
             (`systemctl status ssh`), and is {} the right port?",
            target.port, target.port
        ),
        std::io::ErrorKind::TimedOut => format!(
            "A firewall drops the connection, the address is wrong, or the host is down. {ORIGIN}; \
             allow SSH from there."
        ),
        _ => format!(
            "svrctlrs's machine has no route to {} (another network, a VPN that is down?). {ORIGIN}.",
            target.hostname
        ),
    };
    steps.push(step(
        "tcp",
        "failed",
        format!("Could not connect to port {}: {last}", target.port),
        Some(fix),
        started,
    ));
    None
}

fn dns_failure(target: &ssh::Target, why: &str, started: Instant) -> DiagnosisStep {
    step(
        "dns",
        "failed",
        format!("{} does not resolve: {why}", target.hostname),
        Some(format!(
            "Check the spelling. svrctlrs resolves names with its container's DNS (Docker's, \
             which asks the host's resolver), which may not know names only your laptop knows; \
             use an IP address or add {} to that DNS.",
            target.hostname
        )),
        started,
    )
}

/// The SSH handshake over `stream`. A host key the handler refuses fails the handshake too;
/// the caller tells that apart through the handler's notes.
async fn handshake<S>(
    stream: S,
    handler: Handler,
    steps: &mut Vec<DiagnosisStep>,
    target: &ssh::Target,
) -> Option<client::Handle<Handler>>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let started = Instant::now();
    match tokio::time::timeout(
        HANDSHAKE_TIMEOUT,
        client::connect_stream(ssh::client_config(), stream, handler),
    )
    .await
    {
        Ok(Ok(handle)) => {
            steps.push(step(
                "handshake",
                "ok",
                "SSH protocol and encryption agreed",
                None,
                started,
            ));
            Some(handle)
        }
        Ok(Err(e)) => {
            // Recorded by host_key_refused when the handler refused the key
            steps.push(step(
                "handshake",
                "failed",
                format!("The SSH handshake with {} failed: {e}", target.hostname),
                Some(format!(
                    "Something answers on port {} but the handshake did not finish: not an SSH \
                     server, no algorithm in common, or the connection was cut (fail2ban, \
                     MaxStartups).",
                    target.port
                )),
                started,
            ));
            None
        }
        Err(_) => {
            steps.push(step(
                "handshake",
                "failed",
                format!(
                    "No SSH greeting from {} within {} s",
                    target.hostname,
                    HANDSHAKE_TIMEOUT.as_secs()
                ),
                Some(format!(
                    "Port {} accepts connections but does not speak SSH, or the server is \
                     overloaded.",
                    target.port
                )),
                started,
            ));
            None
        }
    }
}

/// After a failed handshake: if it was the host key, replace the handshake failure with the
/// real reason and its fix.
async fn host_key_refused(
    server: &Server,
    notes: &ssh::HandshakeNotes,
    steps: &mut Vec<DiagnosisStep>,
) {
    let Some(refusal) = notes.refusal.lock().await.take() else {
        steps.push(skipped("host_key", "The handshake did not get that far"));
        steps.push(skipped("login", "The handshake did not get that far"));
        return;
    };
    if let Some(last) = steps.last_mut() {
        if last.step == "handshake" {
            last.status = "ok".into();
            last.detail = "SSH protocol and encryption agreed".into();
            last.fix = None;
        }
    }
    let started = Instant::now();
    let (detail, fix) =
        match &refusal {
            ConnectError::HostKeyUnconfirmed {
                key_type,
                fingerprint,
                ..
            } => (
                format!("First contact: {key_type} {fingerprint} waits for confirmation"),
                format!(
                "Compare it with `ssh-keygen -lf /etc/ssh/ssh_host_{}_key.pub` on the host (over \
                 a path you trust), then confirm it: Keys on the server's row in the app, or \
                 POST /api/v1/servers/{}/host-keys/trust {{\"fingerprint\": \"{fingerprint}\"}}",
                key_type.trim_start_matches("ssh-").trim_start_matches("ecdsa-sha2-"),
                server.id
            ),
            ),
            ConnectError::HostKeyMismatch(msg) => (
                msg.clone(),
                format!(
                "If the host was reinstalled or its keys regenerated, check the new key out of \
                 band, forget the stored ones (Keys → Forget all keys in the app, or \
                 DELETE /api/v1/servers/{}/host-keys) and confirm the new one. If not, something \
                 may be impersonating the server: do not connect.",
                server.id
            ),
            ),
            ConnectError::Failed(msg) => (msg.clone(), "Try again; see the server log".to_string()),
        };
    steps.push(step("host_key", "failed", detail, Some(fix), started));
    steps.push(skipped("login", "The host key is not trusted"));
}

/// The host key passed (or the handshake would not have finished); now log in.
async fn host_key_and_login(
    server: &Server,
    credential: Option<&Credential>,
    target: &ssh::Target,
    mut handle: client::Handle<Handler>,
    notes: &ssh::HandshakeNotes,
    via: Option<Connection>,
    steps: &mut Vec<DiagnosisStep>,
) -> Option<Connection> {
    let presented = notes.presented.lock().await.clone();
    steps.push(step(
        "host_key",
        "ok",
        match presented {
            Some((kind, fp)) => format!("{kind} {fp} is a trusted key of {}", server.name),
            None => "The presented key is trusted".to_string(),
        },
        None,
        Instant::now(),
    ));

    let started = Instant::now();
    let plan = match ssh::plan_auth(credential).await {
        Ok(p) => p,
        Err(e) => {
            steps.push(step(
                "login",
                "failed",
                e,
                Some(format!(
                    "Assign a credential to {} (Credentials, then the server's settings)",
                    server.name
                )),
                started,
            ));
            return None;
        }
    };
    let by_password = matches!(plan, AuthPlan::Password(_));
    match tokio::time::timeout(LOGIN_TIMEOUT, ssh::authenticate(&mut handle, target, plan)).await {
        Ok(Ok(label)) => {
            steps.push(step(
                "login",
                "ok",
                format!("Logged in as {} with {label}", target.username),
                None,
                started,
            ));
            Some(Connection::assemble(handle, target.clone(), label, via))
        }
        Ok(Err(failure)) => {
            let offers = failure.remaining_methods.join(", ");
            let fix = if by_password
                && !failure.remaining_methods.iter().any(|m| m == "password")
                && !failure.remaining_methods.is_empty()
            {
                format!(
                    "The server does not take passwords (it offers: {offers}). Use a key: \
                     generate one and authorize it (Authorize key in the app, or \
                     POST /api/v1/servers/{}/authorize-key) while another way in works.",
                    server.id
                )
            } else if by_password {
                format!(
                    "The stored password is wrong for {}, or the account is locked or expired; \
                     update the credential.",
                    target.username
                )
            } else if !failure.remaining_methods.is_empty()
                && !failure.remaining_methods.iter().any(|m| m == "publickey")
            {
                format!(
                    "The server does not take keys (it offers: {offers}); use a password \
                     credential or enable PubkeyAuthentication."
                )
            } else {
                format!(
                    "The key is not in ~{}/.ssh/authorized_keys on the host (or the user is \
                     wrong). Install it with a way in that works (a password, or another \
                     credential): Authorize key in the app, or POST /api/v1/servers/{}/authorize-key.",
                    target.username, server.id
                )
            };
            steps.push(step("login", "failed", failure.message, Some(fix), started));
            None
        }
        Err(_) => {
            steps.push(step(
                "login",
                "failed",
                format!(
                    "No answer to the login within {} s",
                    LOGIN_TIMEOUT.as_secs()
                ),
                Some(
                    "The server is slow to authenticate (PAM, DNS lookups in sshd: UseDNS no)"
                        .into(),
                ),
                started,
            ));
            None
        }
    }
}
