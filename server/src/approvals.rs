//! Approvals: the few destructive calls an API token may make only after its owner says yes.
//!
//! Deleting a server, deleting a credential and forgetting a server's host keys are the only
//! gated calls ([`GATED`]). For a token with `require_approval`, the auth layer hands such a
//! call to [`hold`] instead of the handler: it is stored as a pending request (one hour), a
//! link goes out through the configured channel (push, webhook, Matrix, or none: the caller
//! has the link in the 202 either way), and the caller may wait on
//! `GET /api/v1/approvals/{id}?wait=25`. The owner approves or denies signed in; no token can
//! (no preset names those routes), so an agent can never approve itself. On approval
//! [`decide`] makes the original call through the same API router, as the token, so the
//! handler's own checks and audit row (naming the token) apply unchanged.

use axum::{
    body::Body,
    http::{header, HeaderMap, Method, Request, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use base64::Engine;
use rand::RngCore;
use serde_json::json;
use svrctlrs_core::notifications::{self, MatrixRoom, NotificationClient};
use svrctlrs_database::{
    models::{Approval, NewApproval, NewAuditEntry},
    queries::{api_tokens, approvals, credentials, servers, settings},
    SqlxPool, SqlxSqlite,
};
use tower::ServiceExt;
use tracing::{info, warn};

use crate::api_tokens::{path_matches, Preset, TokenAuth};
use crate::routes::api::ApiError;
use crate::routes::ui::auth::{client_ip, CurrentUser};
use crate::state::AppState;

/// How long a request waits for its owner
pub const TTL_MINUTES: u32 = 60;
/// Longest a caller may wait in one `GET /approvals/{id}?wait=` (under the 30 s timeout)
pub const MAX_WAIT_SECS: u64 = 25;

/// A call that can need approval.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    ServerDelete,
    CredentialDelete,
    HostKeysForget,
}

/// The gated calls. Everything else a token's preset allows runs at once.
const GATED: &[(&str, &str, Action)] = &[
    ("DELETE", "/api/v1/servers/*", Action::ServerDelete),
    ("DELETE", "/api/v1/credentials/*", Action::CredentialDelete),
    (
        "DELETE",
        "/api/v1/servers/*/host-keys",
        Action::HostKeysForget,
    ),
];

impl Action {
    pub fn as_str(self) -> &'static str {
        match self {
            Action::ServerDelete => "server.delete",
            Action::CredentialDelete => "credential.delete",
            Action::HostKeysForget => "server.host_keys_forget",
        }
    }

    fn target_type(self) -> &'static str {
        match self {
            Action::CredentialDelete => "credential",
            _ => "server",
        }
    }
}

/// "Delete server web-01", for the notification, the page and the 202
pub fn summary(action: &str, target_name: Option<&str>, target_id: i64) -> String {
    let fallback = format!("#{target_id}");
    let name = target_name.unwrap_or(&fallback);
    match action {
        "server.delete" => format!("Delete server {name}"),
        "credential.delete" => format!("Delete credential {name}"),
        "server.host_keys_forget" => format!("Forget the host keys of {name}"),
        other => format!("{other} {name}"),
    }
}

/// The gated action and its target id, if this call is one. A path whose id is not a number
/// is left to the handler, which refuses it.
pub fn gated(method: &Method, path: &str) -> Option<(Action, i64)> {
    let (_, _, action) = GATED
        .iter()
        .find(|(m, p, _)| *m == method.as_str() && path_matches(p, path))?;
    let id = path.split('/').nth(4)?.parse().ok()?;
    Some((*action, id))
}

