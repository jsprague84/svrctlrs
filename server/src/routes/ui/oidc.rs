//! Browser routes for signing in with the OpenID Connect provider (`sign_in::oidc`):
//!
//! - `GET /auth/oidc/start`: sign in (no session needed);
//! - `GET /auth/oidc/link`: link the provider identity to the signed-in account (a session,
//!   and a same-origin navigation from the app);
//! - `GET /auth/oidc/callback`: where the provider sends the browser back.
//!
//! The provider redirects back cross-site, so the session cookie (`SameSite=Strict`) does not
//! come along: the flow is found by `state` and bound to this browser by its own short-lived
//! `SameSite=Lax` cookie, and a link remembers the account it was started for. After signing
//! in, the answer is a small page that moves on to the app, because a redirect at the end of a
//! cross-site chain would still be cross-site and the new session cookie would be held back.

use axum::{
    extract::{Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
    routing::get,
    Router,
};
use serde::Deserialize;
use tower_sessions::Session;

use super::auth::{
    client_ip, html_escape, is_same_origin_request, login_html, start_session, CurrentUser,
};
use crate::login_limiter::LoginLimiter;
use crate::sign_in::oidc::{OidcError, Purpose};
use crate::state::AppState;
use svrctlrs_database::{
    models::{NewAuditEntry, NewUserIdentity},
    queries::{user_identities, users},
};

const FLOW_COOKIE: &str = "svrctlrs_oidc";

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/auth/oidc/start", get(start))
        .route("/auth/oidc/link", get(link))
        .route("/auth/oidc/callback", get(callback))
}

fn secure(state: &AppState) -> bool {
    state
        .sign_in
        .provider()
        .is_some_and(|p| p.config.redirect_uri.starts_with("https://"))
}

fn flow_cookie(state: &AppState, value: &str, max_age: u64) -> String {
    format!(
        "{FLOW_COOKIE}={value}; Path=/auth/oidc; Max-Age={max_age}; HttpOnly; SameSite=Lax{}",
        if secure(state) { "; Secure" } else { "" }
    )
}

fn read_flow_cookie(headers: &HeaderMap) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(name, _)| *name == FLOW_COOKIE)
        .map(|(_, value)| value.to_string())
}

/// The login page with an error, as the answer to a failed step
fn login_error(state: &AppState, status: StatusCode, message: &str) -> Response {
    let cleared = flow_cookie(state, "", 0);
    (
        status,
        [(header::SET_COOKIE, cleared)],
        Html(login_html(&state.sign_in, Some(message))),
    )
        .into_response()
}

/// Send the browser to the provider for `purpose`
async fn begin(state: &AppState, headers: &HeaderMap, purpose: Purpose) -> Response {
    let Some(provider) = state.sign_in.provider() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    // A browser that keeps failing callbacks is limited like one that keeps failing passwords
    let ip_key = [LoginLimiter::keys(&client_ip(headers), "")[0].clone()];
    match state.login_limiter.check(&ip_key).await {
        Ok(None) => {}
        Ok(Some(limited)) => {
            let minutes = limited.retry_after.as_secs().div_ceil(60).max(1);
            return login_error(
                state,
                StatusCode::TOO_MANY_REQUESTS,
                &format!("Too many failed attempts. Try again in {minutes} minute(s)."),
            );
        }
        Err(e) => {
            tracing::error!(error = %e, "Login limiter unavailable");
            return login_error(
                state,
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal server error",
            );
        }
    }
    match provider.begin(purpose).await {
        Ok((url, binding)) => (
            StatusCode::SEE_OTHER,
            [
                (header::LOCATION, url),
                (header::SET_COOKIE, flow_cookie(state, &binding, 600)),
                (header::CACHE_CONTROL, "no-store".to_string()),
            ],
        )
            .into_response(),
        Err(e) => {
            tracing::warn!(error = %e, "Could not start an OpenID Connect sign-in");
            let status = match e {
                OidcError::Busy => StatusCode::SERVICE_UNAVAILABLE,
                _ => StatusCode::BAD_GATEWAY,
            };
            login_error(
                state,
                status,
                &format!("{} is not available right now: {e}", provider.config.name),
            )
        }
    }
}

