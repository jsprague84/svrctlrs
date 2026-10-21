//! The caller's own sign-in methods: two-factor (TOTP with recovery codes) and identities
//! linked at an OpenID Connect provider (`/api/v1/users/me/...`).
//!
//! Only a person signed in reaches these: no API token preset names them, so a token can
//! never turn two-factor off or link a way in. Anything that could lock the owner out (turning
//! TOTP on or off, new recovery codes) takes the current password, checked against the login
//! limiter, so a hijacked session alone cannot do it.

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use svrctlrs_api::sign_in::{
    LinkedIdentity, PasswordConfirmation, RecoveryCodes, SignInMethods, SignInProvider, TotpCode,
    TotpEnrolment, TotpStatus,
};
use svrctlrs_api::{ErrorBody, Message};
use svrctlrs_database::{
    models::{NewAuditEntry, User},
    queries::{user_identities, user_totp, users},
};
use tracing::instrument;
use utoipa_axum::{router::OpenApiRouter, routes};

use super::ApiError;
use crate::login_limiter::LoginLimiter;
use crate::routes::ui::auth::{client_ip, CurrentUser};
use crate::sign_in::totp;
use crate::state::AppState;

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(sign_in_methods))
        .routes(routes!(begin_totp))
        .routes(routes!(confirm_totp))
        .routes(routes!(disable_totp))
        .routes(routes!(renew_recovery_codes))
        .routes(routes!(unlink_identity))
}

/// The caller's account, after checking `password` like a login: rate limited per address and
/// username, failures counted. A wrong password is `403 PASSWORD_INCORRECT` (not 401, which
/// would mean "not signed in").
pub(crate) async fn confirm_password(
    state: &AppState,
    me: i64,
    password: &str,
    headers: &HeaderMap,
) -> Result<User, ApiError> {
    let user = users::get_user(&state.pool, me)
        .await?
        .ok_or_else(|| ApiError::not_found("User"))?;
    let keys = LoginLimiter::keys(&client_ip(headers), &user.username);
    if let Some(limited) = state.login_limiter.check(&keys).await? {
        let minutes = limited.retry_after.as_secs().div_ceil(60).max(1);
        return Err(ApiError::new(
            StatusCode::TOO_MANY_REQUESTS,
            "RATE_LIMITED",
            format!("Too many failed attempts. Try again in {minutes} minute(s)."),
        ));
    }
    if !users::verify_password(password, &user.password_hash)? {
        state.login_limiter.record_failure(&keys).await?;
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "PASSWORD_INCORRECT",
            "The current password is incorrect",
        ));
    }
    Ok(user)
}

fn audit(action: &str, me: i64, headers: &HeaderMap) -> NewAuditEntry {
    NewAuditEntry::new(action)
        .user(me)
        .client_ip(Some(client_ip(headers)))
}

/// How the caller can sign in, and how this server lets anyone sign in
#[utoipa::path(get, path = "/me/sign-in", tag = "users",
    responses((status = 200, body = SignInMethods)))]
#[instrument(skip(state))]
async fn sign_in_methods(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
) -> Result<Json<SignInMethods>, ApiError> {
    let enabled = user_totp::is_enabled(&state.pool, me).await?;
    let recovery_codes_left = if enabled {
        user_totp::recovery_codes_left(&state.pool, me).await?
    } else {
        0
    };
    let identities = user_identities::list_for_user(&state.pool, me)
        .await?
        .into_iter()
        .map(|i| LinkedIdentity {
            id: i.id,
            issuer: i.issuer,
            email: i.email,
            display_name: i.display_name,
            created_at: i.created_at,
            last_used_at: i.last_used_at,
        })
        .collect();
    Ok(Json(SignInMethods {
        password_login: state.sign_in.password_login,
        totp: TotpStatus {
            enabled,
            recovery_codes_left,
        },
        provider: state.sign_in.provider().map(|p| SignInProvider {
            name: p.name.clone(),
            issuer: p.issuer.clone(),
        }),
        identities,
    }))
}

