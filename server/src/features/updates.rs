//! OS updates monitoring and management features
//!
//! Provides OS update detection, execution, and cleanup operations using RemoteExecutor.
//! Works uniformly on local and remote servers.

use anyhow::{Context, Result};
use serde_json::json;
use std::collections::HashMap;
use svrctlrs_core::{NotificationManager, NotificationMessage, RemoteExecutor, Server};
use tracing::{info, instrument};

use super::FeatureResult;

// Re-use the existing update detection logic from the updates plugin
use svrctlrs_plugin_updates::detection::{get_checker, PackageManager, UpdateInfo};

/// Updates monitoring configuration
#[derive(Debug, Clone, Default)]
pub struct UpdatesConfig {
    pub send_summary: bool,
}

/// Check for available OS updates on a server
///
/// # Arguments
///
/// * `server` - Server to check (local or remote)
/// * `executor` - RemoteExecutor for running commands
/// * `notify` - NotificationManager for sending alerts
/// * `config` - Updates monitoring configuration
#[instrument(skip(executor, notify))]
pub async fn check_updates(
    server: &Server,
    executor: &RemoteExecutor,
    notify: &NotificationManager,
    config: &UpdatesConfig,
) -> Result<FeatureResult> {
    info!(server = %server.name, "Checking for OS updates");

    // Detect package manager
    let package_manager = detect_package_manager(server, executor).await?;
    info!(
        server = %server.name,
        pm = ?package_manager,
        "Detected package manager"
    );

    // Get the appropriate checker
    let checker = get_checker(&package_manager);

    // Run update check command
    let (cmd, args) = checker.check_command();
    let output = executor
        .execute(server, cmd, &args)
        .await
        .context("Failed to check for updates")?;

    // Parse updates from output
    let packages = checker.parse_updates(&output);
    let total_updates = packages.len();

    // Count security updates
    let security_updates = packages.iter().filter(|p| p.contains("(security)")).count();

    let update_info = UpdateInfo {
        package_manager: package_manager.display_name().to_string(),
        total_updates,
        security_updates,
        packages: packages.clone(),
    };

    // Send notification if updates available or if summary requested
    if total_updates > 0 {
        send_update_notification(notify, &update_info, &server.name).await?;
    } else if config.send_summary {
        send_no_updates_summary(notify, &update_info, &server.name).await?;
    }

    let message = format!(
        "Updates check on {}: {} packages available ({} security)",
        server.name, total_updates, security_updates
    );

    // Prepare structured data
    let data = json!({
        "server": server.name,
        "package_manager": update_info.package_manager,
        "total_updates": total_updates,
        "security_updates": security_updates,
        "packages": packages,
    });

    // Prepare metrics
    let mut metrics = HashMap::new();
    metrics.insert("total_updates".to_string(), total_updates as f64);
    metrics.insert("security_updates".to_string(), security_updates as f64);

    Ok(FeatureResult::success_with_data(
        message,
        data,
        Some(metrics),
    ))
}

/// Detect the package manager on a server
async fn detect_package_manager(
    server: &Server,
    executor: &RemoteExecutor,
) -> Result<PackageManager> {
    // Try each package manager in order
    for pm in PackageManager::all() {
        // Check if the binary exists
        let check_cmd = format!("command -v {}", pm.binary());
        if let Ok(output) = executor.execute(server, "sh", &["-c", &check_cmd]).await {
            if !output.trim().is_empty() {
                return Ok(pm);
            }
        }
    }

    anyhow::bail!(
        "No supported package manager found on server {}",
        server.name
    );
}

/// Send update notification
async fn send_update_notification(
    notify: &NotificationManager,
    update_info: &UpdateInfo,
    server_name: &str,
) -> Result<()> {
    let title = format!("Updates Available: {}", server_name);

    let mut body = String::new();
    body.push_str(&format!(
        "## {} Updates Available\n\n",
        update_info.total_updates
    ));
    body.push_str(&format!("**Server**: {}\n", server_name));
    body.push_str(&format!(
        "**Package Manager**: {}\n\n",
        update_info.package_manager
    ));

    if update_info.security_updates > 0 {
        body.push_str(&format!(
            "🔒 **Security Updates**: {}\n\n",
            update_info.security_updates
        ));
    }

    // List first 10 packages
    if !update_info.packages.is_empty() {
        body.push_str("**Available Packages**:\n");
        for (i, pkg) in update_info.packages.iter().take(10).enumerate() {
            body.push_str(&format!("{}. {}\n", i + 1, pkg));
        }
        if update_info.packages.len() > 10 {
            body.push_str(&format!(
                "\n...and {} more\n",
                update_info.packages.len() - 10
            ));
        }
    }

    let priority = if update_info.security_updates > 0 {
        4
    } else {
        3
    };

    let message = NotificationMessage {
        title,
        body,
        priority,
        actions: vec![],
    };

    notify
        .send_for_service("updates", &message)
        .await
        .context("Failed to send update notification")?;

    Ok(())
}

