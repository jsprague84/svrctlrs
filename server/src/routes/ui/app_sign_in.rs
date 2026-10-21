//! Signing the desktop app in through the system browser (ROADMAP 22, RFC 8252 shape with
//! svrctlrs as the authority; `sign_in::app_codes`).
//!
//! 1. The app listens on a `127.0.0.1` port and opens `GET /auth/app?port=…&state=…&
//!    code_challenge=…&code_challenge_method=S256` in the system browser.
//! 2. The browser signs in however this server allows (provider, or password and TOTP); an open
//!    session skips that. `require_auth` remembers the page, so sign-in comes back here.
//! 3. The page asks "sign in the desktop app?". Approving (a same-origin POST) mints a one-time
//!    code and sends the browser to `http://127.0.0.1:<port>/callback?code=…&state=…`.
//! 4. The app checks `state` and trades code and PKCE verifier for a session token at
//!    `POST /api/v1/auth/app-token`, which it keeps in the OS keyring.
//!
//! The app never sees a password or the provider; a server without a provider works the same.

use axum::{
    extract::{Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::get,
    Form, Json, Router,
};
use serde::Deserialize;
use svrctlrs_api::auth::{AppTokenRequest, LoggedIn, LoginRefused};
use svrctlrs_database::{models::NewAuditEntry, queries::users};
use tower_sessions::{session::Id, SessionStore};
use tower_sessions_sqlx_store::SqliteStore;

use super::auth::{
    client_ip, html_escape, is_same_origin_request, render_auth_page, CurrentUser, AUTH_METHOD_KEY,
    USER_ID_KEY,
};
use crate::login_limiter::LoginLimiter;
use crate::sign_in::app_codes::valid_challenge;
use crate::state::AppState;

/// How long a desktop app stays signed in (no sliding expiry: after this it signs in through
/// the browser again, usually one click on a browser that is still signed in)
pub const APP_SESSION_DAYS: i64 = 30;

pub fn routes() -> Router<AppState> {
    Router::new().route("/auth/app", get(approve_page).post(approve))
}

#[derive(Debug, Deserialize)]
pub struct AppRequest {
    port: u16,
    state: String,
    code_challenge: String,
    #[serde(default)]
    code_challenge_method: Option<String>,
}

impl AppRequest {
    /// The request is well formed: a port the app could have bound, an opaque state, and an
    /// S256 challenge.
    fn check(&self) -> Result<(), &'static str> {
        if self.port < 1024 {
            return Err("The app's port is not one it could listen on.");
        }
        let state_ok = (16..=128).contains(&self.state.len())
            && self
                .state
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
        if !state_ok {
            return Err("The request from the app is malformed (state).");
        }
        if self.code_challenge_method.as_deref().unwrap_or("S256") != "S256"
            || !valid_challenge(&self.code_challenge)
        {
            return Err("The request from the app is malformed (code challenge).");
        }
        Ok(())
    }
}

fn page(status: StatusCode, subtitle: &str, error: Option<&str>, body: &str) -> Response {
    (
        status,
        [(header::CACHE_CONTROL, "no-store")],
        Html(render_auth_page(subtitle, error, body)),
    )
        .into_response()
}

