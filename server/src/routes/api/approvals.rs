//! Approval requests (see `approvals.rs`): the owner lists, approves and denies them signed
//! in; a token may read one to wait for the answer (`?wait=` seconds, at most 25), never
//! decide one (no preset names those routes).

use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    Json,
};
use svrctlrs_api::approvals::{Approval, ApprovalList, WaitQuery};
use svrctlrs_api::ErrorBody;
use svrctlrs_database::queries::approvals;
use tokio::sync::broadcast::error::RecvError;
use tracing::instrument;

use super::ApiError;
use crate::approvals::{approval_view, decide, MAX_WAIT_SECS};
use crate::routes::ui::auth::CurrentUser;
use crate::state::AppState;
use utoipa_axum::{router::OpenApiRouter, routes};

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list))
        .routes(routes!(get_one))
        .routes(routes!(approve))
        .routes(routes!(deny))
}

/// The caller's requests, newest first (signed-in users only)
#[utoipa::path(get, path = "/", tag = "approvals", responses((status = 200, body = ApprovalList)))]
#[instrument(skip(state))]
async fn list(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
) -> Result<Json<ApprovalList>, ApiError> {
    let list = approvals::list_for_user(&state.pool, me, 50).await?;
    let waiting = approvals::count_waiting(&state.pool, me).await?;
    Ok(Json(ApprovalList {
        approvals: list.iter().map(approval_view).collect(),
        waiting,
    }))
}

/// One request; with `?wait=N`, answers when it is decided or after N seconds (≤ 25).
/// Readable by the owner's tokens, so the asker can wait for the answer.
#[utoipa::path(get, path = "/{id}", tag = "approvals", params(("id" = String, Path), WaitQuery),
    responses((status = 200, body = Approval), (status = 404, body = ErrorBody)))]
#[instrument(skip(state))]
async fn get_one(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
    Path(id): Path<String>,
    Query(q): Query<WaitQuery>,
) -> Result<Json<Approval>, ApiError> {
    // Subscribe before reading, so a decision between the two is not missed
    let mut events = state.approval_events.subscribe();
    let mut current = approvals::get(&state.pool, me, &id).await?;
    let wait = q.wait.unwrap_or(0).min(MAX_WAIT_SECS);
    let undecided = |a: &svrctlrs_database::models::Approval| {
        matches!(a.state_at(chrono::Utc::now()), "pending" | "approved")
    };
    if wait > 0 && undecided(&current) {
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(wait);
        // Never past the request's own expiry either
        let expiry = (current.expires_at - chrono::Utc::now())
            .to_std()
            .unwrap_or_default();
        let deadline = deadline.min(tokio::time::Instant::now() + expiry);
        loop {
            match tokio::time::timeout_at(deadline, events.recv()).await {
                Ok(Ok(decided)) if decided == id => break,
                Ok(Ok(_)) => {}
                Ok(Err(RecvError::Lagged(_))) => {
                    if !undecided(&approvals::get(&state.pool, me, &id).await?) {
                        break;
                    }
                }
                Ok(Err(RecvError::Closed)) | Err(_) => break,
            }
        }
        current = approvals::get(&state.pool, me, &id).await?;
    }
    Ok(Json(approval_view(&current)))
}

/// Approve: the held call runs now, as the token that asked (signed-in owner only)
#[utoipa::path(post, path = "/{id}/approve", tag = "approvals", params(("id" = String, Path)),
    responses(
        (status = 200, description = "Decided; `result` is what the call answered", body = Approval),
        (status = 404, body = ErrorBody),
        (status = 409, description = "Already decided or expired", body = ErrorBody),
    ))]
#[instrument(skip(state, headers))]
async fn approve(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Approval>, ApiError> {
    let decided = decide(&state, me, &id, true, &headers).await?;
    Ok(Json(approval_view(&decided)))
}

/// Deny: nothing runs (signed-in owner only)
#[utoipa::path(post, path = "/{id}/deny", tag = "approvals", params(("id" = String, Path)),
    responses(
        (status = 200, body = Approval),
        (status = 404, body = ErrorBody),
        (status = 409, description = "Already decided or expired", body = ErrorBody),
    ))]
