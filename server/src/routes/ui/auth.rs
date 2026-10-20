//! Authentication routes, middleware and the `CurrentUser` extractor

use axum::{
    extract::{FromRequestParts, Request, State},
    http::{header, request::Parts, HeaderMap, StatusCode, Uri},
    middleware::Next,
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
    Form, Json, Router,
};
use serde::Deserialize;
use tower_sessions::{session::Id, Session, SessionStore};
use tower_sessions_sqlx_store::SqliteStore;

use crate::api_tokens::{self, TokenAuth, TokenCheck};
use crate::routes::api::ApiError;
use crate::state::AppState;
use crate::ws_tickets::TicketOwner;
use svrctlrs_database::models::NewAuditEntry;

/// Session key for the authenticated user's ID
pub(crate) const USER_ID_KEY: &str = "user_id";

/// The authenticated user for this request, set by [`require_auth`] on every request that
/// passes it (cookie or token). Handlers take it as an extractor; it is the only place a
/// handler should learn who is calling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CurrentUser(pub i64);

impl<S: Send + Sync> FromRequestParts<S> for CurrentUser {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<CurrentUser>()
            .copied()
            .ok_or_else(|| ApiError::unauthorized("Not authenticated"))
    }
}

/// Authentication middleware that protects all routes except exempted ones.
///
/// Exempt routes: /auth/login, /auth/logout, /api/v1/auth/login, /api/v1/health, SPA assets
/// (`/_app/*`, icons, fonts, favicon, service worker — none of which carry data) and the web
/// manifest, which reads the session itself and adds the signed-in user's own shortcuts.
///
/// Four ways in, all ending with a [`CurrentUser`] extension on the request:
/// - the session cookie, holding `user_id`;
/// - a one-time ticket on a WebSocket upgrade (`?ticket=`, see `ws_tickets.rs`);
/// - an API token (`Authorization: Bearer svr_…`, see `api_tokens.rs`): the user is the
///   token's owner, the request must fall inside the token's preset (403 otherwise, decided
///   here for every route), a [`TokenAuth`] extension names the token, and every audit entry
///   the request writes names it too. Never accepted in the URL;
/// - a session token (`Authorization: Bearer <session id>`, or `?token=` on WebSocket
///   upgrades, deprecated in favour of tickets). It is looked up as a session record through
///   the store, so it must be unexpired, and the user id comes from *that record* — never from
///   anything the client sends alongside it.
///
/// Unauthenticated: WebSocket upgrades and `/api/*` get 401; browser requests are redirected
/// to the login page.
pub async fn require_auth(session: Session, mut request: Request, next: Next) -> Response {
    let path = request.uri().path().to_string();

    // Let CORS preflight requests through (OPTIONS method)
    if request.method() == axum::http::Method::OPTIONS {
        return next.run(request).await;
    }

    // Exempt routes - no auth required (case-insensitive comparison)
    let path_lower = path.to_ascii_lowercase();
    if path_lower == "/auth/login"
        || path_lower == "/auth/logout"
        || path_lower == "/api/v1/auth/login"
        || path_lower == "/api/v1/health"
        // SPA static assets (SvelteKit immutable hashed chunks) and the installable-app shell
        || path_lower.starts_with("/_app/")
        || path_lower.starts_with("/icons/")
        || path_lower.starts_with("/fonts/")
        || path_lower == "/favicon.svg"
        || path_lower == "/manifest.webmanifest"
        || path_lower == "/service-worker.js"
    {
        return next.run(request).await;
    }

    // Session cookie
    if let Some(user_id) = session.get::<i64>(USER_ID_KEY).await.unwrap_or(None) {
        request.extensions_mut().insert(CurrentUser(user_id));
        return next.run(request).await;
    }

    let pool = request
        .extensions()
        .get::<svrctlrs_database::SqlxPool<svrctlrs_database::SqlxSqlite>>()
        .cloned();

    // One-time ticket on a WebSocket upgrade (see ws_tickets.rs). Consumed on first use.
    if is_websocket_upgrade(request.headers()) {
        if let Some(ticket) = query_param(request.uri(), "ticket").filter(|t| !t.is_empty()) {
            let tickets = request
                .extensions()
                .get::<std::sync::Arc<crate::ws_tickets::WsTickets>>()
                .cloned();
            let Some(owner) = tickets.and_then(|t| t.redeem(&ticket)) else {
                return StatusCode::UNAUTHORIZED.into_response();
            };
            if let Some(token) = &owner.token {
                // Minted with a token: the upgrade gets that token's access, and only while
                // the token is still good (it may have been revoked since).
                if !token.preset.allows(request.method(), &path) {
                    return token_outside_preset(token, request.method(), &path);
                }
                let valid = match &pool {
                    Some(pool) => api_tokens::still_valid(pool, token.id).await,
                    None => false,
                };
                if !valid {
                    return token_error(
                        StatusCode::UNAUTHORIZED,
                        "TOKEN_REVOKED",
                        format!("The API token \"{}\" is no longer valid", token.name),
                    );
                }
            }
            return proceed(request, next, owner.user_id, owner.token).await;
        }
    }

    // API token: header only
    if let Some(value) = bearer_token(request.headers()).filter(|t| api_tokens::is_api_token(t)) {
        let Some(pool) = pool else {
            return StatusCode::UNAUTHORIZED.into_response();
        };
        return match api_tokens::check(&pool, &value, chrono::Utc::now()).await {
            TokenCheck::Valid { token, touch } => {
                if touch {
                    let ip = client_ip(request.headers());
                    let id = token.id;
                    tokio::spawn(async move {
                        if let Err(e) =
                            svrctlrs_database::queries::api_tokens::touch(&pool, id, &ip).await
                        {
                            tracing::warn!(error = %e, token_id = id, "Failed to record token use");
                        }
                    });
                }
                if !token.preset.allows(request.method(), &path) {
                    return token_outside_preset(&token, request.method(), &path);
                }
                proceed(request, next, token.user_id, Some(token)).await
            }
            TokenCheck::Malformed => token_error(
                StatusCode::UNAUTHORIZED,
                "TOKEN_INVALID",
                "Malformed API token: expected svr_ and 40 letters and digits",
            ),
            TokenCheck::Unknown => {
                token_error(StatusCode::UNAUTHORIZED, "TOKEN_INVALID", "Unknown API token")
            }
            TokenCheck::Expired(name) => token_error(
                StatusCode::UNAUTHORIZED,
                "TOKEN_EXPIRED",
                format!("The API token \"{name}\" has expired; create a new one under Settings → Access"),
            ),
            TokenCheck::Revoked(name) => token_error(
                StatusCode::UNAUTHORIZED,
                "TOKEN_REVOKED",
                format!("The API token \"{name}\" was revoked"),
            ),
            TokenCheck::Error => token_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                "Could not check the API token",
            ),
        };
    }

    // Session token: Authorization header first, then the query string (older native
    // clients; deprecated in favour of tickets). API tokens never travel in a URL.
    let query_token = query_param(request.uri(), "token").filter(|t| !t.is_empty());
    if query_token.as_deref().is_some_and(api_tokens::is_api_token) {
        return token_error(
            StatusCode::UNAUTHORIZED,
            "TOKEN_IN_URL",
            "API tokens go in the Authorization header, never in a URL; WebSockets take a \
             ticket from POST /api/v1/auth/ws-ticket",
        );
    }
    let token = bearer_token(request.headers()).or(query_token);

    if let (Some(token), Some(pool)) = (token, pool) {
        if let Some(user_id) = user_id_for_token(&SqliteStore::new(pool), &token).await {
            request.extensions_mut().insert(CurrentUser(user_id));
            return next.run(request).await;
        }
    }

    // Unauthenticated — determine response type

    // WebSocket upgrade requests get 401
    if is_websocket_upgrade(request.headers()) {
        return StatusCode::UNAUTHORIZED.into_response();
    }

    // API requests get 401 JSON
    if path.starts_with("/api/") {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": "Authentication required"})),
        )
            .into_response();
    }

    // Browser requests redirect to login
    Redirect::to("/auth/login").into_response()
}

