//! Servers API endpoints
//!
//! Provides CRUD operations for managing servers.

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;
use svrctlrs_database::{
    models::{CreateServer, NewAuditEntry, UpdateServer},
    queries::servers,
};

use crate::routes::ui::auth::{client_ip, CurrentUser};
use tracing::{error, info, instrument, warn};

use super::ApiError;
use crate::state::AppState;

/// Create servers router
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list_servers).post(create_server))
        .route(
            "/{id}",
            get(get_server).put(update_server).delete(delete_server),
        )
        .route("/{id}/test", post(test_connection))
        .route(
            "/{id}/host-keys",
            get(list_host_keys).delete(forget_host_keys),
        )
        .route("/{id}/authorize-key", post(authorize_key))
}

/// Authorize-key input: which key to install, and how to get in once to install it.
#[derive(Debug, Deserialize)]
struct AuthorizeKeyInput {
    credential_id: i64,
    /// One-time bootstrap: the account's password (never stored) ...
    #[serde(default)]
    password: Option<String>,
    /// ... or an existing credential that already works on the server.
    #[serde(default)]
    bootstrap_credential_id: Option<i64>,
}

/// What `ssh-copy-id` does: append the public key to `~/.ssh/authorized_keys` with sane
/// permissions, idempotently.
fn authorized_keys_install_command(public_key: &str) -> String {
    let escaped = public_key.replace('\'', "'\\''");
    format!(
        "umask 077 && mkdir -p ~/.ssh && touch ~/.ssh/authorized_keys && \
         (grep -qxF '{escaped}' ~/.ssh/authorized_keys || printf '%s\\n' '{escaped}' >> ~/.ssh/authorized_keys) && \
         chmod 700 ~/.ssh && chmod 600 ~/.ssh/authorized_keys && echo svrctlrs-key-installed"
    )
}

