//! Servers API endpoints
//!
//! Provides CRUD operations for managing servers.

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use serde_json::json;
use svrctlrs_api::servers::{
    AuthorizeKey, ConnectionTest, HostKey, HostKeyList, HostKeyTrusted, HostKeysForgotten,
    ImportResult, ImportServers, ImportSkipped, ImportedServer, KeyAuthorized, NewServer, Server,
    ServerChanges, ServerList, ServerOne, TrustHostKey,
};
use svrctlrs_api::{ErrorBody, IdMessage};
use svrctlrs_database::{
    models::{CreateServer, NewAuditEntry, UpdateServer},
    queries::servers,
};

use crate::routes::ui::auth::{client_ip, CurrentUser};
use tracing::{error, info, instrument, warn};

use super::ApiError;
use crate::state::AppState;
use utoipa_axum::{router::OpenApiRouter, routes};

/// Create servers router
pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list_servers, create_server))
        .routes(routes!(import_servers))
        .routes(routes!(get_server, update_server, delete_server))
        .routes(routes!(test_connection))
        .routes(routes!(list_host_keys, forget_host_keys))
        .routes(routes!(trust_host_key))
        .routes(routes!(authorize_key))
}

/// A stored server as the API shows it.
pub(crate) fn server_view(s: svrctlrs_database::models::Server) -> Server {
    Server {
        id: s.id,
        name: s.name,
        hostname: s.hostname,
        port: s.port,
        username: s.username,
        credential_id: s.credential_id,
        via_server_id: s.via_server_id,
        tags: s.tags.0,
        startup_command: s.startup_command,
        record_sessions: s.record_sessions,
        description: s.description,
        enabled: s.enabled,
        last_seen_at: s.last_seen_at,
        last_error: s.last_error,
        created_at: s.created_at,
        updated_at: s.updated_at,
    }
}

/// Install a key credential's public half on the server using one-time bootstrap access,
/// verify that the key now logs in, and make it the server's credential.
#[utoipa::path(post, path = "/{id}/authorize-key", tag = "servers", params(("id" = i64, Path)),
    request_body = AuthorizeKey,
    responses(
        (status = 200, body = KeyAuthorized),
        (status = 400, description = "Bootstrap failed, or the key does not log in", body = ErrorBody),
        (status = 404, body = ErrorBody),
        (status = 409, description = "Host key mismatch", body = ErrorBody),
        (status = 428, description = "Host key not confirmed yet", body = ErrorBody),
    ))]
#[instrument(skip(state, input))]
async fn authorize_key(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<AuthorizeKey>,
) -> Result<Json<KeyAuthorized>, ApiError> {
    use svrctlrs_database::models::{Credential, CredentialType};
    use svrctlrs_database::queries::credentials;

    let server = servers::get_server(&state.pool, id).await?;
    let key_cred = credentials::get_credential(&state.pool, input.credential_id).await?;
    crate::key_ops::public_half(&key_cred)?;

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

    crate::key_ops::install_and_switch(&state.pool, &server, &key_cred, &bootstrap).await?;

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
    Ok(Json(KeyAuthorized {
        success: true,
        server_id: id,
        credential_id: key_cred.id,
        message: format!(
            "Key '{}' authorized on {}; the server now uses it",
            key_cred.name, server.name
        ),
    }))
}

fn host_key_view(k: &svrctlrs_database::models::ServerHostKey) -> HostKey {
    HostKey {
        id: k.id,
        key_type: k.key_type.clone(),
        fingerprint: crate::host_keys::fingerprint(&k.public_key),
        first_seen_at: k.first_seen_at,
        last_seen_at: k.last_seen_at,
        trusted_at: k.trusted_at,
        trusted: k.is_trusted(),
    }
}

/// SSH host keys recorded for a server, with OpenSSH-style fingerprints (no key material):
/// the trusted ones, and the unconfirmed one from a first contact, if any.
#[utoipa::path(get, path = "/{id}/host-keys", tag = "servers", params(("id" = i64, Path)),
    responses((status = 200, body = HostKeyList), (status = 404, body = ErrorBody)))]