/// Let an authenticated request through as `user_id`, naming the API token (if any) in the
/// request's extensions and in every audit entry the request writes.
async fn proceed(
    mut request: Request,
    next: Next,
    user_id: i64,
    token: Option<TokenAuth>,
) -> Response {
    request.extensions_mut().insert(CurrentUser(user_id));
    match token {
        Some(token) => {
            request.extensions_mut().insert(token.clone());
            crate::audit::with_token(Some(token), next.run(request)).await
        }
        None => next.run(request).await,
    }
}

fn token_error(status: StatusCode, code: &'static str, message: impl Into<String>) -> Response {
    ApiError::new(status, code, message).into_response()
}

/// 403 for a request the token's preset does not name.
fn token_outside_preset(token: &TokenAuth, method: &axum::http::Method, path: &str) -> Response {
    tracing::warn!(token_id = token.id, token = %token.name, preset = token.preset.as_str(),
        %method, path, "API token refused: outside its preset");
    ApiError::new(
        StatusCode::FORBIDDEN,
        "TOKEN_SCOPE",
        format!(
            "The {} token \"{}\" does not allow {method} {path}",
            token.preset.label(),
            token.name
        ),
    )
    .with_details(serde_json::json!({
        "preset": token.preset.as_str(),
        "method": method.as_str(),
        "path": path,
    }))
    .into_response()
}

/// Whether a state-changing request came from this site. Browsers send `Sec-Fetch-Site` on
/// every request (`same-origin`, `same-site`, `cross-site`, `none` for a typed URL); older ones
/// at least send `Origin` on POSTs. A request with neither is not a browser form post. This is
/// the CSRF guard for the cookie-authenticated HTML login and logout routes.
pub fn is_same_origin_request(headers: &HeaderMap) -> bool {
    if let Some(site) = headers.get("sec-fetch-site").and_then(|v| v.to_str().ok()) {
        return matches!(site, "same-origin" | "none");
    }
    let Some(origin) = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) else {
        return true;
    };
    let origin_host = origin
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(origin)
        .trim_end_matches('/');
    headers
        .get("x-forwarded-host")
        .or_else(|| headers.get(header::HOST))
        .and_then(|v| v.to_str().ok())
        .map(|h| h.split(',').next().unwrap_or(h).trim())
        .is_some_and(|h| h.eq_ignore_ascii_case(origin_host))
}

fn is_websocket_upgrade(headers: &HeaderMap) -> bool {
    headers
        .get(header::UPGRADE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.eq_ignore_ascii_case("websocket"))
}

/// Mint a one-time WebSocket ticket for the authenticated caller (and the API token it came
/// with, whose access the upgrade then has).
async fn ws_ticket(
    State(state): State<AppState>,
    CurrentUser(user_id): CurrentUser,
    token: Option<axum::Extension<TokenAuth>>,
) -> Json<serde_json::Value> {
    let ticket = state.ws_tickets.issue(TicketOwner {
        user_id,
        token: token.map(|axum::Extension(t)| t),
    });
    Json(serde_json::json!({
        "ticket": ticket,
        "expires_in_secs": state.ws_tickets.ttl().as_secs(),
    }))
}

