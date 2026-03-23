//! Server health monitoring with rstify notification integration
//!
//! Periodically checks SSH connectivity to all enabled servers and sends
//! notifications via rstify when servers go down or come back up.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use svrctlrs_core::notifications::NotificationClient;
use svrctlrs_database::queries::{servers, settings};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use crate::state::AppState;

/// Track health state per server
type HealthState = Arc<RwLock<HashMap<i64, bool>>>;

/// Start the background health monitor task
pub fn start_health_monitor(state: AppState) {
    let health_state: HealthState = Arc::new(RwLock::new(HashMap::new()));

    tokio::spawn(async move {
        info!("Health monitor started");

        loop {
            // Load settings to check if health monitoring is enabled
            let interval_secs = match run_health_check(&state, &health_state).await {
                Ok(interval) => interval,
                Err(e) => {
                    error!(error = %e, "Health check cycle failed");
                    300 // Default 5 minutes on error
                }
            };

            tokio::time::sleep(Duration::from_secs(interval_secs)).await;
        }
    });
}

/// Run one cycle of health checks. Returns the interval for next check.
async fn run_health_check(state: &AppState, health_state: &HealthState) -> anyhow::Result<u64> {
    // Check if health monitoring is enabled
    let enabled =
        settings::get_setting_value_or(&state.pool, "notification.health_check_enabled", "false")
            .await?;
    if enabled != "true" {
        debug!("Health check disabled, sleeping");
        return Ok(60); // Check again in 60s if disabled (in case it gets enabled)
    }

    // Get interval
    let interval_str = settings::get_setting_value_or(
        &state.pool,
        "notification.health_check_interval_seconds",
        "300",
    )
    .await?;
    let interval: u64 = interval_str.parse().unwrap_or(300);

    // Load notification config
    let config = settings::get_notification_config(&state.pool).await?;
    let client = NotificationClient::new(config);

    // Load all enabled servers
    let all_servers = servers::list_servers(&state.pool).await?;
    let enabled_servers: Vec<_> = all_servers
        .into_iter()
        .filter(|s| s.enabled && !s.is_local)
        .collect();

    if enabled_servers.is_empty() {
        debug!("No enabled remote servers to check");
        return Ok(interval);
    }

    debug!(count = enabled_servers.len(), "Running health checks");

    let mut state_map = health_state.write().await;

    for server in &enabled_servers {
        // Simple SSH connection test — try to connect with a short timeout
        let reachable = test_server_reachable(state, server.id).await;
        let previous = state_map.get(&server.id).copied();

        match (previous, reachable) {
            (Some(true), false) | (None, false) => {
                // Server went down (or first check found it down)
                if previous.is_some() {
                    warn!(server = %server.name, "Server went UNREACHABLE");
                    if let Err(e) = client
                        .send(
                            &format!("Server Down: {}", server.name),
                            &format!(
                                "Server '{}' ({}) is unreachable",
                                server.name,
                                server.hostname.as_deref().unwrap_or("unknown")
                            ),
                            8,
                        )
                        .await
                    {
                        error!(error = %e, "Failed to send down notification");
                    }
                }
            }
            (Some(false), true) => {
                // Server came back up
                info!(server = %server.name, "Server is back ONLINE");
                if let Err(e) = client
                    .send(
                        &format!("Server Up: {}", server.name),
                        &format!(
                            "Server '{}' ({}) is back online",
                            server.name,
                            server.hostname.as_deref().unwrap_or("unknown")
                        ),
                        3,
                    )
                    .await
                {
                    error!(error = %e, "Failed to send recovery notification");
                }
            }
            _ => {
                // No state change
                debug!(server = %server.name, reachable, "Health check — no change");
            }
        }

        state_map.insert(server.id, reachable);
    }

    Ok(interval)
}

/// Test if a server is reachable via SSH (simplified connection test)
async fn test_server_reachable(state: &AppState, server_id: i64) -> bool {
    use tokio::net::TcpStream;
    use tokio::time::timeout;

    let server = match servers::get_server(&state.pool, server_id).await {
        Ok(s) => s,
        Err(_) => return false,
    };

    let hostname = match &server.hostname {
        Some(h) => h.clone(),
        None => return false,
    };

    let addr = format!("{}:{}", hostname, server.port);

    // Simple TCP connection test with 5-second timeout
    matches!(
        timeout(Duration::from_secs(5), TcpStream::connect(&addr)).await,
        Ok(Ok(_))
    )
}