#[instrument(skip(state))]
async fn list_host_keys(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<HostKeyList>, ApiError> {
    use svrctlrs_database::queries::server_host_keys;

    servers::get_server(&state.pool, id).await?;
    let keys = server_host_keys::list_host_keys_for_server(&state.pool, id).await?;
    Ok(Json(HostKeyList {
        server_id: id,
        host_keys: keys.iter().map(host_key_view).collect(),
    }))
}

/// Trust the host key recorded on first contact, identified by the fingerprint the user saw
/// (so a key that changed between showing and confirming is not trusted by accident). 404 when
/// no unconfirmed key with that fingerprint is waiting (connect once to record it).
#[utoipa::path(post, path = "/{id}/host-keys/trust", tag = "servers", params(("id" = i64, Path)),
    request_body = TrustHostKey,
    responses((status = 200, body = HostKeyTrusted), (status = 404, body = ErrorBody)))]
#[instrument(skip(state, input))]
async fn trust_host_key(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<TrustHostKey>,
) -> Result<Json<HostKeyTrusted>, ApiError> {
    use svrctlrs_database::queries::server_host_keys;

    let server = servers::get_server(&state.pool, id).await?;
    let wanted = input.fingerprint.trim();
    let keys = server_host_keys::list_host_keys_for_server(&state.pool, id).await?;
    let waiting = keys
        .iter()
        .find(|k| !k.is_trusted() && crate::host_keys::fingerprint(&k.public_key) == wanted)
        .ok_or_else(|| {
            ApiError::not_found(
                "An unconfirmed host key with that fingerprint; connect once to record the server's key, then compare again",
            )
        })?;
    let trusted = server_host_keys::trust_host_key(&state.pool, id, waiting.id).await?;
    info!(
        server_id = id,
        key_type = %trusted.key_type,
        fingerprint = wanted,
        "Host key confirmed by the user"
    );
    crate::audit::record(
        &state.pool,
        NewAuditEntry::new("server.host_key_trusted")
            .user(me)
            .client_ip(Some(client_ip(&headers)))
            .target("server", id, &server.name)
            .details(json!({ "key_type": trusted.key_type, "fingerprint": wanted })),
    );
    Ok(Json(HostKeyTrusted {
        server_id: id,
        host_key: host_key_view(&trusted),
    }))
}

/// Forget every host key recorded for a server. The next connection records the key it sees
/// and asks for confirmation again; do this after the server's key legitimately changed.
#[utoipa::path(delete, path = "/{id}/host-keys", tag = "servers", params(("id" = i64, Path)),
    responses(
        (status = 200, body = HostKeysForgotten),
        (status = 202, description = "Held for the owner's approval (a token set to ask first)"),
        (status = 404, body = ErrorBody),
    ))]
#[instrument(skip(state))]
async fn forget_host_keys(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<HostKeysForgotten>, ApiError> {
    use svrctlrs_database::queries::server_host_keys;

    let server = servers::get_server(&state.pool, id).await?;
    let removed = server_host_keys::delete_host_keys_for_server(&state.pool, id).await?;
    tracing::warn!(
        server_id = id,
        removed,
        "Stored host keys forgotten; the next connection asks for confirmation again"
    );
    crate::audit::record(
        &state.pool,
        NewAuditEntry::new("server.host_keys_forgotten")
            .user(me)
            .client_ip(Some(client_ip(&headers)))
            .target("server", id, &server.name)
            .details(json!({ "removed": removed })),
    );
    Ok(Json(HostKeysForgotten {
        server_id: id,
        removed,
    }))
}

/// List all servers
#[utoipa::path(get, path = "/", tag = "servers", responses((status = 200, body = ServerList)))]
#[instrument(skip(state))]
async fn list_servers(State(state): State<AppState>) -> Result<Json<ServerList>, ApiError> {
    let servers = servers::list_servers(&state.pool)
        .await?
        .into_iter()
        .map(server_view)
        .collect();
    Ok(Json(ServerList { servers }))
}

/// Get server by ID
#[utoipa::path(get, path = "/{id}", tag = "servers", params(("id" = i64, Path)),
    responses((status = 200, body = ServerOne), (status = 404, body = ErrorBody)))]
