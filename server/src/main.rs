//! SvrCtlRS Server — Terminal-focused infrastructure management

#[cfg(test)]
mod api_shapes;
mod api_tokens;
mod app;
mod approvals;
mod audit;
mod diagnose;
mod health_monitor;
mod host_keys;
mod key_ops;
mod login_limiter;
mod recordings;
mod request_id;
mod routes;
mod sessions;
mod sign_in;
mod ssh;
mod ssh_config;
mod ssh_keys;
mod state;
#[cfg(test)]
mod test_support;
mod ws_tickets;

use state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    use clap::Parser;
    use tower_sessions::ExpiredDeletion;
    use tower_sessions_sqlx_store::SqliteStore;
    use tracing::{info, warn};

    /// SvrCtlRS Server
    #[derive(Parser, Debug)]
    #[command(author, version, about, long_about = None)]
    struct Args {
        /// Server address to bind to
        #[arg(short, long, default_value = "0.0.0.0:8080")]
        addr: String,
    }

    // Initialize tracing
    // LOG_FORMAT=json emits one JSON object per line for log shippers; the default stays
    // human-readable for `docker logs`.
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "info,server=debug".into());
    if std::env::var("LOG_FORMAT").is_ok_and(|f| f.eq_ignore_ascii_case("json")) {
        tracing_subscriber::fmt()
            .json()
            .with_env_filter(env_filter)
            .init();
    } else {
        tracing_subscriber::fmt().with_env_filter(env_filter).init();
    }

    // Load environment variables
    dotenvy::dotenv().ok();

    // Credential encryption is not optional. A missing key is generated into data/encryption.key;
    // an invalid one (wrong length, bad hex, unreadable file) must stop the server rather than
    // let credentials be written in plaintext, which is what an empty ENCRYPTION_KEY once did.
    svrctlrs_core::encryption::init_encryption_key().map_err(|e| {
        anyhow::anyhow!(
            "cannot initialise credential encryption ({e}); set ENCRYPTION_KEY to 64 hex \
             characters or make data/encryption.key readable — refusing to start"
        )
    })?;

    // Parse CLI args
    let args = Args::parse();

    info!(addr = %args.addr, "Starting SvrCtlRS server");

    // Initialize database
    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:data/svrctlrs.db".to_string());
    info!(url = %database_url, "Connecting to database");

    let database = svrctlrs_database::Database::new(&database_url).await?;
    database.migrate().await?;

    // A database that lived through a start without a key (the pre-3.0 fallback) holds
    // plaintext rows; encrypt them now that a key is guaranteed.
    let re_encrypted =
        svrctlrs_database::queries::credentials::encrypt_plaintext_credentials(database.pool())
            .await?;
    if re_encrypted > 0 {
        tracing::warn!(
            count = re_encrypted,
            "Encrypted credentials that had been stored in plaintext"
        );
    }

    // Audit entries older than half a year are not worth the disk; the log is for recent
    // "who did what", not long-term records.
    let pruned =
        svrctlrs_database::queries::audit::prune_older_than_days(database.pool(), 180).await?;
    if pruned > 0 {
        info!(count = pruned, "Pruned old audit entries");
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

    // Seed the first account from ADMIN_PASSWORD (and ADMIN_USERNAME, default "admin") when no
    // users exist. Without a password there is nobody to sign in as, so say so.
    {
        use svrctlrs_database::queries::users;

        let user_count = users::count_users(database.pool()).await?;
        if user_count == 0 {
            match std::env::var("ADMIN_PASSWORD") {
                Ok(admin_password) if !admin_password.is_empty() => {
                    let admin_username = std::env::var("ADMIN_USERNAME")
                        .ok()
                        .map(|u| u.trim().to_string())
                        .filter(|u| !u.is_empty())
                        .unwrap_or_else(|| "admin".to_string());
                    let input = svrctlrs_database::models::CreateUser {
                        username: admin_username.clone(),
                        password: admin_password,
                    };
                    users::create_user(database.pool(), &input).await?;
                    info!(username = %admin_username, "Created initial admin user from environment variables");
                }
                _ => warn!(
                    "No users exist and ADMIN_PASSWORD is not set: nobody can sign in until it is \
                     set and the server restarted"
                ),
            }
        }
    }

    // ALLOWED_ORIGINS lists the origins of non-browser clients (Tauri desktop/Android).
    // Compose files often pass it through as an empty string, which must mean "unset":
    // an empty value used to switch the session cookie to SameSite=None for nothing.
    let allowed_origins: Vec<String> = std::env::var("ALLOWED_ORIGINS")
        .ok()
        .map(|s| {
            s.split(',')
                .map(str::trim)
                .filter(|o| !o.is_empty())
                .map(String::from)
                .collect()
        })
        .unwrap_or_default();

    // How people sign in: password, an OpenID Connect provider, or both. A half-made
    // configuration (a provider without a client id, password sign-in off with nothing in
    // its place) stops the server rather than lock everyone out or fall back silently.
    let sign_in = sign_in::SignInConfig::from_env()
        .map_err(|e| anyhow::anyhow!("sign-in configuration: {e} — refusing to start"))?;
    if let Some(provider) = sign_in.provider() {
        info!(
            issuer = %provider.config.issuer,
            name = %provider.config.name,
            redirect_uri = %provider.config.redirect_uri,
            password_login = sign_in.password_login,
            "OpenID Connect sign-in configured"
        );
        let linked =
            svrctlrs_database::queries::user_identities::count_linked_users(database.pool())
                .await?;
        if !sign_in.password_login && linked == 0 {
            warn!(
                "PASSWORD_LOGIN=off but no account has a linked identity: nobody can sign in. \
                 Set PASSWORD_LOGIN=on, sign in with a password and link the provider under \
                 Settings → Access → Sign-in"
            );
        }
    }

    // Initialize application state
    let mut state = AppState::new(database, allowed_origins).await?;
    state.sign_in = std::sync::Arc::new(sign_in);

    // Default secure=true for production (HTTPS). Set SESSION_SECURE=false for local dev.
    let session_secure = std::env::var("SESSION_SECURE")
        .map(|v| v != "false" && v != "0")
        .unwrap_or(true);
    info!(secure = session_secure, "Session cookie configuration");

    let app = app::router(state.clone(), session_store, session_secure);

    // Start background health monitor
    health_monitor::start_health_monitor(state.clone());
    // Recordings: mark the ones a stopped server left running, then retention every hour
    recordings::start_housekeeping(state.pool.clone(), state.recordings.clone());

    // Start server with graceful shutdown
    let listener = tokio::net::TcpListener::bind(&args.addr).await?;
    info!(addr = %args.addr, "Server listening");

    let sessions = state.sessions.clone();
    let recording_store = state.recordings.clone();
    axum::serve(listener, app.into_make_service())
        .with_graceful_shutdown(async move {
            shutdown_signal().await;
            // Open sockets end with the server; tell the shells behind them, too.
            sessions.close_all().await;
            // Their recorders write the last lines and finish their rows.
            recording_store
                .finish_all(std::time::Duration::from_secs(5))
                .await;
        })
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