async fn start(State(state): State<AppState>, headers: HeaderMap) -> Response {
    begin(&state, &headers, Purpose::Login).await
}

/// Linking is started from the app (Settings → Access → Sign-in), never from another site.
async fn link(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
    headers: HeaderMap,
) -> Response {
    if !is_same_origin_request(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    begin(&state, &headers, Purpose::Link { user_id: me }).await
}

#[derive(Debug, Deserialize)]
struct CallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
    iss: Option<String>,
}

/// A page that moves on to `to` from this origin (see the module comment).
fn continue_page(to: &str) -> Response {
    let to = html_escape(to);
    let page = super::auth::render_auth_page(
        "Signed in",
        None,
        &format!(
            r#"<meta http-equiv="refresh" content="0;url={to}"><a class="button" href="{to}">Continue</a>"#
        ),
    );
    (
        StatusCode::OK,
        [(header::CACHE_CONTROL, "no-store")],
        Html(page),
    )
        .into_response()
}

async fn callback(
    State(state): State<AppState>,
    headers: HeaderMap,
    session: Session,
    Query(query): Query<CallbackQuery>,
) -> Response {
    let Some(provider) = state.sign_in.provider() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let ip = client_ip(&headers);
    let ip_key = [LoginLimiter::keys(&ip, "")[0].clone()];
    let failed = |message: String| {
        let state = state.clone();
        let ip_key = ip_key.clone();
        async move {
            if let Err(e) = state.login_limiter.record_failure(&ip_key).await {
                tracing::error!(error = %e, "Failed to record a login failure");
            }
            login_error(&state, StatusCode::BAD_REQUEST, &message)
        }
    };

    let Some(flow_state) = query.state.as_deref() else {
        return failed("That sign-in link is incomplete. Sign in again.".into()).await;
    };
    let flow = match provider.take_flow(flow_state, read_flow_cookie(&headers).as_deref()) {
        Ok(flow) => flow,
        Err(e) => {
            tracing::warn!(ip = %ip, "OpenID Connect callback without a matching sign-in");
            return failed(format!("Sign in again: {e}.")).await;
        }
    };
    if let Some(error) = query.error.as_deref() {
        // The user cancelled or the provider refused; not an attack, so not counted
        let description = query.error_description.as_deref().unwrap_or(error);
        return login_error(
            &state,
            StatusCode::OK,
            &format!(
                "{} did not sign you in: {description}",
                provider.config.name
            ),
        );
    }
    let Some(code) = query.code.as_deref() else {
        return failed("That sign-in link is incomplete. Sign in again.".into()).await;
    };

    let identity = match provider.finish(&flow, code, query.iss.as_deref()).await {
        Ok(identity) => identity,
        Err(OidcError::NotInGroup) => {
            crate::audit::record(
                &state.pool,
                NewAuditEntry::new("login.oidc_refused")
                    .client_ip(Some(ip.clone()))
                    .details(serde_json::json!({ "reason": "group" })),
            );
            return login_error(
                &state,
                StatusCode::FORBIDDEN,
                &format!(
                    "Your {} account is not in a group allowed to use SvrCtlRS.",
                    provider.config.name
                ),
            );
        }
        Err(e) => {
            tracing::warn!(error = %e, "OpenID Connect sign-in failed");
            return failed(format!(
                "Sign-in with {} failed: {e}.",
                provider.config.name
            ))
            .await;
        }
    };

    match flow.purpose {
        Purpose::Link { user_id } => link_identity(&state, &headers, user_id, identity).await,
        Purpose::Login => login_identity(&state, &session, &ip, identity).await,
    }
}