#[instrument(skip(state))]
async fn get_server(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<ServerOne>, ApiError> {
    let server = servers::get_server(&state.pool, id).await?;
    Ok(Json(ServerOne {
        server: server_view(server),
    }))
}

/// Create a new server
#[utoipa::path(post, path = "/", tag = "servers", request_body = NewServer,
    responses(
        (status = 201, body = IdMessage),
        (status = 400, body = ErrorBody),
        (status = 409, description = "The name is taken", body = ErrorBody),
    ))]
#[instrument(skip(state, input))]
async fn create_server(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
    headers: HeaderMap,
    Json(input): Json<NewServer>,
) -> Result<(StatusCode, Json<IdMessage>), ApiError> {
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
        via_server_id: input.via_server_id,
        tags: input.tags,
        startup_command: input.startup_command,
        record_sessions: input.record_sessions,
        description: input.description,
        enabled: input.enabled,
    };
    if let Some(via) = create.via_server_id {
        check_jump_host(&state, None, via).await?;
    }

    let id = servers::create_server(&state.pool, &create).await?;

    info!(id = id, "Server created successfully");
    crate::audit::record(
        &state.pool,
        NewAuditEntry::new("server.created")
            .user(me)
            .client_ip(Some(client_ip(&headers)))
            .target("server", id, &create.name)
            .details(json!({ "record_sessions": create.record_sessions })),
    );

    Ok((
        StatusCode::CREATED,
        Json(IdMessage::new(id, "Server created successfully")),
    ))
}

/// A jump host must exist, must not be the server itself, and must not lead back to it.
pub(crate) async fn check_jump_host(
    state: &AppState,
    server_id: Option<i64>,
    via: i64,
) -> Result<(), ApiError> {
    if Some(via) == server_id {
        return Err(ApiError::bad_request(
            "A server cannot be its own jump host",
        ));
    }
    let mut next = Some(via);
    let mut hops = 0;
    while let Some(id) = next {
        let hop = servers::get_server(&state.pool, id)
            .await
            .map_err(|e| match e {
                svrctlrs_core::Error::NotFound(_) => {
                    ApiError::bad_request(format!("Jump host {id} does not exist"))
                }
                other => ApiError::from(other),
            })?;
        if Some(hop.id) == server_id {
            return Err(ApiError::bad_request(format!(
                "{} is reached through this server; that would be a loop",
                hop.name
            )));
        }
        hops += 1;
        if hops > 4 {
            return Err(ApiError::bad_request(
                "A chain of more than four jump hosts is not supported",
            ));
        }
        next = hop.via_server_id;
    }
    Ok(())
}

/// Update an existing server
#[utoipa::path(put, path = "/{id}", tag = "servers", params(("id" = i64, Path)),
    request_body = ServerChanges,
    responses(
        (status = 200, body = IdMessage),
        (status = 400, body = ErrorBody),
        (status = 404, body = ErrorBody),
        (status = 409, description = "The name is taken", body = ErrorBody),
    ))]
