//! The caller's signed-in devices (`/api/v1/users/me/devices`): list them, sign one out, sign
//! out all the others. Signing out ends the device's session and closes its terminals; its
//! shells stay for the account's other devices (`devices.rs`).
//!
//! Signed-in people only: no API token preset names these routes, so a token can neither see
//! nor end a sign-in. API tokens are not devices; they have their own list.

use axum::{
    extract::{Path, State},
    http::HeaderMap,
    Extension, Json,
};
use serde_json::json;
use svrctlrs_api::devices::{Device, DeviceList, DevicesSignedOut};
use svrctlrs_api::{ErrorBody, IdMessage};
use svrctlrs_database::{models::NewAuditEntry, queries::devices};
use tracing::{info, instrument};
use utoipa_axum::{router::OpenApiRouter, routes};

use super::ApiError;
use crate::devices::DeviceAuth;
use crate::routes::ui::auth::{client_ip, CurrentUser};
use crate::state::AppState;

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list))
        .routes(routes!(sign_out))
        .routes(routes!(sign_out_others))
}

fn view(d: &svrctlrs_database::models::Device, current: Option<i64>) -> Device {
    Device {
        id: d.id,
        label: crate::devices::label(&d.method, d.user_agent.as_deref()),
        method: d.method.clone(),
        current: current == Some(d.id),
        created_at: d.created_at,
        last_seen_at: d.last_seen_at,
        last_ip: d.last_ip.clone(),
    }
}

fn current(device: Option<Extension<DeviceAuth>>) -> Option<i64> {
    device.map(|Extension(DeviceAuth(id))| id)
}

/// The caller's signed-in devices: this one first, then the most recently used
#[utoipa::path(get, path = "/me/devices", tag = "users",
    responses((status = 200, body = DeviceList)))]
#[instrument(skip(state, device))]
async fn list(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
    device: Option<Extension<DeviceAuth>>,
) -> Result<Json<DeviceList>, ApiError> {
    let this = current(device);
    let mut list: Vec<Device> = devices::list_for_user(&state.pool, me)
        .await?
        .iter()
        .map(|d| view(d, this))
        .collect();
    list.sort_by_key(|d| !d.current);
    Ok(Json(DeviceList { devices: list }))
}

/// Sign a device out: its session ends and its terminals close (its shells stay, detached).
/// Signing out this device is signing out.
#[utoipa::path(delete, path = "/me/devices/{id}", tag = "users", params(("id" = i64, Path)),
    responses((status = 200, body = IdMessage), (status = 404, body = ErrorBody)))]
#[instrument(skip(state, device, headers))]
async fn sign_out(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
    device: Option<Extension<DeviceAuth>>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<IdMessage>, ApiError> {
    let this = current(device);
    let gone = devices::delete(&state.pool, me, id).await?;
    crate::devices::signed_out(&state, std::slice::from_ref(&gone)).await;
    let label = crate::devices::label(&gone.method, gone.user_agent.as_deref());
    info!(user_id = me, device_id = id, %label, "Device signed out");
    crate::audit::record(
        &state.pool,
        NewAuditEntry::new("device.signed_out")
            .user(me)
            .client_ip(Some(client_ip(&headers)))
            .target("device", id, &label)
            .details(json!({ "current": this == Some(id), "last_ip": gone.last_ip })),
    );
    Ok(Json(IdMessage::new(id, "Signed out")))
}

/// Sign out every device but this one
#[utoipa::path(post, path = "/me/devices/sign-out-others", tag = "users",
    responses((status = 200, body = DevicesSignedOut)))]
#[instrument(skip(state, device, headers))]
async fn sign_out_others(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
    device: Option<Extension<DeviceAuth>>,
    headers: HeaderMap,
) -> Result<Json<DevicesSignedOut>, ApiError> {
    let Some(this) = current(device) else {
        return Err(ApiError::bad_request(
            "Only a signed-in device can sign out the others",
        ));
    };
    let gone = devices::delete_all_except(&state.pool, me, Some(this)).await?;
    crate::devices::signed_out(&state, &gone).await;
    info!(user_id = me, count = gone.len(), "Other devices signed out");
    crate::audit::record(
        &state.pool,
        NewAuditEntry::new("device.signed_out_others")
            .user(me)
            .client_ip(Some(client_ip(&headers)))
            .details(json!({ "count": gone.len(), "device_ids": gone.iter().map(|d| d.id).collect::<Vec<_>>() })),
    );
    Ok(Json(DevicesSignedOut {
        signed_out: gone.len(),
    }))
}

