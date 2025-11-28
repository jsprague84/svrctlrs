// ! Docker monitoring and management features
//!
//! Provides Docker container monitoring using docker CLI commands via RemoteExecutor.
//! Works uniformly on local and remote servers.

#![allow(dead_code)]

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use svrctlrs_core::{NotificationManager, NotificationMessage, RemoteExecutor, Server};
use tracing::{info, instrument, warn};

use super::FeatureResult;

/// Docker monitoring configuration
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct DockerConfig {
    pub send_summary: bool,
    pub cpu_warn_pct: f64,
    pub mem_warn_pct: f64,
}

impl Default for DockerConfig {
    fn default() -> Self {
        Self {
            send_summary: false,
            cpu_warn_pct: 80.0,
            mem_warn_pct: 80.0,
        }
    }
}

/// Container information from docker ps
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct DockerContainer {
    #[serde(rename = "ID")]
    id: String,
    names: String,
    image: String,
    state: String,
    status: String,
}

/// Container health status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerHealth {
    pub id: String,
    pub name: String,
    pub running: bool,
    pub status: String,
    pub issues: Vec<String>,
}

/// Check Docker container health on a server
///
/// # Arguments
///
/// * `server` - Server to check (local or remote)
/// * `executor` - RemoteExecutor for running commands
/// * `notify` - NotificationManager for sending alerts
/// * `config` - Docker monitoring configuration
#[instrument(skip(executor, notify))]
pub async fn check_health(
    server: &Server,
    executor: &RemoteExecutor,
    notify: &NotificationManager,
    config: &DockerConfig,
) -> Result<FeatureResult> {
    info!(server = %server.name, "Running Docker health check");

    // List all containers using docker ps
    let ps_output = executor
        .execute(server, "docker", &["ps", "-a", "--format", "{{json .}}"])
        .await
        .context("Failed to list Docker containers")?;

    // Parse container list (each line is a JSON object)
    let mut containers: Vec<DockerContainer> = Vec::new();
    for line in ps_output.lines() {
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<DockerContainer>(line) {
            Ok(container) => containers.push(container),
            Err(e) => {
                warn!(error = %e, line = %line, "Failed to parse container JSON");
                continue;
            }
        }
    }

    if containers.is_empty() {
        return Ok(FeatureResult::success("No Docker containers found"));
    }

    // Check health of each container
    let mut health_statuses = Vec::new();
    let mut containers_with_issues = 0;

    for container in &containers {
        let mut issues = Vec::new();
        let running = container.state == "running";

        // Check if container is running
        if !running {
            issues.push(format!("Container is {}", container.state));
        }

        // Check for Exit/Restart status
        if container.status.contains("Exit") || container.status.contains("Restart") {
            issues.push(format!("Status: {}", container.status));
        }

        if !issues.is_empty() {
            containers_with_issues += 1;
        }

        health_statuses.push(ContainerHealth {
            id: container.id.clone(),
            name: container.names.clone(),
            running,
            status: container.status.clone(),
            issues: issues.clone(),
        });
    }

    // Send notification if there are issues or if summary is requested
    if containers_with_issues > 0 || config.send_summary {
        send_health_notification(notify, server, &health_statuses, containers_with_issues).await?;
    }

    let total = containers.len();
    let running = health_statuses.iter().filter(|c| c.running).count();

    let message = format!(
        "Docker health check on {}: {} containers ({} running, {} with issues)",
        server.name, total, running, containers_with_issues
    );

    // Prepare structured data
    let data = json!({
        "server": server.name,
        "total_containers": total,
        "running_containers": running,
        "containers_with_issues": containers_with_issues,
        "health_statuses": health_statuses,
    });

    // Prepare metrics
    let mut metrics = HashMap::new();
    metrics.insert("total_containers".to_string(), total as f64);
    metrics.insert("running_containers".to_string(), running as f64);
    metrics.insert(
        "containers_with_issues".to_string(),
        containers_with_issues as f64,
    );

    Ok(FeatureResult::success_with_data(
        message,
        data,
        Some(metrics),
    ))
}

