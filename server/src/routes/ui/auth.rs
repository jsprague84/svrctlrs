//! Authentication routes

use askama::Template;
use axum::{
    extract::State,
    response::{Html, IntoResponse, Redirect},
    routing::{get, post},
    Router,
};
use axum_extra::extract::Form;

use super::AppError;
use crate::{state::AppState, templates::*};

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
    State(_state): State<AppState>,
    Form(creds): Form<LoginForm>,
) -> Result<impl IntoResponse, AppError> {
    // TODO: Implement authentication
    tracing::info!("Login attempt: {}", creds.username);

    // For now, just redirect to dashboard
    Ok(Redirect::to("/"))
}

/// Logout handler
async fn logout() -> Result<impl IntoResponse, AppError> {
    // TODO: Clear session
    Ok(Redirect::to("/auth/login"))
}
