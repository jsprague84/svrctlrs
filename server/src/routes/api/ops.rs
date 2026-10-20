//! Task-level operations on servers, run by svrctlrs itself, for agents and the UI:
//! `diagnose` (why can or cannot it be reached), `exec` (one command), `onboard` (add,
//! confirm, switch to a generated key) and `rotate-key`. Private keys and passwords stay in
//! svrctlrs: requests name stored credentials, never carry one.
//!
//! Full paths, merged beside the API router outside its 30 s timeout: a diagnosis through
//! two jump hosts, or a command given ten minutes, takes longer.

use axum::{
    extract::{Path, State},
    http::HeaderMap,
    Json,
};
use serde_json::json;
use svrctlrs_api::ops::{
    Diagnosis, ExecRequest, ExecResult, OnboardRequest, OnboardResult, OpStep, RotateKeyRequest,
    RotateKeyResult,
};
use svrctlrs_api::ErrorBody;
use svrctlrs_database::models::{CreateServer, NewAuditEntry};
use svrctlrs_database::queries::{credentials, server_host_keys, servers};
use tracing::{info, instrument, warn};

use super::ApiError;
use crate::routes::ui::auth::{client_ip, CurrentUser};
use crate::ssh::ConnectError;
use crate::state::AppState;
use utoipa_axum::{router::OpenApiRouter, routes};

/// Longest a command may be given
const MAX_EXEC_SECS: u64 = 600;
const DEFAULT_EXEC_SECS: u64 = 60;
/// Output kept per stream
const EXEC_CAP: usize = 1024 * 1024;

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(diagnose))
        .routes(routes!(exec))
        .routes(routes!(onboard))
        .routes(routes!(rotate_key))
}

fn op(step: &str, status: &str, detail: impl Into<String>) -> OpStep {
    OpStep {
        step: step.to_string(),
        status: status.to_string(),
        detail: detail.into(),
    }
}

/// Why svrctlrs can or cannot reach the server: DNS, TCP (or the jump host's tunnel), the SSH
/// handshake, the host key and the login, per hop, with a fix for the first failure. Runs from
/// where svrctlrs runs. A first contact records the presented key as unconfirmed.
#[utoipa::path(post, path = "/api/v1/servers/{id}/diagnose", tag = "operations",
    params(("id" = i64, Path)),
    responses((status = 200, body = Diagnosis), (status = 404, body = ErrorBody)))]
#[instrument(skip(state, headers))]
async fn diagnose(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<Diagnosis>, ApiError> {
    let server = servers::get_server(&state.pool, id).await?;
    let report = crate::diagnose::diagnose(&state.pool, &server).await;
    // What the sidebar dot and the Servers page show, as after a connection test
    let recorded = if report.ok {
        servers::update_server_status(&state.pool, id, None).await
    } else {
        servers::update_server_last_error(&state.pool, id, &report.verdict).await
    };
    if let Err(e) = recorded {
        warn!(id, error = %e, "Failed to record the diagnosis");
    }
    info!(id, ok = report.ok, verdict = %report.verdict, "Server diagnosed");
    crate::audit::record(
        &state.pool,
        NewAuditEntry::new("server.diagnosed")
            .user(me)
            .client_ip(Some(client_ip(&headers)))
            .target("server", id, &server.name)
            .details(json!({ "ok": report.ok, "verdict": report.verdict })),
    );
    Ok(Json(report))
}

/// Run one command on the server through its stored credential, without a terminal, and
/// return its output and exit status. Audited with the command as typed: keep secrets out of
/// command lines.
#[utoipa::path(post, path = "/api/v1/servers/{id}/exec", tag = "operations",
    params(("id" = i64, Path)), request_body = ExecRequest,
    responses(
        (status = 200, body = ExecResult),
        (status = 400, description = "Bad request, or the connection failed", body = ErrorBody),
        (status = 404, body = ErrorBody),
        (status = 409, description = "Host key mismatch", body = ErrorBody),
        (status = 428, description = "Host key not confirmed yet", body = ErrorBody),
    ))]
