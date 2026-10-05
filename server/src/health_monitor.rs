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

    // Probe every server at once; one unreachable host must not delay the others by its
    // whole timeout. The shared state is touched only once the results are in.
    let probes = enabled_servers.iter().map(|server| async move {
        let reachable = test_server_reachable(server).await;
        (server, reachable)
    });
    let results = futures_util::future::join_all(probes).await;

    let mut state_map = health_state.write().await;
    for (server, reachable) in results {
        let previous = state_map.get(&server.id).copied();
        let hostname = server.hostname.as_deref().unwrap_or("unknown");

        // What the Servers page shows: when the host last answered, and why it did not.
        if reachable {
            if let Err(e) = servers::update_server_status(&state.pool, server.id, None).await {
                warn!(server = %server.name, error = %e, "Failed to record server status");
            }
        } else if let Err(e) = servers::update_server_last_error(
            &state.pool,
            server.id,
            &format!(
                "Unreachable on {hostname}:{} (TCP connect timed out)",
                server.port
            ),
        )
        .await
        {
            warn!(server = %server.name, error = %e, "Failed to record server error");
        }

        match (previous, reachable) {
            (Some(true), false) => {
                warn!(server = %server.name, "Server went UNREACHABLE");
                if let Err(e) = client
                    .send(
                        &format!("Server Down: {}", server.name),
                        &format!("Server '{}' ({hostname}) is unreachable", server.name),
                        8,
                    )
                    .await
                {
                    error!(error = %e, "Failed to send down notification");
                }
            }
            (None, false) => {
                // First sight of this server and it is down: say so once rather than
                // staying silent until it flaps.
                warn!(server = %server.name, "Server unreachable at first check");
            }
            (Some(false), true) => {
                info!(server = %server.name, "Server is back ONLINE");
                if let Err(e) = client
                    .send(
                        &format!("Server Up: {}", server.name),
                        &format!("Server '{}' ({hostname}) is back online", server.name),
                        3,
                    )
                    .await
                {
                    error!(error = %e, "Failed to send recovery notification");
                }
            }
            _ => {
                debug!(server = %server.name, reachable, "Health check — no change");
            }
        }
        state_map.insert(server.id, reachable);
    }

    Ok(interval)
}

/// TCP reachability of the SSH port with a short timeout. Not an SSH handshake: cheap, and
/// enough to tell "host is down" from "host is up".
async fn test_server_reachable(server: &svrctlrs_database::models::Server) -> bool {
    use tokio::net::TcpStream;
    use tokio::time::timeout;

    let Some(hostname) = server.hostname.as_deref().filter(|h| !h.is_empty()) else {
        return false;
    };
    let addr = format!("{}:{}", hostname, server.port);
    matches!(
        timeout(Duration::from_secs(5), TcpStream::connect(&addr)).await,
        Ok(Ok(_))
    )
}