#[instrument(skip(state, headers))]
async fn deny(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Approval>, ApiError> {
    let decided = decide(&state, me, &id, false, &headers).await?;
    Ok(Json(approval_view(&decided)))
}

/// Held deletes through the real application: requested by a token, decided by its owner,
/// made on approval as the token; denial, a revoked token, and delivery of the link.
#[cfg(test)]
mod tests {
    use crate::test_support::app::{eventually, AppHarness};
    use axum::http::StatusCode;
    use serde_json::json;
    use std::sync::{Arc, Mutex};
    use svrctlrs_database::models::CredentialType;
    use svrctlrs_database::queries;

    async fn audit_actions(h: &AppHarness) -> Vec<(String, Option<String>)> {
        queries::audit::list(&h.state.pool, 50, None)
            .await
            .unwrap()
            .into_iter()
            .map(|e| (e.action, e.token_name))
            .collect()
    }

    #[tokio::test]
    async fn a_gated_delete_waits_for_the_owner_and_runs_when_approved() {
        let h = AppHarness::new().await;
        let pool = h.state.pool.clone();
        let sid = crate::test_support::add_server(&pool, "web-01", 22, "ops", None, None).await;
        let (_, tok) = h.token_with("hermes", "servers_keys", true).await;
        let path = format!("/api/v1/servers/{sid}");

        let (status, body) = h.with(&tok, "DELETE", &path, None).await;
        assert_eq!(status, StatusCode::ACCEPTED, "{body}");
        let approval = &body["approval"];
        assert_eq!(approval["status"], "pending");
        assert_eq!(approval["summary"], "Delete server web-01");
        assert_eq!(approval["token_name"], "hermes");
        let id = approval["id"].as_str().unwrap().to_string();
        assert!(approval["url"]
            .as_str()
            .unwrap()
            .ends_with(&format!("/approvals?id={id}")));
        assert!(
            queries::servers::get_server(&pool, sid).await.is_ok(),
            "not yet"
        );

        // Asking again is the same request, not a second notification
        let (status, again) = h.with(&tok, "DELETE", &path, None).await;
        assert_eq!(status, StatusCode::ACCEPTED);
        assert_eq!(again["approval"]["id"], id.as_str());

        // The token cannot approve itself, or list what waits; it can read its request
        let (status, _) = h
            .with(
                &tok,
                "POST",
                &format!("/api/v1/approvals/{id}/approve"),
                None,
            )
            .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        let (status, _) = h.with(&tok, "GET", "/api/v1/approvals", None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        let (status, mine) = h
            .with(&tok, "GET", &format!("/api/v1/approvals/{id}"), None)
            .await;
        assert_eq!(
            (status, mine["status"].as_str()),
            (StatusCode::OK, Some("pending"))
        );
        let (_, list) = h.owner("GET", "/api/v1/approvals", None).await;
        assert_eq!(list["waiting"], 1);

        // The caller waits; the owner approves; the waiter hears it at once
        let waiter = {
            let app = h.app.clone();
            let tok = tok.clone();
            let uri = format!("/api/v1/approvals/{id}?wait=10");
            tokio::spawn(async move {
                let started = std::time::Instant::now();
                let (_, body) =
                    crate::test_support::app::call(&app, "GET", &uri, Some(&tok), None).await;
                (body, started.elapsed())
            })
        };
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        let (status, decided) = h
            .owner("POST", &format!("/api/v1/approvals/{id}/approve"), None)
            .await;
        assert_eq!(status, StatusCode::OK, "{decided}");
        assert_eq!(decided["status"], "done");
        assert_eq!(decided["result_status"], 200);
        let (waited, took) = waiter.await.unwrap();
        assert_eq!(waited["status"], "done");
        assert!(took < std::time::Duration::from_secs(5), "{took:?}");
        assert!(
            queries::servers::get_server(&pool, sid).await.is_err(),
            "deleted"
        );

        // Decided once
        let (status, _) = h
            .owner("POST", &format!("/api/v1/approvals/{id}/deny"), None)
            .await;
        assert_eq!(status, StatusCode::CONFLICT);

        // The delete's own row names the token; the decision is the owner's
        assert!(
            eventually(|| async {
                let rows = audit_actions(&h).await;
                rows.contains(&("server.deleted".into(), Some("hermes".into())))
                    && rows.contains(&("approval.approved".into(), None))
                    && rows.contains(&("approval.requested".into(), Some("hermes".into())))
            })
            .await,
            "{:?}",
            audit_actions(&h).await
        );
    }

    #[tokio::test]
    async fn denied_requests_do_nothing_and_unflagged_tokens_are_not_held() {
        let h = AppHarness::new().await;
        let pool = h.state.pool.clone();
        let sid = crate::test_support::add_server(&pool, "db", 22, "ops", None, None).await;
        let cid =
            crate::test_support::add_credential(&pool, "old key", CredentialType::Password, "x")
                .await;
        let (_, careful) = h.token_with("careful", "servers_keys", true).await;
        let (_, quick) = h.token_with("quick", "servers_keys", false).await;

        let (status, body) = h
            .with(
                &careful,
                "DELETE",
                &format!("/api/v1/servers/{sid}/host-keys"),
                None,
            )
            .await;
        assert_eq!(status, StatusCode::ACCEPTED);
        assert_eq!(body["approval"]["summary"], "Forget the host keys of db");
        let id = body["approval"]["id"].as_str().unwrap();
        let (status, decided) = h
            .owner("POST", &format!("/api/v1/approvals/{id}/deny"), None)
            .await;
        assert_eq!(
            (status, decided["status"].as_str()),
            (StatusCode::OK, Some("denied"))
        );
        assert!(decided["result_status"].is_null(), "nothing ran");

        // A missing target is the handler's 404, not a request
        let (status, _) = h
            .with(&careful, "DELETE", "/api/v1/servers/999", None)
            .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        // Edits are never held
        let (status, _) = h
            .with(
                &careful,
                "PUT",
                &format!("/api/v1/servers/{sid}"),
                Some(json!({ "description": "kept" })),
            )
            .await;
        assert_eq!(status, StatusCode::OK);

        let (status, _) = h
            .with(
                &quick,
                "DELETE",
                &format!("/api/v1/credentials/{cid}"),
                None,
            )
            .await;
        assert_eq!(status, StatusCode::OK, "no approval asked");
        assert!(queries::credentials::get_credential(&pool, cid)
            .await
            .is_err());
    }

    #[tokio::test]
    async fn an_approval_for_a_revoked_token_runs_nothing() {
        let h = AppHarness::new().await;
        let pool = h.state.pool.clone();
        let sid = crate::test_support::add_server(&pool, "web", 22, "ops", None, None).await;
        let (tid, tok) = h.token_with("hermes", "full_admin", true).await;
        let (_, body) = h
            .with(&tok, "DELETE", &format!("/api/v1/servers/{sid}"), None)
            .await;
        let id = body["approval"]["id"].as_str().unwrap().to_string();
        h.owner("POST", &format!("/api/v1/tokens/{tid}/revoke"), None)
            .await;
        let (status, decided) = h
            .owner("POST", &format!("/api/v1/approvals/{id}/approve"), None)
            .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(decided["status"], "failed");
        assert_eq!(decided["result_status"], 410);
        assert!(queries::servers::get_server(&pool, sid).await.is_ok());
    }

    /// A local HTTP endpoint that keeps what it receives: (method, path, auth header, body)
    async fn catcher() -> (
        String,
        Arc<Mutex<Vec<(String, String, String, serde_json::Value)>>>,
    ) {
        let got = Arc::new(Mutex::new(Vec::new()));
        let seen = got.clone();
        let app = axum::Router::new().fallback(
            move |method: axum::http::Method,
                  uri: axum::http::Uri,
                  headers: axum::http::HeaderMap,
                  body: String| {
                let seen = seen.clone();
                async move {
                    let auth = headers
                        .get("authorization")
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or("")
                        .to_string();
                    seen.lock().unwrap().push((
                        method.to_string(),
                        uri.path().to_string(),
                        auth,
                        serde_json::from_str(&body).unwrap_or_default(),
                    ));
                    axum::Json(json!({ "event_id": "$1" }))
                }
            },
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await });
        (format!("http://{addr}"), got)
    }

    #[tokio::test]
    async fn the_link_goes_out_by_webhook_or_to_a_matrix_room() {
        let h = AppHarness::new().await;
        let pool = h.state.pool.clone();
        let (base, got) = catcher().await;
        let sid = crate::test_support::add_server(&pool, "web-01", 22, "ops", None, None).await;
        let (_, tok) = h.token_with("hermes", "servers_keys", true).await;
        let (status, _) = h
            .owner(
                "PUT",
                "/api/v1/settings",
                Some(json!({ "values": {
                    "approval.channel": "webhook",
                    "approval.webhook_url": format!("{base}/hook"),
                    "approval.link_base": "https://svrctlrs.example",
                }})),
            )
            .await;
        assert_eq!(status, StatusCode::OK);

        let (_, body) = h
            .with(&tok, "DELETE", &format!("/api/v1/servers/{sid}"), None)
            .await;
        let id = body["approval"]["id"].as_str().unwrap().to_string();
        let link = format!("https://svrctlrs.example/approvals?id={id}");
        assert_eq!(body["approval"]["url"], link.as_str());
        assert!(eventually(|| async { !got.lock().unwrap().is_empty() }).await);
        {
            let hook = got.lock().unwrap().remove(0);
            assert_eq!((hook.0.as_str(), hook.1.as_str()), ("POST", "/hook"));
            assert_eq!(hook.3["url"], link.as_str());
            assert_eq!(hook.3["approval"]["summary"], "Delete server web-01");
            assert!(hook.3["message"]
                .as_str()
                .unwrap()
                .contains("hermes asks to delete server web-01"));
        }

        // Matrix, tried from the form before saving
        let (status, body) = h
            .owner(
                "POST",
                "/api/v1/notifications/test-approval",
                Some(json!({
                    "channel": "matrix",
                    "matrix_homeserver": base,
                    "matrix_room": "!room:example.org",
                    "matrix_token": "syt_secret",
                })),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let posted = got.lock().unwrap().remove(0);
        assert_eq!(posted.0, "PUT");
        assert!(posted
            .1
            .starts_with("/_matrix/client/v3/rooms/!room:example.org/send/m.room.message/"));
        assert_eq!(posted.2, "Bearer syt_secret");
        assert_eq!(posted.3["msgtype"], "m.text");
        assert!(posted.3["formatted_body"]
            .as_str()
            .unwrap()
            .contains("Approve or deny"));

        // The secret settings read back blank
        let (_, setting) = h
            .owner("GET", "/api/v1/settings/approval.webhook_url", None)
            .await;
        assert_eq!(
            (setting["value"].as_str(), setting["is_set"].as_bool()),
            (Some(""), Some(true))
        );
        let (status, _) = h
            .owner(
                "PUT",
                "/api/v1/settings",
                Some(json!({ "values": { "approval.channel": "carrier-pigeon" } })),
            )
            .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }
}