#[instrument(skip(state, input))]
async fn update_server(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<ServerChanges>,
) -> Result<Json<IdMessage>, ApiError> {
    info!(id = id, "Updating server");

    // Verify server exists
    let before = servers::get_server(&state.pool, id).await?;

    let update = UpdateServer {
        name: input.name,
        hostname: input.hostname,
        port: input.port,
        username: input.username,
        credential_id: input.credential_id,
        via_server_id: input.via_server_id,
        tags: input.tags,
        startup_command: input.startup_command,
        record_sessions: input.record_sessions,
        description: input.description,
        enabled: input.enabled,
        last_error: None,
    };
    if let Some(cmd) = &update.startup_command {
        svrctlrs_database::models::validate_startup_command(cmd).map_err(ApiError::bad_request)?;
    }
    if let Some(Some(via)) = update.via_server_id {
        check_jump_host(&state, Some(id), via).await?;
    }

    servers::update_server(&state.pool, id, &update).await?;

    info!(id = id, "Server updated successfully");
    let mut entry = NewAuditEntry::new("server.updated")
        .user(me)
        .client_ip(Some(client_ip(&headers)))
        .target("server", id, update.name.as_deref().unwrap_or(&before.name));
    if let Some(record) = update.record_sessions {
        // Turning recording on or off is worth seeing in the log
        entry = entry.details(json!({ "record_sessions": record }));
    }
    crate::audit::record(&state.pool, entry);

    Ok(Json(IdMessage::new(id, "Server updated successfully")))
}

/// Delete a server
#[utoipa::path(delete, path = "/{id}", tag = "servers", params(("id" = i64, Path)),
    responses(
        (status = 200, body = IdMessage),
        (status = 202, description = "Held for the owner's approval (a token set to ask first)"),
        (status = 404, body = ErrorBody),
    ))]
#[instrument(skip(state))]
async fn delete_server(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<IdMessage>, ApiError> {
    info!(id = id, "Deleting server");

    let name = servers::delete_server(&state.pool, id).await?;

    info!(id = id, "Server deleted successfully");
    crate::audit::record(
        &state.pool,
        NewAuditEntry::new("server.deleted")
            .user(me)
            .client_ip(Some(client_ip(&headers)))
            .target("server", id, &name),
    );

    Ok(Json(IdMessage::new(id, "Server deleted successfully")))
}

/// Create servers from the `Host` blocks of an `ssh_config`. Names that already exist are
/// skipped; `ProxyJump` becomes `via_server_id` when its first hop names a created or existing
/// server (by name, else by hostname). Nothing is connected to.
#[utoipa::path(post, path = "/import", tag = "servers", request_body = ImportServers,
    responses((status = 200, body = ImportResult), (status = 400, body = ErrorBody)))]
#[instrument(skip(state, input))]
async fn import_servers(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
    headers: HeaderMap,
    Json(input): Json<ImportServers>,
) -> Result<Json<ImportResult>, ApiError> {
    if input.config.len() > 256 * 1024 {
        return Err(ApiError::bad_request(
            "That is more than 256 KiB of ssh_config",
        ));
    }
    if let Some(cred_id) = input.credential_id {
        svrctlrs_database::queries::credentials::get_credential(&state.pool, cred_id)
            .await
            .map_err(|e| match e {
                svrctlrs_core::Error::NotFound(_) => {
                    ApiError::bad_request(format!("Credential {cred_id} does not exist"))
                }
                other => ApiError::from(other),
            })?;
    }
    let parsed = crate::ssh_config::parse(&input.config);
    if parsed.hosts.is_empty() {
        return Err(ApiError::bad_request(
            "No Host blocks found; paste the part of ~/.ssh/config with `Host name` entries",
        ));
    }

    let mut existing = servers::list_servers(&state.pool).await?;
    let mut created: Vec<ImportedServer> = Vec::new();
    let mut skipped: Vec<ImportSkipped> = parsed
        .skipped
        .iter()
        .map(|(what, why)| ImportSkipped {
            name: what.to_string(),
            reason: why.to_string(),
        })
        .collect();
    let mut notes: Vec<String> = Vec::new();
    // (new server id, ProxyJump hop) to resolve once everything exists
    let mut jumps: Vec<(i64, String, String)> = Vec::new();

    for host in &parsed.hosts {
        if existing.iter().any(|s| s.name == host.alias) {
            skipped.push(ImportSkipped {
                name: host.alias.clone(),
                reason: "a server with that name exists".to_string(),
            });
            continue;
        }
        let create = CreateServer {
            name: host.alias.clone(),
            hostname: host.hostname.clone(),
            port: host.port,
            username: host.user.clone(),
            credential_id: input.credential_id,
            via_server_id: None,
            tags: input.tags.clone(),
            startup_command: None,
            record_sessions: false,
            description: Some("Imported from ssh_config".to_string()),
            enabled: true,
        };
        match servers::create_server(&state.pool, &create).await {
            Ok(id) => {
                created.push(ImportedServer {
                    id,
                    name: host.alias.clone(),
                    hostname: host.hostname.clone(),
                });
                if let Some(hop) = &host.proxy_jump {
                    jumps.push((id, host.alias.clone(), hop.clone()));
                }
                existing.push(servers::get_server(&state.pool, id).await?);
            }
            Err(e) => skipped.push(ImportSkipped {
                name: host.alias.clone(),
                reason: e.to_string(),
            }),
        }
    }

    for (id, name, hop) in jumps {
        let target = existing
            .iter()
            .find(|s| s.name == hop)
            .or_else(|| existing.iter().find(|s| s.hostname == hop))
            .map(|s| (s.id, s.name.clone()));
        match target {
            Some((via, via_name)) if via != id => {
                let update = UpdateServer {
                    via_server_id: Some(Some(via)),
                    ..Default::default()
                };
                servers::update_server(&state.pool, id, &update).await?;
                notes.push(format!("{name}: connects through {via_name}"));
            }
            _ => notes.push(format!(
                "{name}: ProxyJump {hop} is not a known server; set its jump host by hand"
            )),
        }
    }

    info!(
        created = created.len(),
        skipped = skipped.len(),
        "Imported servers from ssh_config"
    );
    crate::audit::record(
        &state.pool,
        NewAuditEntry::new("server.imported")
            .user(me)
            .client_ip(Some(client_ip(&headers)))
            .details(json!({
                "created": created.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(),
                "skipped": skipped.len(),
            })),
    );
    Ok(Json(ImportResult {
        created,
        skipped,
        notes,
    }))
}