#[instrument(skip(state, headers, input), fields(timeout = input.timeout_secs))]
async fn exec(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<ExecRequest>,
) -> Result<Json<ExecResult>, ApiError> {
    let command = input.command.trim();
    if command.is_empty() {
        return Err(ApiError::bad_request("command is required"));
    }
    let timeout = input.timeout_secs.unwrap_or(DEFAULT_EXEC_SECS);
    if timeout == 0 || timeout > MAX_EXEC_SECS {
        return Err(ApiError::bad_request(format!(
            "timeout_secs must be 1–{MAX_EXEC_SECS}"
        )));
    }
    let server = servers::get_server(&state.pool, id).await?;
    let credential = match server.credential_id {
        Some(cid) => Some(credentials::get_credential(&state.pool, cid).await?),
        None => None,
    };
    let started = std::time::Instant::now();
    let out = crate::ssh::run_bounded(
        &state.pool,
        &server,
        credential.as_ref(),
        command,
        std::time::Duration::from_secs(timeout),
        EXEC_CAP,
    )
    .await?;
    let duration_ms = started.elapsed().as_millis() as u64;
    info!(id, exit = ?out.exit_code, timed_out = out.timed_out, duration_ms, "Command run");
    crate::audit::record(
        &state.pool,
        NewAuditEntry::new("server.exec")
            .user(me)
            .client_ip(Some(client_ip(&headers)))
            .target("server", id, &server.name)
            .details(json!({
                "command": command.chars().take(500).collect::<String>(),
                "exit_code": out.exit_code,
                "timed_out": out.timed_out,
                "duration_ms": duration_ms,
            })),
    );
    Ok(Json(ExecResult {
        server_id: id,
        exit_code: out.exit_code,
        stdout: out.stdout,
        stderr: out.stderr,
        stdout_truncated: out.stdout_truncated,
        stderr_truncated: out.stderr_truncated,
        timed_out: out.timed_out,
        duration_ms,
    }))
}

/// Add a server and get it to a trusted key: create it with a stored credential as the way
/// in, confirm its host key (only with the fingerprint you compared on the host), check the
/// login, then generate a key here, install it with the way in, prove it and switch to it.
/// Stops at the first step that needs you, saying what to do; run it again to continue (a
/// server of the same name and hostname is picked up). `dry_run` changes nothing.
#[utoipa::path(post, path = "/api/v1/servers/onboard", tag = "operations",
    request_body = OnboardRequest,
    responses(
        (status = 200, description = "How far it got; `next` says what to do when not `done`", body = OnboardResult),
        (status = 400, body = ErrorBody),
        (status = 409, description = "A server of that name has another hostname", body = ErrorBody),
    ))]