async fn link_identity(
    state: &AppState,
    headers: &HeaderMap,
    user_id: i64,
    identity: crate::sign_in::oidc::Identity,
) -> Response {
    let linked = user_identities::link(
        &state.pool,
        &NewUserIdentity {
            user_id,
            issuer: identity.issuer.clone(),
            subject: identity.subject.clone(),
            email: identity.email.clone(),
            display_name: identity.name.clone(),
        },
    )
    .await;
    match linked {
        Ok(_) => {
            crate::audit::record(
                &state.pool,
                NewAuditEntry::new("user.identity_linked")
                    .user(user_id)
                    .client_ip(Some(client_ip(headers)))
                    .details(serde_json::json!({
                        "issuer": identity.issuer,
                        "email": identity.email,
                    })),
            );
            let mut response = continue_page("/settings?tab=access&linked=1");
            if let Ok(v) = flow_cookie(state, "", 0).parse() {
                response.headers_mut().insert(header::SET_COOKIE, v);
            }
            response
        }
        Err(svrctlrs_core::Error::Conflict(_)) => login_error(
            state,
            StatusCode::CONFLICT,
            "That identity is already linked to an account.",
        ),
        Err(e) => {
            tracing::error!(error = %e, "Failed to link an identity");
            login_error(
                state,
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal server error",
            )
        }
    }
}

async fn login_identity(
    state: &AppState,
    session: &Session,
    ip: &str,
    identity: crate::sign_in::oidc::Identity,
) -> Response {
    let found = match user_identities::find(&state.pool, &identity.issuer, &identity.subject).await
    {
        Ok(found) => found,
        Err(e) => {
            tracing::error!(error = %e, "Failed to look up an identity");
            return login_error(
                state,
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal server error",
            );
        }
    };
    let user = match &found {
        Some(linked) => users::get_user(&state.pool, linked.user_id)
            .await
            .ok()
            .flatten(),
        None => None,
    };
    let (Some(linked), Some(user)) = (found, user) else {
        crate::audit::record(
            &state.pool,
            NewAuditEntry::new("login.oidc_unlinked")
                .client_ip(Some(ip.to_string()))
                .details(serde_json::json!({
                    "issuer": identity.issuer,
                    "subject": identity.subject,
                    "email": identity.email,
                })),
        );
        let how = if state.sign_in.password_login {
            "Sign in with your password once and link it under Settings → Access → Sign-in."
        } else {
            "An administrator has to link it under Settings → Access → Sign-in."
        };
        let who = identity
            .email
            .as_deref()
            .or(identity.name.as_deref())
            .unwrap_or("That account");
        return login_error(
            state,
            StatusCode::FORBIDDEN,
            &format!("{who} is not linked to a SvrCtlRS account. {how}"),
        );
    };
    if let Err(e) = user_identities::touch(
        &state.pool,
        linked.id,
        identity.email.as_deref(),
        identity.name.as_deref(),
    )
    .await
    {
        tracing::warn!(error = %e, "Failed to record an identity's use");
    }
    if start_session(state, session, user.id, &user.username, ip, "oidc")
        .await
        .is_err()
    {
        return login_error(
            state,
            StatusCode::INTERNAL_SERVER_ERROR,
            "Internal server error",
        );
    }
    let mut response = continue_page("/");
    if let Ok(v) = flow_cookie(state, "", 0).parse() {
        response.headers_mut().insert(header::SET_COOKIE, v);
    }
    response
}

#[derive(Debug, Deserialize)]
pub struct LoginPageQuery {
    #[serde(default)]
    pub signed_out: Option<String>,
}

/// With password sign-in off and a provider, the login page goes straight to the provider,
/// except right after signing out (or the provider's own session would sign you back in).
pub fn auto_redirect(state: &AppState, query: &LoginPageQuery) -> Option<Response> {
    (!state.sign_in.password_login
        && state.sign_in.provider().is_some()
        && query.signed_out.is_none())
    .then(|| Redirect::to("/auth/oidc/start").into_response())
}