/// The host part of the authenticator's label, so two deployments are told apart
fn account_label(username: &str, headers: &HeaderMap) -> String {
    let host = headers
        .get("x-forwarded-host")
        .or_else(|| headers.get(axum::http::header::HOST))
        .and_then(|v| v.to_str().ok())
        .map(|h| h.split(',').next().unwrap_or(h).trim().to_string())
        .filter(|h| !h.is_empty());
    match host {
        Some(host) => format!("{username}@{host}"),
        None => username.to_string(),
    }
}

/// Start turning on two-factor sign-in: a new secret for the authenticator app. Nothing
/// changes for sign-in until a code from it is confirmed.
#[utoipa::path(post, path = "/me/totp", tag = "users", request_body = PasswordConfirmation,
    responses(
        (status = 200, body = TotpEnrolment),
        (status = 403, description = "Wrong password (`PASSWORD_INCORRECT`)", body = ErrorBody),
        (status = 409, description = "Already on", body = ErrorBody),
        (status = 429, body = ErrorBody),
    ))]
#[instrument(skip(state, headers, input))]
async fn begin_totp(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
    headers: HeaderMap,
    Json(input): Json<PasswordConfirmation>,
) -> Result<Json<TotpEnrolment>, ApiError> {
    let user = confirm_password(&state, me, &input.password, &headers).await?;
    let secret = totp::new_secret();
    user_totp::begin(&state.pool, me, &secret).await?;
    let otpauth_uri = totp::otpauth_uri(&account_label(&user.username, &headers), &secret);
    let qr_svg = totp::qr_svg(&otpauth_uri)
        .ok_or_else(|| ApiError::internal_error("Could not draw the QR code"))?;
    Ok(Json(TotpEnrolment {
        secret,
        otpauth_uri,
        qr_svg,
    }))
}

/// Finish turning on two-factor sign-in with a code from the app; answers the recovery codes,
/// once.
#[utoipa::path(post, path = "/me/totp/confirm", tag = "users", request_body = TotpCode,
    responses(
        (status = 200, body = RecoveryCodes),
        (status = 400, description = "The code did not work (`TOTP_INVALID`)", body = ErrorBody),
        (status = 404, description = "No enrolment was started", body = ErrorBody),
    ))]
#[instrument(skip(state, headers, input))]
async fn confirm_totp(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
    headers: HeaderMap,
    Json(input): Json<TotpCode>,
) -> Result<Json<RecoveryCodes>, ApiError> {
    let pending = user_totp::get(&state.pool, me)
        .await?
        .filter(|t| !t.is_enabled())
        .ok_or_else(|| ApiError::not_found("Two-factor enrolment"))?;
    let now = chrono::Utc::now().timestamp();
    let step = totp::verify(&pending.secret, &input.code, now, 0).ok_or_else(|| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "TOTP_INVALID",
            "That code did not work; check the app shows SvrCtlRS and the phone's clock is right",
        )
    })?;
    let (codes, hashes) = totp::new_recovery_codes();
    if !user_totp::confirm(&state.pool, me, step, &hashes).await? {
        return Err(ApiError::not_found("Two-factor enrolment"));
    }
    crate::audit::record(&state.pool, audit("user.totp_enabled", me, &headers));
    Ok(Json(RecoveryCodes { codes }))
}

/// Turn two-factor sign-in off (the recovery codes go with it)
#[utoipa::path(post, path = "/me/totp/disable", tag = "users", request_body = PasswordConfirmation,
    responses(
        (status = 200, body = Message),
        (status = 403, description = "Wrong password (`PASSWORD_INCORRECT`)", body = ErrorBody),
        (status = 429, body = ErrorBody),
    ))]
