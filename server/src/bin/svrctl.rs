//! SvrCtlRS CLI - Command-line tool for server administration
//!
//! This tool provides command-line access to the SvrCtlRS server API
//! for managing plugins, triggering tasks, and viewing system status.

use clap::{Parser, Subcommand};
use reqwest::Client;
use serde_json::Value;
use std::process;
use tracing::error;

/// SvrCtlRS CLI - Server administration tool
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    /// Server URL to connect to
    #[arg(
        short,
        long,
        default_value = "http://localhost:8080",
        env = "SVRCTLRS_URL"
    )]
    url: String,

    /// API token for authentication
    #[arg(short, long, env = "SVRCTLRS_TOKEN")]
    token: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Quick health check (for Docker healthcheck)
    Health,

    /// Server health and status commands
    Status {
        #[command(subcommand)]
        command: StatusCommands,
    },
}

#[derive(Subcommand, Debug)]
enum StatusCommands {
    /// Check server health
    Health,

    /// Get server status
    Server,

    /// Get system metrics
    Metrics {
        /// Get metrics for specific plugin
        plugin_id: Option<String>,
    },
}

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let cli = Cli::parse();
    let client = Client::new();

    let result = match cli.command {
        Commands::Health => handle_health(&client, &cli.url).await,
        Commands::Status { command } => handle_status(&client, &cli.url, command).await,
    };

    if let Err(e) = result {
        error!("Command failed: {}", e);
        process::exit(1);
    }
}

async fn handle_health(client: &Client, base_url: &str) -> anyhow::Result<()> {
    let url = format!("{}/api/v1/health", base_url);
    let response = client.get(&url).send().await?;

    if response.status().is_success() {
        let json: Value = response.json().await?;
        println!("{}", serde_json::to_string_pretty(&json)?);
        Ok(())
    } else {
        anyhow::bail!("Health check failed with status: {}", response.status())
    }
}

async fn handle_status(
    client: &Client,
    base_url: &str,
    command: StatusCommands,
) -> anyhow::Result<()> {
    match command {
        StatusCommands::Health => {
            let url = format!("{}/api/v1/health", base_url);
            let response: Value = client.get(&url).send().await?.json().await?;
            println!("{}", serde_json::to_string_pretty(&response)?);
        }
        StatusCommands::Server => {
            let url = format!("{}/api/v1/status", base_url);
            let response: Value = client.get(&url).send().await?.json().await?;
            println!("{}", serde_json::to_string_pretty(&response)?);
        }
        StatusCommands::Metrics { plugin_id } => {
            let url = if let Some(id) = plugin_id {
                format!("{}/api/v1/metrics/{}", base_url, id)
            } else {
                format!("{}/api/v1/metrics", base_url)
            };
            let response: Value = client.get(&url).send().await?.json().await?;
            println!("{}", serde_json::to_string_pretty(&response)?);
        }
    }
    Ok(())
}
