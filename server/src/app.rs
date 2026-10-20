//! The whole HTTP application: routes, authentication, sessions, CORS, security headers and
//! request ids, in the order `main` serves them. Tests build the same stack.

use axum::error_handling::HandleErrorLayer;
use axum::http::HeaderValue;
use axum::Router;
use tower::timeout::TimeoutLayer;
use tower::ServiceBuilder;
use tower_http::set_header::SetResponseHeaderLayer;
use tower_sessions::SessionManagerLayer;
use tower_sessions_sqlx_store::SqliteStore;
use tracing::info;

use crate::state::AppState;
use crate::{request_id, routes};

/// The application router for `state`, with sessions in `session_store` (cookies marked
/// `Secure` unless `session_secure` is false).
pub fn router(state: AppState, session_store: SqliteStore, session_secure: bool) -> Router {
    let allowed_origins = state.allowed_origins.clone();
    // What an approved request runs through (the API alone, behind no auth layer: the caller
    // supplies the user and token itself)
    let _ = state
        .api_router
        .set(Router::new().nest("/api", routes::api_routes(state.clone())));

    // Build UI router with state (SPA + auth)
    let ui_router = routes::ui::ui_routes().with_state(state.clone());

    // Build the terminal WebSocket router
    let terminal_pty_router = routes::terminal_pty::routes().with_state(state.clone());

    // When cross-origin clients are configured, use SameSite::None so cookies work across
    // origins. Otherwise keep the library default (Strict).
    let mut session_layer = SessionManagerLayer::new(session_store)
        .with_secure(session_secure)
        .with_expiry(tower_sessions::Expiry::OnInactivity(time::Duration::hours(
            24,
        )));
    if !allowed_origins.is_empty() {
        session_layer = session_layer.with_same_site(tower_sessions::cookie::SameSite::None);
        info!("Session cookie SameSite=None for cross-origin access");
    }

    // Build CORS layer
    let cors_layer = if !allowed_origins.is_empty() {
        let allowed: Vec<HeaderValue> = allowed_origins
            .iter()
            .filter_map(|s| s.parse().ok())
            .collect();
        info!(origins = ?allowed_origins, "CORS configured with allowed origins");
        tower_http::cors::CorsLayer::new()
            .allow_origin(allowed)
            .allow_methods([
                axum::http::Method::GET,
                axum::http::Method::POST,
                axum::http::Method::PUT,
                axum::http::Method::DELETE,
                axum::http::Method::PATCH,
            ])
            .allow_headers([
                axum::http::header::CONTENT_TYPE,
                axum::http::header::AUTHORIZATION,
                axum::http::header::COOKIE,
                axum::http::header::ACCEPT,
                axum::http::HeaderName::from_static("x-user-id"),
            ])
            .expose_headers([axum::http::header::SET_COOKIE])
            .allow_credentials(true)
    } else {
        info!("CORS configured for same-origin only (set ALLOWED_ORIGINS to allow cross-origin)");
        tower_http::cors::CorsLayer::new()
            .allow_methods([
                axum::http::Method::GET,
                axum::http::Method::POST,
                axum::http::Method::PUT,
                axum::http::Method::DELETE,
                axum::http::Method::PATCH,
            ])
            .allow_headers(tower_http::cors::Any)
    };

    // HTTP routes WITH 30-second request timeout (API + UI)
    let http_routes = Router::new()
        .nest("/api", routes::api_routes(state.clone()))
        .merge(ui_router)
        .layer(
            ServiceBuilder::new()
                .layer(HandleErrorLayer::new(|_: axum::BoxError| async {
                    axum::http::StatusCode::REQUEST_TIMEOUT
                }))
                .layer(TimeoutLayer::new(std::time::Duration::from_secs(30))),
        );

    // Routes WITHOUT the request timeout: the WebSocket (long-lived) and SFTP transfers (an
    // upload reads its body for as long as the file takes).
    let ws_routes = Router::new()
        .merge(terminal_pty_router)
        .merge(routes::api::files::routes().with_state(state.clone()));

    // Build main router
    let app = Router::new()
        .merge(http_routes)
        .merge(ws_routes)
        // Auth middleware (needs pool for token auth fallback)
        .layer(axum::middleware::from_fn(
            routes::ui::auth::require_auth,
        ))
        .layer(axum::Extension(state.pool.clone()))
        .layer(axum::Extension(state.ws_tickets.clone()))
        .layer(session_layer)
        .layer(
            tower_http::trace::TraceLayer::new_for_http().make_span_with(
                |request: &axum::http::Request<_>| {
                    // Redacted: WebSocket upgrades carry the session token in the query.
                    // The request id is also the `x-request-id` response header.
                    tracing::info_span!(
                        "http_request",
                        request_id = %request_id::from_headers(request.headers()),
                        method = %request.method(),
                        uri = %routes::ui::auth::redacted_uri(request.uri()),
                    )
                },
            ),
        )
        .layer(tower_http::compression::CompressionLayer::new())
        // Security headers
        .layer(SetResponseHeaderLayer::overriding(
            axum::http::header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            axum::http::header::X_FRAME_OPTIONS,
            HeaderValue::from_static("DENY"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            axum::http::HeaderName::from_static("referrer-policy"),
            HeaderValue::from_static("strict-origin-when-cross-origin"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            axum::http::HeaderName::from_static("content-security-policy"),
            HeaderValue::from_static(
                "default-src 'self'; script-src 'self' 'unsafe-inline' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; connect-src 'self' wss:; img-src 'self' data:; font-src 'self'; frame-ancestors 'none'"
            ),
        ))
        .layer(cors_layer);
    // Outermost: mint the request id before anything logs
    request_id::with_request_ids(app)
}
