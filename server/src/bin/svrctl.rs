//! SvrCtlRS CLI - Command-line tool for server administration
//!
//! This tool provides command-line access to the SvrCtlRS server API
//! for managing plugins, triggering tasks, and viewing system status.

use clap::{Parser, Subcommand};
use reqwest::Client;
use serde_json::Value;
use std::process;
use tracing::{error, info};

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

    /// Plugin management commands
    Plugin {
        #[command(subcommand)]
        command: PluginCommands,
    },

    /// Task execution commands
    Task {
        #[command(subcommand)]
        command: TaskCommands,
    },

    /// Webhook trigger commands
    Webhook {
        #[command(subcommand)]
        command: WebhookCommands,
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

#[derive(Subcommand, Debug)]
enum PluginCommands {
    /// List all plugins
    List,

    /// Get plugin information
    Info {
        /// Plugin ID
        plugin_id: String,
    },

    /// List plugin tasks
    Tasks {
        /// Plugin ID
        plugin_id: String,
    },
}

#[derive(Subcommand, Debug)]
enum TaskCommands {
    /// List all scheduled tasks
    List,

    /// Execute a task manually
    Execute {
        /// Plugin ID
        plugin_id: String,

        /// Task ID
        task_id: String,
    },
}

#[derive(Subcommand, Debug)]
enum WebhookCommands {
    /// Trigger Docker health check
    DockerHealth {
        /// Webhook token
        #[arg(short, long, env = "WEBHOOK_SECRET")]
        token: Option<String>,
    },

    /// Trigger Docker cleanup
    DockerCleanup {
        /// Webhook token
        #[arg(short, long, env = "WEBHOOK_SECRET")]
        token: Option<String>,
    },

    /// Trigger Docker analysis
    DockerAnalysis {
        /// Webhook token
        #[arg(short, long, env = "WEBHOOK_SECRET")]
        token: Option<String>,
    },

    /// Trigger updates check
    UpdatesCheck {
        /// Webhook token
        #[arg(short, long, env = "WEBHOOK_SECRET")]
        token: Option<String>,
    },

    /// Trigger updates apply
    UpdatesApply {
        /// Webhook token
        #[arg(short, long, env = "WEBHOOK_SECRET")]
        token: Option<String>,
    },

    /// Trigger OS cleanup
    OsCleanup {
        /// Webhook token
        #[arg(short, long, env = "WEBHOOK_SECRET")]
        token: Option<String>,
    },

    /// Trigger custom task
    Trigger {
        /// Plugin ID
        plugin_id: String,

        /// Task ID
        task_id: String,

        /// Webhook token
        #[arg(short, long, env = "WEBHOOK_SECRET")]
        token: Option<String>,
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
        Commands::Plugin { command } => handle_plugin(&client, &cli.url, command).await,
        Commands::Task { command } => handle_task(&client, &cli.url, command).await,
        Commands::Webhook { command } => handle_webhook(&client, &cli.url, command).await,
    };

    if let Err(e) = result {
        error!("Command failed: {}", e);
        process::exit(1);
    }
}

async fn handle_health(client: &Client, base_url: &str) -> anyhow::Result<()> {
    let url = format!("{}/api/health", base_url);
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
            let url = format!("{}/api/health", base_url);
            let response: Value = client.get(&url).send().await?.json().await?;
            println!("{}", serde_json::to_string_pretty(&response)?);
        }
        StatusCommands::Server => {
            let url = format!("{}/api/status", base_url);
            let response: Value = client.get(&url).send().await?.json().await?;
            println!("{}", serde_json::to_string_pretty(&response)?);
        }
        StatusCommands::Metrics { plugin_id } => {
            let url = if let Some(id) = plugin_id {
                format!("{}/api/metrics/{}", base_url, id)
            } else {
                format!("{}/api/metrics", base_url)
            };
            let response: Value = client.get(&url).send().await?.json().await?;
            println!("{}", serde_json::to_string_pretty(&response)?);
        }
    }
    Ok(())
}

async fn handle_plugin(
    client: &Client,
    base_url: &str,
    command: PluginCommands,
) -> anyhow::Result<()> {
    match command {
        PluginCommands::List => {
            let url = format!("{}/api/plugins", base_url);
            let response: Value = client.get(&url).send().await?.json().await?;
            println!("{}", serde_json::to_string_pretty(&response)?);
        }
        PluginCommands::Info { plugin_id } => {
            let url = format!("{}/api/plugins/{}", base_url, plugin_id);
            let response: Value = client.get(&url).send().await?.json().await?;
            println!("{}", serde_json::to_string_pretty(&response)?);
        }
        PluginCommands::Tasks { plugin_id } => {
            let url = format!("{}/api/plugins/{}/tasks", base_url, plugin_id);
            let response: Value = client.get(&url).send().await?.json().await?;
            println!("{}", serde_json::to_string_pretty(&response)?);
        }
    }
    Ok(())
}

async fn handle_task(client: &Client, base_url: &str, command: TaskCommands) -> anyhow::Result<()> {
    match command {
        TaskCommands::List => {
            let url = format!("{}/api/tasks", base_url);
            let response: Value = client.get(&url).send().await?.json().await?;
            println!("{}", serde_json::to_string_pretty(&response)?);
        }
        TaskCommands::Execute { plugin_id, task_id } => {
            let url = format!("{}/api/tasks/execute", base_url);
            let body = serde_json::json!({
                "plugin_id": plugin_id,
                "task_id": task_id
            });
            info!("Executing task: {} / {}", plugin_id, task_id);
            let response: Value = client.post(&url).json(&body).send().await?.json().await?;
            println!("{}", serde_json::to_string_pretty(&response)?);
        }
    }
    Ok(())
}

async fn handle_webhook(
    client: &Client,
    base_url: &str,
    command: WebhookCommands,
) -> anyhow::Result<()> {
    match command {
        WebhookCommands::DockerHealth { token } => {
            trigger_webhook(client, base_url, "/api/webhooks/docker/health", token).await
        }
        WebhookCommands::DockerCleanup { token } => {
            trigger_webhook(client, base_url, "/api/webhooks/docker/cleanup", token).await
        }
        WebhookCommands::DockerAnalysis { token } => {
            trigger_webhook(client, base_url, "/api/webhooks/docker/analysis", token).await
        }
        WebhookCommands::UpdatesCheck { token } => {
            trigger_webhook(client, base_url, "/api/webhooks/updates/check", token).await
        }
        WebhookCommands::UpdatesApply { token } => {
            trigger_webhook(client, base_url, "/api/webhooks/updates/apply", token).await
        }
        WebhookCommands::OsCleanup { token } => {
            trigger_webhook(client, base_url, "/api/webhooks/updates/cleanup", token).await
        }
        WebhookCommands::Trigger {
            plugin_id,
            task_id,
            token,
        } => {
            let path = format!("/api/webhooks/trigger/{}/{}", plugin_id, task_id);
            trigger_webhook(client, base_url, &path, token).await
        }
    }
}

async fn trigger_webhook(
    client: &Client,
    base_url: &str,
    path: &str,
    token: Option<String>,
) -> anyhow::Result<()> {
    let url = format!("{}{}", base_url, path);
    let mut request = client.post(&url);

    if let Some(t) = token {
        request = request.header("Authorization", format!("Bearer {}", t));
    }

    let body = serde_json::json!({});
    info!("Triggering webhook: {}", path);
    let response: Value = request.json(&body).send().await?.json().await?;
    println!("{}", serde_json::to_string_pretty(&response)?);

    Ok(())
}
