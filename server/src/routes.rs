//! Route modules
//!
//! This module organizes all HTTP routes for the SvrCtlRS server.
//!
//! ## Route Structure
//!
//! ```text
//! /api/v1/          REST API endpoints (JSON)
//! /ws/              WebSocket endpoints
//! /auth/            Authentication routes
//! /                 Svelte SPA (static files)
//! ```
//!
//! The API routers are `utoipa_axum::router::OpenApiRouter`s: a handler registered with
//! `routes!(…)` is routed and documented by the same line, so `GET /api/v1/openapi.json`
//! cannot miss a route or describe one that is not there.

pub mod api;
pub mod terminal_pty;
pub mod ui;

use axum::Router;
use utoipa::OpenApi;
use utoipa_axum::router::OpenApiRouter;

use crate::state::AppState;

/// Create REST API router
///
/// All API endpoints are nested under /api/v1/
pub fn api_routes(state: AppState) -> Router {
    Router::new()
        .nest("/v1", Router::from(api::routes()))
        .with_state(state)
}

/// The document's fixed parts: title, version, and how to authenticate.
#[derive(OpenApi)]
#[openapi(
    info(
        title = "SvrCtlRS API",
        description = "Self-hosted terminal/SSH management. Authenticate with an API token \
                       (`Authorization: Bearer svr_…`, Settings → Access) or the session cookie. \
                       Errors use the `ErrorBody` envelope. The terminal WebSocket \
                       (`/ws/terminal/pty`) is described in docs/API.md.",
        license(name = "MIT OR Apache-2.0")
    ),
    modifiers(&BearerAuth),
    security(("bearer" = []), ("cookie" = [])),
    components(schemas(svrctlrs_api::ErrorBody))
)]
struct ApiDoc;

struct BearerAuth;

impl utoipa::Modify for BearerAuth {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        use utoipa::openapi::security::SecurityScheme;
        use utoipa::openapi::security::{ApiKey, ApiKeyValue, HttpAuthScheme, HttpBuilder};
        let components = openapi.components.get_or_insert_with(Default::default);
        components.add_security_scheme(
            "bearer",
            SecurityScheme::Http(
                HttpBuilder::new()
                    .scheme(HttpAuthScheme::Bearer)
                    .bearer_format("svr_ API token")
                    .build(),
            ),
        );
        components.add_security_scheme(
            "cookie",
            SecurityScheme::ApiKey(ApiKey::Cookie(ApiKeyValue::new("id"))),
        );
    }
}

/// The OpenAPI document of every documented route, at the paths clients call.
pub fn openapi() -> utoipa::openapi::OpenApi {
    let mut doc = OpenApiRouter::<AppState>::with_openapi(ApiDoc::openapi())
        .nest("/api/v1", api::routes())
        .merge(api::files::routes())
        .merge(api::ops::routes())
        .merge(ui::auth::api_routes())
        .into_openapi();
    doc.info.version = env!("CARGO_PKG_VERSION").to_string();
    doc
}

#[cfg(test)]
mod tests {
    /// `docs/openapi.json` is the document the code generates. After an API change:
    /// `UPDATE_OPENAPI=1 cargo test -p server openapi_document`.
    #[test]
    fn openapi_document_is_current() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../docs/openapi.json");
        let mut generated = serde_json::to_string_pretty(&super::openapi()).unwrap();
        generated.push('\n');
        if std::env::var("UPDATE_OPENAPI").is_ok() {
            std::fs::write(path, &generated).unwrap();
            return;
        }
        let committed = std::fs::read_to_string(path).unwrap_or_default();
        assert!(
            committed == generated,
            "docs/openapi.json is stale; regenerate with UPDATE_OPENAPI=1 cargo test -p server \
             openapi_document"
        );
    }
}
