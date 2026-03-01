//! REST API v1 endpoints
//!
//! ## API Structure
//!
//! ```text
//! /api/v1/
//! ├── health                      GET     Health check
//! ├── status                      GET     Server status
//! ├── servers/                    Full CRUD + test connection
//! ├── credentials/                Full CRUD
//! └── settings/                   List + get + update
//! ```

pub mod credentials;
pub mod servers;
pub mod settings;

use axum::{extract::State, http::StatusCode, response::IntoResponse, routing::get, Json, Router};
use serde::Serialize;
use serde_json::json;
use tracing::{debug, instrument};

use crate::state::AppState;

/// Standard API error response
#[derive(Debug, Serialize)]
pub struct ApiError {
    pub success: bool,
    pub error: ApiErrorDetails,
}

#[derive(Debug, Serialize)]
pub struct ApiErrorDetails {
    pub code: String,
    pub message: String,
}

impl ApiError {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            success: false,
            error: ApiErrorDetails {
                code: code.to_string(),
                message: message.into(),
            },
        }
    }

    pub fn not_found(resource: &str) -> (StatusCode, Json<Self>) {
        (
            StatusCode::NOT_FOUND,
            Json(Self::new("NOT_FOUND", format!("{} not found", resource))),
        )
    }

    pub fn bad_request(message: impl Into<String>) -> (StatusCode, Json<Self>) {
        (
            StatusCode::BAD_REQUEST,
            Json(Self::new("BAD_REQUEST", message)),
        )
    }

    pub fn internal_error(message: impl Into<String>) -> (StatusCode, Json<Self>) {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(Self::new("INTERNAL_ERROR", message)),
        )
    }

    pub fn conflict(message: impl Into<String>) -> (StatusCode, Json<Self>) {
        (StatusCode::CONFLICT, Json(Self::new("CONFLICT", message)))
    }
}

/// Create the complete v1 API router
pub fn routes() -> Router<AppState> {
    Router::new()
        // System endpoints
        .route("/health", get(health_check))
        .route("/status", get(server_status))
        // Resource endpoints
        .nest("/servers", servers::routes())
        .nest("/credentials", credentials::routes())
        .nest("/settings", settings::routes())
}

/// Health check endpoint
#[instrument]
async fn health_check() -> impl IntoResponse {
    debug!("Health check requested");
    Json(json!({
        "status": "ok",
        "service": "svrctlrs",
        "version": env!("CARGO_PKG_VERSION")
    }))
}

/// Server status endpoint
#[instrument(skip(state))]
async fn server_status(State(state): State<AppState>) -> impl IntoResponse {
    use svrctlrs_database::queries;

    let server_count = queries::servers::list_servers(&state.pool)
        .await
        .map(|s| s.len())
        .unwrap_or(0);

    Json(json!({
        "status": "running",
        "version": env!("CARGO_PKG_VERSION"),
        "resources": {
            "servers": server_count
        }
    }))
}
