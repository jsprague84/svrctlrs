//! SvrCtlRS Server — Terminal-focused infrastructure management

#![allow(non_snake_case)]

mod config;
mod health_monitor;
mod routes;
mod ssh;
mod state;

use config::Config;
use state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    use axum::error_handling::HandleErrorLayer;
    use axum::http::HeaderValue;
    use axum::Router;
    use clap::Parser;
    use tower::timeout::TimeoutLayer;
    use tower::ServiceBuilder;
    use tower_http::set_header::SetResponseHeaderLayer;
    use tower_sessions::{ExpiredDeletion, SessionManagerLayer};
    use tower_sessions_sqlx_store::SqliteStore;
    use tracing::info;

    /// SvrCtlRS Server
    #[derive(Parser, Debug)]
    #[command(author, version, about, long_about = None)]
    struct Args {
        /// Server address to bind to
        #[arg(short, long, default_value = "0.0.0.0:8080")]
        addr: String,

        /// Path to configuration file
        #[arg(short, long)]
        config: Option<String>,
    }

    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,server=debug".into()),
        )
        .init();

    // Load environment variables
    dotenvy::dotenv().ok();

    // Initialize encryption key for credential at-rest encryption
    if let Err(e) = svrctlrs_core::encryption::init_encryption_key() {
        tracing::warn!(error = %e, "Failed to initialize encryption key — credentials will be stored in plaintext");
    }

    // Parse CLI args
    let args = Args::parse();

    // Load configuration
    let config = Config::load(args.config.as_deref())?;
    info!(addr = %args.addr, "Starting SvrCtlRS server");

    // Initialize database
    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:data/svrctlrs.db".to_string());
    info!(url = %database_url, "Connecting to database");

    let database = svrctlrs_database::Database::new(&database_url).await?;
    database.migrate().await?;

    // Set up session store (SQLite-backed)
    let session_store = SqliteStore::new(database.pool().clone());
    session_store.migrate().await?;
    info!("Session store initialized");

    // Spawn task to periodically clean up expired sessions
    let deletion_task_store = session_store.clone();
    tokio::task::spawn(
        deletion_task_store.continuously_delete_expired(tokio::time::Duration::from_secs(300)),
    );

    // Seed initial admin user if ADMIN_USERNAME and ADMIN_PASSWORD are set and no users exist
    {
        use svrctlrs_database::queries::users;

        let user_count = users::count_users(database.pool()).await?;
        if user_count == 0 {
            if let (Ok(admin_username), Ok(admin_password)) = (
                std::env::var("ADMIN_USERNAME"),
                std::env::var("ADMIN_PASSWORD"),
            ) {
                let input = svrctlrs_database::models::CreateUser {
                    username: admin_username.clone(),
                    password: admin_password,
                };
                users::create_user(database.pool(), &input).await?;
                info!(username = %admin_username, "Created initial admin user from environment variables");
            }
        }
    }

    // Initialize application state
    let state = AppState::new(config, database).await?;

    // Build UI router with state (SPA + auth)
    let ui_router = routes::ui::ui_routes().with_state(state.clone());

    // Build terminal WebSocket routers
    let terminal_router = routes::terminal::routes().with_state(state.clone());
    let terminal_pty_router = routes::terminal_pty::routes().with_state(state.clone());

    // Build session layer
    // Default secure=true for production (HTTPS). Set SESSION_SECURE=false for local dev.
    let session_secure = std::env::var("SESSION_SECURE")
        .map(|v| v != "false" && v != "0")
        .unwrap_or(true);
    info!(secure = session_secure, "Session cookie configuration");

    let session_layer = SessionManagerLayer::new(session_store)
        .with_secure(session_secure)
        .with_expiry(tower_sessions::Expiry::OnInactivity(time::Duration::hours(
            24,
        )));

    // Build CORS layer
    let cors_layer = if let Ok(origins) = std::env::var("ALLOWED_ORIGINS") {
        let allowed: Vec<HeaderValue> = origins
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect();
        info!(origins = %origins, "CORS configured with allowed origins");
        tower_http::cors::CorsLayer::new()
            .allow_origin(allowed)
            .allow_methods([
                axum::http::Method::GET,
                axum::http::Method::POST,
                axum::http::Method::PUT,
                axum::http::Method::DELETE,
                axum::http::Method::PATCH,
            ])
            .allow_headers(tower_http::cors::Any)
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

    // WebSocket routes WITHOUT timeout (long-lived connections)
    let ws_routes = Router::new()
        .merge(terminal_router)
        .merge(terminal_pty_router);

    // Build main router
    let app = Router::new()
        .merge(http_routes)
        .merge(ws_routes)
        // Auth middleware
        .layer(axum::middleware::from_fn(
            routes::ui::auth::require_auth,
        ))
        .layer(session_layer)
        .layer(
            tower_http::trace::TraceLayer::new_for_http().make_span_with(
                |request: &axum::http::Request<_>| {
                    tracing::info_span!(
                        "http_request",
                        method = %request.method(),
                        uri = %request.uri(),
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

    // Start background health monitor
    health_monitor::start_health_monitor(state.clone());

    // Start server with graceful shutdown
    let listener = tokio::net::TcpListener::bind(&args.addr).await?;
    info!(addr = %args.addr, "Server listening");

    axum::serve(listener, app.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    info!("Server shut down gracefully");
    Ok(())
}

/// Signal handler for graceful shutdown.
async fn shutdown_signal() {
    use tokio::signal;

    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            tracing::info!("Received SIGINT (Ctrl+C), starting graceful shutdown...");
        }
        _ = terminate => {
            tracing::info!("Received SIGTERM, starting graceful shutdown...");
        }
    }
}
