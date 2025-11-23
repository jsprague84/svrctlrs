//! SvrCtlRS Server
//!
//! Main server application with Dioxus fullstack UI and Axum backend.

#![allow(non_snake_case)]

use axum::Router;
use clap::Parser;
use dioxus::prelude::*;
use dioxus_server::DioxusRouterExt;
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

    // Build Axum router with Dioxus integration
    let app = Router::new()
        // API routes
        .nest("/api", routes::api_routes(state.clone()))
        // Serve Dioxus application (handles all other routes)
        .serve_dioxus_application(dioxus_server::ServeConfig::new(), App)
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

/// Main Dioxus application component
fn App() -> Element {
    rsx! {
        document::Link { rel: "stylesheet", href: "/assets/styles.css" }
        Router::<Route> {}
    }
}

/// Application routes
#[derive(Clone, Routable, Debug, PartialEq)]
enum Route {
    #[route("/")]
    Home {},
    #[route("/servers")]
    Servers {},
    #[route("/servers/:id")]
    ServerDetail { id: String },
    #[route("/plugins")]
    Plugins {},
    #[route("/schedule")]
    Schedule {},
    #[route("/settings")]
    Settings {},
}

/// Home page
#[component]
fn Home() -> Element {
    let dashboard_data = use_server_future(get_dashboard_data)?;

    rsx! {
        div { class: "container mx-auto p-6",
            h1 { class: "text-3xl font-bold mb-6", "SvrCtlRS Dashboard" }

            if let Some(data) = dashboard_data() {
                div { class: "grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4",
                    StatCard {
                        title: "Servers",
                        value: "{data.server_count}",
                        icon: "🖥️"
                    }
                    StatCard {
                        title: "Plugins",
                        value: "{data.plugin_count}",
                        icon: "🔌"
                    }
                    StatCard {
                        title: "Scheduled Tasks",
                        value: "{data.task_count}",
                        icon: "📅"
                    }
                    StatCard {
                        title: "Status",
                        value: if data.healthy { "Healthy" } else { "Issues" },
                        icon: "✅"
                    }
                }

                div { class: "mt-8",
                    h2 { class: "text-2xl font-bold mb-4", "Recent Activity" }
                    ActivityFeed { activities: data.recent_activity }
                }
            }
        }
    }
}

/// Servers page
#[component]
fn Servers() -> Element {
    let servers = use_server_future(get_servers)?;

    rsx! {
        div { class: "container mx-auto p-6",
            h1 { class: "text-3xl font-bold mb-6", "Servers" }

            if let Some(server_list) = servers() {
                div { class: "grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4",
                    for server in server_list {
                        ServerCard { server }
                    }
                }
            }
        }
    }
}

/// Server detail page
#[component]
fn ServerDetail(id: String) -> Element {
    rsx! {
        div { class: "container mx-auto p-6",
            h1 { class: "text-3xl font-bold mb-6", "Server: {id}" }
            p { "Server details coming soon..." }
        }
    }
}

/// Plugins page
#[component]
fn Plugins() -> Element {
    rsx! {
        div { class: "container mx-auto p-6",
            h1 { class: "text-3xl font-bold mb-6", "Plugins" }
            p { "Plugin management coming soon..." }
        }
    }
}

/// Schedule page
#[component]
fn Schedule() -> Element {
    rsx! {
        div { class: "container mx-auto p-6",
            h1 { class: "text-3xl font-bold mb-6", "Scheduled Tasks" }
            p { "Task scheduling coming soon..." }
        }
    }
}

/// Settings page
#[component]
fn Settings() -> Element {
    rsx! {
        div { class: "container mx-auto p-6",
            h1 { class: "text-3xl font-bold mb-6", "Settings" }
            p { "Settings coming soon..." }
        }
    }
}

// ============================================================================
// UI Components
// ============================================================================

#[component]
fn StatCard(title: String, value: String, icon: String) -> Element {
    rsx! {
        div { class: "bg-white dark:bg-gray-800 rounded-lg shadow p-6",
            div { class: "flex items-center justify-between",
                div {
                    p { class: "text-sm text-gray-600 dark:text-gray-400", "{title}" }
                    p { class: "text-2xl font-bold mt-2", "{value}" }
                }
                div { class: "text-4xl", "{icon}" }
            }
        }
    }
}

#[component]
fn ServerCard(server: svrctlrs_core::Server) -> Element {
    rsx! {
        Link {
            to: Route::ServerDetail { id: server.name.clone() },
            div { class: "bg-white dark:bg-gray-800 rounded-lg shadow p-6 hover:shadow-lg transition-shadow cursor-pointer",
                h3 { class: "text-xl font-bold mb-2", "{server.name}" }
                p { class: "text-sm text-gray-600 dark:text-gray-400",
                    if server.is_local() {
                        "Local server"
                    } else {
                        "{server.ssh_host.as_ref().unwrap()}"
                    }
                }
            }
        }
    }
}

#[component]
fn ActivityFeed(activities: Vec<String>) -> Element {
    rsx! {
        div { class: "bg-white dark:bg-gray-800 rounded-lg shadow p-6",
            ul { class: "space-y-2",
                for activity in activities {
                    li { class: "text-sm", "{activity}" }
                }
            }
        }
    }
}

// ============================================================================
// Server Functions (Compile to Axum routes automatically)
// ============================================================================

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
struct DashboardData {
    server_count: usize,
    plugin_count: usize,
    task_count: usize,
    healthy: bool,
    recent_activity: Vec<String>,
}

#[server]
async fn get_dashboard_data() -> Result<DashboardData, ServerFnError> {
    // TODO: Get from actual app state
    Ok(DashboardData {
        server_count: 3,
        plugin_count: 3,
        task_count: 5,
        healthy: true,
        recent_activity: vec![
            "Docker health check completed".to_string(),
            "OS updates checked".to_string(),
            "All systems operational".to_string(),
        ],
    })
}

#[server]
async fn get_servers() -> Result<Vec<svrctlrs_core::Server>, ServerFnError> {
    // TODO: Get from actual app state
    Ok(vec![
        svrctlrs_core::Server::local("localhost"),
        svrctlrs_core::Server::remote("docker-vm", "user@docker-vm"),
        svrctlrs_core::Server::remote("nas", "user@nas"),
    ])
}