#[instrument(skip(state, headers, input))]
async fn disable_totp(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
    headers: HeaderMap,
    Json(input): Json<PasswordConfirmation>,
) -> Result<Json<Message>, ApiError> {
    confirm_password(&state, me, &input.password, &headers).await?;
    if user_totp::disable(&state.pool, me).await? {
        crate::audit::record(&state.pool, audit("user.totp_disabled", me, &headers));
    }
    Ok(Json(Message::new("Two-factor sign-in is off")))
}

/// Ten new recovery codes in place of the old ones
#[utoipa::path(post, path = "/me/totp/recovery-codes", tag = "users",
    request_body = PasswordConfirmation,
    responses(
        (status = 200, body = RecoveryCodes),
        (status = 403, description = "Wrong password (`PASSWORD_INCORRECT`)", body = ErrorBody),
        (status = 404, description = "Two-factor sign-in is off", body = ErrorBody),
        (status = 429, body = ErrorBody),
    ))]
#[instrument(skip(state, headers, input))]
async fn renew_recovery_codes(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
    headers: HeaderMap,
    Json(input): Json<PasswordConfirmation>,
) -> Result<Json<RecoveryCodes>, ApiError> {
    confirm_password(&state, me, &input.password, &headers).await?;
    if !user_totp::is_enabled(&state.pool, me).await? {
        return Err(ApiError::not_found("Two-factor sign-in"));
    }
    let (codes, hashes) = totp::new_recovery_codes();
    user_totp::replace_recovery_codes(&state.pool, me, &hashes).await?;
    crate::audit::record(
        &state.pool,
        audit("user.recovery_codes_renewed", me, &headers),
    );
    Ok(Json(RecoveryCodes { codes }))
}

/// Unlink one of the caller's provider identities. With password sign-in off, the last one
/// stays (it is the only way in).
#[utoipa::path(delete, path = "/me/identities/{id}", tag = "users",
    params(("id" = i64, Path)),
    responses(
        (status = 200, body = Message),
        (status = 404, body = ErrorBody),
        (status = 409, description = "The last way to sign in", body = ErrorBody),
    ))]
