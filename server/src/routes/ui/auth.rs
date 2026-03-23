//! Authentication routes and middleware

use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
    Form, Json, Router,
};
use serde::Deserialize;
use tower_sessions::Session;

use crate::state::AppState;

/// Session key for the authenticated user's ID
const USER_ID_KEY: &str = "user_id";

/// Authentication middleware that protects all routes except exempted ones.
///
/// Exempt routes: /auth/login, /auth/logout, /api/v1/health, SPA static assets
///
/// Behavior for unauthenticated requests:
/// - WebSocket upgrades: 401 Unauthorized
/// - API requests (/api/*): 401 JSON response
/// - Browser requests: redirect to /auth/login
pub async fn require_auth(session: Session, request: Request, next: Next) -> Response {
    let path = request.uri().path();

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
        // SPA static assets (SvelteKit immutable hashed chunks)
        || path_lower.starts_with("/_app/")
    {
        return next.run(request).await;
    }

    // Check session cookie for authenticated user
    let user_id: Option<i64> = session.get(USER_ID_KEY).await.unwrap_or(None);

    if user_id.is_some() {
        return next.run(request).await;
    }

    // Fallback: token-based auth for Tauri clients
    // (WebKitGTK doesn't send cross-origin cookies)
    // Check Authorization header first, then query params (for WebSocket which can't send headers)
    let (token, uid_str) = {
        // Try Authorization header + X-User-Id header
        let header_token = request
            .headers()
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.strip_prefix("Bearer "))
            .filter(|t| !t.is_empty())
            .map(String::from);
        let header_uid = request
            .headers()
            .get("x-user-id")
            .and_then(|v| v.to_str().ok())
            .map(String::from);

        if header_token.is_some() {
            (header_token, header_uid)
        } else {
            // Try query params (?token=...&user_id=...) for WebSocket connections
            let query = request.uri().query().unwrap_or("");
            let params: std::collections::HashMap<String, String> = query
                .split('&')
                .filter_map(|pair| {
                    let mut parts = pair.splitn(2, '=');
                    Some((
                        parts.next()?.to_string(),
                        parts.next().unwrap_or("").to_string(),
                    ))
                })
                .collect();
            (
                params.get("token").cloned().filter(|t| !t.is_empty()),
                params.get("user_id").cloned(),
            )
        }
    };

    if let Some(token) = token {
        if let Some(pool) = request
            .extensions()
            .get::<svrctlrs_database::SqlxPool<svrctlrs_database::SqlxSqlite>>()
        {
            let exists =
                svrctlrs_database::sqlx::query("SELECT id FROM tower_sessions WHERE id = ?")
                    .bind(&token)
                    .fetch_optional(pool)
                    .await
                    .ok()
                    .flatten()
                    .is_some();

            if exists {
                if let Some(uid) = uid_str.and_then(|s| s.parse::<i64>().ok()) {
                    let _ = session.insert(USER_ID_KEY, uid).await;
                }
                return next.run(request).await;
            }
        }
    }

    // Unauthenticated — determine response type

    // WebSocket upgrade requests get 401
    let is_websocket = request
        .headers()
        .get("upgrade")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.eq_ignore_ascii_case("websocket"));

    if is_websocket {
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

/// Create auth router
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/auth/login", get(login_page).post(login))
        .route("/api/v1/auth/login", post(login_json))
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

/// Login form submission handler
async fn login(
    State(state): State<AppState>,
    session: Session,
    Form(creds): Form<LoginForm>,
) -> Result<Response, (StatusCode, Html<String>)> {
    use svrctlrs_database::queries::users;

    // Look up the user by username
    let user = users::get_user_by_username(&state.pool, &creds.username)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Database error during login");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(render_login_html(Some("Internal server error"))),
            )
        })?;

    let user = match user {
        Some(u) => u,
        None => {
            tracing::warn!(username = %creds.username, "Login failed: user not found");
            return Err((
                StatusCode::OK,
                Html(render_login_html(Some("Invalid username or password"))),
            ));
        }
    };

    // Verify password
    let valid = users::verify_password(&creds.password, &user.password_hash).map_err(|e| {
        tracing::error!(error = %e, "Password verification error");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Html(render_login_html(Some("Internal server error"))),
        )
    })?;

    if !valid {
        tracing::warn!(username = %creds.username, "Login failed: invalid password");
        return Err((
            StatusCode::OK,
            Html(render_login_html(Some("Invalid username or password"))),
        ));
    }

    // Store user_id in session
    session.insert(USER_ID_KEY, user.id).await.map_err(|e| {
        tracing::error!(error = %e, "Failed to store session");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Html(render_login_html(Some("Internal server error"))),
        )
    })?;

    tracing::info!(username = %user.username, user_id = user.id, "User logged in");
    Ok(Redirect::to("/").into_response())
}

/// JSON login handler for Tauri/SPA clients (no redirect, returns JSON)
async fn login_json(
    State(state): State<AppState>,
    session: Session,
    Json(creds): Json<LoginForm>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    use svrctlrs_database::queries::users;

    let user = users::get_user_by_username(&state.pool, &creds.username)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Database error during login");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Internal server error"})),
            )
        })?;

    let user = match user {
        Some(u) => u,
        None => {
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"error": "Invalid username or password"})),
            ));
        }
    };

    let valid = users::verify_password(&creds.password, &user.password_hash).map_err(|e| {
        tracing::error!(error = %e, "Password verification error");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": "Internal server error"})),
        )
    })?;

    if !valid {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": "Invalid username or password"})),
        ));
    }

    session.insert(USER_ID_KEY, user.id).await.map_err(|e| {
        tracing::error!(error = %e, "Failed to store session");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": "Session error"})),
        )
    })?;

    // Force save the session so it gets an ID assigned
    session.save().await.map_err(|e| {
        tracing::error!(error = %e, "Failed to save session");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": "Session error"})),
        )
    })?;

    // Return session ID so Tauri clients can use token-based auth
    // (WebKitGTK doesn't reliably send cross-origin cookies)
    let session_id = session.id().map(|id| id.to_string()).unwrap_or_default();

    tracing::info!(username = %user.username, user_id = user.id, "User logged in (JSON)");
    Ok(Json(
        serde_json::json!({"success": true, "user_id": user.id, "session_id": session_id}),
    ))
}

/// Logout handler
async fn logout(session: Session) -> Result<Redirect, StatusCode> {
    session
        .flush()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    tracing::info!("User logged out");
    Ok(Redirect::to("/auth/login"))
}