/// "Sign in the desktop app?" for the signed-in user
async fn approve_page(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
    Query(request): Query<AppRequest>,
) -> Response {
    if let Err(problem) = request.check() {
        return page(StatusCode::BAD_REQUEST, "Desktop app", Some(problem), "");
    }
    let username = users::get_user(&state.pool, me)
        .await
        .ok()
        .flatten()
        .map(|u| u.username)
        .unwrap_or_default();
    let body = format!(
        r#"<p style="margin-bottom:1.25rem;font-size:0.875rem;line-height:1.5">The SvrCtlRS desktop app on this computer asks to sign in as <strong>{user}</strong>. It gets a session of its own for {days} days, with a shell on every server, like this browser.</p>
<p style="margin-bottom:1.5rem;font-size:0.8125rem;color:#787fa0">Only approve if you just chose "Sign in" in the app.</p>
<form method="post" action="/auth/app">
<input type="hidden" name="port" value="{port}">
<input type="hidden" name="state" value="{state}">
<input type="hidden" name="code_challenge" value="{challenge}">
<input type="hidden" name="code_challenge_method" value="S256">
<button type="submit">Sign in the desktop app</button>
</form>
<p class="links"><a href="/">Cancel</a></p>"#,
        user = html_escape(&username),
        days = APP_SESSION_DAYS,
        port = request.port,
        state = html_escape(&request.state),
        challenge = html_escape(&request.code_challenge),
    );
    page(StatusCode::OK, "Desktop app", None, &body)
}

/// Approved: a one-time code, handed to the app's loopback port
async fn approve(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
    headers: HeaderMap,
    Form(request): Form<AppRequest>,
) -> Response {
    if !is_same_origin_request(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    if let Err(problem) = request.check() {
        return page(StatusCode::BAD_REQUEST, "Desktop app", Some(problem), "");
    }
    let Some(code) = state.app_codes.issue(me, &request.code_challenge) else {
        return page(
            StatusCode::SERVICE_UNAVAILABLE,
            "Desktop app",
            Some("Too many sign-ins are waiting; try again in a minute."),
            "",
        );
    };
    crate::audit::record(
        &state.pool,
        NewAuditEntry::new("app.sign_in_approved")
            .user(me)
            .client_ip(Some(client_ip(&headers))),
    );
    let target = format!(
        "http://127.0.0.1:{}/callback?code={code}&state={}",
        request.port, request.state
    );
    (
        StatusCode::SEE_OTHER,
        [
            (header::LOCATION, target),
            (header::CACHE_CONTROL, "no-store".to_string()),
        ],
    )
        .into_response()
}

fn refused(status: StatusCode, code: &str, message: &str) -> Response {
    (
        status,
        Json(LoginRefused {
            error: message.to_string(),
            code: Some(code.to_string()),
        }),
    )
        .into_response()
}

/// Trade the one-time code from the browser, and the PKCE verifier only the app holds, for a
/// session token (`Authorization: Bearer <session_id>`, 30 days). The token is the app's; the
/// browser's session is not touched.
#[utoipa::path(post, path = "/api/v1/auth/app-token", tag = "auth", security(()),
    request_body = AppTokenRequest,
    responses(
        (status = 200, description = "Signed in; `session_id` is the app's Bearer token", body = LoggedIn),
        (status = 400, description = "Unknown, used or expired code, or the wrong verifier (`APP_CODE_INVALID`)", body = LoginRefused),
        (status = 429, description = "Too many failures from this address", body = LoginRefused),
    ))]