#[instrument(skip(state, headers))]
async fn unlink_identity(
    State(state): State<AppState>,
    CurrentUser(me): CurrentUser,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<Message>, ApiError> {
    let mine = user_identities::list_for_user(&state.pool, me).await?;
    if !mine.iter().any(|i| i.id == id) {
        return Err(ApiError::not_found("Identity"));
    }
    if !state.sign_in.password_login && mine.len() == 1 {
        return Err(ApiError::conflict(
            "Password sign-in is off on this server, so this identity is your only way in; \
             link another first",
        ));
    }
    let gone = user_identities::unlink(&state.pool, me, id).await?;
    crate::audit::record(
        &state.pool,
        audit("user.identity_unlinked", me, &headers).details(serde_json::json!({
            "issuer": gone.issuer,
            "email": gone.email,
        })),
    );
    Ok(Json(Message::new("Identity unlinked")))
}

#[cfg(test)]
mod tests {
    use crate::test_support::app::AppHarness;
    use axum::http::StatusCode;
    use serde_json::json;

    const PASSWORD: &str = "correct horse battery";

    fn code_now(secret: &str) -> String {
        crate::sign_in::totp::code_at(secret, chrono::Utc::now().timestamp())
    }

    #[tokio::test]
    async fn turning_two_factor_on_takes_the_password_and_a_code() {
        let h = AppHarness::new().await;
        let (status, body) = h.owner("GET", "/api/v1/users/me/sign-in", None).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["totp"]["enabled"], false);
        assert_eq!(body["password_login"], true);

        // A wrong password is 403, never 401 (the web app would think it was signed out)
        let (status, body) = h
            .owner(
                "POST",
                "/api/v1/users/me/totp",
                Some(json!({ "password": "nope nope nope" })),
            )
            .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
        assert_eq!(body["error"]["code"], "PASSWORD_INCORRECT");

        let (status, enrolment) = h
            .owner(
                "POST",
                "/api/v1/users/me/totp",
                Some(json!({ "password": PASSWORD })),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{enrolment}");
        let secret = enrolment["secret"].as_str().unwrap().to_string();
        assert!(enrolment["otpauth_uri"]
            .as_str()
            .unwrap()
            .starts_with("otpauth://totp/SvrCtlRS:ann"));
        assert!(enrolment["qr_svg"].as_str().unwrap().contains("<svg"));

        let (status, body) = h
            .owner(
                "POST",
                "/api/v1/users/me/totp/confirm",
                Some(json!({ "code": "000000" })),
            )
            .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        assert_eq!(body["error"]["code"], "TOTP_INVALID");

        let (status, codes) = h
            .owner(
                "POST",
                "/api/v1/users/me/totp/confirm",
                Some(json!({ "code": code_now(&secret) })),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{codes}");
        assert_eq!(codes["codes"].as_array().unwrap().len(), 10);

        let (_, body) = h.owner("GET", "/api/v1/users/me/sign-in", None).await;
        assert_eq!(body["totp"]["enabled"], true);
        assert_eq!(body["totp"]["recovery_codes_left"], 10);

        // On: a second enrolment is refused until it is turned off
        let (status, _) = h
            .owner(
                "POST",
                "/api/v1/users/me/totp",
                Some(json!({ "password": PASSWORD })),
            )
            .await;
        assert_eq!(status, StatusCode::CONFLICT);
        let (status, _) = h
            .owner(
                "POST",
                "/api/v1/users/me/totp/disable",
                Some(json!({ "password": PASSWORD })),
            )
            .await;
        assert_eq!(status, StatusCode::OK);
        let (_, body) = h.owner("GET", "/api/v1/users/me/sign-in", None).await;
        assert_eq!(body["totp"]["enabled"], false);
    }

    /// The owner's agent integration must not reach any of this, whatever its preset.
    #[tokio::test]
    async fn no_token_preset_reaches_the_sign_in_routes() {
        let h = AppHarness::new().await;
        for preset in ["read_only", "servers_keys", "full_admin"] {
            let (_, token) = h.token(preset, preset).await;
            for (method, uri) in [
                ("GET", "/api/v1/users/me/sign-in"),
                ("POST", "/api/v1/users/me/totp"),
                ("POST", "/api/v1/users/me/totp/confirm"),
                ("POST", "/api/v1/users/me/totp/disable"),
                ("POST", "/api/v1/users/me/totp/recovery-codes"),
                ("DELETE", "/api/v1/users/me/identities/1"),
            ] {
                let (status, body) = h
                    .with(&token, method, uri, Some(json!({ "password": PASSWORD })))
                    .await;
                assert_eq!(
                    status,
                    StatusCode::FORBIDDEN,
                    "{preset} {method} {uri}: {body}"
                );
                assert_eq!(
                    body["error"]["code"], "TOKEN_SCOPE",
                    "{preset} {method} {uri}"
                );
            }
        }
    }

    /// Turn TOTP on for the harness owner; the secret and the recovery codes.
    async fn enable_totp(h: &AppHarness) -> (String, Vec<String>) {
        let (_, enrolment) = h
            .owner(
                "POST",
                "/api/v1/users/me/totp",
                Some(json!({ "password": PASSWORD })),
            )
            .await;
        let secret = enrolment["secret"].as_str().unwrap().to_string();
        // The confirmation uses the current step; sign-ins below use the next one
        let (status, codes) = h
            .owner(
                "POST",
                "/api/v1/users/me/totp/confirm",
                Some(json!({ "code": code_now(&secret) })),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{codes}");
        let codes = codes["codes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c.as_str().unwrap().to_string())
            .collect();
        (secret, codes)
    }

    fn next_code(secret: &str) -> String {
        crate::sign_in::totp::code_at(secret, chrono::Utc::now().timestamp() + 30)
    }

    async fn json_login(h: &AppHarness, totp: Option<&str>) -> (StatusCode, serde_json::Value) {
        let mut body = json!({ "username": "ann", "password": PASSWORD });
        if let Some(code) = totp {
            body["totp"] = json!(code);
        }
        crate::test_support::app::call(&h.app, "POST", "/api/v1/auth/login", None, Some(body)).await
    }

    #[tokio::test]
    async fn a_password_login_needs_the_code_once_two_factor_is_on() {
        let h = AppHarness::new().await;
        let (secret, recovery) = enable_totp(&h).await;

        let (status, body) = json_login(&h, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
        assert_eq!(body["code"], "TOTP_REQUIRED");
        let (status, body) = json_login(&h, Some("123456")).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
        assert_eq!(body["code"], "TOTP_INVALID");

        let code = next_code(&secret);
        let (status, body) = json_login(&h, Some(&code)).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert!(body["session_id"].as_str().is_some_and(|s| !s.is_empty()));
        // The same code twice: no
        let (status, body) = json_login(&h, Some(&code)).await;
        assert_eq!(
            (status, body["code"].as_str()),
            (StatusCode::UNAUTHORIZED, Some("TOTP_INVALID"))
        );

        // A recovery code works once, typed any old way
        let typed = recovery[0].to_ascii_uppercase().replace('-', " ");
        let (status, body) = json_login(&h, Some(&typed)).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let (status, _) = json_login(&h, Some(&recovery[0])).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        let (_, body) = h.owner("GET", "/api/v1/users/me/sign-in", None).await;
        assert_eq!(body["totp"]["recovery_codes_left"], 9);

        // The audit says how each sign-in was made (rows are written in the background)
        assert!(
            crate::test_support::app::eventually(|| async {
                let n: i64 = svrctlrs_database::sqlx::query_scalar(
                    "SELECT COUNT(*) FROM audit_log WHERE action = 'login'",
                )
                .fetch_one(&h.state.pool)
                .await
                .unwrap();
                n >= 3
            })
            .await
        );
        let methods: Vec<String> = svrctlrs_database::sqlx::query_scalar(
            "SELECT json_extract(details, '$.method') FROM audit_log WHERE action = 'login' ORDER BY id",
        )
        .fetch_all(&h.state.pool)
        .await
        .unwrap();
        assert_eq!(
            methods,
            ["password", "password+totp", "password+recovery_code"]
        );
    }

    #[tokio::test]
    async fn wrong_codes_count_against_the_login_limiter() {
        let h = AppHarness::new().await;
        enable_totp(&h).await;
        for _ in 0..crate::login_limiter::LoginLimiter::DEFAULT_MAX_FAILURES {
            let (status, _) = json_login(&h, Some("000000")).await;
            assert_eq!(status, StatusCode::UNAUTHORIZED);
        }
        let (status, body) = json_login(&h, Some("000000")).await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{body}");
    }

    mod html {
        use super::*;
        use axum::body::{to_bytes, Body};
        use axum::http::{header, Request};
        use tower::ServiceExt;

        /// A form post as a browser on this origin makes it: (status, cookie it set, body)
        async fn post_form(
            h: &AppHarness,
            uri: &str,
            cookie: Option<&str>,
            form: &str,
        ) -> (StatusCode, Option<String>, Option<String>, String) {
            let mut request = Request::builder()
                .method("POST")
                .uri(uri)
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header("sec-fetch-site", "same-origin");
            if let Some(c) = cookie {
                request = request.header(header::COOKIE, c);
            }
            let response = h
                .app
                .clone()
                .oneshot(request.body(Body::from(form.to_string())).unwrap())
                .await
                .unwrap();
            let status = response.status();
            let set_cookie = response
                .headers()
                .get(header::SET_COOKIE)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.split(';').next())
                .map(str::to_string);
            let location = response
                .headers()
                .get(header::LOCATION)
                .and_then(|v| v.to_str().ok())
                .map(str::to_string);
            let bytes = to_bytes(response.into_body(), 1 << 20).await.unwrap();
            (
                status,
                set_cookie,
                location,
                String::from_utf8_lossy(&bytes).into_owned(),
            )
        }

        async fn signed_in(h: &AppHarness, cookie: &str) -> bool {
            let request = Request::builder()
                .uri("/api/v1/sessions")
                .header(header::COOKIE, cookie)
                .body(Body::empty())
                .unwrap();
            h.app.clone().oneshot(request).await.unwrap().status() == StatusCode::OK
        }

        const LOGIN: &str = "username=ann&password=correct+horse+battery";

        #[tokio::test]
        async fn the_code_page_signs_in_only_after_a_good_code() {
            let h = AppHarness::new().await;
            let (secret, _) = enable_totp(&h).await;

            let (status, cookie, _, page) = post_form(&h, "/auth/login", None, LOGIN).await;
            assert_eq!(status, StatusCode::OK);
            assert!(page.contains("/auth/login/totp"), "{page}");
            let pending = cookie.expect("a session for the second step");
            // A right password alone is not a session
            assert!(!signed_in(&h, &pending).await);

            let (status, _, _, page) =
                post_form(&h, "/auth/login/totp", Some(&pending), "code=000000").await;
            assert_eq!(status, StatusCode::OK);
            assert!(page.contains("That code did not work"), "{page}");

            let form = format!("code={}", next_code(&secret));
            let (status, cookie, location, _) =
                post_form(&h, "/auth/login/totp", Some(&pending), &form).await;
            assert_eq!(status, StatusCode::SEE_OTHER);
            assert_eq!(location.as_deref(), Some("/"));
            let session = cookie.expect("a new session id");
            assert_ne!(session, pending);
            assert!(signed_in(&h, &session).await);
        }

        #[tokio::test]
        async fn five_wrong_codes_send_you_back_to_the_password() {
            let h = AppHarness::new().await;
            let (secret, _) = enable_totp(&h).await;
            let (_, cookie, _, _) = post_form(&h, "/auth/login", None, LOGIN).await;
            let pending = cookie.unwrap();
            for _ in 0..4 {
                let (_, _, _, page) =
                    post_form(&h, "/auth/login/totp", Some(&pending), "code=000000").await;
                assert!(page.contains("That code did not work"));
            }
            let (_, _, _, page) =
                post_form(&h, "/auth/login/totp", Some(&pending), "code=000000").await;
            assert!(page.contains("Too many wrong codes"), "{page}");
            // Even the right code no longer helps that session
            let form = format!("code={}", next_code(&secret));
            let (status, _, _, page) =
                post_form(&h, "/auth/login/totp", Some(&pending), &form).await;
            assert_eq!(status, StatusCode::OK);
            assert!(page.contains("Sign in again"), "{page}");
            assert!(!signed_in(&h, &pending).await);
        }

        #[tokio::test]
        async fn the_code_step_refuses_cross_site_posts_and_authorization_headers() {
            let h = AppHarness::new().await;
            let request = Request::builder()
                .method("POST")
                .uri("/auth/login/totp")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header("sec-fetch-site", "cross-site")
                .body(Body::from("code=123456"))
                .unwrap();
            let status = h.app.clone().oneshot(request).await.unwrap().status();
            assert_eq!(status, StatusCode::FORBIDDEN);

            let request = Request::builder()
                .method("POST")
                .uri("/auth/login/totp")
                .header(header::AUTHORIZATION, "Bearer svr_x")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from("code=123456"))
                .unwrap();
            let status = h.app.clone().oneshot(request).await.unwrap().status();
            assert_eq!(status, StatusCode::BAD_REQUEST);
        }
    }
}