fn bearer_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .filter(|t| !t.is_empty())
        .map(String::from)
}

fn query_param(uri: &Uri, name: &str) -> Option<String> {
    uri.query()?.split('&').find_map(|pair| {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        (k == name).then(|| v.to_string())
    })
}

/// Resolve a token to the user id stored in its session record. `load` only returns
/// unexpired records, so an expired session is not a valid token either.
async fn user_id_for_token(store: &SqliteStore, token: &str) -> Option<i64> {
    let id: Id = token.parse().ok()?;
    let record = store.load(&id).await.ok().flatten()?;
    record.data.get(USER_ID_KEY)?.as_i64()
}

/// Whether a WebSocket upgrade may proceed, judged by its `Origin`.
///
/// Browsers always send `Origin` on a WebSocket handshake and attach cookies to it even
/// cross-site, so without this check any page the user visits could open a shell with the
/// user's session (cross-site WebSocket hijacking). The origin must be this server's own
/// (same host as the request) or one of the configured `ALLOWED_ORIGINS`. Requests without
/// an `Origin` come from non-browser clients and are left to the authentication step.
pub fn websocket_origin_allowed(headers: &HeaderMap, allowed_origins: &[String]) -> bool {
    let Some(origin) = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) else {
        return true;
    };
    if allowed_origins
        .iter()
        .any(|a| a.eq_ignore_ascii_case(origin))
    {
        return true;
    }
    let origin_host = origin
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(origin)
        .trim_end_matches('/');
    let request_host = headers
        .get("x-forwarded-host")
        .or_else(|| headers.get(header::HOST))
        .and_then(|v| v.to_str().ok())
        .map(|h| h.split(',').next().unwrap_or(h).trim());
    request_host.is_some_and(|h| h.eq_ignore_ascii_case(origin_host))
}

/// The request URI with `token` and `user_id` query values blanked, for logging.
pub fn redacted_uri(uri: &Uri) -> String {
    let Some(query) = uri.query() else {
        return uri.path().to_string();
    };
    let redacted: Vec<String> = query
        .split('&')
        .map(|pair| match pair.split_once('=') {
            Some((k, _)) if k == "token" || k == "ticket" || k == "user_id" => {
                format!("{k}=[redacted]")
            }
            _ => pair.to_string(),
        })
        .collect();
    format!("{}?{}", uri.path(), redacted.join("&"))
}

/// Create auth router
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/auth/login", get(login_page).post(login))
        .route("/api/v1/auth/login", post(login_json))
        .route("/api/v1/auth/ws-ticket", post(ws_ticket))
        .route("/auth/logout", post(logout))
}

/// Login form data
#[derive(Debug, Deserialize)]
struct LoginForm {
    username: String,
    password: String,
}