#[instrument(skip(state, headers, input), fields(name = %input.name))]
async fn onboard(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
    headers: HeaderMap,
    Json(input): Json<OnboardRequest>,
) -> Result<Json<OnboardResult>, ApiError> {
    let pool = &state.pool;
    let name = input.name.trim().to_string();
    let hostname = input.hostname.trim().to_string();
    if name.is_empty() || hostname.is_empty() {
        return Err(ApiError::bad_request("name and hostname are required"));
    }
    let way_in = credentials::get_credential(pool, input.credential_id)
        .await
        .map_err(|e| match e {
            svrctlrs_core::Error::NotFound(_) => {
                ApiError::bad_request(format!("Credential {} does not exist", input.credential_id))
            }
            other => other.into(),
        })?;
    if let Some(via) = input.via_server_id {
        super::servers::check_jump_host(&state, None, via).await?;
    }
    let existing = servers::list_servers(pool)
        .await?
        .into_iter()
        .find(|s| s.name == name);
    if let Some(s) = &existing {
        if !s.hostname.eq_ignore_ascii_case(&hostname) {
            return Err(ApiError::conflict(format!(
                "A server named {name} exists with hostname {}; pick another name",
                s.hostname
            )));
        }
    }
    let new_key = input.new_key.unwrap_or(true);
    let mut steps = Vec::new();

    if input.dry_run {
        steps.push(op(
            "create",
            "planned",
            match &existing {
                Some(s) => format!("Continue with server {} (id {})", s.name, s.id),
                None => format!(
                    "Create {name} ({hostname}), logging in with {}",
                    way_in.name
                ),
            },
        ));
        steps.push(op(
            "host_key",
            "planned",
            match &input.fingerprint {
                Some(f) => format!("Trust the host key if it is {f}"),
                None => "Record the host key and stop for you to compare it".to_string(),
            },
        ));
        steps.push(op(
            "login",
            "planned",
            format!("Log in with {}", way_in.name),
        ));
        steps.push(op(
            "key",
            if new_key { "planned" } else { "skipped" },
            if new_key {
                format!(
                    "Generate \"{name} key\", install it with {}, prove it, switch to it",
                    way_in.name
                )
            } else {
                format!("Keep {}", way_in.name)
            },
        ));
        return Ok(Json(OnboardResult {
            server_id: existing.map(|s| s.id),
            done: false,
            dry_run: true,
            steps,
            next: Some("Run it again without dry_run".to_string()),
        }));
    }

    let server = match existing {
        Some(s) => {
            steps.push(op(
                "create",
                "ok",
                format!("Continuing with {} (id {})", s.name, s.id),
            ));
            s
        }
        None => {
            let id = servers::create_server(
                pool,
                &CreateServer {
                    name: name.clone(),
                    hostname: hostname.clone(),
                    port: input.port.unwrap_or(22),
                    username: input.username.clone(),
                    credential_id: Some(way_in.id),
                    via_server_id: input.via_server_id,
                    tags: input.tags.clone(),
                    startup_command: None,
                    record_sessions: false,
                    description: None,
                    enabled: true,
                },
            )
            .await?;
            crate::audit::record(
                pool,
                NewAuditEntry::new("server.created")
                    .user(me)
                    .client_ip(Some(client_ip(&headers)))
                    .target("server", id, &name)
                    .details(json!({ "via": "onboard" })),
            );
            steps.push(op("create", "ok", format!("Created {name} (id {id})")));
            servers::get_server(pool, id).await?
        }
    };
    let id = server.id;
    let stop = |steps: Vec<OpStep>, next: String| {
        Ok(Json(OnboardResult {
            server_id: Some(id),
            done: false,
            dry_run: false,
            steps,
            next: Some(next),
        }))
    };

    // Host key: confirmed only with the fingerprint the caller compared out of band
    let mut connected = crate::ssh::connect(pool, &server, Some(&way_in)).await;
    if let Err(ConnectError::HostKeyUnconfirmed {
        server_id,
        server_name,
        key_type,
        fingerprint,
    }) = &connected
    {
        if *server_id != id {
            steps.push(op(
                "host_key",
                "waiting",
                format!("The jump host {server_name}'s key {key_type} {fingerprint} waits for confirmation"),
            ));
            return stop(
                steps,
                format!("Confirm the jump host's key first (POST /api/v1/servers/{server_id}/host-keys/trust)"),
            );
        }
        match input.fingerprint.as_deref().map(str::trim) {
            None => {
                steps.push(op(
                    "host_key",
                    "waiting",
                    format!("First contact: {key_type} {fingerprint}"),
                ));
                return stop(
                    steps,
                    format!(
                        "Compare {fingerprint} with `ssh-keygen -lf /etc/ssh/ssh_host_*_key.pub` \
                         on the host over a path you trust, then run onboard again with \
                         \"fingerprint\": \"{fingerprint}\""
                    ),
                );
            }
            Some(given) if given != fingerprint => {
                steps.push(op(
                    "host_key",
                    "failed",
                    format!("The server presents {key_type} {fingerprint}, not {given}"),
                ));
                return stop(
                    steps,
                    "Do not trust it until you know why the keys differ (a different host, or \
                     a man in the middle)"
                        .to_string(),
                );
            }
            Some(_) => {
                let waiting = server_host_keys::list_host_keys_for_server(pool, id)
                    .await?
                    .into_iter()
                    .find(|k| {
                        !k.is_trusted()
                            && crate::host_keys::fingerprint(&k.public_key) == *fingerprint
                    })
                    .ok_or_else(|| ApiError::internal_error("The recorded key went missing"))?;
                server_host_keys::trust_host_key(pool, id, waiting.id).await?;
                crate::audit::record(
                    pool,
                    NewAuditEntry::new("server.host_key_trusted")
                        .user(me)
                        .client_ip(Some(client_ip(&headers)))
                        .target("server", id, &server.name)
                        .details(json!({ "key_type": key_type, "fingerprint": fingerprint, "via": "onboard" })),
                );
                steps.push(op(
                    "host_key",
                    "ok",
                    format!("Confirmed {key_type} {fingerprint}"),
                ));
                connected = crate::ssh::connect(pool, &server, Some(&way_in)).await;
            }
        }
    } else if connected.is_ok() {
        steps.push(op("host_key", "ok", "The host key is trusted"));
    }

    match connected {
        Ok(conn) => {
            steps.push(op(
                "login",
                "ok",
                format!("Logged in as {} with {}", conn.target.username, conn.auth),
            ));
            let _ = conn
                .handle
                .disconnect(russh::Disconnect::ByApplication, "", "English")
                .await;
        }
        Err(ConnectError::HostKeyMismatch(msg)) => {
            steps.push(op("host_key", "failed", msg));
            return stop(
                steps,
                format!("Find out why the key changed before forgetting the stored keys (DELETE /api/v1/servers/{id}/host-keys)"),
            );
        }
        Err(e) => {
            steps.push(op("login", "failed", e.to_string()));
            return stop(
                steps,
                format!("Find out why with POST /api/v1/servers/{id}/diagnose, fix it, and run onboard again"),
            );
        }
    }

    if !new_key {
        steps.push(op(
            "key",
            "skipped",
            format!("Keeps logging in with {}", way_in.name),
        ));
    } else {
        let key = crate::key_ops::generate_credential(pool, &format!("{name} key")).await?;
        match crate::key_ops::install_and_switch(pool, &server, &key, &way_in).await {
            Ok(()) => {
                crate::audit::record(
                    pool,
                    NewAuditEntry::new("server.key_authorized")
                        .user(me)
                        .client_ip(Some(client_ip(&headers)))
                        .target("server", id, &server.name)
                        .details(json!({ "credential_id": key.id, "credential": key.name, "via": "onboard" })),
                );
                steps.push(op(
                    "key",
                    "ok",
                    format!(
                        "Generated {}, installed and proved it; {name} now uses it",
                        key.name
                    ),
                ));
            }
            Err(e) => {
                // Do not leave an unused key behind
                let _ = credentials::delete_credential(pool, key.id).await;
                steps.push(op("key", "failed", e.message));
                return stop(
                    steps,
                    format!("The server still logs in with {}; fix the key install and run onboard again", way_in.name),
                );
            }
        }
    }

    crate::audit::record(
        pool,
        NewAuditEntry::new("server.onboarded")
            .user(me)
            .client_ip(Some(client_ip(&headers)))
            .target("server", id, &server.name)
            .details(json!({ "new_key": new_key })),
    );
    Ok(Json(OnboardResult {
        server_id: Some(id),
        done: true,
        dry_run: false,
        steps,
        next: None,
    }))
}

