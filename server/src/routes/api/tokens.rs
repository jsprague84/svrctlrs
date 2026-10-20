//! The caller's API tokens: list, create (the secret is in this one response and nowhere
//! else), revoke, remove. Signed-in users only: no token preset names these routes, so a
//! token can never mint, change or revoke tokens (`api_tokens.rs`).

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use serde_json::json;
use svrctlrs_api::tokens::{CreatedToken, NewToken, TokenChanges, TokenList, TokenOne};
use svrctlrs_api::{ErrorBody, IdMessage};
use svrctlrs_database::{
    models::{ApiToken, NewApiToken, NewAuditEntry},
    queries::api_tokens,
};
use tracing::{info, instrument};

use super::ApiError;
use crate::api_tokens::{self as tokens, Preset};
use crate::routes::ui::auth::{client_ip, CurrentUser};
use crate::state::AppState;
use utoipa_axum::{router::OpenApiRouter, routes};

/// Longest lifetime a token may be given in days; `null` (never) is always allowed.
const MAX_LIFETIME_DAYS: u32 = 3650;

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list, create))
        .routes(routes!(revoke))
        .routes(routes!(update, remove))
}

fn token_view(t: &ApiToken) -> svrctlrs_api::tokens::ApiToken {
    svrctlrs_api::tokens::ApiToken {
        id: t.id,
        name: t.name.clone(),
        prefix: t.prefix.clone(),
        preset: t.preset.clone(),
        status: t.status_at(chrono::Utc::now()).to_string(),
        created_at: t.created_at,
        expires_at: t.expires_at,
        last_used_at: t.last_used_at,
        last_used_ip: t.last_used_ip.clone(),
        revoked_at: t.revoked_at,
        require_approval: t.require_approval,
    }
}

/// The caller's tokens, newest first, revoked and expired ones included
#[utoipa::path(get, path = "/", tag = "tokens", responses((status = 200, body = TokenList)))]
#[instrument(skip(state))]
async fn list(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
) -> Result<Json<TokenList>, ApiError> {
    let list = api_tokens::list_for_user(&state.pool, me).await?;
    Ok(Json(TokenList {
        tokens: list.iter().map(token_view).collect(),
    }))
}

/// Create a token; the response is the only place its secret appears
#[utoipa::path(post, path = "/", tag = "tokens", request_body = NewToken,
    responses(
        (status = 201, body = CreatedToken),
        (status = 400, body = ErrorBody),
        (status = 409, description = "You already have a token with that name", body = ErrorBody),
    ))]
#[instrument(skip(state, headers))]
async fn create(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
    headers: HeaderMap,
    Json(input): Json<NewToken>,
) -> Result<(StatusCode, Json<CreatedToken>), ApiError> {
    let name = input.name.trim().to_string();
    if name.is_empty() || name.chars().count() > 64 {
        return Err(ApiError::bad_request("name must be 1–64 characters"));
    }
    let Some(preset) = Preset::parse(&input.preset) else {
        return Err(ApiError::bad_request(
            "preset must be full_admin, servers_keys or read_only",
        ));
    };
    let Some(expires_in_days) = input.expires_in_days else {
        return Err(ApiError::bad_request(
            "expires_in_days is required: a number of days, or null for never",
        ));
    };
    if expires_in_days.is_some_and(|d| d == 0 || d > MAX_LIFETIME_DAYS) {
        return Err(ApiError::bad_request(format!(
            "expires_in_days must be 1–{MAX_LIFETIME_DAYS}, or null for never"
        )));
    }

    let secret = tokens::generate();
    let token = api_tokens::create(
        &state.pool,
        &NewApiToken {
            user_id: me,
            name: name.clone(),
            prefix: secret.prefix,
            token_hash: secret.hash,
            preset: preset.as_str().to_string(),
            expires_in_days,
            require_approval: input.require_approval.unwrap_or(true),
        },
    )
    .await?;
    info!(user_id = me, token_id = token.id, name = %name, preset = preset.as_str(), "API token created");
    crate::audit::record(
        &state.pool,
        NewAuditEntry::new("token.created")
            .user(me)
            .client_ip(Some(client_ip(&headers)))
            .target("token", token.id, &token.name)
            .details(json!({
                "preset": token.preset,
                "expires_at": token.expires_at,
                "prefix": token.prefix,
                "require_approval": token.require_approval,
            })),
    );
    Ok((
        StatusCode::CREATED,
        Json(CreatedToken {
            token: token_view(&token),
            secret: secret.secret,
        }),
    ))
}

/// Turn approvals on or off for one of the caller's tokens.
#[utoipa::path(put, path = "/{id}", tag = "tokens", params(("id" = i64, Path)),
    request_body = TokenChanges,
    responses((status = 200, body = TokenOne), (status = 404, body = ErrorBody)))]
#[instrument(skip(state, headers))]
async fn update(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<TokenChanges>,
) -> Result<Json<TokenOne>, ApiError> {
    let token =
        api_tokens::set_require_approval(&state.pool, me, id, input.require_approval).await?;
    crate::audit::record(
        &state.pool,
        NewAuditEntry::new("token.updated")
            .user(me)
            .client_ip(Some(client_ip(&headers)))
            .target("token", token.id, &token.name)
            .details(json!({ "require_approval": token.require_approval })),
    );
    Ok(Json(TokenOne {
        token: token_view(&token),
    }))
}

