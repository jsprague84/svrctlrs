//! UI routes module - HTMX frontend

use axum::{response::IntoResponse, Router};

use crate::state::AppState;
use crate::templates::User;

// Sub-modules
pub mod auth;
pub mod dashboard;
pub mod plugins;
pub mod servers;
pub mod settings;
pub mod tasks;

/// Create UI router with all page and component routes
pub fn ui_routes() -> Router<AppState> {
    Router::new()
        .merge(dashboard::routes())
        .merge(servers::routes())
        .merge(tasks::routes())
        .merge(plugins::routes())
        .merge(settings::routes())
        .merge(auth::routes())
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
    use askama::Template;
    use axum::response::Html;
    use crate::templates::NotFoundTemplate;

    let user = get_user_from_session().await;
    let template = NotFoundTemplate { user };
    Ok(Html(template.render()?))
}

// ============================================================================
// Error Handling
// ============================================================================

/// Custom error type for UI routes
pub struct AppError(anyhow::Error);

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        tracing::error!("Application error: {:?}", self.0);
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("Internal server error: {}", self.0),
        )
            .into_response()
    }
}

impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}