/// Test server SSH connection. Always 200: `success` says how it went, and a failure carries
/// the error's `code` and `details` (the fingerprint to confirm, on first contact).
#[utoipa::path(post, path = "/{id}/test", tag = "servers", params(("id" = i64, Path)),
    responses((status = 200, body = ConnectionTest), (status = 404, body = ErrorBody)))]
#[instrument(skip(state))]
async fn test_connection(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<ConnectionTest>, ApiError> {
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

            Ok(Json(ConnectionTest {
                success: true,
                message: output,
                code: None,
                details: None,
                server_id: id,
                hostname: server.hostname,
                port: server.port,
                latency_ms: elapsed.as_millis() as u64,
            }))
        }
        Err(e) => {
            let elapsed = started.elapsed();
            // The same shape as the error envelope, so the client can offer the fix: confirm
            // the fingerprint, or forget the stored keys.
            let api_err = ApiError::from(e);
            let recorded = match api_err.code {
                "HOST_KEY_UNCONFIRMED" => {
                    // An expected step on first contact, not a failure
                    info!(
                        id,
                        elapsed_ms = elapsed.as_millis(),
                        "Connection test: host key waits for confirmation"
                    );
                    "Host key not confirmed yet".to_string()
                }
                "HOST_KEY_MISMATCH" => {
                    error!(
                        id,
                        elapsed_ms = elapsed.as_millis(),
                        "Connection test refused: host key mismatch"
                    );
                    "Host key mismatch".to_string()
                }
                _ => {
                    error!(id, error = %api_err.message, elapsed_ms = elapsed.as_millis(), "Connection test failed");
                    api_err.message.clone()
                }
            };
            if let Err(db) = servers::update_server_last_error(&state.pool, id, &recorded).await {
                warn!(id, error = %db, "Failed to record test failure");
            }

            Ok(Json(ConnectionTest {
                success: false,
                message: api_err.message,
                code: Some(api_err.code.to_string()),
                details: Some(api_err.details),
                server_id: id,
                hostname: server.hostname,
                port: server.port,
                latency_ms: elapsed.as_millis() as u64,
            }))
        }
    }
}