fn new_id() -> String {
    let mut bytes = [0u8; 16];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

/// Where links point: the `approval.link_base` setting, else the address the request came to.
pub(crate) async fn link_base(pool: &SqlxPool<SqlxSqlite>, headers: &HeaderMap) -> String {
    let configured = settings::get_setting_value_or(pool, "approval.link_base", "")
        .await
        .unwrap_or_default();
    if !configured.trim().is_empty() {
        return configured.trim().trim_end_matches('/').to_string();
    }
    let first = |name: &str| {
        headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .map(|v| v.split(',').next().unwrap_or(v).trim().to_string())
            .filter(|v| !v.is_empty())
    };
    let host = first("x-forwarded-host")
        .or_else(|| first(header::HOST.as_str()))
        .unwrap_or_else(|| "localhost".to_string());
    let proto = first("x-forwarded-proto").unwrap_or_else(|| "http".to_string());
    format!("{proto}://{host}")
}

/// What every approval endpoint (and the 202) shows.
pub fn approval_view(a: &Approval) -> svrctlrs_api::approvals::Approval {
    svrctlrs_api::approvals::Approval {
        id: a.id.clone(),
        status: a.state_at(chrono::Utc::now()).to_string(),
        action: a.action.clone(),
        summary: summary(&a.action, a.target_name.as_deref(), a.target_id),
        method: a.method.clone(),
        path: a.path.clone(),
        target_type: a.target_type.clone(),
        target_id: a.target_id,
        target_name: a.target_name.clone(),
        token_id: a.token_id,
        token_name: a.token_name.clone(),
        client_ip: a.client_ip.clone(),
        url: a.url.clone(),
        created_at: a.created_at,
        expires_at: a.expires_at,
        decided_at: a.decided_at,
        result_status: a.result_status,
        result: a
            .result
            .as_deref()
            .map(|r| serde_json::from_str(r).unwrap_or_else(|_| json!(r))),
    }
}

fn accepted(a: &Approval) -> Response {
    let wait = format!("/api/v1/approvals/{}?wait={MAX_WAIT_SECS}", a.id);
    (
        StatusCode::ACCEPTED,
        [(header::LOCATION, wait.clone())],
        Json(svrctlrs_api::approvals::HeldForApproval {
            approval: approval_view(a),
            message: format!(
                "{} needs the owner's approval: they approve or deny it signed in at {} \
                 within {TTL_MINUTES} minutes",
                summary(&a.action, a.target_name.as_deref(), a.target_id),
                a.url
            ),
            wait_url: wait,
        }),
    )
        .into_response()
}

/// Hold a gated call from `token`: answer 202 with a pending request (the one already
/// waiting for the same call, if any) and send its link out. None when the target does not
/// exist, so the handler answers 404 as it would without approvals.
pub async fn hold(
    pool: &SqlxPool<SqlxSqlite>,
    token: &TokenAuth,
    action: Action,
    target_id: i64,
    method: &Method,
    path: &str,
    headers: &HeaderMap,
) -> Option<Response> {
    match approvals::find_waiting(pool, token.id, method.as_str(), path).await {
        Ok(Some(waiting)) => return Some(accepted(&waiting)),
        Ok(None) => {}
        Err(e) => return Some(ApiError::from(e).into_response()),
    }
    let target_name = match action {
        Action::CredentialDelete => credentials::get_credential(pool, target_id)
            .await
            .ok()
            .map(|c| c.name),
        _ => servers::get_server(pool, target_id)
            .await
            .ok()
            .map(|s| s.name),
    }?;

    let id = new_id();
    let url = format!("{}/approvals?id={id}", link_base(pool, headers).await);
    let ip = client_ip(headers);
    let approval = match approvals::create(
        pool,
        &NewApproval {
            id,
            user_id: token.user_id,
            token_id: token.id,
            token_name: token.name.clone(),
            action: action.as_str().to_string(),
            method: method.as_str().to_string(),
            path: path.to_string(),
            target_type: action.target_type().to_string(),
            target_id,
            target_name: Some(target_name.clone()),
            client_ip: Some(ip.clone()),
            url,
            ttl_minutes: TTL_MINUTES,
        },
    )
    .await
    {
        Ok(a) => a,
        Err(e) => return Some(ApiError::from(e).into_response()),
    };
    info!(approval = %approval.id, token_id = token.id, action = action.as_str(), target_id,
        "Call held for approval");
    crate::audit::record(
        pool,
        NewAuditEntry::new("approval.requested")
            .user(token.user_id)
            .client_ip(Some(ip))
            .token(token.id, &token.name)
            .target(action.target_type(), target_id, &target_name)
            .details(json!({ "approval": approval.id, "action": action.as_str() })),
    );
    let pool = pool.clone();
    let sent = approval.clone();
    tokio::spawn(async move {
        if let Err(e) = notify(&pool, &sent, None).await {
            warn!(error = %e, approval = %sent.id, "Approval request not delivered");
        }
    });
    Some(accepted(&approval))
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Channel settings, saved or (for a test) as the form has them.
pub type ChannelOverride = svrctlrs_api::notifications::ApprovalChannelTest;

/// Send the request's link through the configured channel. `overrides` replace saved values
/// (for a test from the settings form); an empty secret means "the saved one".
pub async fn notify(
    pool: &SqlxPool<SqlxSqlite>,
    a: &Approval,
    overrides: Option<&ChannelOverride>,
) -> anyhow::Result<&'static str> {
    async fn setting(
        pool: &SqlxPool<SqlxSqlite>,
        key: &str,
        over: Option<&String>,
        secret: bool,
    ) -> String {
        match over {
            Some(v) if !(secret && v.is_empty()) => v.trim().to_string(),
            _ => settings::get_setting_value_or(pool, key, "")
                .await
                .unwrap_or_default(),
        }
    }
    let o = overrides.cloned().unwrap_or_default();
    let channel = setting(pool, "approval.channel", o.channel.as_ref(), false).await;
    let what = summary(&a.action, a.target_name.as_deref(), a.target_id);
    let text = format!(
        "{} asks to {} on svrctlrs. Approve or deny it signed in: {} (expires in {TTL_MINUTES} min)",
        a.token_name,
        lowercase_first(&what),
        a.url
    );
    match channel.as_str() {
        "none" => Ok("none"),
        "webhook" => {
            let url = setting(pool, "approval.webhook_url", o.webhook_url.as_ref(), true).await;
            anyhow::ensure!(!url.is_empty(), "No approval webhook URL is set");
            notifications::post_webhook(
                &url,
                &json!({
                    "title": "svrctlrs: approval needed",
                    "message": text,
                    "url": a.url,
                    "approval": approval_view(a),
                }),
            )
            .await?;
            Ok("webhook")
        }
        "matrix" => {
            let room = MatrixRoom {
                homeserver: setting(
                    pool,
                    "approval.matrix_homeserver",
                    o.matrix_homeserver.as_ref(),
                    false,
                )
                .await,
                room_id: setting(pool, "approval.matrix_room", o.matrix_room.as_ref(), false).await,
                access_token: setting(pool, "approval.matrix_token", o.matrix_token.as_ref(), true)
                    .await,
            };
            anyhow::ensure!(
                !room.homeserver.is_empty()
                    && !room.room_id.is_empty()
                    && !room.access_token.is_empty(),
                "Set the Matrix homeserver, room and access token first"
            );
            let html = format!(
                "🔐 <b>{}</b> asks to <b>{}</b> on svrctlrs.<br><a href=\"{}\">Approve or deny</a> \
                 (signed in) within {TTL_MINUTES} minutes.",
                escape_html(&a.token_name),
                escape_html(&lowercase_first(&what)),
                escape_html(&a.url)
            );
            room.send(&format!("svrctlrs-{}", a.id), &text, &html)
                .await?;
            Ok("matrix")
        }
        // "push" and anything unknown: the notification provider, when it is on
        _ => {
            let config = settings::get_notification_config(pool).await?;
            anyhow::ensure!(
                config.enabled && !config.url.is_empty(),
                "Push notifications are off; turn them on or pick another channel"
            );
            NotificationClient::new(config)
                .send("svrctlrs: approval needed", &text, 8)
                .await?;
            Ok("push")
        }
    }
}

fn lowercase_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => c.to_lowercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// Approve or deny one of `user_id`'s requests. On approval the held call is made now, as
/// the token, through the API router; its answer is stored and returned in `result`.
pub async fn decide(
    state: &AppState,
    user_id: i64,
    id: &str,
    approve: bool,
    headers: &HeaderMap,
) -> Result<Approval, ApiError> {
    let Some(decided) = approvals::decide(&state.pool, user_id, id, approve).await? else {
        let existing = approvals::get(&state.pool, user_id, id).await?;
        return Err(ApiError::conflict(format!(
            "This request is {} and can no longer be decided",
            existing.state_at(chrono::Utc::now())
        )));
    };
    crate::audit::record(
        &state.pool,
        NewAuditEntry::new(if approve {
            "approval.approved"
        } else {
            "approval.denied"
        })
        .user(user_id)
        .client_ip(Some(client_ip(headers)))
        .target(
            &decided.target_type,
            decided.target_id,
            decided.target_name.as_deref().unwrap_or(""),
        )
        .details(json!({
            "approval": decided.id,
            "action": decided.action,
            "token": decided.token_name,
        })),
    );
    let finished = if approve {
        let (status, body) = run(state, &decided).await;
        approvals::set_result(&state.pool, &decided.id, status.as_u16(), &body).await?
    } else {
        decided
    };
    info!(approval = %finished.id, state = finished.state_at(chrono::Utc::now()), "Approval decided");
    let _ = state.approval_events.send(finished.id.clone());
    Ok(finished)
}

/// Make the held call as the token that asked, if the token is still good.
async fn run(state: &AppState, a: &Approval) -> (StatusCode, String) {
    let gone = |why: &str| {
        (
            StatusCode::GONE,
            json!({ "success": false, "error": { "code": "TOKEN_GONE", "message": why } })
                .to_string(),
        )
    };
    let Some(token_id) = a.token_id else {
        return gone("The token was removed before the approval; nothing was done");
    };
    let row = match api_tokens::get(&state.pool, token_id).await {
        Ok(Some(row)) => row,
        Ok(None) => return gone("The token was removed before the approval; nothing was done"),
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                json!({ "error": e.to_string() }).to_string(),
            )
        }
    };
    if row.status_at(chrono::Utc::now()) != "active" {
        return gone("The token was revoked or expired before the approval; nothing was done");
    }
    let Some(preset) = Preset::parse(&row.preset) else {
        return gone("The token has an unknown preset; nothing was done");
    };
    let Ok(method) = a.method.parse::<Method>() else {
        return gone("The held call is not valid; nothing was done");
    };
    if !preset.allows(&method, &a.path) {
        return gone("The token's access no longer covers this call; nothing was done");
    }
    let Some(router) = state.api_router.get().cloned() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            json!({ "error": "The server is still starting" }).to_string(),
        );
    };
    let token = TokenAuth {
        id: row.id,
        user_id: row.user_id,
        name: row.name,
        preset,
        require_approval: row.require_approval,
    };
    let mut request = Request::builder().method(method).uri(&a.path);
    if let Some(ip) = &a.client_ip {
        request = request.header("x-forwarded-for", ip);
    }
    let Ok(mut request) = request.body(Body::empty()) else {
        return gone("The held call is not valid; nothing was done");
    };
    request.extensions_mut().insert(CurrentUser(token.user_id));
    request.extensions_mut().insert(token.clone());
    let response = match crate::audit::with_token(Some(token), router.oneshot(request)).await {
        Ok(r) => r,
        Err(never) => match never {},
    };
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), 1 << 20)
        .await
        .map(|b| String::from_utf8_lossy(&b).into_owned())
        .unwrap_or_default();
    let body = if serde_json::from_str::<serde_json::Value>(&body).is_ok() {
        body
    } else {
        json!(body).to_string()
    };
    (status, body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_three_calls_are_gated() {
        let del = Method::DELETE;
        assert_eq!(
            gated(&del, "/api/v1/servers/5"),
            Some((Action::ServerDelete, 5))
        );
        assert_eq!(
            gated(&del, "/api/v1/credentials/7"),
            Some((Action::CredentialDelete, 7))
        );
        assert_eq!(
            gated(&del, "/api/v1/servers/5/host-keys"),
            Some((Action::HostKeysForget, 5))
        );
        assert_eq!(gated(&del, "/api/v1/servers/abc"), None);
        assert_eq!(gated(&Method::PUT, "/api/v1/servers/5"), None);
        assert_eq!(
            gated(&Method::POST, "/api/v1/servers/5/host-keys/trust"),
            None
        );
        assert_eq!(gated(&del, "/api/v1/servers/5/files"), None);
        assert_eq!(gated(&del, "/api/v1/recordings/5"), None);
        assert_eq!(gated(&del, "/api/v1/sessions/5"), None);
    }

    #[test]
    fn summaries_read_as_a_request() {
        assert_eq!(
            summary("server.delete", Some("web-01"), 5),
            "Delete server web-01"
        );
        assert_eq!(
            summary("server.host_keys_forget", None, 5),
            "Forget the host keys of #5"
        );
        assert_eq!(escape_html("<b>&\""), "&lt;b&gt;&amp;&quot;");
    }
}