/// Replace the server's key: check the current one logs in, generate a new key here, install
/// it with the current one, prove it, switch, and remove the old public key from the server's
/// `authorized_keys`. The old credential is kept (other servers may use it). `dry_run`
/// changes nothing.
#[utoipa::path(post, path = "/api/v1/servers/{id}/rotate-key", tag = "operations",
    params(("id" = i64, Path)), request_body = RotateKeyRequest,
    responses(
        (status = 200, description = "How far it got; `next` says what to do", body = RotateKeyResult),
        (status = 400, description = "The server does not log in with a stored key", body = ErrorBody),
        (status = 404, body = ErrorBody),
    ))]
#[instrument(skip(state, headers, input))]
async fn rotate_key(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<RotateKeyRequest>,
) -> Result<Json<RotateKeyResult>, ApiError> {
    let pool = &state.pool;
    let server = servers::get_server(pool, id).await?;
    let Some(old_id) = server.credential_id else {
        return Err(ApiError::bad_request(
            "The server has no credential to rotate; give it a key with onboard or authorize-key",
        ));
    };
    let old = credentials::get_credential(pool, old_id).await?;
    if crate::key_ops::public_half(&old).is_err() {
        return Err(ApiError::bad_request(format!(
            "{} logs in with {}, which is not a stored key; switch it to a key with \
             POST /api/v1/servers/{id}/authorize-key instead",
            server.name, old.name
        )));
    }
    let remove_old = input.remove_old.unwrap_or(true);
    let others = servers::list_servers(pool)
        .await?
        .into_iter()
        .filter(|s| s.id != id && s.credential_id == Some(old_id))
        .count();
    let today = chrono::Utc::now().format("%Y-%m-%d");
    let new_name = format!("{} key {today}", server.name);
    let mut steps = Vec::new();
    let finish = |steps: Vec<OpStep>,
                  done: bool,
                  new_id: Option<i64>,
                  next: Option<String>,
                  dry_run: bool| {
        Ok(Json(RotateKeyResult {
            server_id: id,
            done,
            dry_run,
            old_credential_id: Some(old_id),
            new_credential_id: new_id,
            steps,
            next,
        }))
    };

    if input.dry_run {
        steps.push(op(
            "check",
            "planned",
            format!("Check that {} logs in", old.name),
        ));
        steps.push(op("generate", "planned", format!("Generate {new_name}")));
        steps.push(op(
            "install",
            "planned",
            format!(
                "Install it with {}, prove it, switch {} to it",
                old.name, server.name
            ),
        ));
        steps.push(op(
            "remove_old",
            if remove_old { "planned" } else { "skipped" },
            format!("Remove {}'s public key from {}", old.name, server.name),
        ));
        return finish(
            steps,
            false,
            None,
            Some("Run it again without dry_run".into()),
            true,
        );
    }

    match crate::ssh::run(pool, &server, Some(&old), "echo svrctlrs-key-ok").await {
        Ok((_, _, 0)) => steps.push(op("check", "ok", format!("{} logs in", old.name))),
        Ok((_, stderr, code)) => {
            steps.push(op(
                "check",
                "failed",
                format!("Exit {code}: {}", stderr.trim()),
            ));
            return finish(
                steps,
                false,
                None,
                Some(format!(
                    "Find out why with POST /api/v1/servers/{id}/diagnose"
                )),
                false,
            );
        }
        Err(e) => {
            steps.push(op("check", "failed", e.to_string()));
            return finish(
                steps,
                false,
                None,
                Some(format!(
                    "Find out why with POST /api/v1/servers/{id}/diagnose"
                )),
                false,
            );
        }
    }

    let new = crate::key_ops::generate_credential(pool, &new_name).await?;
    steps.push(op("generate", "ok", format!("Generated {}", new.name)));
    if let Err(e) = crate::key_ops::install_and_switch(pool, &server, &new, &old).await {
        let _ = credentials::delete_credential(pool, new.id).await;
        steps.push(op("install", "failed", e.message));
        return finish(
            steps,
            false,
            None,
            Some(format!(
                "{} still uses {}; nothing changed",
                server.name, old.name
            )),
            false,
        );
    }
    steps.push(op(
        "install",
        "ok",
        format!(
            "Installed and proved {}; {} now uses it",
            new.name, server.name
        ),
    ));

    let mut next = if others == 0 {
        Some(format!(
            "{} is no longer used by any server; delete it when you are sure \
             (DELETE /api/v1/credentials/{old_id})",
            old.name
        ))
    } else {
        Some(format!(
            "{} still logs in to {others} other server(s); rotate those too",
            old.name
        ))
    };
    if remove_old {
        match crate::key_ops::remove(pool, &server, &old, &new).await {
            Ok(()) => steps.push(op(
                "remove_old",
                "ok",
                format!("Removed {}'s public key from {}", old.name, server.name),
            )),
            Err(e) => {
                steps.push(op("remove_old", "failed", e.message));
                next = Some(format!(
                    "The new key works, but {}'s public key is still in authorized_keys on {}; \
                     remove it by hand",
                    old.name, server.name
                ));
            }
        }
    } else {
        steps.push(op(
            "remove_old",
            "skipped",
            "The old public key stays on the server",
        ));
    }

    info!(id, old = old_id, new = new.id, "Key rotated");
    crate::audit::record(
        pool,
        NewAuditEntry::new("server.key_rotated")
            .user(me)
            .client_ip(Some(client_ip(&headers)))
            .target("server", id, &server.name)
            .details(json!({
                "old_credential_id": old_id,
                "new_credential_id": new.id,
                "old_removed": remove_old && steps.last().is_some_and(|s| s.status == "ok"),
            })),
    );
    let done = steps.iter().all(|s| s.status != "failed");
    finish(steps, done, Some(new.id), next, false)
}