pub(crate) async fn app_token(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<AppTokenRequest>,
) -> Response {
    let ip = client_ip(&headers);
    let ip_key = [LoginLimiter::keys(&ip, "")[0].clone()];
    match state.login_limiter.check(&ip_key).await {
        Ok(None) => {}
        Ok(Some(_)) => {
            return refused(
                StatusCode::TOO_MANY_REQUESTS,
                "RATE_LIMITED",
                "Too many failed attempts; try again later",
            )
        }
        Err(e) => {
            tracing::error!(error = %e, "Login limiter unavailable");
            return refused(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL",
                "Internal error",
            );
        }
    }
    let Some(user_id) = state.app_codes.redeem(&input.code, &input.code_verifier) else {
        if let Err(e) = state.login_limiter.record_failure(&ip_key).await {
            tracing::error!(error = %e, "Failed to record a login failure");
        }
        return refused(
            StatusCode::BAD_REQUEST,
            "APP_CODE_INVALID",
            "That sign-in code is unknown, used or expired; sign in again from the app",
        );
    };
    let username = match users::get_user(&state.pool, user_id).await {
        Ok(Some(u)) => u.username,
        _ => {
            return refused(
                StatusCode::BAD_REQUEST,
                "APP_CODE_INVALID",
                "That account no longer exists",
            )
        }
    };

    let store = SqliteStore::new(state.pool.clone());
    let mut record = tower_sessions::session::Record {
        id: Id::default(),
        data: std::collections::HashMap::from([
            (USER_ID_KEY.to_string(), serde_json::json!(user_id)),
            (AUTH_METHOD_KEY.to_string(), serde_json::json!("app")),
        ]),
        expiry_date: time::OffsetDateTime::now_utc() + time::Duration::days(APP_SESSION_DAYS),
    };
    if let Err(e) = store.create(&mut record).await {
        tracing::error!(error = %e, "Failed to create the app's session");
        return refused(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL",
            "Internal error",
        );
    }
    crate::audit::record(
        &state.pool,
        NewAuditEntry::new("login")
            .user(user_id)
            .username(&username)
            .client_ip(Some(ip))
            .details(serde_json::json!({ "method": "app" })),
    );
    tracing::info!(user_id, "Desktop app signed in");
    Json(LoggedIn {
        success: true,
        user_id,
        session_id: record.id.to_string(),
    })
    .into_response()
}

#[cfg(test)]
mod tests {
    use crate::sign_in::SignInConfig;
    use crate::test_support::app::{call, AppHarness};
    use crate::test_support::oidc_provider::{query_of, FakeProvider};
    use axum::body::{to_bytes, Body};
    use axum::http::{header, Request, StatusCode};
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;
    use serde_json::json;
    use sha2::{Digest, Sha256};
    use tower::ServiceExt;

    const VERIFIER: &str = "the-desktop-app-keeps-this-verifier-to-itself-0123";
    const STATE: &str = "app-state-0123456789abcdef";

