//! Authentication routes

use askama::Template;
use axum::{
    extract::State,
    response::{Html, IntoResponse, Redirect},
    routing::{get, post},
    Router,
};
use axum_extra::extract::Form;
use tower_sessions::Session;

use super::AppError;
use crate::{state::AppState, templates::*};

/// Session key for the authenticated user's ID
const USER_ID_KEY: &str = "user_id";

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
