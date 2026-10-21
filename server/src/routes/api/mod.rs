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
//! ├── recordings/                 The caller's session recordings
//! ├── settings/                   List + get + update
//! ├── tokens/                     The caller's API tokens (signed-in users only)
//! └── approvals/                  Held token calls the owner approves or denies
//! ```

pub mod approvals;
pub mod audit;
pub mod credentials;
pub mod files;
pub mod notifications;
pub mod ops;
pub mod profiles;
pub mod quick_commands;
pub mod recordings;
pub mod servers;
pub mod sessions;
pub mod settings;
pub mod sign_in;
pub mod tokens;
pub mod users;

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::Serialize;
use serde_json::json;
use tracing::{debug, instrument};

use crate::state::AppState;
use svrctlrs_api::system::{Health, Status, StatusResources};
use utoipa_axum::{router::OpenApiRouter, routes};

/// The API's error: a status code plus the `{ success: false, error: { code, message } }`
/// envelope every client expects. Handlers return `Result<_, ApiError>` and use `?` on query
/// results: [`svrctlrs_core::Error`] converts by kind (404 / 409 / 400, else a logged 500),
/// and so does an `anyhow::Error` that wraps one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiError {
    pub status: StatusCode,
    pub code: &'static str,
    pub message: String,
    /// Machine-readable extras for errors a client can act on (for example the fingerprint of
    /// an unconfirmed host key). Serialized as `error.details` when present.
    pub details: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
struct ApiErrorBody<'a> {
    success: bool,
    error: ApiErrorDetails<'a>,
}

#[derive(Debug, Serialize)]
struct ApiErrorDetails<'a> {
    code: &'a str,
    message: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<&'a serde_json::Value>,
}

impl ApiError {
    pub fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
            details: None,
        }
    }

    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }

    pub fn not_found(resource: &str) -> Self {
        Self::new(
            StatusCode::NOT_FOUND,
            "NOT_FOUND",
            format!("{resource} not found"),
        )
    }

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "BAD_REQUEST", message)
    }

    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self::new(StatusCode::UNAUTHORIZED, "UNAUTHORIZED", message)
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::new(StatusCode::FORBIDDEN, "FORBIDDEN", message)
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new(StatusCode::CONFLICT, "CONFLICT", message)
    }

    pub fn internal_error(message: impl Into<String>) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR", message)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let body = ApiErrorBody {
            success: false,
            error: ApiErrorDetails {
                code: self.code,
                message: &self.message,
                details: self.details.as_ref(),
            },
        };
        (self.status, Json(body)).into_response()
    }
}

/// An SSH connection that did not happen. An unconfirmed host key is `428 HOST_KEY_UNCONFIRMED`
/// with the `key_type` and `fingerprint` to confirm in `details`; a mismatch is
/// `409 HOST_KEY_MISMATCH`; anything else (unreachable, authentication) is a 400 with the
/// reason, since it is the server's configuration that is wrong, not ours.
impl From<crate::ssh::ConnectError> for ApiError {
    fn from(e: crate::ssh::ConnectError) -> Self {
        use crate::ssh::ConnectError;
        match &e {
            ConnectError::HostKeyUnconfirmed {
                server_id,
                server_name,
                key_type,
                fingerprint,
            } => ApiError::new(
                StatusCode::PRECONDITION_REQUIRED,
                "HOST_KEY_UNCONFIRMED",
                e.to_string(),
            )
            .with_details(json!({
                "server_id": server_id,
                "server_name": server_name,
                "key_type": key_type,
                "fingerprint": fingerprint,
            })),
            ConnectError::HostKeyMismatch(msg) => {
                ApiError::new(StatusCode::CONFLICT, "HOST_KEY_MISMATCH", msg.clone())
            }
            ConnectError::Failed(msg) => ApiError::bad_request(msg.clone()),
        }
    }
}

/// "server 3 not found" reads better as "Server 3 not found" in a toast.
fn capitalized(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

impl From<svrctlrs_core::Error> for ApiError {
    fn from(e: svrctlrs_core::Error) -> Self {
        use svrctlrs_core::Error;
        match e {
            Error::NotFound(_) => Self::new(
                StatusCode::NOT_FOUND,
                "NOT_FOUND",
                capitalized(&e.to_string()),
            ),
            Error::Conflict(msg) => Self::conflict(msg),
            Error::InvalidInput(msg) => Self::bad_request(msg),
            other => {
                let detail = other.detail();
                tracing::error!(error = %detail, "Request failed");
                Self::internal_error(detail)
            }
        }
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(e: anyhow::Error) -> Self {
        Self::from(svrctlrs_core::Error::from(e))
    }
}

/// Create the complete v1 API router
pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        // System endpoints
        .routes(routes!(health_check))
        .routes(routes!(server_status))
        .routes(routes!(openapi_document))
        // Resource endpoints
        .nest("/servers", servers::routes())
        .nest("/credentials", credentials::routes())
        .nest("/settings", settings::routes())
        .nest("/profiles", profiles::routes())
        .nest("/quick-commands", quick_commands::routes())
        .nest("/notifications", notifications::routes())
        .nest("/users", users::routes().merge(sign_in::routes()))
        .nest("/audit", audit::routes())
        .nest("/sessions", sessions::routes())
        .nest("/recordings", recordings::routes())
        .nest("/tokens", tokens::routes())
        .nest("/approvals", approvals::routes())
}

/// The OpenAPI document of this server's API (the same as `docs/openapi.json` in its
/// source).
#[utoipa::path(get, path = "/openapi.json", tag = "system",
    responses((status = 200, description = "This document")))]