/// Revoke a token now; terminals opened with it close
#[utoipa::path(post, path = "/{id}/revoke", tag = "tokens", params(("id" = i64, Path)),
    responses((status = 200, body = TokenOne), (status = 404, body = ErrorBody)))]
#[instrument(skip(state, headers))]
async fn revoke(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<TokenOne>, ApiError> {
    let token = api_tokens::revoke(&state.pool, me, id).await?;
    // Terminals opened with it end now
    let _ = state.token_revocations.send(token.id);
    info!(user_id = me, token_id = token.id, name = %token.name, "API token revoked");
    crate::audit::record(
        &state.pool,
        NewAuditEntry::new("token.revoked")
            .user(me)
            .client_ip(Some(client_ip(&headers)))
            .target("token", token.id, &token.name),
    );
    Ok(Json(TokenOne {
        token: token_view(&token),
    }))
}

/// Remove a token, whatever its state
#[utoipa::path(delete, path = "/{id}", tag = "tokens", params(("id" = i64, Path)),
    responses((status = 200, body = IdMessage), (status = 404, body = ErrorBody)))]
#[instrument(skip(state, headers))]
async fn remove(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<IdMessage>, ApiError> {
    let token = api_tokens::delete(&state.pool, me, id).await?;
    let _ = state.token_revocations.send(token.id);
    info!(user_id = me, token_id = token.id, name = %token.name, "API token removed");
    crate::audit::record(
        &state.pool,
        NewAuditEntry::new("token.deleted")
            .user(me)
            .client_ip(Some(client_ip(&headers)))
            .target("token", token.id, &token.name),
    );
    Ok(Json(IdMessage::new(token.id, "Token removed")))
}

/// The tokens API and a token's whole life through the real application stack: created by a
/// signed-in owner, used for a shell through the WebSocket ticket exchange against the
/// loopback SSH server, and revoked, which ends that shell.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::app::{call, eventually, AppHarness};
    use futures_util::{SinkExt, StreamExt};
    use std::time::Duration;
    use svrctlrs_database::models::CredentialType;
    use tokio_tungstenite::tungstenite::Message as WsMessage;

    #[tokio::test]
    async fn tokens_are_created_once_listed_without_secrets_revoked_and_removed() {
        let h = AppHarness::new().await;
        let (status, body) = h
            .owner(
                "POST",
                "/api/v1/tokens",
                Some(
                    json!({ "name": " hermes ", "preset": "servers_keys", "expires_in_days": 90 }),
                ),
            )
            .await;
        assert_eq!(status, StatusCode::CREATED);
        let secret = body["secret"].as_str().unwrap().to_string();
        assert!(secret.starts_with("svr_") && secret.len() == 44);
        let token = &body["token"];
        assert_eq!(token["name"], "hermes");
        assert_eq!(token["preset"], "servers_keys");
        assert_eq!(token["status"], "active");
        assert_eq!(token["prefix"], &secret[..10]);
        assert!(token["expires_at"].is_string());
        assert!(token.get("token_hash").is_none());
        let id = token["id"].as_i64().unwrap();

        for bad in [
            json!({ "name": "x", "preset": "servers_keys" }),
            json!({ "name": "x", "preset": "admin", "expires_in_days": null }),
            json!({ "name": "x", "preset": "read_only", "expires_in_days": 0 }),
            json!({ "name": "x", "preset": "read_only", "expires_in_days": 4000 }),
            json!({ "name": "  ", "preset": "read_only", "expires_in_days": null }),
        ] {
            let (status, _) = h.owner("POST", "/api/v1/tokens", Some(bad.clone())).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{bad}");
        }
        let (status, _) = h
            .owner(
                "POST",
                "/api/v1/tokens",
                Some(json!({ "name": "hermes", "preset": "read_only", "expires_in_days": null })),
            )
            .await;
        assert_eq!(status, StatusCode::CONFLICT);

        let (status, list) = h.owner("GET", "/api/v1/tokens", None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(list["tokens"].as_array().unwrap().len(), 1);
        assert!(
            !list.to_string().contains(&secret),
            "the list never shows a secret"
        );

        // The token works, but not on its own kind
        let (status, _) = call(&h.app, "GET", "/api/v1/servers", Some(&secret), None).await;
        assert_eq!(status, StatusCode::OK);
        let (status, body) = call(&h.app, "GET", "/api/v1/tokens", Some(&secret), None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["error"]["code"], "TOKEN_SCOPE");

        let (status, body) = h
            .owner("POST", &format!("/api/v1/tokens/{id}/revoke"), None)
            .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["token"]["status"], "revoked");
        let (status, body) = call(&h.app, "GET", "/api/v1/servers", Some(&secret), None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["error"]["code"], "TOKEN_REVOKED");

        let (status, _) = h.owner("POST", "/api/v1/tokens/999/revoke", None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        let (status, _) = h
            .owner("DELETE", &format!("/api/v1/tokens/{id}"), None)
            .await;
        assert_eq!(status, StatusCode::OK);
        let (_, list) = h.owner("GET", "/api/v1/tokens", None).await;
        assert!(list["tokens"].as_array().unwrap().is_empty());
        let (status, body) = call(&h.app, "GET", "/api/v1/servers", Some(&secret), None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["error"]["code"], "TOKEN_INVALID");

        let pool = h.state.pool.clone();
        assert!(
            eventually(|| async {
                let actions: Vec<String> = svrctlrs_database::queries::audit::list(&pool, 20, None)
                    .await
                    .unwrap()
                    .into_iter()
                    .map(|e| e.action)
                    .collect();
                ["token.created", "token.revoked", "token.deleted"]
                    .iter()
                    .all(|a| actions.iter().any(|x| x == a))
            })
            .await
        );
    }

    /// Read text messages until one satisfies `want`; None on timeout or close.
    async fn next_matching<S>(
        ws: &mut S,
        want: impl Fn(&serde_json::Value) -> bool,
    ) -> Option<serde_json::Value>
    where
        S: futures_util::Stream<Item = Result<WsMessage, tokio_tungstenite::tungstenite::Error>>
            + Unpin,
    {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        loop {
            let msg = tokio::time::timeout_at(deadline, ws.next())
                .await
                .ok()??
                .ok()?;
            if let WsMessage::Text(text) = msg {
                let v: serde_json::Value = serde_json::from_str(&text).ok()?;
                if want(&v) {
                    return Some(v);
                }
            }
        }
    }

    #[tokio::test]
    async fn a_full_admin_token_opens_a_shell_through_a_ticket_and_revoking_it_ends_the_shell() {
        use crate::test_support::ssh_server::{Options, TestSshServer};
        let h = AppHarness::new().await;
        let ssh = TestSshServer::start(Options::password("alice", "pw")).await;
        let cred = crate::test_support::add_credential(
            &h.state.pool,
            "pw",
            CredentialType::Password,
            "pw",
        )
        .await;
        let server_id = crate::test_support::add_server(
            &h.state.pool,
            "loop",
            ssh.port,
            "alice",
            Some(cred),
            None,
        )
        .await;

        // Narrower tokens get no terminal
        let (_, keys) = h.token("keys", "servers_keys").await;
        let (status, _) = call(&h.app, "POST", "/api/v1/auth/ws-ticket", Some(&keys), None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);

        let (token_id, full) = h.token("hermes", "full_admin").await;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let served = h.app.clone();
        tokio::spawn(async move { axum::serve(listener, served.into_make_service()).await });

        let ticket = || async {
            let (status, body) =
                call(&h.app, "POST", "/api/v1/auth/ws-ticket", Some(&full), None).await;
            assert_eq!(status, StatusCode::OK, "{body}");
            body["ticket"].as_str().unwrap().to_string()
        };
        let (mut ws, _) = tokio_tungstenite::connect_async(format!(
            "ws://{addr}/ws/terminal/pty?ticket={}",
            ticket().await
        ))
        .await
        .expect("the upgrade is accepted");
        let shell = json!({ "type": "shell", "server_id": server_id, "cols": 80, "rows": 24 });

        // First contact: confirm the host key with the same token, as an agent would
        ws.send(WsMessage::text(shell.to_string())).await.unwrap();
        let refused = next_matching(&mut ws, |v| v["type"] == "error")
            .await
            .expect("first contact is refused");
        assert_eq!(refused["code"], "host_key_unconfirmed");
        let fingerprint = refused["details"]["fingerprint"].as_str().unwrap();
        let (status, body) = call(
            &h.app,
            "POST",
            &format!("/api/v1/servers/{server_id}/host-keys/trust"),
            Some(&full),
            Some(json!({ "fingerprint": fingerprint })),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");

        ws.send(WsMessage::text(shell.to_string())).await.unwrap();
        next_matching(&mut ws, |v| v["type"] == "connected")
            .await
            .expect("a shell");
        assert_eq!(h.state.sessions.list_for_user(1).await.len(), 1);

        // The shell's audit row names the token
        let pool = h.state.pool.clone();
        assert!(
            eventually(|| async {
                svrctlrs_database::queries::audit::list(&pool, 20, None)
                    .await
                    .unwrap()
                    .iter()
                    .any(|e| {
                        e.action == "session.pty_opened"
                            && e.token_id == Some(token_id)
                            && e.token_name.as_deref() == Some("hermes")
                    })
            })
            .await
        );

        // Revoking ends the terminal and the shell it started
        let (status, _) = h
            .owner("POST", &format!("/api/v1/tokens/{token_id}/revoke"), None)
            .await;
        assert_eq!(status, StatusCode::OK);
        let ended = next_matching(&mut ws, |v| v["type"] == "error")
            .await
            .expect("told why");
        assert_eq!(ended["code"], "token_revoked");
        let sessions = h.state.sessions.clone();
        assert!(eventually(|| async { sessions.list_for_user(1).await.is_empty() }).await);
    }
}
