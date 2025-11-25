//! SvrCtlRS Server
//!
//! Server application with HTMX + Askama UI

#![allow(non_snake_case)]

// Server-side modules
mod config;
mod routes;
mod state;
mod templates;
mod ui_routes;

use config::Config;
use state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    use axum::Router;
    use clap::Parser;
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
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:data/svrctlrs.db".to_string());
    info!(url = %database_url, "Connecting to database");
    
    let database = svrctlrs_database::Database::new(&database_url).await?;
    database.migrate().await?;

    // Initialize application state
    let state = AppState::new(config, database).await?;

    // Initialize plugins
    info!("Initializing plugins");
    state.init_plugins().await?;

    // Start scheduler
    info!("Starting scheduler");
    state.start_scheduler().await?;

    // Initialize global state for compatibility
    AppState::set_global(state.clone());

    // Build UI router with state
    let ui_router = ui_routes::ui_routes().with_state(state.clone());
    
    // Build main router
    let app = Router::new()
        // API routes
        .nest("/api", routes::api_routes(state.clone()))
        // UI routes (HTMX + Askama)
        .merge(ui_router)
        // Middleware
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
