//! `svrctl` — command-line companion to the SvrCtlRS server.
//!
//! `health` and `status` talk to a running server over HTTP (the container healthcheck runs
//! `svrctl health`). `backup` opens the SQLite database directly and writes a consistent copy,
//! so it is meant to run next to the server, e.g.
//! `docker exec svrctlrs /app/svrctl backup --out /app/data/backups`.

use clap::{Parser, Subcommand};
use reqwest::Client;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process;
use svrctlrs_database::sqlx::sqlite::SqlitePoolOptions;
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

    /// Session token (from `POST /api/v1/auth/login`) for authenticated commands
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

    /// Write a consistent copy of the database (and the key file, when the key is not in the
    /// environment) into a directory, keeping only the newest copies.
    Backup {
        /// Directory to write into (created if missing)
        #[arg(long, default_value = "data/backups")]
        out: PathBuf,

        /// Number of backups to keep in the directory
        #[arg(long, default_value_t = 7)]
        keep: usize,

        /// Database to back up
        #[arg(long, env = "DATABASE_URL", default_value = "sqlite:data/svrctlrs.db")]
        database_url: String,
    },
}

#[derive(Subcommand, Debug)]
enum StatusCommands {
    /// Check server health
    Health,

    /// Get server status (requires --token)
    Server,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let cli = Cli::parse();
    let client = Client::new();

    let result = match cli.command {
        Commands::Health => handle_health(&client, &cli.url).await,
        Commands::Status { command } => {
            handle_status(&client, &cli.url, cli.token.as_deref(), command).await
        }
        Commands::Backup {
            out,
            keep,
            database_url,
        } => handle_backup(&database_url, &out, keep).await,
    };

    if let Err(e) = result {
        error!("Command failed: {}", e);
        process::exit(1);
    }
}

fn get(client: &Client, url: &str, token: Option<&str>) -> reqwest::RequestBuilder {
    let req = client.get(url);
    match token {
        Some(t) => req.bearer_auth(t),
        None => req,
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
    token: Option<&str>,
    command: StatusCommands,
) -> anyhow::Result<()> {
    let path = match command {
        StatusCommands::Health => "/api/v1/health",
        StatusCommands::Server => "/api/v1/status",
    };
    let response = get(client, &format!("{base_url}{path}"), token)
        .send()
        .await?;
    if !response.status().is_success() {
        anyhow::bail!("{} answered {}", path, response.status());
    }
    let json: Value = response.json().await?;
    println!("{}", serde_json::to_string_pretty(&json)?);
    Ok(())
}

/// Where the server keeps its generated key file: next to the database.
fn key_file_for(database_url: &str) -> Option<PathBuf> {
    let path = database_url
        .strip_prefix("sqlite:")?
        .trim_start_matches("//");
    if path.is_empty() || path == ":memory:" {
        return None;
    }
    Some(
        Path::new(path)
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("encryption.key"),
    )
}

/// `VACUUM INTO` a fresh file: a consistent snapshot while the server keeps running.
async fn backup_database(database_url: &str, target: &Path) -> anyhow::Result<()> {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(database_url)
        .await?;
    let target_str = target
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("backup path is not valid UTF-8"))?
        .replace('\'', "''");
    svrctlrs_database::sqlx::query(&format!("VACUUM INTO '{target_str}'"))
        .execute(&pool)
        .await?;
    pool.close().await;
    Ok(())
}

/// Backups beyond `keep`, oldest first, by file name (names carry the timestamp).
fn prunable(mut backups: Vec<PathBuf>, keep: usize) -> Vec<PathBuf> {
    backups.sort();
    if backups.len() <= keep {
        return Vec::new();
    }
    let excess = backups.len() - keep;
    backups.into_iter().take(excess).collect()
}

async fn handle_backup(database_url: &str, out: &Path, keep: usize) -> anyhow::Result<()> {
    std::fs::create_dir_all(out)?;
    let stamp = chrono::Utc::now().format("%Y%m%d-%H%M%SZ");
    let target = out.join(format!("svrctlrs-{stamp}.db"));
    backup_database(database_url, &target).await?;
    let size = std::fs::metadata(&target)?.len();

    // The key: without it the backup's credentials are unreadable.
    let key = if std::env::var("ENCRYPTION_KEY").is_ok_and(|k| !k.trim().is_empty()) {
        "in ENCRYPTION_KEY — back up the environment (.env) with this file"
    } else if let Some(key_file) = key_file_for(database_url).filter(|p| p.exists()) {
        let dest = out.join("encryption.key");
        std::fs::copy(&key_file, &dest)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(0o600))?;
        }
        "copied to encryption.key"
    } else {
        "not found — credentials in this backup cannot be decrypted without it"
    };

    let existing: Vec<PathBuf> = std::fs::read_dir(out)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("svrctlrs-") && n.ends_with(".db"))
        })
        .collect();
    let kept = existing.len().min(keep.max(1));
    let removed = prunable(existing, keep.max(1));
    for old in &removed {
        std::fs::remove_file(old)?;
    }

    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "backup": target,
            "bytes": size,
            "key": key,
            "kept": kept,
            "removed": removed.len(),
        }))?
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_file_sits_next_to_the_database() {
        assert_eq!(
            key_file_for("sqlite:/app/data/svrctlrs.db"),
            Some(PathBuf::from("/app/data/encryption.key"))
        );
        assert_eq!(
            key_file_for("sqlite:data/svrctlrs.db"),
            Some(PathBuf::from("data/encryption.key"))
        );
        assert_eq!(key_file_for("sqlite::memory:"), None);
        assert_eq!(key_file_for("postgres://x"), None);
    }

    #[test]
    fn prunes_oldest_beyond_keep() {
        let files: Vec<PathBuf> = [
            "svrctlrs-20260103-000000Z.db",
            "svrctlrs-20260101-000000Z.db",
            "svrctlrs-20260102-000000Z.db",
        ]
        .iter()
        .map(PathBuf::from)
        .collect();
        let gone = prunable(files.clone(), 2);
        assert_eq!(gone, vec![PathBuf::from("svrctlrs-20260101-000000Z.db")]);
        assert!(prunable(files, 3).is_empty());
    }

    #[tokio::test]
    async fn backup_is_a_consistent_copy() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("live.db");
        let url = format!("sqlite:{}?mode=rwc", db.display());
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect(&url)
            .await
            .unwrap();
        svrctlrs_database::sqlx::query("CREATE TABLE t (v TEXT); INSERT INTO t VALUES ('hello')")
            .execute(&pool)
            .await
            .unwrap();

        let target = dir.path().join("copy.db");
        backup_database(&url, &target).await.unwrap();

        let copy = SqlitePoolOptions::new()
            .max_connections(1)
            .connect(&format!("sqlite:{}", target.display()))
            .await
            .unwrap();
        let (v,): (String,) = svrctlrs_database::sqlx::query_as("SELECT v FROM t")
            .fetch_one(&copy)
            .await
            .unwrap();
        assert_eq!(v, "hello");
    }
}
