//! SvrCtlRS Server
//!
//! Server application with HTMX + Askama UI

#![allow(non_snake_case)]

// Server-side modules
mod config;
mod filters; // Custom Askama template filters
mod routes;
mod ssh;
mod state;
mod templates;

use config::Config;
use state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    use axum::Router;
    use clap::Parser;
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

    // Clean up stale running jobs (jobs stuck in 'running' status for more than 1 hour)
    // This handles cases where the server crashed or was restarted while jobs were running
    let stale_jobs_count = svrctlrs_database::queries::job_runs::fail_stale_running_jobs(
        database.pool(),
        1, // Mark jobs as failed if running for more than 1 hour
    )
    .await?;
    if stale_jobs_count > 0 {
        tracing::warn!(
            count = stale_jobs_count,
            "Marked stale running jobs as failed on startup"
        );
    }

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

    // Start scheduler
    info!("Starting scheduler");
    state.start_scheduler().await?;

    // Build UI router with state
    let ui_router = routes::ui::ui_routes().with_state(state.clone());

    // Build terminal WebSocket router
    let terminal_router = routes::terminal::routes().with_state(state.clone());

    // Build interactive PTY terminal router
    let terminal_pty_router = routes::terminal_pty::routes().with_state(state.clone());

    // Build job runs WebSocket router
    let job_runs_ws_router = routes::job_runs_ws::routes().with_state(state.clone());

    // Build session layer
    let session_layer = SessionManagerLayer::new(session_store)
        .with_secure(false) // Set to true if using HTTPS
        .with_expiry(tower_sessions::Expiry::OnInactivity(
            time::Duration::hours(24),
        ));

    // Build main router
    let app = Router::new()
        // API routes
        .nest("/api", routes::api_routes(state.clone()))
        // Terminal WebSocket routes
        .merge(terminal_router)
        // Interactive PTY terminal routes
        .merge(terminal_pty_router)
        // Job runs WebSocket routes
        .merge(job_runs_ws_router)
        // UI routes (HTMX + Askama)
        .merge(ui_router)
        // Auth middleware (runs after session layer processes the request)
        .layer(axum::middleware::from_fn(
            routes::ui::auth::require_auth,
        ))
        // Session layer (must wrap auth middleware so Session extractor works)
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
        .layer(tower_http::cors::CorsLayer::permissive());

    // Start server
    let listener = tokio::net::TcpListener::bind(&args.addr).await?;
    info!(addr = %args.addr, "Server listening");

    axum::serve(listener, app.into_make_service()).await?;

    Ok(())
}
