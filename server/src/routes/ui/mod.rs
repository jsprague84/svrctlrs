//! UI routes module - HTMX frontend

use axum::{response::IntoResponse, routing::get, Router};
use tower_http::services::ServeDir;

use crate::state::AppState;
use crate::templates::User;

// Sub-modules
pub mod auth;
pub mod credentials;
pub mod dashboard;
// TODO: Re-enable these routes after fixing missing imports/queries
// pub mod job_runs;
// pub mod job_schedules;
// pub mod job_templates;
// pub mod job_types;
// pub mod notifications;
pub mod servers;
pub mod settings;
pub mod tags;

/// Create UI router with all page and component routes
pub fn ui_routes() -> Router<AppState> {
    Router::new()
        // Dashboard
        .merge(dashboard::routes())
        // Infrastructure
        .merge(servers::routes())
        .merge(credentials::routes())
        .merge(tags::routes())
        // Jobs - TODO: Re-enable after fixing imports
        // .merge(job_types::routes())
        // .merge(job_templates::routes())
        // .merge(job_schedules::routes())
        // .merge(job_runs::routes())
        // Notifications - TODO: Re-enable after fixing imports
        // .merge(notifications::routes())
        // Settings
        .merge(settings::routes())
        // Auth
        .merge(auth::routes())
        // Static files
        .nest_service(
            "/static",
            ServeDir::new(
                std::env::var("STATIC_DIR").unwrap_or_else(|_| "server/static".to_string()),
            ),
        )
        // 404 handler
        .fallback(not_found)
}

/// Helper: Get user from session (placeholder for now)
pub async fn get_user_from_session() -> Option<User> {
    // TODO: Implement session management
    None
}

/// 404 handler
async fn not_found() -> Result<impl IntoResponse, AppError> {
    use crate::templates::NotFoundTemplate;
    use askama::Template;
    use axum::response::Html;

    let user = get_user_from_session().await;
    let template = NotFoundTemplate { user };
    Ok(Html(template.render()?))
}

// ============================================================================
// Error Handling
// ============================================================================

/// Custom error type for UI routes
#[derive(Debug)]
pub enum AppError {
    DatabaseError(String),
    TemplateError(String),
    NotFound(String),
    ValidationError(String),
    InternalError(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        use axum::http::StatusCode;

        let (status, message) = match &self {
            AppError::DatabaseError(msg) => {
                tracing::error!("Database error: {}", msg);
                (StatusCode::INTERNAL_SERVER_ERROR, format!("Database error: {}", msg))
            }
            AppError::TemplateError(msg) => {
                tracing::error!("Template error: {}", msg);
                (StatusCode::INTERNAL_SERVER_ERROR, format!("Template error: {}", msg))
            }
            AppError::NotFound(msg) => {
                tracing::warn!("Not found: {}", msg);
                (StatusCode::NOT_FOUND, msg.clone())
            }
            AppError::ValidationError(msg) => {
                tracing::warn!("Validation error: {}", msg);
                (StatusCode::BAD_REQUEST, msg.clone())
            }
            AppError::InternalError(msg) => {
                tracing::error!("Internal error: {}", msg);
                (StatusCode::INTERNAL_SERVER_ERROR, format!("Internal error: {}", msg))
            }
        };

        (status, message).into_response()
    }
}

impl From<askama::Error> for AppError {
    fn from(err: askama::Error) -> Self {
        AppError::TemplateError(err.to_string())
    }
}

impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        AppError::DatabaseError(err.to_string())
    }
}

impl From<anyhow::Error> for AppError {
    fn from(err: anyhow::Error) -> Self {
        AppError::InternalError(err.to_string())
    }
}