/// Send Docker health notification
async fn send_health_notification(
    notify: &NotificationManager,
    server: &Server,
    health_statuses: &[ContainerHealth],
    containers_with_issues: usize,
) -> Result<()> {
    let title = if containers_with_issues > 0 {
        format!(
            "Docker Health Alert: {} - {} issues",
            server.name, containers_with_issues
        )
    } else {
        format!("Docker Health Summary: {}", server.name)
    };

    let mut body = String::from("## Docker Container Health\n\n");

    if containers_with_issues > 0 {
        body.push_str(&format!(
            "⚠️ **Containers with Issues**: {}\n\n",
            containers_with_issues
        ));

        for health in health_statuses.iter().filter(|h| !h.issues.is_empty()) {
            body.push_str(&format!("**{}**\n", health.name));
            body.push_str(&format!("  - ID: {}\n", &health.id[..12]));
            body.push_str(&format!("  - Running: {}\n", health.running));
            for issue in &health.issues {
                body.push_str(&format!("  - ⚠️ {}\n", issue));
            }
            body.push('\n');
        }
    } else {
        body.push_str("✅ All containers healthy\n\n");
        body.push_str(&format!(
            "**Summary**: {} containers running\n",
            health_statuses.iter().filter(|h| h.running).count()
        ));
    }

    let priority = if containers_with_issues > 0 { 4 } else { 3 };

    let message = NotificationMessage {
        title,
        body,
        priority,
        actions: vec![],
    };

    notify
        .send_for_service("docker", &message)
        .await
        .context("Failed to send Docker health notification")?;

    Ok(())
}

/// Check for Docker image updates
///
/// Lists running containers and their images
#[instrument(skip(executor, notify))]
pub async fn check_image_updates(
    server: &Server,
    executor: &RemoteExecutor,
    notify: &NotificationManager,
) -> Result<FeatureResult> {
    info!(server = %server.name, "Checking Docker image updates");

    // List running containers
    let ps_output = executor
        .execute(server, "docker", &["ps", "--format", "{{json .}}"])
        .await
        .context("Failed to list Docker containers")?;

    // Parse containers and collect unique images
    let mut unique_images = std::collections::HashSet::new();
    let mut container_count = 0;

    for line in ps_output.lines() {
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(container) = serde_json::from_str::<DockerContainer>(line) {
            container_count += 1;
            unique_images.insert(container.image);
        }
    }

    let image_list: Vec<String> = unique_images.iter().cloned().collect();

    // Send notification
    send_image_update_report(notify, server, &image_list, container_count).await?;

    let message = format!(
        "Docker image report on {}: {} unique images in use across {} containers",
        server.name,
        image_list.len(),
        container_count
    );

    let mut metrics = HashMap::new();
    metrics.insert("unique_images".to_string(), image_list.len() as f64);
    metrics.insert("running_containers".to_string(), container_count as f64);

    Ok(FeatureResult::success_with_data(
        message,
        json!({
            "server": server.name,
            "images": image_list,
            "container_count": container_count,
        }),
        Some(metrics),
    ))
}

/// Send Docker image update report notification
async fn send_image_update_report(
    notify: &NotificationManager,
    server: &Server,
    images: &[String],
    container_count: usize,
) -> Result<()> {
    let title = format!("Docker Image Status: {}", server.name);

    let mut body = String::from("📦 **Docker Image Status**\n\n");
    body.push_str(&format!("**Images in Use**: {}\n", images.len()));
    body.push_str(&format!("**Running Containers**: {}\n\n", container_count));

    if images.is_empty() {
        body.push_str("No running containers found.\n");
    } else {
        body.push_str("**Active Images**:\n");
        for (i, image) in images.iter().enumerate() {
            body.push_str(&format!("{}. {}\n", i + 1, image));
        }
        body.push_str("\n💡 **Tip**: Run `docker pull <image>` to check for updates\n");
    }

    let message = NotificationMessage {
        title,
        body,
        priority: 3,
        actions: vec![],
    };

    notify
        .send_for_service("docker", &message)
        .await
        .context("Failed to send image update report")?;

    Ok(())
}