/// The operations through the real application against the loopback SSH server.
#[cfg(test)]
mod tests {
    use crate::test_support::app::{eventually, AppHarness};
    use crate::test_support::ssh_server::{new_key, private_openssh, Options, TestSshServer};
    use crate::test_support::{add_credential, add_server};
    use axum::http::StatusCode;
    use serde_json::{json, Value};
    use svrctlrs_database::models::CredentialType;
    use svrctlrs_database::queries;

    fn step<'a>(d: &'a Value, hop: usize, name: &str) -> &'a Value {
        d["hops"][hop]["steps"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["step"] == name)
            .unwrap_or_else(|| panic!("no step {name} in {d}"))
    }

    /// A port nothing listens on
    async fn closed_port() -> u16 {
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        l.local_addr().unwrap().port()
    }

    #[tokio::test]
    async fn diagnose_walks_every_step_and_names_the_fix() {
        let h = AppHarness::new().await;
        let pool = h.state.pool.clone();
        let ssh = TestSshServer::start(Options::password("alice", "pw")).await;
        let pw = add_credential(&pool, "pw", CredentialType::Password, "pw").await;
        let bad = add_credential(&pool, "bad", CredentialType::Password, "nope").await;
        let sid = add_server(&pool, "loop", ssh.port, "alice", Some(pw), None).await;
        let uri = format!("/api/v1/servers/{sid}/diagnose");

        // First contact: everything up to the host key passes, which then waits
        let (status, d) = h.owner("POST", &uri, None).await;
        assert_eq!(status, StatusCode::OK, "{d}");
        assert_eq!(d["ok"], false);
        assert_eq!(step(&d, 0, "dns")["status"], "ok");
        assert_eq!(step(&d, 0, "tcp")["status"], "ok");
        assert_eq!(step(&d, 0, "handshake")["status"], "ok");
        let key = step(&d, 0, "host_key");
        assert_eq!(key["status"], "failed");
        assert!(
            key["fix"].as_str().unwrap().contains(&ssh.fingerprint()),
            "{key}"
        );
        assert_eq!(step(&d, 0, "login")["status"], "skipped");
        assert!(
            d["verdict"].as_str().unwrap().contains("host key"),
            "{}",
            d["verdict"]
        );

        h.owner(
            "POST",
            &format!("/api/v1/servers/{sid}/host-keys/trust"),
            Some(json!({ "fingerprint": ssh.fingerprint() })),
        )
        .await;
        let (_, d) = h.owner("POST", &uri, None).await;
        assert_eq!(d["ok"], true, "{d}");
        assert!(d["hops"][0]["steps"]
            .as_array()
            .unwrap()
            .iter()
            .all(|s| s["status"] == "ok"));
        assert!(
            d["verdict"]
                .as_str()
                .unwrap()
                .starts_with("loop is reachable"),
            "{}",
            d["verdict"]
        );

        // A wrong password: the login says so and how to fix it
        let wrong = add_server(&pool, "wrong", ssh.port, "alice", Some(bad), None).await;
        crate::test_support::trust_recorded_keys(&pool, wrong).await;
        h.owner("POST", &format!("/api/v1/servers/{wrong}/diagnose"), None)
            .await;
        crate::test_support::trust_recorded_keys(&pool, wrong).await;
        let (_, d) = h
            .owner("POST", &format!("/api/v1/servers/{wrong}/diagnose"), None)
            .await;
        let login = step(&d, 0, "login");
        assert_eq!(login["status"], "failed", "{d}");
        assert!(
            login["fix"].as_str().unwrap().contains("password"),
            "{login}"
        );
        let row = queries::servers::get_server(&pool, wrong).await.unwrap();
        assert!(
            row.last_error.is_some_and(|e| e.contains("login")),
            "recorded like a test"
        );

        // Nothing listening, and a name that does not resolve
        let closed = add_server(
            &pool,
            "closed",
            closed_port().await,
            "alice",
            Some(pw),
            None,
        )
        .await;
        let (_, d) = h
            .owner("POST", &format!("/api/v1/servers/{closed}/diagnose"), None)
            .await;
        let tcp = step(&d, 0, "tcp");
        assert_eq!(tcp["status"], "failed", "{d}");
        assert!(
            tcp["fix"].as_str().unwrap().contains("nothing listens"),
            "{tcp}"
        );
        assert_eq!(step(&d, 0, "handshake")["status"], "skipped");
        let nowhere = queries::servers::create_server(
            &pool,
            &svrctlrs_database::models::CreateServer {
                name: "nowhere".into(),
                // Fully qualified, so no search domain is appended (a CI resolver once turned
                // the bare name into a wildcard record's address)
                hostname: "svrctlrs-test.invalid.".into(),
                port: 22,
                username: None,
                credential_id: Some(pw),
                via_server_id: None,
                tags: vec![],
                startup_command: None,
                record_sessions: false,
                description: None,
                enabled: true,
            },
        )
        .await
        .unwrap();
        let (_, d) = h
            .owner("POST", &format!("/api/v1/servers/{nowhere}/diagnose"), None)
            .await;
        if step(&d, 0, "dns")["status"] == "ok" {
            // A resolver that answers even for .invalid (NXDOMAIN rewriting): nothing to test here
            eprintln!("skipped the DNS failure check: this resolver resolves .invalid names");
        } else {
            assert_eq!(step(&d, 0, "dns")["status"], "failed", "{d}");
            assert!(step(&d, 0, "dns")["fix"].is_string());
            assert_eq!(step(&d, 0, "tcp")["status"], "skipped");
        }

        // Behind the (working) loopback server as jump host: the jump hop passes, the tunnel
        // to a closed port fails
        let behind = add_server(
            &pool,
            "behind",
            closed_port().await,
            "alice",
            Some(pw),
            Some(sid),
        )
        .await;
        let (_, d) = h
            .owner("POST", &format!("/api/v1/servers/{behind}/diagnose"), None)
            .await;
        assert_eq!(d["hops"].as_array().unwrap().len(), 2, "{d}");
        assert_eq!(d["hops"][0]["server_name"], "loop");
        assert!(d["hops"][0]["steps"]
            .as_array()
            .unwrap()
            .iter()
            .all(|s| s["status"] == "ok"));
        assert_eq!(step(&d, 1, "tunnel")["status"], "failed", "{d}");
    }

    #[tokio::test]
    async fn exec_runs_one_command_for_full_admin_tokens_only() {
        let h = AppHarness::new().await;
        let pool = h.state.pool.clone();
        let ssh = TestSshServer::start(Options::password("alice", "pw")).await;
        let pw = add_credential(&pool, "pw", CredentialType::Password, "pw").await;
        let sid = add_server(&pool, "loop", ssh.port, "alice", Some(pw), None).await;
        let uri = format!("/api/v1/servers/{sid}/exec");

        // An unconfirmed key is refused with the fingerprint, as for a shell
        let (status, body) = h
            .owner("POST", &uri, Some(json!({ "command": "echo hi" })))
            .await;
        assert_eq!(status, StatusCode::PRECONDITION_REQUIRED, "{body}");
        crate::test_support::trust_recorded_keys(&pool, sid).await;

        let (_, full) = h.token("hermes", "full_admin").await;
        let (status, out) = h
            .with(
                &full,
                "POST",
                &uri,
                Some(json!({ "command": "echo hello" })),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{out}");
        assert_eq!(out["stdout"], "hello\n");
        assert_eq!(out["exit_code"], 0);
        assert_eq!(out["timed_out"], false);
        let (_, out) = h
            .owner("POST", &uri, Some(json!({ "command": "exit 3" })))
            .await;
        assert_eq!(out["exit_code"], 3);

        for bad in [
            json!({ "command": "  " }),
            json!({ "command": "x", "timeout_secs": 0 }),
            json!({ "command": "x", "timeout_secs": 601 }),
        ] {
            let (status, _) = h.owner("POST", &uri, Some(bad.clone())).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{bad}");
        }
        let (_, keys) = h.token("keys", "servers_keys").await;
        let (status, _) = h
            .with(&keys, "POST", &uri, Some(json!({ "command": "id" })))
            .await;
        assert_eq!(
            status,
            StatusCode::FORBIDDEN,
            "exec is a shell: full admin only"
        );

        assert!(
            eventually(|| async {
                queries::audit::list(&pool, 50, None)
                    .await
                    .unwrap()
                    .iter()
                    .any(|e| {
                        e.action == "server.exec"
                            && e.token_name.as_deref() == Some("hermes")
                            && e.details
                                .as_deref()
                                .is_some_and(|d| d.contains("echo hello"))
                    })
            })
            .await
        );
    }

    #[tokio::test]
    async fn onboard_stops_for_the_fingerprint_then_switches_to_a_new_key() {
        let h = AppHarness::new().await;
        let pool = h.state.pool.clone();
        let ssh = TestSshServer::start(Options::password("alice", "pw")).await;
        let pw = add_credential(&pool, "lab password", CredentialType::Password, "pw").await;
        let (_, keys) = h.token("hermes", "servers_keys").await;
        let request = |fingerprint: Option<String>, dry_run: bool| {
            json!({
                "name": "lab-02", "hostname": "127.0.0.1", "port": ssh.port, "username": "alice",
                "credential_id": pw, "tags": ["lab"], "fingerprint": fingerprint, "dry_run": dry_run,
            })
        };

        let (status, plan) = h
            .with(
                &keys,
                "POST",
                "/api/v1/servers/onboard",
                Some(request(None, true)),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{plan}");
        assert_eq!(plan["dry_run"], true);
        assert!(plan["server_id"].is_null());
        assert!(
            queries::servers::list_servers(&pool)
                .await
                .unwrap()
                .is_empty(),
            "nothing created"
        );

        let (_, first) = h
            .with(
                &keys,
                "POST",
                "/api/v1/servers/onboard",
                Some(request(None, false)),
            )
            .await;
        assert_eq!(first["done"], false, "{first}");
        let sid = first["server_id"].as_i64().unwrap();
        let waiting = first["steps"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["step"] == "host_key")
            .unwrap();
        assert_eq!(waiting["status"], "waiting");
        assert!(first["next"].as_str().unwrap().contains(&ssh.fingerprint()));

        let (_, wrong) = h
            .with(
                &keys,
                "POST",
                "/api/v1/servers/onboard",
                Some(request(Some("SHA256:not-it".into()), false)),
            )
            .await;
        assert_eq!(wrong["done"], false);
        assert_eq!(wrong["server_id"], sid, "the same server, picked up again");

        let (_, done) = h
            .with(
                &keys,
                "POST",
                "/api/v1/servers/onboard",
                Some(request(Some(ssh.fingerprint()), false)),
            )
            .await;
        assert_eq!(done["done"], true, "{done}");
        let server = queries::servers::get_server(&pool, sid).await.unwrap();
        let key = queries::credentials::get_credential(&pool, server.credential_id.unwrap())
            .await
            .unwrap();
        assert!(key.is_ssh_key() && key.name == "lab-02 key", "{}", key.name);
        // The new key really logs in
        let (_, d) = h
            .owner("POST", &format!("/api/v1/servers/{sid}/diagnose"), None)
            .await;
        assert_eq!(d["ok"], true, "{d}");
        assert!(step(&d, 0, "login")["detail"]
            .as_str()
            .unwrap()
            .contains("lab-02 key"));
    }

    #[tokio::test]
    async fn rotate_key_installs_a_new_key_and_removes_the_old_one() {
        let h = AppHarness::new().await;
        let pool = h.state.pool.clone();
        let old_key = new_key();
        let ssh = TestSshServer::start(Options::key("alice", old_key.public_key().clone())).await;
        let old = add_credential(
            &pool,
            "old key",
            CredentialType::SshKey,
            &private_openssh(&old_key),
        )
        .await;
        let sid = add_server(&pool, "web", ssh.port, "alice", Some(old), None).await;
        // A second server row using the old key, to prove it no longer logs in
        let probe = add_server(&pool, "probe", ssh.port, "alice", Some(old), None).await;
        for id in [sid, probe] {
            h.owner("POST", &format!("/api/v1/servers/{id}/diagnose"), None)
                .await;
            crate::test_support::trust_recorded_keys(&pool, id).await;
        }
        let uri = format!("/api/v1/servers/{sid}/rotate-key");

        let (_, plan) = h
            .owner("POST", &uri, Some(json!({ "dry_run": true })))
            .await;
        assert_eq!(plan["dry_run"], true, "{plan}");
        assert_eq!(
            queries::servers::get_server(&pool, sid)
                .await
                .unwrap()
                .credential_id,
            Some(old)
        );

        let (status, r) = h.owner("POST", &uri, Some(json!({}))).await;
        assert_eq!(status, StatusCode::OK, "{r}");
        assert_eq!(r["done"], true, "{r}");
        let new_id = r["new_credential_id"].as_i64().unwrap();
        assert_eq!(
            queries::servers::get_server(&pool, sid)
                .await
                .unwrap()
                .credential_id,
            Some(new_id)
        );
        assert!(
            r["next"].as_str().unwrap().contains("1 other server"),
            "{}",
            r["next"]
        );

        let (_, d) = h
            .owner("POST", &format!("/api/v1/servers/{sid}/diagnose"), None)
            .await;
        assert_eq!(d["ok"], true, "the new key logs in: {d}");
        let (_, d) = h
            .owner("POST", &format!("/api/v1/servers/{probe}/diagnose"), None)
            .await;
        assert_eq!(
            step(&d, 0, "login")["status"],
            "failed",
            "the old key is gone: {d}"
        );

        // A password server has nothing to rotate
        let pw = add_credential(&pool, "pw", CredentialType::Password, "pw").await;
        let pw_server = add_server(&pool, "pw-host", ssh.port, "alice", Some(pw), None).await;
        let (status, _) = h
            .owner(
                "POST",
                &format!("/api/v1/servers/{pw_server}/rotate-key"),
                Some(json!({})),
            )
            .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }
}