/// Render minimal standalone login page
fn render_login_html(error: Option<&str>) -> String {
    let error_block = error
        .map(|msg| {
            format!(
                r#"<div style="background:#f85149;color:#fff;padding:0.75rem 1rem;border-radius:6px;margin-bottom:1.5rem;font-size:0.875rem;">{}</div>"#,
                msg
            )
        })
        .unwrap_or_default();

    format!(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>SvrCtlRS — Login</title>
<style>
*,*::before,*::after{{box-sizing:border-box;margin:0;padding:0}}
body{{font-family:-apple-system,BlinkMacSystemFont,"Segoe UI",Roboto,sans-serif;background:#1a1b26;color:#c0caf5;display:flex;align-items:center;justify-content:center;min-height:100vh}}
.card{{background:#24283b;border:1px solid #414868;border-radius:12px;padding:2.5rem;width:100%;max-width:400px;box-shadow:0 8px 32px rgba(0,0,0,0.3)}}
h1{{font-size:1.5rem;font-weight:600;margin-bottom:0.25rem;text-align:center}}
.sub{{color:#565f89;font-size:0.875rem;text-align:center;margin-bottom:2rem}}
label{{display:block;font-size:0.875rem;font-weight:500;color:#a9b1d6;margin-bottom:0.375rem}}
input{{width:100%;padding:0.625rem 0.75rem;background:#1a1b26;border:1px solid #414868;border-radius:6px;color:#c0caf5;font-size:0.875rem;outline:none;transition:border-color 0.15s}}
input:focus{{border-color:#7aa2f7}}
.field{{margin-bottom:1.25rem}}
button{{width:100%;padding:0.75rem;background:#7aa2f7;color:#1a1b26;border:none;border-radius:6px;font-size:0.875rem;font-weight:600;cursor:pointer;transition:background 0.15s}}
button:hover{{background:#89b4fa}}
</style>
</head>
<body>
<div class="card">
<h1>SvrCtlRS</h1>
<p class="sub">Sign in to continue</p>
{error_block}
<form method="post" action="/auth/login">
<div class="field"><label for="username">Username</label><input id="username" name="username" type="text" autocomplete="username" required autofocus></div>
<div class="field"><label for="password">Password</label><input id="password" name="password" type="password" autocomplete="current-password" required></div>
<button type="submit">Sign in</button>
</form>
</div>
</body>
</html>"##
    )
}

/// Login page handler
async fn login_page() -> Html<String> {
    Html(render_login_html(None))
}

/// Why a login attempt was refused. Credential and rate-limit failures are told apart only in
/// the status code; the message never says whether the username exists.
enum LoginFailure {
    RateLimited(std::time::Duration),
    InvalidCredentials,
    Internal,
}

/// The client address as seen through the reverse proxy (first `X-Forwarded-For` hop, then
/// `X-Real-IP`). Without a proxy there is no header and all direct clients share one bucket,
/// which still bounds a brute-force attempt; the per-username key does the targeted work.
pub(crate) fn client_ip(headers: &HeaderMap) -> String {
    headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .or_else(|| headers.get("x-real-ip").and_then(|v| v.to_str().ok()))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "direct".to_string())
}

/// Shared by both login handlers: rate limit, look up, verify, record the outcome, and
/// start a fresh session for the user (a new id, so a pre-login session cannot be fixated).
async fn authenticate(
    state: &AppState,
    headers: &HeaderMap,
    session: &Session,
    creds: &LoginForm,
) -> Result<svrctlrs_database::models::User, LoginFailure> {
    use svrctlrs_database::queries::users;

    let ip = client_ip(headers);
    let keys = crate::login_limiter::LoginLimiter::keys(&ip, &creds.username);
    for key in &keys {
        if let Err(retry_after) = state.login_limiter.check(key) {
            tracing::warn!(key = %key, retry_after_secs = retry_after.as_secs(), "Login rate limited");
            crate::audit::record(
                &state.pool,
                NewAuditEntry::new("login.rate_limited")
                    .username(&creds.username)
                    .client_ip(Some(ip.clone())),
            );
            return Err(LoginFailure::RateLimited(retry_after));
        }
    }

    let user = users::get_user_by_username(&state.pool, &creds.username)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Database error during login");
            LoginFailure::Internal
        })?;

    let valid = match &user {
        Some(u) => users::verify_password(&creds.password, &u.password_hash).map_err(|e| {
            tracing::error!(error = %e, "Password verification error");
            LoginFailure::Internal
        })?,
        None => false,
    };

    let Some(user) = user.filter(|_| valid) else {
        for key in &keys {
            state.login_limiter.record_failure(key);
        }
        tracing::warn!(username = %creds.username, "Login failed");
        crate::audit::record(
            &state.pool,
            NewAuditEntry::new("login.failed")
                .username(&creds.username)
                .client_ip(Some(ip.clone())),
        );
        return Err(LoginFailure::InvalidCredentials);
    };

    for key in &keys {
        state.login_limiter.clear(key);
    }
    crate::audit::record(
        &state.pool,
        NewAuditEntry::new("login")
            .user(user.id)
            .username(&user.username)
            .client_ip(Some(ip)),
    );

    session.cycle_id().await.map_err(|e| {
        tracing::error!(error = %e, "Failed to cycle session id");
        LoginFailure::Internal
    })?;
    session.insert(USER_ID_KEY, user.id).await.map_err(|e| {
        tracing::error!(error = %e, "Failed to store session");
        LoginFailure::Internal
    })?;

    Ok(user)
}

fn retry_after_message(retry_after: std::time::Duration) -> String {
    let minutes = retry_after.as_secs().div_ceil(60).max(1);
    format!("Too many failed attempts. Try again in {minutes} minute(s).")
}

/// Login form submission handler
async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    session: Session,
    Form(creds): Form<LoginForm>,
) -> Response {
    if !is_same_origin_request(&headers) {
        return (
            StatusCode::FORBIDDEN,
            Html(render_login_html(Some("Cross-site login refused"))),
        )
            .into_response();
    }
    match authenticate(&state, &headers, &session, &creds).await {
        Ok(user) => {
            tracing::info!(username = %user.username, user_id = user.id, "User logged in");
            Redirect::to("/").into_response()
        }
        Err(LoginFailure::RateLimited(retry_after)) => (
            StatusCode::TOO_MANY_REQUESTS,
            [(
                header::RETRY_AFTER,
                retry_after.as_secs().max(1).to_string(),
            )],
            Html(render_login_html(Some(&retry_after_message(retry_after)))),
        )
            .into_response(),
        Err(LoginFailure::InvalidCredentials) => (
            StatusCode::OK,
            Html(render_login_html(Some("Invalid username or password"))),
        )
            .into_response(),
        Err(LoginFailure::Internal) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Html(render_login_html(Some("Internal server error"))),
        )
            .into_response(),
    }
}

/// JSON login handler for Tauri/SPA clients (no redirect, returns JSON)
async fn login_json(
    State(state): State<AppState>,
    headers: HeaderMap,
    session: Session,
    Json(creds): Json<LoginForm>,
) -> Response {
    let user = match authenticate(&state, &headers, &session, &creds).await {
        Ok(user) => user,
        Err(LoginFailure::RateLimited(retry_after)) => {
            return (
                StatusCode::TOO_MANY_REQUESTS,
                [(
                    header::RETRY_AFTER,
                    retry_after.as_secs().max(1).to_string(),
                )],
                Json(serde_json::json!({"error": retry_after_message(retry_after)})),
            )
                .into_response();
        }
        Err(LoginFailure::InvalidCredentials) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"error": "Invalid username or password"})),
            )
                .into_response();
        }
        Err(LoginFailure::Internal) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Internal server error"})),
            )
                .into_response();
        }
    };

    // Save now so the (cycled) id exists for clients that use it as a Bearer token
    if let Err(e) = session.save().await {
        tracing::error!(error = %e, "Failed to save session");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": "Session error"})),
        )
            .into_response();
    }
    let session_id = session.id().map(|id| id.to_string()).unwrap_or_default();

    tracing::info!(username = %user.username, user_id = user.id, "User logged in (JSON)");
    Json(serde_json::json!({"success": true, "user_id": user.id, "session_id": session_id}))
        .into_response()
}

/// Logout handler
async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap,
    session: Session,
) -> Result<Redirect, StatusCode> {
    if !is_same_origin_request(&headers) {
        return Err(StatusCode::FORBIDDEN);
    }
    if let Some(user_id) = session.get::<i64>(USER_ID_KEY).await.unwrap_or(None) {
        crate::audit::record(
            &state.pool,
            NewAuditEntry::new("logout")
                .user(user_id)
                .client_ip(Some(client_ip(&headers))),
        );
    }
    session
        .flush()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    tracing::info!("User logged out");
    Ok(Redirect::to("/auth/login"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request, middleware::from_fn, Extension, Router};
    use std::collections::HashMap;
    use svrctlrs_database::sqlx::sqlite::SqlitePoolOptions;
    use tower::ServiceExt;
    use tower_sessions::{session::Record, SessionManagerLayer};

    fn headers(pairs: &[(&str, &str)]) -> HeaderMap {
        let mut h = HeaderMap::new();
        for (k, v) in pairs {
            h.insert(
                axum::http::HeaderName::from_bytes(k.as_bytes()).unwrap(),
                v.parse().unwrap(),
            );
        }
        h
    }

    #[test]
    fn redacts_token_and_user_id_only() {
        let uri: Uri = "/ws/terminal/pty?token=abc&user_id=7&cols=80"
            .parse()
            .unwrap();
        assert_eq!(
            redacted_uri(&uri),
            "/ws/terminal/pty?token=[redacted]&user_id=[redacted]&cols=80"
        );
        let plain: Uri = "/api/v1/servers".parse().unwrap();
        assert_eq!(redacted_uri(&plain), "/api/v1/servers");
        // One-time tickets are short-lived, but a log line is not
        let ticket: Uri = "/ws/terminal/pty?ticket=abc".parse().unwrap();
        assert_eq!(redacted_uri(&ticket), "/ws/terminal/pty?ticket=[redacted]");
    }

    #[test]
    fn same_origin_rules_for_form_posts() {
        let host = ("host", "svrctlrs.js-node.app");
        assert!(is_same_origin_request(&headers(&[
            host,
            ("sec-fetch-site", "same-origin")
        ])));
        assert!(is_same_origin_request(&headers(&[
            host,
            ("sec-fetch-site", "none")
        ])));
        assert!(!is_same_origin_request(&headers(&[
            host,
            ("sec-fetch-site", "cross-site")
        ])));
        assert!(!is_same_origin_request(&headers(&[
            host,
            ("sec-fetch-site", "same-site")
        ])));
        // Older browsers: Origin against the (forwarded) host
        assert!(is_same_origin_request(&headers(&[
            host,
            ("origin", "https://svrctlrs.js-node.app")
        ])));
        assert!(!is_same_origin_request(&headers(&[
            host,
            ("origin", "https://evil.example")
        ])));
        // Not a browser at all
        assert!(is_same_origin_request(&headers(&[host])));
    }

    #[test]
    fn websocket_origin_rules() {
        let allowed = vec!["tauri://localhost".to_string()];
        let host = ("host", "svrctlrs.js-node.app");

        // Same host as the request, with and without a proxy in front.
        assert!(websocket_origin_allowed(
            &headers(&[host, ("origin", "https://svrctlrs.js-node.app")]),
            &allowed
        ));
        assert!(websocket_origin_allowed(
            &headers(&[
                ("host", "10.0.0.5:8080"),
                ("x-forwarded-host", "svrctlrs.js-node.app"),
                ("origin", "https://svrctlrs.js-node.app"),
            ]),
            &allowed
        ));
        // A configured native-client origin.
        assert!(websocket_origin_allowed(
            &headers(&[host, ("origin", "tauri://localhost")]),
            &allowed
        ));
        // Any other page: refused.
        assert!(!websocket_origin_allowed(
            &headers(&[host, ("origin", "https://evil.example")]),
            &allowed
        ));
        assert!(!websocket_origin_allowed(
            &headers(&[
                host,
                ("origin", "https://svrctlrs.js-node.app.evil.example")
            ]),
            &allowed
        ));
        // No Origin header: not a browser; authentication decides.
        assert!(websocket_origin_allowed(&headers(&[host]), &allowed));
    }

    async fn app_with_store() -> (
        Router,
        SqliteStore,
        std::sync::Arc<crate::ws_tickets::WsTickets>,
    ) {
        // One connection, or each pooled connection would get its own in-memory database.
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        let store = SqliteStore::new(pool.clone());
        store.migrate().await.unwrap();
        let tickets = std::sync::Arc::new(crate::ws_tickets::WsTickets::default());
        let app = Router::new()
            .route(
                "/api/v1/whoami",
                get(|CurrentUser(id): CurrentUser| async move { id.to_string() }),
            )
            .route("/page", get(|| async { "page" }))
            // Exempt login route: writes the session so SessionManagerLayer issues a cookie.
            .route(
                "/api/v1/auth/login",
                get(
                    |session: Session,
                     axum::extract::Query(q): axum::extract::Query<
                        HashMap<String, String>,
                    >| async move {
                        let id: i64 = q.get("id").and_then(|s| s.parse().ok()).unwrap_or(0);
                        session.insert(USER_ID_KEY, id).await.unwrap();
                        "ok"
                    },
                ),
            )
            .layer(from_fn(require_auth))
            .layer(Extension(pool))
            .layer(Extension(tickets.clone()))
            .layer(SessionManagerLayer::new(store.clone()));
        (app, store, tickets)
    }

    async fn session_token(store: &SqliteStore, user_id: i64, ttl: time::Duration) -> String {
        let mut record = Record {
            id: Id::default(),
            data: HashMap::from([(USER_ID_KEY.to_string(), serde_json::json!(user_id))]),
            expiry_date: time::OffsetDateTime::now_utc() + ttl,
        };
        store.create(&mut record).await.unwrap();
        record.id.to_string()
    }

    async fn body_text(resp: Response) -> String {
        let bytes = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
        String::from_utf8(bytes.to_vec()).unwrap()
    }

    #[tokio::test]
    async fn unauthenticated_requests_are_refused() {
        let (app, _, _) = app_with_store().await;

        let api = app
            .clone()
            .oneshot(Request::get("/api/v1/whoami").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(api.status(), StatusCode::UNAUTHORIZED);

        let page = app
            .oneshot(Request::get("/page").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(page.status(), StatusCode::SEE_OTHER);
        assert_eq!(page.headers()[header::LOCATION], "/auth/login");
    }

    #[tokio::test]
    async fn bearer_token_identity_comes_from_the_session_not_the_client() {
        let (app, store, _) = app_with_store().await;
        let token = session_token(&store, 42, time::Duration::hours(1)).await;

        let resp = app
            .oneshot(
                Request::get("/api/v1/whoami")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .header("x-user-id", "999")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(body_text(resp).await, "42");
    }

    #[tokio::test]
    async fn query_token_works_for_websocket_style_requests() {
        let (app, store, _) = app_with_store().await;
        let token = session_token(&store, 7, time::Duration::hours(1)).await;

        let resp = app
            .oneshot(
                Request::get(format!("/api/v1/whoami?token={token}&user_id=999"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(body_text(resp).await, "7");
    }

    #[tokio::test]
    async fn expired_and_unknown_tokens_are_refused() {
        let (app, store, _) = app_with_store().await;
        let expired = session_token(&store, 42, -time::Duration::minutes(1)).await;

        for token in [expired.as_str(), "not-a-session-id", ""] {
            let resp = app
                .clone()
                .oneshot(
                    Request::get("/api/v1/whoami")
                        .header(header::AUTHORIZATION, format!("Bearer {token}"))
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(resp.status(), StatusCode::UNAUTHORIZED, "token {token:?}");
        }
    }
    #[tokio::test]
    async fn websocket_tickets_redeem_once_and_only_on_upgrades() {
        let (app, _, tickets) = app_with_store().await;
        let ticket = tickets.issue(TicketOwner {
            user_id: 9,
            token: None,
        });

        let upgrade = |t: &str| {
            Request::get(format!("/api/v1/whoami?ticket={t}"))
                .header(header::UPGRADE, "websocket")
                .body(Body::empty())
                .unwrap()
        };
        let resp = app.clone().oneshot(upgrade(&ticket)).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(body_text(resp).await, "9");

        // Spent.
        let resp = app.clone().oneshot(upgrade(&ticket)).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

        // A ticket is not a credential for ordinary requests.
        let fresh = tickets.issue(TicketOwner {
            user_id: 9,
            token: None,
        });
        let resp = app
            .oneshot(
                Request::get(format!("/api/v1/whoami?ticket={fresh}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn a_session_cookie_authenticates() {
        let (app, _, _) = app_with_store().await;

        // Log in: the handler writes the session, the layer returns a Set-Cookie.
        let login = app
            .clone()
            .oneshot(
                Request::get("/api/v1/auth/login?id=5")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(login.status(), StatusCode::OK);
        let cookie = login
            .headers()
            .get(header::SET_COOKIE)
            .expect("a session cookie was set")
            .to_str()
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .to_string();

        // Replaying that cookie identifies the user, with no token or header.
        let resp = app
            .oneshot(
                Request::get("/api/v1/whoami")
                    .header(header::COOKIE, &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(body_text(resp).await, "5");
    }

    #[tokio::test]
    async fn exempt_routes_bypass_auth() {
        let (app, _, _) = app_with_store().await;

        // None of these are mounted, so passing auth lands on 404 — not 401 or a login redirect.
        for path in [
            "/api/v1/health",
            "/auth/login",
            "/auth/logout",
            "/_app/immutable/chunk.js",
            "/icons/icon-192.png",
            "/fonts/JetBrainsMono-Regular.woff2",
            "/favicon.svg",
            "/manifest.webmanifest",
            "/service-worker.js",
        ] {
            let resp = app
                .clone()
                .oneshot(Request::get(path).body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(
                resp.status(),
                StatusCode::NOT_FOUND,
                "exempt path {path:?} should pass auth and reach routing"
            );
        }
    }

    #[tokio::test]
    async fn exempt_lookalikes_are_still_protected() {
        let (app, _, _) = app_with_store().await;

        // A near-miss of an exact or prefix match must not inherit the exemption.
        for path in ["/api/v1/healthz", "/api/v1/health/extra"] {
            let resp = app
                .clone()
                .oneshot(Request::get(path).body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(
                resp.status(),
                StatusCode::UNAUTHORIZED,
                "api lookalike {path:?} must require auth"
            );
        }
        for path in ["/fonts", "/_app", "/manifest.webmanifest.evil"] {
            let resp = app
                .clone()
                .oneshot(Request::get(path).body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(
                resp.status(),
                StatusCode::SEE_OTHER,
                "page lookalike {path:?} must redirect to login"
            );
        }
    }

    #[tokio::test]
    async fn options_preflight_bypasses_auth() {
        let (app, _, _) = app_with_store().await;
        let resp = app
            .oneshot(
                Request::builder()
                    .method(axum::http::Method::OPTIONS)
                    .uri("/api/v1/whoami")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        // The route only handles GET, so reaching routing yields 405 — proof auth was bypassed.
        assert_ne!(resp.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(resp.status(), StatusCode::METHOD_NOT_ALLOWED);
    }

    // ---- API tokens -------------------------------------------------------------------

    /// A fully migrated database with the session store, one user (ann, id 1), and a router
    /// whose routes sit on real paths so the presets apply: `GET /api/v1/status` echoes the
    /// caller, `POST /api/v1/servers` writes an audit entry, `/ws/terminal/pty` echoes the
    /// caller of an upgrade.
    async fn token_app() -> (
        Router,
        svrctlrs_database::SqlxPool<svrctlrs_database::SqlxSqlite>,
        std::sync::Arc<crate::ws_tickets::WsTickets>,
    ) {
        let pool = crate::test_support::database().await;
        let store = SqliteStore::new(pool.clone());
        store.migrate().await.unwrap();
        svrctlrs_database::sqlx::query(
            "INSERT INTO users (username, password_hash) VALUES ('ann', 'x')",
        )
        .execute(&pool)
        .await
        .unwrap();
        let tickets = std::sync::Arc::new(crate::ws_tickets::WsTickets::default());

        async fn echo(
            CurrentUser(id): CurrentUser,
            token: Option<axum::Extension<TokenAuth>>,
        ) -> String {
            match token {
                Some(axum::Extension(t)) => format!("{id}:{}", t.name),
                None => id.to_string(),
            }
        }
        let audit_pool = pool.clone();
        let app = Router::new()
            .route("/api/v1/status", get(echo))
            .route("/ws/terminal/pty", get(echo))
            .route(
                "/api/v1/servers",
                post(move |CurrentUser(id): CurrentUser| {
                    let pool = audit_pool.clone();
                    async move {
                        crate::audit::record(
                            &pool,
                            NewAuditEntry::new("server.created")
                                .user(id)
                                .target("server", 7, "web"),
                        );
                        "created"
                    }
                }),
            )
            .route(
                "/api/v1/auth/login",
                get(|session: Session| async move {
                    session.insert(USER_ID_KEY, 1_i64).await.unwrap();
                    "ok"
                }),
            )
            .layer(from_fn(require_auth))
            .layer(Extension(pool.clone()))
            .layer(Extension(tickets.clone()))
            .layer(SessionManagerLayer::new(store));
        (app, pool, tickets)
    }

    /// A token for ann with `preset`; `expired`/`revoked` set it up that way.
    async fn make_token(
        pool: &svrctlrs_database::SqlxPool<svrctlrs_database::SqlxSqlite>,
        name: &str,
        preset: api_tokens::Preset,
    ) -> (i64, String) {
        let secret = api_tokens::generate();
        let row = svrctlrs_database::queries::api_tokens::create(
            pool,
            &svrctlrs_database::models::NewApiToken {
                user_id: 1,
                name: name.to_string(),
                prefix: secret.prefix,
                token_hash: secret.hash,
                preset: preset.as_str().to_string(),
                expires_in_days: Some(30),
            },
        )
        .await
        .unwrap();
        (row.id, secret.secret)
    }

    fn with_bearer(method: &str, uri: &str, token: &str) -> Request<Body> {
        Request::builder()
            .method(method)
            .uri(uri)
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap()
    }

    async fn error_code(resp: Response) -> (StatusCode, String) {
        let status = resp.status();
        let body: serde_json::Value = serde_json::from_str(&body_text(resp).await).unwrap();
        (
            status,
            body["error"]["code"].as_str().unwrap_or("").to_string(),
        )
    }

    #[tokio::test]
    async fn an_api_token_acts_as_its_owner() {
        let (app, pool, _) = token_app().await;
        let (_, token) = make_token(&pool, "hermes", api_tokens::Preset::ReadOnly).await;
        let resp = app
            .oneshot(
                Request::get("/api/v1/status")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .header("x-user-id", "999")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(body_text(resp).await, "1:hermes");
    }

    #[tokio::test]
    async fn expired_revoked_unknown_and_malformed_tokens_are_refused() {
        let (app, pool, _) = token_app().await;
        let (expired_id, expired) = make_token(&pool, "old", api_tokens::Preset::FullAdmin).await;
        svrctlrs_database::sqlx::query(
            "UPDATE api_tokens SET expires_at = datetime('now', '-1 minute') WHERE id = ?",
        )
        .bind(expired_id)
        .execute(&pool)
        .await
        .unwrap();
        let (revoked_id, revoked) = make_token(&pool, "gone", api_tokens::Preset::FullAdmin).await;
        svrctlrs_database::queries::api_tokens::revoke(&pool, 1, revoked_id)
            .await
            .unwrap();
        let unknown = api_tokens::generate().secret;

        for (token, code) in [
            (expired.as_str(), "TOKEN_EXPIRED"),
            (revoked.as_str(), "TOKEN_REVOKED"),
            (unknown.as_str(), "TOKEN_INVALID"),
            ("svr_tooshort", "TOKEN_INVALID"),
            (&format!("svr_{}!", "a".repeat(39)), "TOKEN_INVALID"),
        ] {
            let resp = app
                .clone()
                .oneshot(with_bearer("GET", "/api/v1/status", token))
                .await
                .unwrap();
            assert_eq!(
                error_code(resp).await,
                (StatusCode::UNAUTHORIZED, code.to_string()),
                "{token}"
            );
        }
    }

    #[tokio::test]
    async fn a_token_outside_its_preset_gets_403() {
        let (app, pool, _) = token_app().await;
        let (_, read) = make_token(&pool, "reader", api_tokens::Preset::ReadOnly).await;
        let (_, keys) = make_token(&pool, "keys", api_tokens::Preset::ServersKeys).await;
        let (_, full) = make_token(&pool, "admin", api_tokens::Preset::FullAdmin).await;

        let resp = app
            .clone()
            .oneshot(with_bearer("POST", "/api/v1/servers", &read))
            .await
            .unwrap();
        assert_eq!(
            error_code(resp).await,
            (StatusCode::FORBIDDEN, "TOKEN_SCOPE".to_string())
        );
        let resp = app
            .clone()
            .oneshot(with_bearer("POST", "/api/v1/servers", &keys))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        // Not mounted here, but refused before routing: no token manages tokens or accounts
        for (method, path) in [
            ("GET", "/api/v1/tokens"),
            ("POST", "/api/v1/tokens"),
            ("POST", "/api/v1/users"),
            ("PUT", "/api/v1/users/me/password"),
        ] {
            let resp = app
                .clone()
                .oneshot(with_bearer(method, path, &full))
                .await
                .unwrap();
            assert_eq!(resp.status(), StatusCode::FORBIDDEN, "{method} {path}");
        }
        // A terminal only for full admin
        let upgrade = |token: &str| {
            Request::get("/ws/terminal/pty")
                .header(header::UPGRADE, "websocket")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap()
        };
        let resp = app.clone().oneshot(upgrade(&keys)).await.unwrap();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
        let resp = app.clone().oneshot(upgrade(&full)).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn api_tokens_are_refused_in_the_url() {
        let (app, pool, _) = token_app().await;
        let (_, full) = make_token(&pool, "admin", api_tokens::Preset::FullAdmin).await;
        for req in [
            Request::get(format!("/api/v1/status?token={full}"))
                .body(Body::empty())
                .unwrap(),
            Request::get(format!("/ws/terminal/pty?token={full}"))
                .header(header::UPGRADE, "websocket")
                .body(Body::empty())
                .unwrap(),
        ] {
            let resp = app.clone().oneshot(req).await.unwrap();
            assert_eq!(
                error_code(resp).await,
                (StatusCode::UNAUTHORIZED, "TOKEN_IN_URL".to_string())
            );
        }
    }

    /// Poll for a condition the request kicked off without waiting (audit rows, last-used).
    async fn eventually<F, Fut>(mut check: F) -> bool
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = bool>,
    {
        for _ in 0..100 {
            if check().await {
                return true;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        false
    }

    #[tokio::test]
    async fn every_audit_row_a_token_request_writes_names_the_token() {
        let (app, pool, _) = token_app().await;
        let (id, keys) = make_token(&pool, "hermes", api_tokens::Preset::ServersKeys).await;
        let resp = app
            .clone()
            .oneshot(with_bearer("POST", "/api/v1/servers", &keys))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        // The same action from a signed-in browser names no token
        let login = app
            .clone()
            .oneshot(
                Request::get("/api/v1/auth/login")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let cookie = login.headers()[header::SET_COOKIE]
            .to_str()
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .to_string();
        let resp = app
            .clone()
            .oneshot(
                Request::post("/api/v1/servers")
                    .header(header::COOKIE, cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let rows = || async {
            svrctlrs_database::queries::audit::list(&pool, 10, None)
                .await
                .unwrap()
        };
        assert!(eventually(|| async { rows().await.len() == 2 }).await);
        let rows = rows().await;
        let by_token = rows.iter().find(|r| r.token_id.is_some()).unwrap();
        assert_eq!(by_token.token_id, Some(id));
        assert_eq!(by_token.token_name.as_deref(), Some("hermes"));
        assert_eq!(by_token.user_id, Some(1));
        assert_eq!(rows.iter().filter(|r| r.token_id.is_none()).count(), 1);
    }

    #[tokio::test]
    async fn token_use_is_recorded_with_time_and_address() {
        let (app, pool, _) = token_app().await;
        let (id, token) = make_token(&pool, "hermes", api_tokens::Preset::ReadOnly).await;
        let resp = app
            .oneshot(
                Request::get("/api/v1/status")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .header("x-forwarded-for", "10.10.1.35")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert!(
            eventually(|| async {
                svrctlrs_database::queries::api_tokens::get(&pool, id)
                    .await
                    .unwrap()
                    .and_then(|t| t.last_used_ip)
                    .as_deref()
                    == Some("10.10.1.35")
            })
            .await
        );
    }

    #[tokio::test]
    async fn a_ticket_minted_with_a_token_carries_it_and_dies_with_it() {
        let (app, pool, tickets) = token_app().await;
        let (id, _) = make_token(&pool, "hermes", api_tokens::Preset::FullAdmin).await;
        let owner = TicketOwner {
            user_id: 1,
            token: Some(TokenAuth {
                id,
                user_id: 1,
                name: "hermes".into(),
                preset: api_tokens::Preset::FullAdmin,
            }),
        };
        let upgrade = |t: &str| {
            Request::get(format!("/ws/terminal/pty?ticket={t}"))
                .header(header::UPGRADE, "websocket")
                .body(Body::empty())
                .unwrap()
        };
        let resp = app
            .clone()
            .oneshot(upgrade(&tickets.issue(owner.clone())))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(body_text(resp).await, "1:hermes");

        // Revoked between minting and upgrading
        let ticket = tickets.issue(owner.clone());
        svrctlrs_database::queries::api_tokens::revoke(&pool, 1, id)
            .await
            .unwrap();
        let resp = app.clone().oneshot(upgrade(&ticket)).await.unwrap();
        assert_eq!(
            error_code(resp).await,
            (StatusCode::UNAUTHORIZED, "TOKEN_REVOKED".to_string())
        );

        // A ticket from a narrower token cannot open what its token could not
        let narrow = TicketOwner {
            user_id: 1,
            token: Some(TokenAuth {
                preset: api_tokens::Preset::ReadOnly,
                ..owner.token.clone().unwrap()
            }),
        };
        let resp = app
            .clone()
            .oneshot(upgrade(&tickets.issue(narrow)))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }
}