/// Send no updates summary
async fn send_no_updates_summary(
    notify: &NotificationManager,
    update_info: &UpdateInfo,
    server_name: &str,
) -> Result<()> {
    let title = format!("Updates Status: {}", server_name);

    let body = format!(
        "📊 System Up to Date ✓\n\n\
        **Server**: {}\n\
        **Package Manager**: {}\n\
        **Status**: No updates available\n\n\
        All packages are current.",
        server_name, update_info.package_manager
    );

    let message = NotificationMessage {
        title,
        body,
        priority: 3,
        actions: vec![],
    };

    notify
        .send_for_service("updates", &message)
        .await
        .context("Failed to send summary notification")?;

    Ok(())
}

/// Generate update status report for all servers
///
/// # Arguments
///
/// * `servers` - List of servers to check
/// * `executor` - RemoteExecutor for running commands
/// * `notify` - NotificationManager for sending alerts
#[instrument(skip(executor, notify))]
pub async fn generate_updates_report(
    servers: &[Server],
    executor: &RemoteExecutor,
    notify: &NotificationManager,
) -> Result<FeatureResult> {
    info!(
        "Generating OS update status report for {} servers",
        servers.len()
    );

    if servers.is_empty() {
        return Ok(FeatureResult::success("No servers configured"));
    }

    let mut server_statuses = Vec::new();
    let mut total_updates = 0;
    let mut total_security = 0;
    let mut servers_with_updates = 0;

    // Check updates for each server
    for server in servers {
        info!(server = %server.name, "Checking updates for server");

        // Detect package manager and check updates
        let update_info = match detect_and_check_updates(server, executor).await {
            Ok(info) => info,
            Err(e) => {
                info!(server = %server.name, error = %e, "Failed to check updates");
                continue;
            }
        };

        if update_info.total_updates > 0 {
            servers_with_updates += 1;
        }

        total_updates += update_info.total_updates;
        total_security += update_info.security_updates;

        server_statuses.push(json!({
            "server": server.name,
            "total_updates": update_info.total_updates,
            "security_updates": update_info.security_updates,
            "package_manager": update_info.package_manager,
        }));
    }

    // Send report notification
    send_multi_server_report(
        notify,
        &server_statuses,
        total_updates,
        total_security,
        servers_with_updates,
    )
    .await?;

    let message = format!(
        "Update report: {} servers checked, {} with updates available ({} security)",
        server_statuses.len(),
        servers_with_updates,
        total_security
    );

    let mut metrics = HashMap::new();
    metrics.insert("servers_checked".to_string(), server_statuses.len() as f64);
    metrics.insert(
        "servers_with_updates".to_string(),
        servers_with_updates as f64,
    );
    metrics.insert("total_updates".to_string(), total_updates as f64);
    metrics.insert("total_security_updates".to_string(), total_security as f64);

    Ok(FeatureResult::success_with_data(
        message,
        json!({
            "servers": server_statuses,
            "summary": {
                "total_updates": total_updates,
                "total_security": total_security,
                "servers_with_updates": servers_with_updates,
            }
        }),
        Some(metrics),
    ))
}

/// Helper: Detect package manager and check updates
async fn detect_and_check_updates(
    server: &Server,
    executor: &RemoteExecutor,
) -> Result<UpdateInfo> {
    let package_manager = detect_package_manager(server, executor).await?;
    let checker = get_checker(&package_manager);
    let (cmd, args) = checker.check_command();
    let output = executor.execute(server, cmd, &args).await?;
    let packages = checker.parse_updates(&output);

    let security_updates = packages.iter().filter(|p| p.contains("(security)")).count();

    Ok(UpdateInfo {
        package_manager: package_manager.display_name().to_string(),
        total_updates: packages.len(),
        security_updates,
        packages,
    })
}

/// Send multi-server report notification
async fn send_multi_server_report(
    notify: &NotificationManager,
    server_statuses: &[serde_json::Value],
    total_updates: usize,
    total_security: usize,
    servers_with_updates: usize,
) -> Result<()> {
    let title = "OS Update Status Report".to_string();

    let mut body = String::new();
    body.push_str("📊 **Multi-Server Update Status**\n\n");

    body.push_str(&format!(
        "**Summary**: {} servers checked\n",
        server_statuses.len()
    ));
    body.push_str(&format!(
        "**Updates Available**: {} servers\n",
        servers_with_updates
    ));
    body.push_str(&format!("**Total Updates**: {}\n", total_updates));

    if total_security > 0 {
        body.push_str(&format!("🔒 **Security Updates**: {}\n", total_security));
    }

    body.push_str("\n**Server Details**:\n\n");

    // List each server status
    for status in server_statuses {
        let server_name = status["server"].as_str().unwrap_or("unknown");
        let updates = status["total_updates"].as_u64().unwrap_or(0);
        let security = status["security_updates"].as_u64().unwrap_or(0);
        let pkg_mgr = status["package_manager"].as_str().unwrap_or("unknown");

        if updates > 0 {
            body.push_str(&format!(
                "⚠️  **{}**: {} updates ({} security) [{}]\n",
                server_name, updates, security, pkg_mgr
            ));
        } else {
            body.push_str(&format!(
                "✓ **{}**: Up to date [{}]\n",
                server_name, pkg_mgr
            ));
        }
    }

    let priority = if total_security > 0 { 4 } else { 3 };

    let message = NotificationMessage {
        title,
        body,
        priority,
        actions: vec![],
    };

    notify
        .send_for_service("updates", &message)
        .await
        .context("Failed to send multi-server report")?;

    Ok(())
}