    fn app_url(port: u16) -> String {
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(VERIFIER.as_bytes()));
        format!(
            "/auth/app?port={port}&state={STATE}&code_challenge={challenge}&code_challenge_method=S256"
        )
    }

    struct Answer {
        status: StatusCode,
        location: Option<String>,
        cookie: Option<String>,
        body: String,
    }

    async fn send(h: &AppHarness, request: Request<Body>) -> Answer {
        let response = h.app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let location = response
            .headers()
            .get(header::LOCATION)
            .map(|v| v.to_str().unwrap().to_string());
        let cookie = response
            .headers()
            .get_all(header::SET_COOKIE)
            .iter()
            .map(|v| v.to_str().unwrap().split(';').next().unwrap().to_string())
            .find(|c| c.starts_with("id="));
        let body = to_bytes(response.into_body(), 1 << 20).await.unwrap();
        Answer {
            status,
            location,
            cookie,
            body: String::from_utf8_lossy(&body).into_owned(),
        }
    }

    fn get(uri: &str, cookie: Option<&str>) -> Request<Body> {
        let mut r = Request::get(uri);
        if let Some(c) = cookie {
            r = r.header(header::COOKIE, c);
        }
        r.body(Body::empty()).unwrap()
    }

    fn form(uri: &str, cookie: Option<&str>, body: &str, site: &str) -> Request<Body> {
        let mut r = Request::post(uri)
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .header("sec-fetch-site", site);
        if let Some(c) = cookie {
            r = r.header(header::COOKIE, c);
        }
        r.body(Body::from(body.to_string())).unwrap()
    }

    /// The approval form's fields, as the browser would post them
    fn approval_form(port: u16) -> String {
        let q = query_of(&format!("http://x{}", app_url(port)));
        format!(
            "port={port}&state={}&code_challenge={}&code_challenge_method=S256",
            q["state"], q["code_challenge"]
        )
    }

    /// Approve as the signed-in browser `cookie`; the code the app's port receives.
    async fn approve(h: &AppHarness, cookie: &str, port: u16) -> String {
        let page = send(h, get(&app_url(port), Some(cookie))).await;
        assert_eq!(page.status, StatusCode::OK, "{}", page.body);
        assert!(
            page.body.contains("Sign in the desktop app"),
            "{}",
            page.body
        );
        assert!(page.body.contains("<strong>ann</strong>"));
        let approved = send(
            h,
            form(
                "/auth/app",
                Some(cookie),
                &approval_form(port),
                "same-origin",
            ),
        )
        .await;
        assert_eq!(approved.status, StatusCode::SEE_OTHER, "{}", approved.body);
        let to = approved.location.unwrap();
        assert!(
            to.starts_with(&format!("http://127.0.0.1:{port}/callback?")),
            "{to}"
        );
        let q = query_of(&to);
        assert_eq!(q["state"], STATE);
        q["code"].clone()
    }

    async fn trade(h: &AppHarness, code: &str, verifier: &str) -> (StatusCode, serde_json::Value) {
        call(
            &h.app,
            "POST",
            "/api/v1/auth/app-token",
            None,
            Some(json!({ "code": code, "code_verifier": verifier })),
        )
        .await
    }

    async fn password_sign_in(h: &AppHarness, cookie: Option<&str>) -> Answer {
        send(
            h,
            form(
                "/auth/login",
                cookie,
                "username=ann&password=correct+horse+battery",
                "same-origin",
            ),
        )
        .await
    }

    #[tokio::test]
    async fn a_signed_out_browser_signs_in_and_comes_back_to_approve_the_app() {
        let h = AppHarness::new().await;
        // Signed out: to the login page, remembering where it was going
        let first = send(&h, get(&app_url(41234), None)).await;
        assert_eq!(first.status, StatusCode::SEE_OTHER);
        assert_eq!(first.location.as_deref(), Some("/auth/login"));
        let pre = first.cookie.expect("the session that remembers the page");
        let signed_in = password_sign_in(&h, Some(&pre)).await;
        assert_eq!(signed_in.status, StatusCode::SEE_OTHER);
        assert_eq!(signed_in.location.as_deref(), Some(app_url(41234).as_str()));
        let browser = signed_in.cookie.expect("a fresh session id");

        let code = approve(&h, &browser, 41234).await;
        let (status, body) = trade(&h, &code, VERIFIER).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let token = body["session_id"].as_str().unwrap().to_string();
        assert_ne!(
            format!("id={token}"),
            browser,
            "the app gets a session of its own"
        );

        // The app's token works as a Bearer token, for 30 days
        let (status, _) = h.with(&token, "GET", "/api/v1/servers", None).await;
        assert_eq!(status, StatusCode::OK);
        // A code works once
        let (status, body) = trade(&h, &code, VERIFIER).await;
        assert_eq!(
            (status, body["code"].as_str()),
            (StatusCode::BAD_REQUEST, Some("APP_CODE_INVALID"))
        );
        // The app signs out with its token: that session ends, the browser's stays
        let out = send(
            &h,
            Request::post("/auth/logout")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header("sec-fetch-site", "cross-site")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(out.status, StatusCode::NO_CONTENT);
        let (status, _) = h.with(&token, "GET", "/api/v1/servers", None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        let still = send(&h, get("/api/v1/servers", Some(&browser))).await;
        assert_eq!(still.status, StatusCode::OK);

        // A later sign-in goes to the app, not back to the old page
        let again = password_sign_in(&h, None).await;
        assert_eq!(again.location.as_deref(), Some("/"));
    }

    #[tokio::test]
    async fn a_code_without_the_verifier_is_worthless() {
        let h = AppHarness::new().await;
        let browser = password_sign_in(&h, None).await.cookie.unwrap();
        let code = approve(&h, &browser, 41235).await;
        let (status, body) = trade(&h, &code, "a program that only saw the redirect").await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        // And it is used up by the attempt
        let (status, _) = trade(&h, &code, VERIFIER).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn approval_needs_a_same_origin_post_and_a_well_formed_request() {
        let h = AppHarness::new().await;
        let browser = password_sign_in(&h, None).await.cookie.unwrap();
        let cross = send(
            &h,
            form(
                "/auth/app",
                Some(&browser),
                &approval_form(41236),
                "cross-site",
            ),
        )
        .await;
        assert_eq!(cross.status, StatusCode::FORBIDDEN);
        for bad in [
            app_url(80),
            app_url(41236).replace(STATE, "short"),
            app_url(41236).replace("S256", "plain"),
            "/auth/app?port=41236&state=app-state-0123456789abcdef&code_challenge=abc".to_string(),
        ] {
            let page = send(&h, get(&bad, Some(&browser))).await;
            assert_eq!(page.status, StatusCode::BAD_REQUEST, "{bad}");
        }
        // Signed out, the POST goes nowhere
        let anonymous = send(
            &h,
            form("/auth/app", None, &approval_form(41236), "same-origin"),
        )
        .await;
        assert_eq!(anonymous.status, StatusCode::SEE_OTHER);
        assert_eq!(anonymous.location.as_deref(), Some("/auth/login"));
    }

    #[tokio::test]
    async fn wrong_codes_count_against_the_limiter() {
        let h = AppHarness::new().await;
        for _ in 0..crate::login_limiter::LoginLimiter::DEFAULT_MAX_FAILURES {
            let (status, _) = trade(&h, "guess", VERIFIER).await;
            assert_eq!(status, StatusCode::BAD_REQUEST);
        }
        let (status, _) = trade(&h, "guess", VERIFIER).await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    }

    /// With the provider as the only way in, the browser's trip through it still ends on the
    /// app's approval page.
    #[tokio::test]
    async fn the_provider_sign_in_comes_back_to_the_app_too() {
        let provider = FakeProvider::start().await;
        let h = AppHarness::with_sign_in(SignInConfig::with_provider(provider.config(&[]), false))
            .await;
        // Link the owner's identity (as in Settings)
        let bearer = format!("Bearer {}", h.session);
        let link = h
            .app
            .clone()
            .oneshot(
                Request::get("/auth/oidc/link")
                    .header(header::AUTHORIZATION, &bearer)
                    .header("sec-fetch-site", "same-origin")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let authorize = link.headers()[header::LOCATION]
            .to_str()
            .unwrap()
            .to_string();
        let cookie = link.headers()[header::SET_COOKIE]
            .to_str()
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .to_string();
        let code = provider.approve(&authorize, "ann-at-id", json!({}));
        let linked = send(
            &h,
            get(
                &format!(
                    "/auth/oidc/callback?code={code}&state={}",
                    query_of(&authorize)["state"]
                ),
                Some(&cookie),
            ),
        )
        .await;
        assert_eq!(linked.status, StatusCode::OK, "{}", linked.body);

        // Signed out, the app's page → login → provider → back to the app's page
        let first = send(&h, get(&app_url(41237), None)).await;
        let pre = first.cookie.unwrap();
        let login = send(&h, get("/auth/login", Some(&pre))).await;
        assert_eq!(login.location.as_deref(), Some("/auth/oidc/start"));
        let start = h
            .app
            .clone()
            .oneshot(get("/auth/oidc/start", Some(&pre)))
            .await
            .unwrap();
        let authorize = start.headers()[header::LOCATION]
            .to_str()
            .unwrap()
            .to_string();
        let flow = start.headers()[header::SET_COOKIE]
            .to_str()
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .to_string();
        let code = provider.approve(&authorize, "ann-at-id", json!({}));
        let back = send(
            &h,
            get(
                &format!(
                    "/auth/oidc/callback?code={code}&state={}",
                    query_of(&authorize)["state"]
                ),
                Some(&flow),
            ),
        )
        .await;
        assert_eq!(back.status, StatusCode::OK, "{}", back.body);
        assert!(back.body.contains("/auth/app?port=41237"), "{}", back.body);
    }
}
