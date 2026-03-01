//! Authentication routes and middleware

use askama::Template;
use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
    Json, Router,
};
use axum_extra::extract::Form;
use tower_sessions::Session;

use super::AppError;
use crate::{state::AppState, templates::*};

/// Session key for the authenticated user's ID
const USER_ID_KEY: &str = "user_id";

/// Authentication middleware that protects all routes except exempted ones.
///
/// Exempt routes: /auth/login, /auth/logout, /static/*, /api/v1/health
///
/// Behavior for unauthenticated requests:
/// - WebSocket upgrades: 401 Unauthorized
/// - API requests (/api/*): 401 JSON response
/// - Browser requests: redirect to /auth/login
pub async fn require_auth(session: Session, request: Request, next: Next) -> Response {
    let path = request.uri().path();

    // Exempt routes - no auth required
    if path == "/auth/login"
        || path == "/auth/logout"
        || path.starts_with("/static/")
        || path == "/api/v1/health"
    {
        return next.run(request).await;
    }

    // Check session for authenticated user
    let user_id: Option<i64> = session.get(USER_ID_KEY).await.unwrap_or(None);

    if user_id.is_some() {
        return next.run(request).await;
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
        .route("/auth/logout", post(logout))
}

/// Login page handler
async fn login_page() -> Result<Html<String>, AppError> {
    let template = LoginTemplate { error: None };
    Ok(Html(template.render()?))
}

/// Login form submission handler
async fn login(
    State(state): State<AppState>,
    session: Session,
    Form(creds): Form<LoginForm>,
) -> Result<impl IntoResponse, AppError> {
    use svrctlrs_database::queries::users;

    // Look up the user by username
    let user = users::get_user_by_username(&state.pool, &creds.username)
        .await
        .map_err(|e| AppError::InternalError(e.to_string()))?;

    let user = match user {
        Some(u) => u,
        None => {
            tracing::warn!(username = %creds.username, "Login failed: user not found");
            let template = LoginTemplate {
                error: Some("Invalid username or password".to_string()),
            };
            return Ok(Html(template.render()?).into_response());
        }
    };

    // Verify password
    let valid = users::verify_password(&creds.password, &user.password_hash)
        .map_err(|e| AppError::InternalError(e.to_string()))?;

    if !valid {
        tracing::warn!(username = %creds.username, "Login failed: invalid password");
        let template = LoginTemplate {
            error: Some("Invalid username or password".to_string()),
        };
        return Ok(Html(template.render()?).into_response());
    }

    // Store user_id in session
    session
        .insert(USER_ID_KEY, user.id)
        .await
        .map_err(|e| AppError::InternalError(format!("Failed to store session: {}", e)))?;

    tracing::info!(username = %user.username, user_id = user.id, "User logged in");
    Ok(Redirect::to("/").into_response())
}

/// Logout handler
async fn logout(session: Session) -> Result<impl IntoResponse, AppError> {
    session
        .flush()
        .await
        .map_err(|e| AppError::InternalError(format!("Failed to clear session: {}", e)))?;

    tracing::info!("User logged out");
    Ok(Redirect::to("/auth/login"))
}
