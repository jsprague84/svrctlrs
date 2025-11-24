//! SvrCtlRS Server
//!
//! Fullstack application with conditional compilation for server/web targets.

#![allow(non_snake_case)]

#[cfg(feature = "server")]
use dioxus::server::{DioxusRouterExt, ServeConfig};

mod ui;

// Server-side modules
#[cfg(feature = "server")]
mod config;
#[cfg(feature = "server")]
mod routes;
#[cfg(feature = "server")]
mod state;

#[cfg(feature = "server")]
use config::Config;
#[cfg(feature = "server")]
use state::AppState;

// Server-side entry point
#[cfg(feature = "server")]
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

    // Initialize application state
    let state = AppState::new(config).await?;

    // Initialize plugins
    info!("Initializing plugins");
    state.init_plugins().await?;

    // Start scheduler
    info!("Starting scheduler");
    state.start_scheduler().await?;

    // Initialize global state for server functions
    AppState::set_global(state.clone());

    // Build UI router separately so it can be nested without affecting API state
    let ui_router = Router::new()
        .serve_dioxus_application(ServeConfig::new(), ui::App);

    // Build Axum router with API + UI
    let app = Router::new()
        .nest("/api", routes::api_routes(state.clone()))
        .nest_service("/", ui_router.into_service())
        // Add middleware
        .layer(
            tower_http::trace::TraceLayer::new_for_http()
                .make_span_with(|request: &axum::http::Request<_>| {
                    tracing::info_span!(
                        "http_request",
                        method = %request.method(),
                        uri = %request.uri(),
                    )
                }),
        )
        .layer(tower_http::compression::CompressionLayer::new())
        .layer(tower_http::cors::CorsLayer::permissive());

    // Start server
    let listener = tokio::net::TcpListener::bind(&args.addr).await?;
    info!(addr = %args.addr, "Server listening");

    axum::serve(listener, app.into_make_service()).await?;

    Ok(())
}

// Client-side entry point (WASM)
#[cfg(not(feature = "server"))]
fn main() {
    dioxus::launch(ui::App);
}