/// Devices through the real application stack: every sign-in is one, the list is the
/// caller's, a signed-out device is refused at once (even if a request still running writes
/// its session back), tokens cannot touch them, and a signed-out device's terminal closes
/// while its shell waits for another device.
#[cfg(test)]
mod tests {
    use crate::test_support::app::{call, eventually, AppHarness};
    use axum::body::Body;
    use axum::http::{header, Request, StatusCode};
    use futures_util::SinkExt;
    use serde_json::json;
    use svrctlrs_database::models::{CreateUser, CredentialType};
    use tokio_tungstenite::tungstenite::Message as WsMessage;
    use tower::ServiceExt;
    use tower_sessions::SessionStore;
    use tower_sessions_sqlx_store::SqliteStore;

    const FIREFOX_ANDROID: &str =
        "Mozilla/5.0 (Android 14; Mobile; rv:131.0) Gecko/131.0 Firefox/131.0";

    /// A password sign-in from `user_agent`: (session id, the `Set-Cookie` cookie pair)
    async fn sign_in(h: &AppHarness, username: &str, user_agent: &str) -> (String, String) {
        let response = h
            .app
            .clone()
            .oneshot(
                Request::post("/api/v1/auth/login")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::USER_AGENT, user_agent)
                    .header("x-forwarded-for", "203.0.113.7")
                    .body(Body::from(
                        json!({ "username": username, "password": "correct horse battery" })
                            .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let cookie = response.headers()[header::SET_COOKIE]
            .to_str()
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .to_string();
        let bytes = axum::body::to_bytes(response.into_body(), 1 << 16)
            .await
            .unwrap();
        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        (body["session_id"].as_str().unwrap().to_string(), cookie)
    }

    async fn with_cookie(h: &AppHarness, cookie: &str, uri: &str) -> StatusCode {
        h.app
            .clone()
            .oneshot(
                Request::get(uri)
                    .header(header::COOKIE, cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap()
            .status()
    }

    async fn devices(h: &AppHarness, session: &str) -> Vec<serde_json::Value> {
        let (status, body) = call(
            &h.app,
            "GET",
            "/api/v1/users/me/devices",
            Some(session),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        body["devices"].as_array().unwrap().clone()
    }

    async fn audited(h: &AppHarness, action: &str) -> bool {
        let pool = h.state.pool.clone();
        eventually(|| async {
            svrctlrs_database::queries::audit::list(&pool, 50, None)
                .await
                .unwrap()
                .iter()
                .any(|e| e.action == action)
        })
        .await
    }

    #[tokio::test]
    async fn sign_ins_are_listed_per_account_and_signed_out_one_by_one() {
        let h = AppHarness::new().await;
        let (phone, phone_cookie) = sign_in(&h, "ann", FIREFOX_ANDROID).await;

        // The owner's own sign-in comes first and says so
        let list = devices(&h, &h.session).await;
        assert_eq!(list.len(), 2);
        assert_eq!(list[0]["current"], true);
        assert_eq!(list[1]["current"], false);
        assert_eq!(list[1]["label"], "Firefox on Android");
        assert_eq!(list[1]["method"], "password");
        assert_eq!(list[1]["last_ip"], "203.0.113.7");
        let phone_id = list[1]["id"].as_i64().unwrap();
        assert_eq!(
            with_cookie(&h, &phone_cookie, "/api/v1/users/me/devices").await,
            StatusCode::OK
        );

        // Another account sees only its own, and cannot end the owner's
        svrctlrs_database::queries::users::create_user(
            &h.state.pool,
            &CreateUser {
                username: "bob".into(),
                password: "correct horse battery".into(),
            },
        )
        .await
        .unwrap();
        let (bob, _) = sign_in(&h, "bob", "curl/8.9.1").await;
        let bobs = devices(&h, &bob).await;
        assert_eq!(bobs.len(), 1);
        assert_eq!(bobs[0]["label"], "curl");
        let (status, _) = call(
            &h.app,
            "DELETE",
            &format!("/api/v1/users/me/devices/{phone_id}"),
            Some(&bob),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);

        // The lost phone is signed out: its cookie and its token stop working at once
        let (status, body) = h
            .owner(
                "DELETE",
                &format!("/api/v1/users/me/devices/{phone_id}"),
                None,
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(
            with_cookie(&h, &phone_cookie, "/api/v1/users/me/devices").await,
            StatusCode::UNAUTHORIZED
        );
        let (status, _) = call(&h.app, "GET", "/api/v1/servers", Some(&phone), None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(devices(&h, &h.session).await.len(), 1);
        assert!(audited(&h, "device.signed_out").await);

        // Sign out the others: everything but this device ends, other accounts untouched
        let (laptop, _) = sign_in(&h, "ann", "Mozilla/5.0 (X11; Linux x86_64) Firefox/131.0").await;
        let (tablet, _) = sign_in(
            &h,
            "ann",
            "Mozilla/5.0 (iPad; CPU OS 18_0 like Mac OS X) Safari/604.1",
        )
        .await;
        let (status, body) = h
            .owner("POST", "/api/v1/users/me/devices/sign-out-others", None)
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["signed_out"], 2);
        for gone in [&laptop, &tablet] {
            let (status, _) = call(&h.app, "GET", "/api/v1/servers", Some(gone), None).await;
            assert_eq!(status, StatusCode::UNAUTHORIZED);
        }
        assert_eq!(devices(&h, &h.session).await.len(), 1);
        assert_eq!(devices(&h, &bob).await.len(), 1);
        assert!(audited(&h, "device.signed_out_others").await);

        // Signing out this device is signing out
        let me = devices(&h, &h.session).await[0]["id"].as_i64().unwrap();
        let (status, _) = h
            .owner("DELETE", &format!("/api/v1/users/me/devices/{me}"), None)
            .await;
        assert_eq!(status, StatusCode::OK);
        let (status, _) = h.owner("GET", "/api/v1/servers", None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn logging_out_forgets_the_device() {
        let h = AppHarness::new().await;
        let (phone, _) = sign_in(&h, "ann", FIREFOX_ANDROID).await;
        let response = h
            .app
            .clone()
            .oneshot(
                Request::post("/auth/logout")
                    .header(header::AUTHORIZATION, format!("Bearer {phone}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert_eq!(devices(&h, &h.session).await.len(), 1);
    }

    #[tokio::test]
    async fn a_session_written_back_after_its_device_was_signed_out_is_refused() {
        let h = AppHarness::new().await;
        let (phone, _) = sign_in(&h, "ann", FIREFOX_ANDROID).await;
        let store = SqliteStore::new(h.state.pool.clone());
        let record = store.load(&phone.parse().unwrap()).await.unwrap().unwrap();
        let phone_id = devices(&h, &h.session).await[1]["id"].as_i64().unwrap();
        let (status, _) = h
            .owner(
                "DELETE",
                &format!("/api/v1/users/me/devices/{phone_id}"),
                None,
            )
            .await;
        assert_eq!(status, StatusCode::OK);

        // A request that was still running saves the session it loaded before the sign-out
        store.save(&record).await.unwrap();
        let (status, _) = call(&h.app, "GET", "/api/v1/servers", Some(&phone), None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        // and the written-back session is gone again
        assert!(store.load(&record.id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn a_sign_in_from_before_the_device_list_is_recorded_on_its_next_use() {
        // With password sign-in off, the harness writes the owner's session into the store
        // directly, as it was before devices existed
        let provider = crate::test_support::oidc_provider::FakeProvider::start().await;
        let h = AppHarness::with_sign_in(crate::sign_in::SignInConfig::with_provider(
            provider.config(&[]),
            false,
        ))
        .await;
        let list = devices(&h, &h.session).await;
        assert_eq!(list.len(), 1);
        assert_eq!(list[0]["current"], true);
        assert_eq!(list[0]["method"], "oidc");
        // and it is the same device the next time
        assert_eq!(devices(&h, &h.session).await[0]["id"], list[0]["id"]);
    }

    #[tokio::test]
    async fn last_use_is_written_once_a_minute_and_keeps_a_browser_session_alive() {
        let h = AppHarness::new().await;
        let (session, cookie) = sign_in(&h, "ann", FIREFOX_ANDROID).await;
        let pool = h.state.pool.clone();
        // The store keeps the expiry as text; compare it as a day number
        let expiry = || async {
            svrctlrs_database::sqlx::query_scalar::<_, f64>(
                "SELECT julianday(expiry_date) FROM tower_sessions WHERE id = ?",
            )
            .bind(&session)
            .fetch_one(&pool)
            .await
            .unwrap()
        };
        let hash = crate::devices::session_hash(&session);
        let seen = || async {
            svrctlrs_database::sqlx::query_scalar::<_, String>(
                "SELECT last_seen_at FROM devices WHERE session_hash = ?",
            )
            .bind(&hash)
            .fetch_one(&pool)
            .await
            .unwrap()
        };

        // Used within the minute: nothing is written, not even the session
        let (first_seen, first_expiry) = (seen().await, expiry().await);
        assert_eq!(
            with_cookie(&h, &cookie, "/api/v1/servers").await,
            StatusCode::OK
        );
        assert_eq!(expiry().await, first_expiry);
        assert_eq!(seen().await, first_seen);

        // Used again two minutes later: last use is written, and the session is saved again,
        // so its 24 hours start over
        svrctlrs_database::sqlx::query(
            "UPDATE devices SET last_seen_at = datetime('now', '-2 minutes')",
        )
        .execute(&pool)
        .await
        .unwrap();
        let stale = seen().await;
        assert_eq!(
            with_cookie(&h, &cookie, "/api/v1/servers").await,
            StatusCode::OK
        );
        assert!(expiry().await > first_expiry, "the session was saved again");
        assert!(eventually(|| async { seen().await > stale }).await);
    }

    #[tokio::test]
    async fn devices_whose_session_ended_are_forgotten() {
        let h = AppHarness::new().await;
        let (phone, _) = sign_in(&h, "ann", FIREFOX_ANDROID).await;
        svrctlrs_database::sqlx::query("DELETE FROM tower_sessions WHERE id = ?")
            .bind(&phone)
            .execute(&h.state.pool)
            .await
            .unwrap();
        assert_eq!(crate::devices::prune(&h.state.pool).await.unwrap(), 1);
        assert_eq!(devices(&h, &h.session).await.len(), 1);
    }

    #[tokio::test]
    async fn no_token_lists_or_signs_out_devices() {
        let h = AppHarness::new().await;
        let me = devices(&h, &h.session).await[0]["id"].as_i64().unwrap();
        for preset in ["read_only", "servers_keys", "full_admin"] {
            let (_, token) = h.token(preset, preset).await;
            for (method, uri) in [
                ("GET", "/api/v1/users/me/devices".to_string()),
                ("DELETE", format!("/api/v1/users/me/devices/{me}")),
                (
                    "POST",
                    "/api/v1/users/me/devices/sign-out-others".to_string(),
                ),
            ] {
                let (status, body) = call(&h.app, method, &uri, Some(&token), None).await;
                assert_eq!(status, StatusCode::FORBIDDEN, "{preset} {method} {uri}");
                assert_eq!(body["error"]["code"], "TOKEN_SCOPE");
            }
        }
        assert_eq!(devices(&h, &h.session).await.len(), 1);
    }

    #[tokio::test]
    async fn signing_a_device_out_closes_its_terminal_and_keeps_its_shell() {
        use crate::routes::api::tokens::tests::next_matching;
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
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let served = h.app.clone();
        tokio::spawn(async move { axum::serve(listener, served.into_make_service()).await });

        // The phone (an app-style session token) opens a shell through a ticket
        let (phone, _) = sign_in(&h, "ann", FIREFOX_ANDROID).await;
        let ticket = || async {
            let (status, body) =
                call(&h.app, "POST", "/api/v1/auth/ws-ticket", Some(&phone), None).await;
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
        ws.send(WsMessage::text(shell.to_string())).await.unwrap();
        let refused = next_matching(&mut ws, |v| v["type"] == "error")
            .await
            .unwrap();
        let fingerprint = refused["details"]["fingerprint"].as_str().unwrap();
        let (status, _) = h
            .owner(
                "POST",
                &format!("/api/v1/servers/{server_id}/host-keys/trust"),
                Some(json!({ "fingerprint": fingerprint })),
            )
            .await;
        assert_eq!(status, StatusCode::OK);
        ws.send(WsMessage::text(shell.to_string())).await.unwrap();
        next_matching(&mut ws, |v| v["type"] == "connected")
            .await
            .expect("a shell");

        // Signed out from the desktop: the phone's terminal closes and says why
        let phone_id = devices(&h, &h.session).await[1]["id"].as_i64().unwrap();
        let (status, _) = h
            .owner(
                "DELETE",
                &format!("/api/v1/users/me/devices/{phone_id}"),
                None,
            )
            .await;
        assert_eq!(status, StatusCode::OK);
        let ended = next_matching(&mut ws, |v| v["type"] == "error")
            .await
            .expect("told why");
        assert_eq!(ended["code"], "device_signed_out");

        // The shell is the account's: it waits, detached, for another device
        let sessions = h.state.sessions.clone();
        assert!(
            eventually(|| async {
                let list = sessions.list_for_user(1).await;
                list.len() == 1 && list[0].attachments == 0
            })
            .await
        );

        // and the phone cannot come back with a ticket it already had
        let (status, _) = call(&h.app, "POST", "/api/v1/auth/ws-ticket", Some(&phone), None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }
}