#[cfg(test)]
mod tests {
    use crate::sign_in::SignInConfig;
    use crate::test_support::app::AppHarness;
    use crate::test_support::oidc_provider::FakeProvider;
    use axum::body::{to_bytes, Body};
    use axum::http::{header, Request, StatusCode};
    use serde_json::json;
    use tower::ServiceExt;

    struct Answer {
        status: StatusCode,
        location: Option<String>,
        cookies: Vec<String>,
        body: String,
    }

    impl Answer {
        fn cookie(&self, name: &str) -> Option<String> {
            self.cookies
                .iter()
                .find(|c| c.starts_with(&format!("{name}=")))
                .map(|c| c.split(';').next().unwrap().to_string())
        }
    }

    async fn get(h: &AppHarness, uri: &str, headers: &[(&str, &str)]) -> Answer {
        let mut request = Request::builder().uri(uri);
        for (k, v) in headers {
            request = request.header(*k, *v);
        }
        let response = h
            .app
            .clone()
            .oneshot(request.body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let location = response
            .headers()
            .get(header::LOCATION)
            .map(|v| v.to_str().unwrap().to_string());
        let cookies = response
            .headers()
            .get_all(header::SET_COOKIE)
            .iter()
            .map(|v| v.to_str().unwrap().to_string())
            .collect();
        let body = to_bytes(response.into_body(), 1 << 20).await.unwrap();
        Answer {
            status,
            location,
            cookies,
            body: String::from_utf8_lossy(&body).into_owned(),
        }
    }

    async fn harness(provider: &FakeProvider, groups: &[&str], password_login: bool) -> AppHarness {
        AppHarness::with_sign_in(SignInConfig::with_provider(
            provider.config(groups),
            password_login,
        ))
        .await
    }

    /// Start at `start` (with `headers`), sign in at the provider as `subject`, and come back:
    /// the callback's answer.
    async fn round_trip(
        h: &AppHarness,
        provider: &FakeProvider,
        start: &str,
        headers: &[(&str, &str)],
        subject: &str,
        extra: serde_json::Value,
    ) -> Answer {
        let begun = get(h, start, headers).await;
        assert_eq!(begun.status, StatusCode::SEE_OTHER, "{}", begun.body);
        let authorize = begun.location.clone().unwrap();
        assert!(authorize.starts_with(&format!("{}/authorize?", provider.issuer)));
        let binding = begun.cookie("svrctlrs_oidc").expect("the flow cookie");
        assert!(begun.cookies[0].contains("SameSite=Lax"));
        let q = crate::test_support::oidc_provider::query_of(&authorize);
        let code = provider.approve(&authorize, subject, extra);
        get(
            h,
            &format!("/auth/oidc/callback?code={code}&state={}", q["state"]),
            &[("cookie", &binding)],
        )
        .await
    }

    async fn signed_in(h: &AppHarness, cookie: &str) -> bool {
        get(h, "/api/v1/sessions", &[("cookie", cookie)])
            .await
            .status
            == StatusCode::OK
    }

    async fn link_as_owner(h: &AppHarness, provider: &FakeProvider, subject: &str) {
        let bearer = format!("Bearer {}", h.session);
        let linked = round_trip(
            h,
            provider,
            "/auth/oidc/link",
            &[
                ("authorization", &bearer),
                ("sec-fetch-site", "same-origin"),
            ],
            subject,
            json!({}),
        )
        .await;
        assert_eq!(linked.status, StatusCode::OK, "{}", linked.body);
        assert!(
            linked.body.contains("/settings?tab=access"),
            "{}",
            linked.body
        );
    }

    #[tokio::test]
    async fn a_linked_identity_signs_in_and_an_unlinked_one_does_not() {
        let provider = FakeProvider::start().await;
        let h = harness(&provider, &[], true).await;

        // Not linked yet: refused, and nobody is signed in
        let answer = round_trip(
            &h,
            &provider,
            "/auth/oidc/start",
            &[],
            "ann-at-id",
            json!({}),
        )
        .await;
        assert_eq!(answer.status, StatusCode::FORBIDDEN, "{}", answer.body);
        assert!(answer.body.contains("is not linked"), "{}", answer.body);
        assert!(answer.cookie("id").is_none());

        link_as_owner(&h, &provider, "ann-at-id").await;
        let (_, methods) = h.owner("GET", "/api/v1/users/me/sign-in", None).await;
        assert_eq!(methods["identities"][0]["email"], "ann-at-id@example.com");
        assert_eq!(methods["provider"]["name"], "Test ID");

        let answer = round_trip(
            &h,
            &provider,
            "/auth/oidc/start",
            &[],
            "ann-at-id",
            json!({}),
        )
        .await;
        assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
        // A page that moves on, not a redirect (the Strict cookie would be held back)
        assert!(
            answer.body.contains(r#"http-equiv="refresh""#),
            "{}",
            answer.body
        );
        let session = answer.cookie("id").expect("a session cookie");
        assert!(signed_in(&h, &session).await);

        // Another subject at the same provider is someone else
        let other = round_trip(
            &h,
            &provider,
            "/auth/oidc/start",
            &[],
            "bob-at-id",
            json!({}),
        )
        .await;
        assert_eq!(other.status, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn a_callback_works_once_and_only_in_the_browser_that_started_it() {
        let provider = FakeProvider::start().await;
        let h = harness(&provider, &[], true).await;
        link_as_owner(&h, &provider, "ann-at-id").await;

        let begun = get(&h, "/auth/oidc/start", &[]).await;
        let authorize = begun.location.unwrap();
        let q = crate::test_support::oidc_provider::query_of(&authorize);
        let code = provider.approve(&authorize, "ann-at-id", json!({}));
        let callback = format!("/auth/oidc/callback?code={code}&state={}", q["state"]);

        // Another browser (no binding cookie, or a different one)
        let stolen = get(&h, &callback, &[("cookie", "svrctlrs_oidc=someone-else")]).await;
        assert_eq!(stolen.status, StatusCode::BAD_REQUEST, "{}", stolen.body);
        assert!(stolen.cookie("id").is_none());
        // The flow is used up by that attempt, so the right browser starts again
        let binding = begun.cookies[0].split(';').next().unwrap().to_string();
        let again = get(&h, &callback, &[("cookie", &binding)]).await;
        assert_eq!(again.status, StatusCode::BAD_REQUEST);
        // No state at all
        let bare = get(&h, &format!("/auth/oidc/callback?code={code}"), &[]).await;
        assert_eq!(bare.status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn a_token_that_fails_a_check_signs_nobody_in() {
        let provider = FakeProvider::start().await;
        let h = harness(&provider, &["owner"], true).await;
        link_as_owner(&h, &provider, "ann-at-id").await;
        for (what, extra, status) in [
            ("expired", json!({ "exp": 1_000 }), StatusCode::BAD_REQUEST),
            (
                "other client",
                json!({ "aud": "another-app" }),
                StatusCode::BAD_REQUEST,
            ),
            (
                "other issuer",
                json!({ "iss": "https://evil.example" }),
                StatusCode::BAD_REQUEST,
            ),
            (
                "wrong nonce",
                json!({ "nonce": "replayed" }),
                StatusCode::BAD_REQUEST,
            ),
            (
                "not in the group",
                json!({ "groups": ["family"] }),
                StatusCode::FORBIDDEN,
            ),
        ] {
            let answer =
                round_trip(&h, &provider, "/auth/oidc/start", &[], "ann-at-id", extra).await;
            assert_eq!(answer.status, status, "{what}: {}", answer.body);
            assert!(answer.cookie("id").is_none(), "{what}");
        }
    }

    #[tokio::test]
    async fn linking_needs_a_session_and_a_same_origin_navigation() {
        let provider = FakeProvider::start().await;
        let h = harness(&provider, &[], true).await;
        // Signed out: to the login page
        let answer = get(&h, "/auth/oidc/link", &[]).await;
        assert_eq!(answer.status, StatusCode::SEE_OTHER);
        assert_eq!(answer.location.as_deref(), Some("/auth/login"));
        // From another site
        let bearer = format!("Bearer {}", h.session);
        let answer = get(
            &h,
            "/auth/oidc/link",
            &[("authorization", &bearer), ("sec-fetch-site", "cross-site")],
        )
        .await;
        assert_eq!(answer.status, StatusCode::FORBIDDEN);
        // An identity linked to one account cannot be linked to another
        link_as_owner(&h, &provider, "ann-at-id").await;
        let bearer = format!("Bearer {}", h.session);
        let again = round_trip(
            &h,
            &provider,
            "/auth/oidc/link",
            &[
                ("authorization", &bearer),
                ("sec-fetch-site", "same-origin"),
            ],
            "ann-at-id",
            json!({}),
        )
        .await;
        assert_eq!(again.status, StatusCode::CONFLICT);
    }

    /// With the provider as the only way in: no password anywhere, the login page goes to the
    /// provider (except after signing out), the last identity stays — and API tokens work
    /// exactly as before.
    #[tokio::test]
    async fn password_sign_in_off_leaves_the_provider_and_tokens() {
        let provider = FakeProvider::start().await;
        let h = harness(&provider, &[], false).await;

        let (status, body) = crate::test_support::app::call(
            &h.app,
            "POST",
            "/api/v1/auth/login",
            None,
            Some(json!({ "username": "ann", "password": "correct horse battery" })),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
        assert_eq!(body["code"], "PASSWORD_LOGIN_OFF");

        let form = Request::builder()
            .method("POST")
            .uri("/auth/login")
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .header("sec-fetch-site", "same-origin")
            .body(Body::from("username=ann&password=correct+horse+battery"))
            .unwrap();
        let response = h.app.clone().oneshot(form).await.unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert!(response.headers().get(header::SET_COOKIE).is_none());

        let page = get(&h, "/auth/login", &[]).await;
        assert_eq!(page.status, StatusCode::SEE_OTHER);
        assert_eq!(page.location.as_deref(), Some("/auth/oidc/start"));
        let page = get(&h, "/auth/login?signed_out=1", &[]).await;
        assert_eq!(page.status, StatusCode::OK);
        assert!(page.body.contains("Sign in with Test ID"));
        assert!(!page.body.contains(r#"name="password""#), "{}", page.body);

        // The only identity cannot be unlinked
        link_as_owner(&h, &provider, "ann-at-id").await;
        let (_, methods) = h.owner("GET", "/api/v1/users/me/sign-in", None).await;
        assert_eq!(methods["password_login"], false);
        let id = methods["identities"][0]["id"].as_i64().unwrap();
        let (status, _) = h
            .owner("DELETE", &format!("/api/v1/users/me/identities/{id}"), None)
            .await;
        assert_eq!(status, StatusCode::CONFLICT);

        // Agents: a token reads, writes and is refused outside its preset as always
        let (_, token) = h.token("agent", "servers_keys").await;
        let (status, body) = h.with(&token, "GET", "/api/v1/servers", None).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let (status, body) = h
            .with(
                &token,
                "POST",
                "/api/v1/servers",
                Some(json!({ "name": "web", "hostname": "web.example", "port": 22, "username": "ops" })),
            )
            .await;
        assert!(status.is_success(), "{status} {body}");
        let (status, _) = h.with(&token, "GET", "/api/v1/tokens", None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }
}
