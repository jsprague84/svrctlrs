//! Authentication routes, middleware and the `CurrentUser` extractor

use axum::{
    extract::{FromRequestParts, Query, Request, State},
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
use crate::devices::{self, DeviceAuth, Verdict};
use crate::routes::api::ApiError;
use crate::state::AppState;
use crate::ws_tickets::TicketOwner;
use svrctlrs_api::auth::{LoggedIn, LoginRefused};
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
/// - the session cookie, holding `user_id`, while its device is signed in (`devices.rs`);
/// - a one-time ticket on a WebSocket upgrade (`?ticket=`, see `ws_tickets.rs`);
/// - an API token (`Authorization: Bearer svr_…`, see `api_tokens.rs`): the user is the
///   token's owner, the request must fall inside the token's preset (403 otherwise, decided
///   here for every route), a [`TokenAuth`] extension names the token, and every audit entry
///   the request writes names it too. Never accepted in the URL;
/// - a session token (`Authorization: Bearer <session id>`, or `?token=` on WebSocket
///   upgrades, deprecated in favour of tickets). It is looked up as a session record through
///   the store, so it must be unexpired, and the user id comes from *that record* — never from
///   anything the client sends alongside it. Its device must be signed in too.
///
/// A cookie or session token also gets a [`DeviceAuth`] extension naming its device.
///
/// A request that carries an API token is judged by that token alone, before any cookie is
/// looked at. The reference edge lets requests with `Authorization: Bearer svr_…` past its
/// passkey login, so nothing else may ride along on them: not a session cookie, not a second
/// `Authorization` header, and never a password login (a login with an `Authorization`
/// header is refused).
///
/// Unauthenticated: WebSocket upgrades and `/api/*` get 401; browser requests are redirected
/// to the login page.
pub async fn require_auth(session: Session, mut request: Request, next: Next) -> Response {
    let path = request.uri().path().to_string();

    // Let CORS preflight requests through (OPTIONS method)
    if request.method() == axum::http::Method::OPTIONS {
        return next.run(request).await;
    }

    // One credential per request: a proxy could route on one header and the app read the other
    if request
        .headers()
        .get_all(header::AUTHORIZATION)
        .iter()
        .count()
        > 1
    {
        return ApiError::bad_request("Send at most one Authorization header").into_response();
    }

    // Exempt routes - no auth required (case-insensitive comparison)
    let path_lower = path.to_ascii_lowercase();
    let is_login = path_lower == "/auth/login"
        || path_lower == "/auth/login/totp"
        || path_lower == "/auth/oidc/start"
        || path_lower == "/auth/oidc/callback"
        || path_lower == "/api/v1/auth/login"
        || path_lower == "/api/v1/auth/app-token";
    if is_login && request.headers().contains_key(header::AUTHORIZATION) {
        return ApiError::bad_request(
            "A login request carries no Authorization header; agents use their API token instead",
        )
        .into_response();
    }
    if is_login
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

    let pool = request
        .extensions()
        .get::<svrctlrs_database::SqlxPool<svrctlrs_database::SqlxSqlite>>()
        .cloned();

    // API token: header only, and first, so a cookie sent alongside it counts for nothing
    if let Some(value) = bearer_token(request.headers()).filter(|t| api_tokens::is_api_token(t)) {
        return token_auth(pool, &value, &path, request, next).await;
    }

    // Session cookie, while its device is signed in
    if let Some(user_id) = session.get::<i64>(USER_ID_KEY).await.unwrap_or(None) {
        let (Some(pool), Some(id)) = (pool.as_ref(), session.id()) else {
            return StatusCode::UNAUTHORIZED.into_response();
        };
        let marker = session
            .get::<i64>(devices::DEVICE_KEY)
            .await
            .unwrap_or(None);
        let method = session.get::<String>(AUTH_METHOD_KEY).await.unwrap_or(None);
        match devices::judge(
            pool,
            &id.to_string(),
            user_id,
            marker,
            method.as_deref(),
            request.headers(),
        )
        .await
        {
            Verdict::Valid {
                device_id,
                adopted,
                touched,
            } => {
                if adopted {
                    let _ = session.insert(devices::DEVICE_KEY, device_id).await;
                }
                if touched {
                    // Saved again, so the 24 hours of inactivity start over
                    let now = chrono::Utc::now().timestamp();
                    let _ = session.insert(devices::SEEN_KEY, now).await;
                }
                request.extensions_mut().insert(CurrentUser(user_id));
                request.extensions_mut().insert(DeviceAuth(device_id));
                return next.run(request).await;
            }
            // Signed out from another device: drop the session and answer as signed out
            Verdict::SignedOut => {
                let _ = session.flush().await;
            }
            Verdict::Error => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        }
    }

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
            if let Some(device) = owner.device {
                // Minted by a signed-in device that may have been signed out since
                let valid = match &pool {
                    Some(pool) => svrctlrs_database::queries::devices::exists(pool, device)
                        .await
                        .unwrap_or(false),
                    None => false,
                };
                if !valid {
                    return StatusCode::UNAUTHORIZED.into_response();
                }
            }
            if let Some(token) = &owner.token {
                // Minted with a token: the upgrade gets that token's access, and only while
                // the token is still good (it may have been revoked since).
                if !token.preset.allows(request.method().as_str(), &path) {
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
            if let Some(device) = owner.device {
                request.extensions_mut().insert(DeviceAuth(device));
            }
            return proceed(request, next, owner.user_id, owner.token).await;
        }
    }

    // Session token (the native apps: `Authorization: Bearer <session id>`). Never in a URL:
    // the deprecated `?token=` is gone, and an API token there gets a clear refusal.
    if query_param(request.uri(), "token").is_some_and(|t| !t.is_empty()) {
        return token_error(
            StatusCode::UNAUTHORIZED,
            "TOKEN_IN_URL",
            "Tokens go in the Authorization header, never in a URL; WebSockets take a ticket \
             from POST /api/v1/auth/ws-ticket",
        );
    }
    let token = bearer_token(request.headers());

    if let (Some(token), Some(pool)) = (token, pool) {
        let store = SqliteStore::new(pool.clone());
        if let Some(mut record) = session_for_token(&store, &token).await {
            if let Some(user_id) = record.data.get(USER_ID_KEY).and_then(|v| v.as_i64()) {
                let marker = record
                    .data
                    .get(devices::DEVICE_KEY)
                    .and_then(|v| v.as_i64());
                let method = record
                    .data
                    .get(AUTH_METHOD_KEY)
                    .and_then(|v| v.as_str())
                    .map(String::from);
                match devices::judge(
                    &pool,
                    &token,
                    user_id,
                    marker,
                    method.as_deref(),
                    request.headers(),
                )
                .await
                {
                    Verdict::Valid {
                        device_id, adopted, ..
                    } => {
                        if adopted {
                            record
                                .data
                                .insert(devices::DEVICE_KEY.to_string(), device_id.into());
                            if let Err(e) = store.save(&record).await {
                                tracing::warn!(error = %e, "Could not mark the session's device");
                            }
                        }
                        request.extensions_mut().insert(CurrentUser(user_id));
                        request.extensions_mut().insert(DeviceAuth(device_id));
                        return next.run(request).await;
                    }
                    Verdict::SignedOut => {
                        let _ = store.delete(&record.id).await;
                    }
                    Verdict::Error => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
                }
            }
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

    // Browser requests redirect to login, which comes back to the page (the desktop app's
    // `/auth/app` above all) once signed in
    if request.method() == axum::http::Method::GET {
        if let Some(target) = request
            .uri()
            .path_and_query()
            .map(|p| p.as_str().to_string())
        {
            if is_local_target(&target) {
                let _ = session.insert(AFTER_LOGIN_KEY, target).await;
            }
        }
    }
    Redirect::to("/auth/login").into_response()
}

/// Authenticate by an API token (`value` starts with `svr_`): the token's owner, inside the
/// token's preset, with the approval hold for the gated deletes.
async fn token_auth(
    pool: Option<svrctlrs_database::SqlxPool<svrctlrs_database::SqlxSqlite>>,
    value: &str,
    path: &str,
    request: Request,
    next: Next,
) -> Response {
    let Some(pool) = pool else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    match api_tokens::check(&pool, value, chrono::Utc::now()).await {
        TokenCheck::Valid { token, touch } => {
            if touch {
                let ip = client_ip(request.headers());
                let id = token.id;
                let pool = pool.clone();
                tokio::spawn(async move {
                    if let Err(e) =
                        svrctlrs_database::queries::api_tokens::touch(&pool, id, &ip).await
                    {
                        tracing::warn!(error = %e, token_id = id, "Failed to record token use");
                    }
                });
            }
            if !token.preset.allows(request.method().as_str(), path) {
                return token_outside_preset(&token, request.method(), path);
            }
            // The few destructive calls a token may make only with its owner's yes
            if token.require_approval {
                if let Some((action, target)) = crate::approvals::gated(request.method(), path) {
                    let held = crate::approvals::hold(
                        &pool,
                        &token,
                        action,
                        target,
                        request.method(),
                        path,
                        request.headers(),
                    )
                    .await;
                    if let Some(response) = held {
                        return response;
                    }
                }
            }
            proceed(request, next, token.user_id, Some(token)).await
        }
        TokenCheck::Malformed => token_error(
            StatusCode::UNAUTHORIZED,
            "TOKEN_INVALID",
            "Malformed API token: expected svr_ and 40 letters and digits",
        ),
        TokenCheck::Unknown => token_error(
            StatusCode::UNAUTHORIZED,
            "TOKEN_INVALID",
            "Unknown API token",
        ),
        TokenCheck::Expired(name) => token_error(
            StatusCode::UNAUTHORIZED,
            "TOKEN_EXPIRED",
            format!(
                "The API token \"{name}\" has expired; create a new one under Settings → Access"
            ),
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
    }
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
#[utoipa::path(post, path = "/api/v1/auth/ws-ticket", tag = "auth",
    responses(
        (status = 200, description = "Use as `?ticket=` on `/ws/terminal/pty` within 30 s, once",
            body = svrctlrs_api::auth::WsTicket),
        (status = 403, description = "The token's preset has no terminal", body = svrctlrs_api::ErrorBody),
    ))]
async fn ws_ticket(
    State(state): State<AppState>,
    CurrentUser(user_id): CurrentUser,
    token: Option<axum::Extension<TokenAuth>>,
    device: Option<axum::Extension<DeviceAuth>>,
) -> Json<svrctlrs_api::auth::WsTicket> {
    let ticket = state.ws_tickets.issue(TicketOwner {
        user_id,
        token: token.map(|axum::Extension(t)| t),
        device: device.map(|axum::Extension(DeviceAuth(id))| id),
    });
    Json(svrctlrs_api::auth::WsTicket {
        ticket,
        expires_in_secs: state.ws_tickets.ttl().as_secs(),
    })
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

/// Resolve a session token to its session record. `load` only returns unexpired records, so
/// an expired session is not a valid token either.
async fn session_for_token(
    store: &SqliteStore,
    token: &str,
) -> Option<tower_sessions::session::Record> {
    let id: Id = token.parse().ok()?;
    store.load(&id).await.ok().flatten()
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
    Router::from(api_routes())
        .route("/auth/login", get(login_page).post(login))
        .route("/auth/login/totp", post(login_totp))
        .route("/auth/logout", post(logout))
        .merge(super::oidc::routes())
        .merge(super::app_sign_in::routes())
}

/// The JSON auth routes, which the OpenAPI document describes.
pub fn api_routes() -> utoipa_axum::router::OpenApiRouter<AppState> {
    utoipa_axum::router::OpenApiRouter::new()
        .routes(utoipa_axum::routes!(login_json))
        .routes(utoipa_axum::routes!(super::app_sign_in::app_token))
        .routes(utoipa_axum::routes!(ws_ticket))
}

/// Login form data
#[derive(Debug, Deserialize)]
struct LoginForm {
    username: String,
    password: String,
}

/// The second step's form: a TOTP code or a recovery code
#[derive(Debug, Deserialize)]
struct CodeForm {
    code: String,
}

/// Session key of the authentication method of the signed-in session (`password`,
/// `password+totp`, `password+recovery_code`, `oidc`), for the session list to come.
pub(crate) const AUTH_METHOD_KEY: &str = "auth_method";
/// Session key of the page a signed-out browser asked for, to return to after signing in
pub(crate) const AFTER_LOGIN_KEY: &str = "after_login";

/// A path on this server to send the browser back to: absolute, not protocol-relative, and not
/// an API or socket route.
pub(crate) fn is_local_target(target: &str) -> bool {
    if target.starts_with("/auth/app?") {
        return true;
    }
    target.starts_with('/')
        && !target.starts_with("//")
        && !target.contains('\\')
        && !["/api/", "/ws/", "/auth/"]
            .iter()
            .any(|prefix| target.starts_with(prefix))
}

/// Where a browser goes after signing in: the page it asked for, once, or the app.
pub(crate) async fn take_after_login(session: &Session) -> String {
    session
        .remove::<String>(AFTER_LOGIN_KEY)
        .await
        .ok()
        .flatten()
        .filter(|t| is_local_target(t))
        .unwrap_or_else(|| "/".to_string())
}

/// Session key of a password sign-in waiting for its second factor
const PENDING_LOGIN_KEY: &str = "pending_login";
/// How long the second step may take, and how many wrong codes it allows, before the
/// password has to be entered again.
const PENDING_LOGIN_SECS: i64 = 5 * 60;
const PENDING_LOGIN_TRIES: u32 = 5;

/// A password that was right, for an account that also wants a code. Held in the (cycled)
/// session; it signs nobody in.
#[derive(Debug, Clone, serde::Serialize, Deserialize)]
struct PendingLogin {
    user_id: i64,
    username: String,
    started_at: i64,
    tries: u32,
}

pub(crate) fn html_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// The standalone sign-in page around `body` (the server renders it; the SPA is not loaded
/// until someone is signed in).
pub(crate) fn render_auth_page(subtitle: &str, error: Option<&str>, body: &str) -> String {
    let error_block = error
        .map(|msg| {
            format!(
                r#"<div class="error" role="alert">{}</div>"#,
                html_escape(msg)
            )
        })
        .unwrap_or_default();
    let subtitle = html_escape(subtitle);

    format!(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="color-scheme" content="dark light">
<link rel="icon" href="/favicon.svg" type="image/svg+xml">
<title>SvrCtlRS — Sign in</title>
<style>
:root{{--page:#1a1b26;--card:#1f2335;--line:#292e42;--field:#16161e;--field-line:#6b74a6;--ink:#c0caf5;--ink-2:#a9b1d6;--muted:#8a90b1;--accent:#7aa2f7;--on-accent:#1a1b26;--error:#f7768e;--shadow:0 8px 32px rgba(0,0,0,.35);color-scheme:dark}}
@media (prefers-color-scheme:light){{:root{{--page:#f3f4f7;--card:#fff;--line:#d4d8e1;--field:#fff;--field-line:#7b8197;--ink:#1f2335;--ink-2:#3b4261;--muted:#5a6075;--accent:#2c58b0;--on-accent:#fff;--error:#b3323d;--shadow:0 1px 2px rgba(31,35,53,.06),0 8px 24px rgba(31,35,53,.08);color-scheme:light}}}}
*,*::before,*::after{{box-sizing:border-box;margin:0;padding:0}}
body{{font-family:system-ui,-apple-system,"Segoe UI",Roboto,sans-serif;background:var(--page);color:var(--ink);display:flex;align-items:center;justify-content:center;min-height:100vh;min-height:100dvh;padding:1rem}}
.card{{background:var(--card);border:1px solid var(--line);border-radius:12px;padding:2rem 2rem 2.25rem;width:100%;max-width:380px;box-shadow:var(--shadow)}}
.brand{{display:flex;align-items:center;justify-content:center;gap:.6rem;margin-bottom:.35rem}}
.brand svg{{width:32px;height:32px;flex:none}}
h1{{font-size:1.375rem;font-weight:650;letter-spacing:-.01em}}
.sub{{color:var(--muted);font-size:.875rem;text-align:center;margin-bottom:1.75rem}}
.error{{background:color-mix(in srgb,var(--error) 10%,var(--card));color:var(--error);border:1px solid color-mix(in srgb,var(--error) 35%,var(--card));padding:.7rem .9rem;border-radius:8px;margin-bottom:1.25rem;font-size:.875rem}}
label{{display:block;font-size:.875rem;font-weight:500;color:var(--ink-2);margin-bottom:.375rem}}
input{{width:100%;padding:.625rem .75rem;background:var(--field);border:1px solid var(--field-line);border-radius:8px;color:var(--ink);font-size:1rem;outline:none;transition:border-color .15s,box-shadow .15s}}
input:focus{{border-color:var(--accent);box-shadow:0 0 0 3px color-mix(in srgb,var(--accent) 25%,transparent)}}
.field{{margin-bottom:1.1rem}}
button,.button{{display:block;width:100%;min-height:44px;padding:.75rem;background:var(--accent);color:var(--on-accent);border:1px solid transparent;border-radius:8px;font:inherit;font-size:.9375rem;font-weight:600;cursor:pointer;transition:filter .15s,background .15s;text-align:center;text-decoration:none}}
button:hover,.button:hover{{filter:brightness(1.08)}}
button:focus-visible,.button:focus-visible,a:focus-visible{{outline:2px solid var(--accent);outline-offset:2px}}
.secondary{{background:transparent;color:var(--ink);border-color:var(--field-line)}}
.secondary:hover{{filter:none;background:color-mix(in srgb,var(--ink) 6%,transparent)}}
.or{{display:flex;align-items:center;gap:.75rem;color:var(--muted);font-size:.75rem;margin:1.5rem 0}}
.or::before,.or::after{{content:"";flex:1;border-top:1px solid var(--line)}}
.hint{{color:var(--muted);font-size:.75rem;margin-top:.375rem}}
.links{{margin-top:1.25rem;text-align:center;font-size:.8125rem}}
.links a{{color:var(--accent)}}
</style>
</head>
<body>
<main class="card">
<div class="brand"><svg viewBox="0 0 32 32" aria-hidden="true"><rect width="32" height="32" rx="7" fill="#1a1b26"/><path d="M8 11l5 5-5 5" stroke="#7aa2f7" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" fill="none"/><line x1="16" y1="21" x2="24" y2="21" stroke="#565f89" stroke-width="2.5" stroke-linecap="round"/></svg><h1>SvrCtlRS</h1></div>
<p class="sub">{subtitle}</p>
{error_block}
{body}
</main>
</body>
</html>"##
    )
}

const PASSWORD_FORM: &str = r#"<form method="post" action="/auth/login">
<div class="field"><label for="username">Username</label><input id="username" name="username" type="text" autocomplete="username" required autofocus></div>
<div class="field"><label for="password">Password</label><input id="password" name="password" type="password" autocomplete="current-password" required></div>
<button type="submit">Sign in</button>
</form>"#;

const CODE_FORM: &str = r#"<form method="post" action="/auth/login/totp">
<div class="field"><label for="code">Code</label><input id="code" name="code" type="text" autocomplete="one-time-code" autocapitalize="none" spellcheck="false" required autofocus>
<p class="hint">Six digits from your authenticator app, or one of your recovery codes.</p></div>
<button type="submit">Continue</button>
</form>
<p class="links"><a href="/auth/login">Start over</a></p>"#;

/// The login page for this server's sign-in methods: the provider's button, the password form,
/// or both.
pub(crate) fn login_html(sign_in: &crate::sign_in::SignInConfig, error: Option<&str>) -> String {
    let mut body = String::new();
    if let Some(provider) = sign_in.provider() {
        body.push_str(&format!(
            r#"<a class="button" href="/auth/oidc/start">Sign in with {}</a>"#,
            html_escape(&provider.config.name)
        ));
        if sign_in.password_login {
            body.push_str(r#"<div class="or">or with a password</div>"#);
            body.push_str(
                &PASSWORD_FORM
                    .replace(
                        r#"<button type="submit">"#,
                        r#"<button type="submit" class="secondary">"#,
                    )
                    .replace(" autofocus", ""),
            );
        }
    } else {
        body.push_str(PASSWORD_FORM);
    }
    render_auth_page("Sign in to continue", error, &body)
}

fn render_code_html(error: Option<&str>) -> String {
    render_auth_page("Two-factor sign-in", error, CODE_FORM)
}

/// Login page handler
async fn login_page(
    State(state): State<AppState>,
    Query(query): Query<super::oidc::LoginPageQuery>,
) -> Response {
    if let Some(redirect) = super::oidc::auto_redirect(&state, &query) {
        return redirect;
    }
    Html(login_html(&state.sign_in, None)).into_response()
}

/// The answer to a password sent to a server that takes none
fn password_login_off(state: &AppState) -> Response {
    (
        StatusCode::FORBIDDEN,
        Html(login_html(
            &state.sign_in,
            Some("Password sign-in is off on this server."),
        )),
    )
        .into_response()
}

/// Why a sign-in step was refused. Credential and rate-limit failures are told apart only in
/// the status code; the message never says whether the username exists.
#[derive(Debug)]
enum LoginFailure {
    RateLimited(std::time::Duration),
    InvalidCredentials,
    /// The password was right; the code was not
    InvalidCode,
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

/// Refuse the attempt when one of `keys` is over the limit (or the limiter cannot be read).
async fn check_limit(
    state: &AppState,
    keys: &[String],
    username: &str,
    ip: &str,
) -> Result<(), LoginFailure> {
    match state.login_limiter.check(keys).await {
        Ok(None) => Ok(()),
        Ok(Some(limited)) => {
            tracing::warn!(key = %limited.key, retry_after_secs = limited.retry_after.as_secs(), "Login rate limited");
            crate::audit::record(
                &state.pool,
                NewAuditEntry::new("login.rate_limited")
                    .username(username)
                    .client_ip(Some(ip.to_string())),
            );
            Err(LoginFailure::RateLimited(limited.retry_after))
        }
        Err(e) => {
            tracing::error!(error = %e, "Login limiter unavailable; refusing the attempt");
            Err(LoginFailure::Internal)
        }
    }
}

async fn record_failure(state: &AppState, keys: &[String]) {
    if let Err(e) = state.login_limiter.record_failure(keys).await {
        tracing::error!(error = %e, "Failed to record a login failure");
    }
}

/// The password step: rate limit, look up, verify, record a failure. On success, the user and
/// whether the account also wants a TOTP code. Nobody is signed in yet.
async fn check_password(
    state: &AppState,
    ip: &str,
    creds: &LoginForm,
) -> Result<(svrctlrs_database::models::User, bool), LoginFailure> {
    use svrctlrs_database::queries::{user_totp, users};

    let keys = crate::login_limiter::LoginLimiter::keys(ip, &creds.username);
    check_limit(state, &keys, &creds.username, ip).await?;

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
        record_failure(state, &keys).await;
        tracing::warn!(username = %creds.username, "Login failed");
        crate::audit::record(
            &state.pool,
            NewAuditEntry::new("login.failed")
                .username(&creds.username)
                .client_ip(Some(ip.to_string())),
        );
        return Err(LoginFailure::InvalidCredentials);
    };

    let needs_code = user_totp::is_enabled(&state.pool, user.id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Could not read the TOTP enrolment");
            LoginFailure::Internal
        })?;
    Ok((user, needs_code))
}

/// The second step: a TOTP code (each time step once) or an unused recovery code. Failures
/// count against the same limiter keys as the password. Returns the sign-in method.
async fn check_code(
    state: &AppState,
    ip: &str,
    user_id: i64,
    username: &str,
    code: &str,
) -> Result<&'static str, LoginFailure> {
    use crate::sign_in::totp;
    use svrctlrs_database::queries::user_totp;

    let keys = crate::login_limiter::LoginLimiter::keys(ip, username);
    check_limit(state, &keys, username, ip).await?;
    let internal = |e: svrctlrs_core::Error| {
        tracing::error!(error = %e, "Database error during the second sign-in step");
        LoginFailure::Internal
    };

    let enrolment = user_totp::get(&state.pool, user_id)
        .await
        .map_err(internal)?;
    let method = match enrolment.filter(|t| t.is_enabled()) {
        // The enrolment went away in between (turned off): the password alone is enough now
        None => Some("password"),
        Some(t) => {
            if totp::normalise_code(code).is_some() {
                let now = chrono::Utc::now().timestamp();
                match totp::verify(&t.secret, code, now, t.last_step) {
                    Some(step) => user_totp::accept_step(&state.pool, user_id, step)
                        .await
                        .map_err(internal)?
                        .then_some("password+totp"),
                    None => None,
                }
            } else if let Some(hash) = totp::recovery_hash(code) {
                user_totp::use_recovery_code(&state.pool, user_id, &hash)
                    .await
                    .map_err(internal)?
                    .then_some("password+recovery_code")
            } else {
                None
            }
        }
    };

    if let Some(method) = method {
        return Ok(method);
    }
    record_failure(state, &keys).await;
    tracing::warn!(username = %username, "Second sign-in step failed");
    crate::audit::record(
        &state.pool,
        NewAuditEntry::new("login.code_failed")
            .user(user_id)
            .username(username)
            .client_ip(Some(ip.to_string())),
    );
    Err(LoginFailure::InvalidCode)
}

/// Sign `user` in: forget the limiter's failures, start a fresh session (a new id, so a
/// pre-login session cannot be fixated), record it as a device, and audit.
pub(crate) async fn start_session(
    state: &AppState,
    session: &Session,
    user_id: i64,
    username: &str,
    headers: &HeaderMap,
    method: &str,
) -> Result<(), String> {
    let ip = client_ip(headers);
    let keys = crate::login_limiter::LoginLimiter::keys(&ip, username);
    if let Err(e) = state.login_limiter.clear(&keys).await {
        tracing::error!(error = %e, "Failed to clear login failures");
    }

    session.cycle_id().await.map_err(|e| {
        tracing::error!(error = %e, "Failed to cycle session id");
        "Failed to cycle session id".to_string()
    })?;
    let _ = session.remove::<PendingLogin>(PENDING_LOGIN_KEY).await;
    session.insert(USER_ID_KEY, user_id).await.map_err(|e| {
        tracing::error!(error = %e, "Failed to store session");
        "Failed to store session".to_string()
    })?;
    session
        .insert(AUTH_METHOD_KEY, method)
        .await
        .map_err(|e| e.to_string())?;
    // Saved now, so the session has the id its device is recorded under
    session.save().await.map_err(|e| {
        tracing::error!(error = %e, "Failed to save session");
        "Failed to save session".to_string()
    })?;
    let id = session.id().ok_or("The session has no id")?;
    let device = devices::record(&state.pool, user_id, &id.to_string(), method, headers)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to record the sign-in");
            "Failed to record the sign-in".to_string()
        })?;
    session
        .insert(devices::DEVICE_KEY, device.id)
        .await
        .map_err(|e| e.to_string())?;

    crate::audit::record(
        &state.pool,
        NewAuditEntry::new("login")
            .user(user_id)
            .username(username)
            .client_ip(Some(ip))
            .details(serde_json::json!({ "method": method, "device_id": device.id })),
    );
    tracing::info!(username = %username, user_id, method, device_id = device.id, "User logged in");
    Ok(())
}

fn retry_after_message(retry_after: std::time::Duration) -> String {
    let minutes = retry_after.as_secs().div_ceil(60).max(1);
    format!("Too many failed attempts. Try again in {minutes} minute(s).")
}

fn html_failure(failure: LoginFailure, page: &dyn Fn(Option<&str>) -> String) -> Response {
    match failure {
        LoginFailure::RateLimited(retry_after) => (
            StatusCode::TOO_MANY_REQUESTS,
            [(
                header::RETRY_AFTER,
                retry_after.as_secs().max(1).to_string(),
            )],
            Html(page(Some(&retry_after_message(retry_after)))),
        )
            .into_response(),
        LoginFailure::InvalidCredentials => (
            StatusCode::OK,
            Html(page(Some("Invalid username or password"))),
        )
            .into_response(),
        LoginFailure::InvalidCode => {
            (StatusCode::OK, Html(page(Some("That code did not work")))).into_response()
        }
        LoginFailure::Internal => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Html(page(Some("Internal server error"))),
        )
            .into_response(),
    }
}

fn internal_page(state: &AppState) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Html(login_html(&state.sign_in, Some("Internal server error"))),
    )
        .into_response()
}

/// Login form submission handler
async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    session: Session,
    Form(creds): Form<LoginForm>,
) -> Response {
    if !state.sign_in.password_login {
        return password_login_off(&state);
    }
    if !is_same_origin_request(&headers) {
        return (
            StatusCode::FORBIDDEN,
            Html(login_html(&state.sign_in, Some("Cross-site login refused"))),
        )
            .into_response();
    }
    let page = |e: Option<&str>| login_html(&state.sign_in, e);
    let ip = client_ip(&headers);
    match check_password(&state, &ip, &creds).await {
        Ok((user, false)) => {
            match start_session(
                &state,
                &session,
                user.id,
                &user.username,
                &headers,
                "password",
            )
            .await
            {
                Ok(()) => Redirect::to(&take_after_login(&session).await).into_response(),
                Err(_) => internal_page(&state),
            }
        }
        Ok((user, true)) => {
            // Hold the right password in a fresh session until the code arrives
            let pending = PendingLogin {
                user_id: user.id,
                username: user.username.clone(),
                started_at: chrono::Utc::now().timestamp(),
                tries: 0,
            };
            if session.cycle_id().await.is_err()
                || session.insert(PENDING_LOGIN_KEY, &pending).await.is_err()
            {
                return internal_page(&state);
            }
            Html(render_code_html(None)).into_response()
        }
        Err(failure) => html_failure(failure, &page),
    }
}

/// The second step of a password sign-in (TOTP or recovery code)
async fn login_totp(
    State(state): State<AppState>,
    headers: HeaderMap,
    session: Session,
    Form(form): Form<CodeForm>,
) -> Response {
    if !state.sign_in.password_login {
        return password_login_off(&state);
    }
    if !is_same_origin_request(&headers) {
        return (
            StatusCode::FORBIDDEN,
            Html(login_html(&state.sign_in, Some("Cross-site login refused"))),
        )
            .into_response();
    }
    let page = |e: Option<&str>| login_html(&state.sign_in, e);
    let Some(mut pending) = session
        .get::<PendingLogin>(PENDING_LOGIN_KEY)
        .await
        .unwrap_or(None)
    else {
        return Html(page(Some("Sign in again"))).into_response();
    };
    if chrono::Utc::now().timestamp() - pending.started_at > PENDING_LOGIN_SECS {
        let _ = session.remove::<PendingLogin>(PENDING_LOGIN_KEY).await;
        return Html(page(Some("That took too long. Sign in again."))).into_response();
    }
    let ip = client_ip(&headers);
    match check_code(&state, &ip, pending.user_id, &pending.username, &form.code).await {
        Ok(method) => {
            match start_session(
                &state,
                &session,
                pending.user_id,
                &pending.username,
                &headers,
                method,
            )
            .await
            {
                Ok(()) => Redirect::to(&take_after_login(&session).await).into_response(),
                Err(_) => internal_page(&state),
            }
        }
        Err(LoginFailure::InvalidCode) => {
            pending.tries += 1;
            if pending.tries >= PENDING_LOGIN_TRIES {
                let _ = session.remove::<PendingLogin>(PENDING_LOGIN_KEY).await;
                return Html(page(Some("Too many wrong codes. Sign in again."))).into_response();
            }
            if session.insert(PENDING_LOGIN_KEY, &pending).await.is_err() {
                return internal_page(&state);
            }
            html_failure(LoginFailure::InvalidCode, &render_code_html)
        }
        Err(failure) => {
            let _ = session.remove::<PendingLogin>(PENDING_LOGIN_KEY).await;
            html_failure(failure, &page)
        }
    }
}

fn refused(status: StatusCode, message: impl Into<String>) -> (StatusCode, Json<LoginRefused>) {
    (
        status,
        Json(LoginRefused {
            error: message.into(),
            code: None,
        }),
    )
}

fn refused_with(
    status: StatusCode,
    code: &str,
    message: impl Into<String>,
) -> (StatusCode, Json<LoginRefused>) {
    (
        status,
        Json(LoginRefused {
            error: message.into(),
            code: Some(code.to_string()),
        }),
    )
}

fn json_failure(failure: LoginFailure) -> Response {
    match failure {
        LoginFailure::RateLimited(retry_after) => (
            [(
                header::RETRY_AFTER,
                retry_after.as_secs().max(1).to_string(),
            )],
            refused(
                StatusCode::TOO_MANY_REQUESTS,
                retry_after_message(retry_after),
            ),
        )
            .into_response(),
        LoginFailure::InvalidCredentials => {
            refused(StatusCode::UNAUTHORIZED, "Invalid username or password").into_response()
        }
        LoginFailure::InvalidCode => refused_with(
            StatusCode::UNAUTHORIZED,
            "TOTP_INVALID",
            "That code did not work",
        )
        .into_response(),
        LoginFailure::Internal => {
            refused(StatusCode::INTERNAL_SERVER_ERROR, "Internal server error").into_response()
        }
    }
}

/// JSON login handler for Tauri/SPA clients (no redirect, returns JSON). An account with
/// two-factor sign-in on also needs `totp` (a code or a recovery code); without it the answer
/// is 401 with `code: TOTP_REQUIRED`.
#[utoipa::path(post, path = "/api/v1/auth/login", tag = "auth", security(()),
    request_body = svrctlrs_api::auth::Login,
    responses(
        (status = 200, description = "Signed in; the session cookie is set too", body = LoggedIn),
        (status = 401, description = "Wrong password, or a code is required (`TOTP_REQUIRED`) or wrong (`TOTP_INVALID`)", body = LoginRefused),
        (status = 403, description = "Password sign-in is off (`PASSWORD_LOGIN_OFF`)", body = LoginRefused),
        (status = 429, description = "Too many failures; see Retry-After", body = LoginRefused),
    ))]
async fn login_json(
    State(state): State<AppState>,
    headers: HeaderMap,
    session: Session,
    Json(login): Json<svrctlrs_api::auth::Login>,
) -> Response {
    if !state.sign_in.password_login {
        return refused_with(
            StatusCode::FORBIDDEN,
            "PASSWORD_LOGIN_OFF",
            "Password sign-in is off on this server",
        )
        .into_response();
    }
    let creds = LoginForm {
        username: login.username,
        password: login.password,
    };
    let ip = client_ip(&headers);
    let (user, needs_code) = match check_password(&state, &ip, &creds).await {
        Ok(found) => found,
        Err(failure) => return json_failure(failure),
    };
    let method = if needs_code {
        let Some(code) = login.totp.filter(|c| !c.trim().is_empty()) else {
            return refused_with(
                StatusCode::UNAUTHORIZED,
                "TOTP_REQUIRED",
                "This account also needs a code from its authenticator app",
            )
            .into_response();
        };
        match check_code(&state, &ip, user.id, &user.username, &code).await {
            Ok(method) => method,
            Err(failure) => return json_failure(failure),
        }
    } else {
        "password"
    };
    if start_session(&state, &session, user.id, &user.username, &headers, method)
        .await
        .is_err()
    {
        return refused(StatusCode::INTERNAL_SERVER_ERROR, "Session error").into_response();
    }

    // Save now so the (cycled) id exists for clients that use it as a Bearer token
    if let Err(e) = session.save().await {
        tracing::error!(error = %e, "Failed to save session");
        return refused(StatusCode::INTERNAL_SERVER_ERROR, "Session error").into_response();
    }
    let session_id = session.id().map(|id| id.to_string()).unwrap_or_default();

    Json(LoggedIn {
        success: true,
        user_id: user.id,
        session_id,
    })
    .into_response()
}

/// Logout handler
async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap,
    session: Session,
) -> Result<Response, StatusCode> {
    // The desktop app signs out with its own session token: that session ends (the browser's
    // cookie session is a different one). No cross-site check is needed, since a page cannot
    // send an Authorization header to another site without a CORS preflight it would fail.
    if let Some(token) = bearer_token(&headers).filter(|t| !api_tokens::is_api_token(t)) {
        let store = SqliteStore::new(state.pool.clone());
        if let Ok(id) = token.parse::<Id>() {
            if let Ok(Some(record)) = store.load(&id).await {
                let device = devices::session_ending(&state, &token).await;
                if let Some(user_id) = record.data.get(USER_ID_KEY).and_then(|v| v.as_i64()) {
                    crate::audit::record(
                        &state.pool,
                        NewAuditEntry::new("logout")
                            .user(user_id)
                            .client_ip(Some(client_ip(&headers)))
                            .details(serde_json::json!({ "device_id": device })),
                    );
                }
                store
                    .delete(&id)
                    .await
                    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            }
        }
        return Ok(StatusCode::NO_CONTENT.into_response());
    }

    if !is_same_origin_request(&headers) {
        return Err(StatusCode::FORBIDDEN);
    }
    if let Some(user_id) = session.get::<i64>(USER_ID_KEY).await.unwrap_or(None) {
        let device = match session.id() {
            Some(id) => devices::session_ending(&state, &id.to_string()).await,
            None => None,
        };
        crate::audit::record(
            &state.pool,
            NewAuditEntry::new("logout")
                .user(user_id)
                .client_ip(Some(client_ip(&headers)))
                .details(serde_json::json!({ "device_id": device })),
        );
    }
    session
        .flush()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    tracing::info!("User logged out");
    // Not straight back to the provider, whose own session would sign you in again
    Ok(Redirect::to("/auth/login?signed_out=1").into_response())
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
        // Signed-in sessions are recorded as devices, which belong to accounts
        svrctlrs_database::run_migrations(&pool).await.unwrap();
        for id in [5, 7, 9, 42] {
            svrctlrs_database::sqlx::query(
                "INSERT INTO users (id, username, password_hash) VALUES (?, ?, 'x')",
            )
            .bind(id)
            .bind(format!("user{id}"))
            .execute(&pool)
            .await
            .unwrap();
        }
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
    async fn a_session_token_in_the_url_is_refused() {
        // `?token=` was the native apps' old way onto a WebSocket; tickets replaced it and a
        // durable credential in a URL ends up in proxy and access logs.
        let (app, store, _) = app_with_store().await;
        let token = session_token(&store, 7, time::Duration::hours(1)).await;
        for uri in [
            format!("/api/v1/whoami?token={token}"),
            format!("/ws/terminal/pty?token={token}"),
        ] {
            let resp = app
                .clone()
                .oneshot(Request::get(&uri).body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(resp.status(), StatusCode::UNAUTHORIZED, "{uri}");
            assert!(body_text(resp).await.contains("TOKEN_IN_URL"), "{uri}");
        }
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
            device: None,
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
            device: None,
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
        let cookie = session_cookie(&login);

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

    /// The `name=value` part of the session cookie a login response sets.
    fn session_cookie(resp: &Response) -> String {
        resp.headers()
            .get(header::SET_COOKIE)
            .expect("a session cookie was set")
            .to_str()
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .to_string()
    }

    // The reference edge lets `Authorization: Bearer svr_…` requests past its passkey login.
    // These three keep that door from leading anywhere a token does not.

    #[tokio::test]
    async fn a_login_with_an_authorization_header_is_refused() {
        let (app, _, _) = app_with_store().await;
        let fake = format!("Bearer svr_{}", "a".repeat(40));
        for (path, value) in [
            ("/api/v1/auth/login?id=5", fake.as_str()),
            ("/API/V1/AUTH/LOGIN?id=5", fake.as_str()),
            ("/api/v1/auth/login?id=5", "Basic YW5uOng="),
            ("/auth/login", fake.as_str()),
        ] {
            let resp = app
                .clone()
                .oneshot(
                    Request::get(path)
                        .header(header::AUTHORIZATION, value)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(
                resp.status(),
                StatusCode::BAD_REQUEST,
                "{path} with {value}"
            );
            assert!(
                resp.headers().get(header::SET_COOKIE).is_none(),
                "{path}: no session may come of it"
            );
        }
        // Without the header the same login goes through.
        let resp = app
            .oneshot(
                Request::get("/api/v1/auth/login?id=5")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn two_authorization_headers_are_refused() {
        let (app, store, _) = app_with_store().await;
        let session = session_token(&store, 42, time::Duration::hours(1)).await;
        let resp = app
            .oneshot(
                Request::get("/api/v1/whoami")
                    .header(
                        header::AUTHORIZATION,
                        format!("Bearer svr_{}", "a".repeat(40)),
                    )
                    .header(header::AUTHORIZATION, format!("Bearer {session}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
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
                require_approval: false,
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
    async fn a_session_cookie_does_not_ride_along_with_an_api_token() {
        let (app, pool, _) = token_app().await;
        let login = app
            .clone()
            .oneshot(
                Request::get("/api/v1/auth/login")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let cookie = session_cookie(&login);
        let with_cookie = |token: &str| {
            let mut request = with_bearer("GET", "/api/v1/status", token);
            request
                .headers_mut()
                .insert(header::COOKIE, cookie.parse().unwrap());
            request
        };

        // The cookie alone is ann.
        let resp = app
            .clone()
            .oneshot(
                Request::get("/api/v1/status")
                    .header(header::COOKIE, &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(body_text(resp).await, "1");

        // With a made-up token next to it, the token decides: refused.
        let resp = app
            .clone()
            .oneshot(with_cookie(&format!("svr_{}", "a".repeat(40))))
            .await
            .unwrap();
        assert_eq!(
            error_code(resp).await,
            (StatusCode::UNAUTHORIZED, "TOKEN_INVALID".into())
        );

        // With a real token next to it, the request is the token's, preset and all.
        let (_, token) = make_token(&pool, "hermes", api_tokens::Preset::ReadOnly).await;
        let resp = app.oneshot(with_cookie(&token)).await.unwrap();
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
                require_approval: false,
            }),
            device: None,
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
            device: None,
        };
        let resp = app
            .clone()
            .oneshot(upgrade(&tickets.issue(narrow)))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }
}