async fn openapi_document() -> Json<serde_json::Value> {
    static DOC: std::sync::OnceLock<serde_json::Value> = std::sync::OnceLock::new();
    Json(
        DOC.get_or_init(|| serde_json::to_value(crate::routes::openapi()).unwrap_or_default())
            .clone(),
    )
}

/// Health check endpoint. Unauthenticated, so it reveals only version and revision; it does
/// touch the database, so a broken volume turns the container unhealthy (503) and the deploy
/// script and compose healthcheck notice.
#[utoipa::path(get, path = "/health", tag = "system", security(()),
    responses(
        (status = 200, description = "Up, database reachable", body = Health),
        (status = 503, description = "The database is unreachable", body = Health),
    ))]
#[instrument(skip(state))]
async fn health_check(State(state): State<AppState>) -> (StatusCode, Json<Health>) {
    debug!("Health check requested");
    let database = match svrctlrs_database::sqlx::query("SELECT 1")
        .execute(&state.pool)
        .await
    {
        Ok(_) => "ok",
        Err(e) => {
            tracing::error!(error = %e, "Health check: database unreachable");
            "error"
        }
    };
    let status = if database == "ok" {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (
        status,
        Json(Health {
            status: if database == "ok" { "ok" } else { "degraded" }.to_string(),
            database: database.to_string(),
            service: "svrctlrs".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            // Baked into the container image by CI; lets a deploy prove which commit is running.
            revision: std::env::var("SVRCTLRS_REVISION").unwrap_or_else(|_| "unknown".to_string()),
        }),
    )
}

/// Server status endpoint
#[utoipa::path(get, path = "/status", tag = "system",
    responses((status = 200, body = Status)))]
#[instrument(skip(state))]
async fn server_status(State(state): State<AppState>) -> Json<Status> {
    use svrctlrs_database::queries;

    let server_count = queries::servers::list_servers(&state.pool)
        .await
        .map(|s| s.len())
        .unwrap_or(0);

    Json(Status {
        status: "running".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        resources: StatusResources {
            servers: server_count,
        },
    })
}

#[cfg(test)]
mod api_error_tests {
    use super::*;
    use axum::body::to_bytes;

    async fn body_of(e: ApiError) -> (StatusCode, serde_json::Value) {
        let response = e.into_response();
        let status = response.status();
        let bytes = to_bytes(response.into_body(), 1 << 16).await.unwrap();
        (status, serde_json::from_slice(&bytes).unwrap())
    }

    #[tokio::test]
    async fn envelope_is_unchanged() {
        let (status, body) = body_of(ApiError::bad_request("name is required")).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(
            body,
            serde_json::json!({
                "success": false,
                "error": { "code": "BAD_REQUEST", "message": "name is required" }
            })
        );
    }

    #[test]
    fn core_errors_map_by_kind() {
        use svrctlrs_core::Error;
        let e = ApiError::from(Error::NotFound("server 3".into()));
        assert_eq!((e.status, e.code), (StatusCode::NOT_FOUND, "NOT_FOUND"));
        assert_eq!(e.message, "Server 3 not found");

        let e = ApiError::from(Error::Conflict(
            "A server with that name already exists".into(),
        ));
        assert_eq!(e.status, StatusCode::CONFLICT);

        let e = ApiError::from(Error::InvalidInput("password must be longer".into()));
        assert_eq!(
            (e.status, e.message.as_str()),
            (StatusCode::BAD_REQUEST, "password must be longer")
        );

        let e = ApiError::from(Error::Database(
            anyhow::anyhow!("locked").context("Failed to list servers"),
        ));
        assert_eq!(e.status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(e.message, "Database error: Failed to list servers: locked");

        let e = ApiError::from(Error::Config("no key".into()));
        assert_eq!(e.status, StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn anyhow_wrapping_a_core_error_keeps_its_status() {
        let wrapped = anyhow::Error::from(svrctlrs_core::Error::NotFound("quick command 9".into()))
            .context("Failed to load quick command");
        let e = ApiError::from(wrapped);
        assert_eq!(e.status, StatusCode::NOT_FOUND);
        assert_eq!(e.message, "Quick command 9 not found");

        let e = ApiError::from(anyhow::anyhow!("boom"));
        assert_eq!(e.status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(e.message, "boom");
    }
}
