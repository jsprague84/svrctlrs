//! SvrCtlRS Server
//!
//! Main server application with Axum backend.

use axum::Router;
use clap::Parser;
use std::sync::Arc;
use tracing::{info, instrument};

mod config;
mod routes;
mod state;

use config::Config;
use state::AppState;

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

#[tokio::main]
#[instrument]
async fn main() -> anyhow::Result<()> {
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
    let state = Arc::new(state);

    // Initialize plugins
    info!("Initializing plugins");
    state.init_plugins().await?;

    // Start scheduler
    info!("Starting scheduler");
    state.start_scheduler().await?;

    // Build Axum router
    let app = Router::new()
        // API routes
        .nest("/api", routes::api_routes(state.clone()))
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