/// Install a key credential's public half on the server using one-time bootstrap access,
/// verify that the key now logs in, and make it the server's credential.
#[instrument(skip(state, input))]
async fn authorize_key(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<AuthorizeKeyInput>,
) -> Result<impl IntoResponse, ApiError> {
    use svrctlrs_database::models::{Credential, CredentialType};
    use svrctlrs_database::queries::credentials;

    let server = servers::get_server(&state.pool, id).await?;
    let hostname = server.hostname.clone();
    let port = u16::try_from(server.port)
        .map_err(|_| ApiError::bad_request("Server port is out of range"))?;

    let key_cred = credentials::get_credential(&state.pool, input.credential_id).await?;
    if !key_cred.is_ssh_key() {
        return Err(ApiError::bad_request(
            "The credential to authorize must be an SSH key",
        ));
    }
    let public_key = crate::ssh_keys::public_key_of(&key_cred).ok_or_else(|| {
        ApiError::bad_request(
            "Cannot derive the public key: the credential must hold private key material \
             (generate a key, or paste one), not a path",
        )
    })?;

    // Bootstrap identity: the typed password (as a throwaway credential) or a stored one.
    let bootstrap: Credential = match (input.password, input.bootstrap_credential_id) {
        (Some(pw), _) if !pw.is_empty() => Credential {
            id: 0,
            name: "bootstrap".to_string(),
            credential_type_str: CredentialType::Password.as_str().to_string(),
            description: None,
            value: pw,
            username: server.username.clone(),
            metadata: None,
            encrypted: false,
            metadata_encrypted: false,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        },
        (_, Some(cid)) => credentials::get_credential(&state.pool, cid).await?,
        _ => {
            return Err(ApiError::bad_request(
                "Give the server password (used once, never stored) or an existing credential \
                 that already works on it",
            ))
        }
    };

    let install = authorized_keys_install_command(&public_key);
    let (stdout, stderr, code) = crate::ssh::run(&state.pool, &server, Some(&bootstrap), &install)
        .await
        .map_err(|e| {
            let login = format!(
                "{}@{}:{}",
                server.username.as_deref().unwrap_or("root"),
                hostname,
                port
            );
            let hint = if e.contains("Password authentication failed") {
                " Either the password is wrong, or this server does not allow password logins at \
             all (sshd `PasswordAuthentication no`, the default on hardened hosts). In that case \
             bootstrap with an existing key credential instead, or copy the public key into \
             ~/.ssh/authorized_keys by hand (Credentials → Public key)."
            } else {
                ""
            };
            ApiError::bad_request(format!(
                "Could not log in as {login} to install the key: {e}.{hint}"
            ))
        })?;
    if code != 0 || !stdout.contains("svrctlrs-key-installed") {
        return Err(ApiError::bad_request(format!(
            "Installing the key failed (exit {code}): {}",
            stderr.trim()
        )));
    }

    // Prove the key works before switching the server over to it.
    let (_, stderr, code) = crate::ssh::run(
        &state.pool,
        &server,
        Some(&key_cred),
        "echo svrctlrs-key-ok",
    )
    .await
    .map_err(|e| {
        ApiError::bad_request(format!("Key installed, but logging in with it failed: {e}"))
    })?;
    if code != 0 {
        return Err(ApiError::bad_request(format!(
            "Key installed, but logging in with it failed (exit {code}): {}",
            stderr.trim()
        )));
    }

    let update = UpdateServer {
        credential_id: Some(Some(key_cred.id)),
        ..Default::default()
    };
    servers::update_server(&state.pool, id, &update)
        .await
        .map_err(|e| {
            ApiError::internal_error(format!("Key works, but updating the server failed: {e}"))
        })?;

    info!(
        server_id = id,
        credential_id = key_cred.id,
        "SSH key authorized on server"
    );
    crate::audit::record(
        &state.pool,
        NewAuditEntry::new("server.key_authorized")
            .user(me)
            .client_ip(Some(client_ip(&headers)))
            .target("server", id, &server.name)
            .details(json!({ "credential_id": key_cred.id, "credential": key_cred.name })),
    );
    Ok(Json(json!({
        "success": true,
        "server_id": id,
        "credential_id": key_cred.id,
        "message": format!("Key '{}' authorized on {}; the server now uses it", key_cred.name, server.name)
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_command_is_idempotent_and_quotes_the_key() {
        let cmd = authorized_keys_install_command("ssh-ed25519 AAAA it's");
        assert!(cmd.contains("grep -qxF 'ssh-ed25519 AAAA it'\\''s'"));
        assert!(cmd.contains("chmod 600 ~/.ssh/authorized_keys"));
        assert!(cmd.ends_with("echo svrctlrs-key-installed"));
    }
}

/// Stored SSH host keys for a server, with OpenSSH-style fingerprints (no key material).
#[instrument(skip(state))]
async fn list_host_keys(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, ApiError> {
    use svrctlrs_database::queries::server_host_keys;

    servers::get_server(&state.pool, id).await?;
    let keys = server_host_keys::list_host_keys_for_server(&state.pool, id).await?;
    let keys: Vec<_> = keys
        .iter()
        .map(|k| {
            json!({
                "id": k.id,
                "key_type": k.key_type,
                "fingerprint": crate::host_keys::fingerprint(&k.public_key),
                "first_seen_at": k.first_seen_at,
                "last_seen_at": k.last_seen_at,
            })
        })
        .collect();
    Ok(Json(json!({ "server_id": id, "host_keys": keys })))
}

/// Forget every stored host key for a server. The next connection trusts the key it sees
/// (TOFU), so do this only after confirming out of band that the server's key changed.
#[instrument(skip(state))]
async fn forget_host_keys(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, ApiError> {
    use svrctlrs_database::queries::server_host_keys;

    servers::get_server(&state.pool, id).await?;
    let removed = server_host_keys::delete_host_keys_for_server(&state.pool, id).await?;
    tracing::warn!(
        server_id = id,
        removed,
        "Stored host keys forgotten; next connection re-trusts"
    );
    Ok(Json(json!({ "server_id": id, "removed": removed })))
}

/// List all servers
#[instrument(skip(state))]
async fn list_servers(State(state): State<AppState>) -> Result<impl IntoResponse, ApiError> {
    let servers_list = servers::list_servers(&state.pool).await?;

    Ok(Json(json!({ "servers": servers_list })))
}

/// Get server by ID
#[instrument(skip(state))]
async fn get_server(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, ApiError> {
    let server = servers::get_server(&state.pool, id).await?;

    Ok(Json(json!({
        "server": server
    })))
}

/// Create server input
#[derive(Debug, Deserialize)]
struct CreateServerInput {
    name: String,
    #[serde(default)]
    hostname: Option<String>,
    #[serde(default = "default_port")]
    port: i32,
    #[serde(default)]
    username: Option<String>,
    #[serde(default)]
    credential_id: Option<i64>,
    #[serde(default)]
    description: Option<String>,
    /// Retired with command mode; `true` is refused so old clients get a clear answer.
    #[serde(default)]
    is_local: bool,
    #[serde(default = "default_enabled")]
    enabled: bool,
}

fn default_port() -> i32 {
    22
}

fn default_enabled() -> bool {
    true
}

/// Create a new server
#[instrument(skip(state, input))]
async fn create_server(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
    headers: HeaderMap,
    Json(input): Json<CreateServerInput>,
) -> Result<impl IntoResponse, ApiError> {
    info!(name = %input.name, "Creating server");
    if input.is_local {
        return Err(ApiError::bad_request(
            "Local servers are not supported: SvrCtlRS connects over SSH only. Use a hostname \
             (localhost works) and a credential instead.",
        ));
    }

    let create = CreateServer {
        name: input.name,
        hostname: input
            .hostname
            .map(|h| h.trim().to_string())
            .filter(|h| !h.is_empty())
            .ok_or_else(|| ApiError::bad_request("A server needs a hostname"))?,
        port: input.port,
        username: input.username,
        credential_id: input.credential_id,
        description: input.description,
        enabled: input.enabled,
    };

    let id = servers::create_server(&state.pool, &create).await?;

    info!(id = id, "Server created successfully");
    crate::audit::record(
        &state.pool,
        NewAuditEntry::new("server.created")
            .user(me)
            .client_ip(Some(client_ip(&headers)))
            .target("server", id, &create.name),
    );

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "id": id,
            "message": "Server created successfully"
        })),
    ))
}

/// Update server input
#[derive(Debug, Deserialize)]
struct UpdateServerInput {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    hostname: Option<String>,
    #[serde(default)]
    port: Option<i32>,
    #[serde(default)]
    username: Option<String>,
    /// Absent keeps the credential, `null` unassigns it, a number sets it
    #[serde(default, deserialize_with = "super::double_option")]
    credential_id: Option<Option<i64>>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    enabled: Option<bool>,
}

/// Update an existing server
#[instrument(skip(state, input))]
async fn update_server(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<UpdateServerInput>,
) -> Result<impl IntoResponse, ApiError> {
    info!(id = id, "Updating server");

    // Verify server exists
    servers::get_server(&state.pool, id).await?;

    let update = UpdateServer {
        name: input.name,
        hostname: input.hostname,
        port: input.port,
        username: input.username,
        credential_id: input.credential_id,
        description: input.description,
        enabled: input.enabled,
        last_error: None,
    };

    servers::update_server(&state.pool, id, &update).await?;

    info!(id = id, "Server updated successfully");
    crate::audit::record(
        &state.pool,
        NewAuditEntry::new("server.updated")
            .user(me)
            .client_ip(Some(client_ip(&headers)))
            .target("server", id, ""),
    );

    Ok(Json(json!({
        "id": id,
        "message": "Server updated successfully"
    })))
}

/// Delete a server
#[instrument(skip(state))]
async fn delete_server(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, ApiError> {
    info!(id = id, "Deleting server");

    servers::delete_server(&state.pool, id).await?;

    info!(id = id, "Server deleted successfully");
    crate::audit::record(
        &state.pool,
        NewAuditEntry::new("server.deleted")
            .user(me)
            .client_ip(Some(client_ip(&headers)))
            .target("server", id, ""),
    );

    Ok(Json(json!({
        "id": id,
        "message": "Server deleted successfully"
    })))
}

/// Test server SSH connection
#[instrument(skip(state))]
async fn test_connection(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, ApiError> {
    info!(id = id, "Testing server connection");

    let server = servers::get_server(&state.pool, id).await?;

    // Load credential if assigned
    let credential = match server.credential_id {
        Some(cred_id) => {
            match svrctlrs_database::queries::credentials::get_credential(&state.pool, cred_id)
                .await
            {
                Ok(cred) => Some(cred),
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to load credential for connection test");
                    None
                }
            }
        }
        None => None,
    };

    let started = std::time::Instant::now();
    // The same client as the terminal: the host key is verified (trust on first use) during
    // the handshake, so a test never authenticates against an unverified server.
    let result = crate::ssh::run(
        &state.pool,
        &server,
        credential.as_ref(),
        "echo svrctlrs-connection-test",
    )
    .await
    .map(|(stdout, stderr, exit_code)| {
        if exit_code == 0 {
            format!("Connected: {}", stdout.trim())
        } else {
            format!(
                "Connected, but the test command exited {exit_code}: {}",
                stderr.trim()
            )
        }
    });

    match result {
        Ok(output) => {
            let elapsed = started.elapsed();
            info!(
                id = id,
                elapsed_ms = elapsed.as_millis(),
                "Connection test succeeded"
            );
            // What the sidebar dot and the Servers page show; a green test must count as online
            if let Err(e) = servers::update_server_status(&state.pool, id, None).await {
                warn!(id, error = %e, "Failed to record test result");
            }

            Ok(Json(json!({
                "success": true,
                "message": output,
                "server_id": id,
                "hostname": server.hostname,
                "port": server.port,
                "latency_ms": elapsed.as_millis()
            })))
        }
        Err(e) => {
            let elapsed = started.elapsed();
            error!(id = id, error = %e, elapsed_ms = elapsed.as_millis(), "Connection test failed");
            if let Err(db) = servers::update_server_last_error(&state.pool, id, &e).await {
                warn!(id, error = %db, "Failed to record test failure");
            }

            Ok(Json(json!({
                "success": false,
                "message": e,
                "server_id": id,
                "hostname": server.hostname,
                "port": server.port,
                "latency_ms": elapsed.as_millis()
            })))
        }
    }
}
