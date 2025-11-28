//! System health monitoring features
//!
//! Monitors system resources (CPU, memory, disk) using RemoteExecutor.
//! Works uniformly on local and remote servers.

#![allow(dead_code)]

use anyhow::{Context, Result};
use serde_json::json;
use std::collections::HashMap;
use svrctlrs_core::{NotificationManager, NotificationMessage, RemoteExecutor, Server};
use tracing::{info, instrument, warn};

use super::FeatureResult;

/// Health monitoring configuration
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct HealthConfig {
    pub send_summary: bool,
    pub cpu_warn_pct: f64,
    pub mem_warn_pct: f64,
    pub disk_warn_pct: f64,
}

impl Default for HealthConfig {
    fn default() -> Self {
        Self {
            send_summary: false,
            cpu_warn_pct: 80.0,
            mem_warn_pct: 80.0,
            disk_warn_pct: 85.0,
        }
    }
}

/// System metrics
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SystemMetrics {
    pub cpu_percent: Option<f64>,
    pub mem_total_mb: u64,
    pub mem_used_mb: u64,
    pub mem_percent: f64,
    pub disk_total_gb: f64,
    pub disk_used_gb: f64,
    pub disk_percent: f64,
    pub load_1min: Option<f64>,
    pub load_5min: Option<f64>,
    pub load_15min: Option<f64>,
}

/// Collect system metrics from a server
///
/// # Arguments
///
/// * `server` - Server to check (local or remote)
/// * `executor` - RemoteExecutor for running commands
/// * `notify` - NotificationManager for sending alerts
/// * `config` - Health monitoring configuration
#[instrument(skip(executor, notify))]
pub async fn collect_metrics(
    server: &Server,
    executor: &RemoteExecutor,
    notify: &NotificationManager,
    config: &HealthConfig,
) -> Result<FeatureResult> {
    info!(server = %server.name, "Collecting system metrics");

    // Collect various metrics via CLI commands
    let mem_metrics = collect_memory_metrics(server, executor).await?;
    let disk_metrics = collect_disk_metrics(server, executor).await?;
    let load_metrics = collect_load_metrics(server, executor).await?;

    let metrics = SystemMetrics {
        cpu_percent: None, // CPU percentage is tricky to get accurately via single command
        mem_total_mb: mem_metrics.0,
        mem_used_mb: mem_metrics.1,
        mem_percent: mem_metrics.2,
        disk_total_gb: disk_metrics.0,
        disk_used_gb: disk_metrics.1,
        disk_percent: disk_metrics.2,
        load_1min: load_metrics.0,
        load_5min: load_metrics.1,
        load_15min: load_metrics.2,
    };

    // Check thresholds and collect issues
    let mut issues = Vec::new();

    if metrics.mem_percent > config.mem_warn_pct {
        issues.push(format!(
            "High memory usage: {:.1}% (threshold: {:.0}%)",
            metrics.mem_percent, config.mem_warn_pct
        ));
    }

    if metrics.disk_percent > config.disk_warn_pct {
        issues.push(format!(
            "High disk usage: {:.1}% (threshold: {:.0}%)",
            metrics.disk_percent, config.disk_warn_pct
        ));
    }

    // Send notification if issues found or if summary requested
    if !issues.is_empty() || config.send_summary {
        send_health_notification(notify, &server.name, &metrics, &issues).await?;
    }

    let message = if issues.is_empty() {
        format!(
            "System health on {}: OK (mem: {:.1}%, disk: {:.1}%)",
            server.name, metrics.mem_percent, metrics.disk_percent
        )
    } else {
        format!(
            "System health on {}: {} issues detected",
            server.name,
            issues.len()
        )
    };

    // Prepare structured data
    let data = json!({
        "server": server.name,
        "memory_total_mb": metrics.mem_total_mb,
        "memory_used_mb": metrics.mem_used_mb,
        "memory_percent": metrics.mem_percent,
        "disk_total_gb": metrics.disk_total_gb,
        "disk_used_gb": metrics.disk_used_gb,
        "disk_percent": metrics.disk_percent,
        "load_1min": metrics.load_1min,
        "load_5min": metrics.load_5min,
        "load_15min": metrics.load_15min,
        "issues": issues,
    });

    // Prepare metrics map
    let mut metrics_map = HashMap::new();
    metrics_map.insert("memory_percent".to_string(), metrics.mem_percent);
    metrics_map.insert("disk_percent".to_string(), metrics.disk_percent);
    metrics_map.insert("memory_used_mb".to_string(), metrics.mem_used_mb as f64);
    metrics_map.insert("disk_used_gb".to_string(), metrics.disk_used_gb);
    if let Some(load) = metrics.load_1min {
        metrics_map.insert("load_1min".to_string(), load);
    }

    Ok(FeatureResult::success_with_data(
        message,
        data,
        Some(metrics_map),
    ))
}

/// Collect memory metrics using `free` command
async fn collect_memory_metrics(
    server: &Server,
    executor: &RemoteExecutor,
) -> Result<(u64, u64, f64)> {
    // Run 'free -m' to get memory in MB
    let output = executor
        .execute(server, "free", &["-m"])
        .await
        .context("Failed to get memory metrics")?;

    // Parse output - looking for "Mem:" line
    for line in output.lines() {
        if line.starts_with("Mem:") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 {
                let total: u64 = parts[1].parse().unwrap_or(0);
                let used: u64 = parts[2].parse().unwrap_or(0);
                let percent = if total > 0 {
                    (used as f64 / total as f64) * 100.0
                } else {
                    0.0
                };
                return Ok((total, used, percent));
            }
        }
    }

    anyhow::bail!("Failed to parse memory metrics from: {}", output);
}

/// Collect disk metrics using `df` command
async fn collect_disk_metrics(
    server: &Server,
    executor: &RemoteExecutor,
) -> Result<(f64, f64, f64)> {
    // Run 'df -BG /' to get root filesystem in GB
    let output = executor
        .execute(server, "df", &["-BG", "/"])
        .await
        .context("Failed to get disk metrics")?;

    // Parse output - looking for filesystem line (second line)
    let lines: Vec<&str> = output.lines().collect();
    if lines.len() >= 2 {
        let parts: Vec<&str> = lines[1].split_whitespace().collect();
        if parts.len() >= 5 {
            // Parse size (remove 'G' suffix)
            let total_str = parts[1].trim_end_matches('G');
            let used_str = parts[2].trim_end_matches('G');
            let percent_str = parts[4].trim_end_matches('%');

            let total: f64 = total_str.parse().unwrap_or(0.0);
            let used: f64 = used_str.parse().unwrap_or(0.0);
            let percent: f64 = percent_str.parse().unwrap_or(0.0);

            return Ok((total, used, percent));
        }
    }

    warn!("Failed to parse disk metrics from: {}", output);
    Ok((0.0, 0.0, 0.0))
}

/// Collect load average using `uptime` command
async fn collect_load_metrics(
    server: &Server,
    executor: &RemoteExecutor,
) -> Result<(Option<f64>, Option<f64>, Option<f64>)> {
    // Run 'uptime' to get load averages
    let output = executor
        .execute(server, "uptime", &[])
        .await
        .context("Failed to get load metrics")?;

    // Parse output - looking for "load average: X.XX, X.XX, X.XX"
    if let Some(load_part) = output.split("load average:").nth(1) {
        let loads: Vec<&str> = load_part.split(',').collect();
        if loads.len() >= 3 {
            let load_1: Option<f64> = loads[0].trim().parse().ok();
            let load_5: Option<f64> = loads[1].trim().parse().ok();
            let load_15: Option<f64> = loads[2].trim().parse().ok();
            return Ok((load_1, load_5, load_15));
        }
    }

    warn!("Failed to parse load metrics from: {}", output);
    Ok((None, None, None))
}

/// Send health notification
async fn send_health_notification(
    notify: &NotificationManager,
    server_name: &str,
    metrics: &SystemMetrics,
    issues: &[String],
) -> Result<()> {
    let title = if issues.is_empty() {
        format!("System Health: {} ✓", server_name)
    } else {
        format!(
            "System Health Alert: {} ({} issues)",
            server_name,
            issues.len()
        )
    };

    let mut body = String::new();
    body.push_str("## System Metrics\n\n");
    body.push_str(&format!("**Server**: {}\n\n", server_name));

    // Memory
    body.push_str(&format!(
        "💾 **Memory**: {:.1}% used ({} MB / {} MB)\n",
        metrics.mem_percent, metrics.mem_used_mb, metrics.mem_total_mb
    ));

    // Disk
    body.push_str(&format!(
        "💿 **Disk**: {:.1}% used ({:.1} GB / {:.1} GB)\n",
        metrics.disk_percent, metrics.disk_used_gb, metrics.disk_total_gb
    ));

    // Load
    if let (Some(l1), Some(l5), Some(l15)) =
        (metrics.load_1min, metrics.load_5min, metrics.load_15min)
    {
        body.push_str(&format!(
            "⚙️  **Load**: {:.2}, {:.2}, {:.2} (1m, 5m, 15m)\n",
            l1, l5, l15
        ));
    }

    // Issues
    if !issues.is_empty() {
        body.push_str("\n**Issues Detected**:\n");
        for issue in issues {
            body.push_str(&format!("  - ⚠️ {}\n", issue));
        }
    }

    let priority = if issues.is_empty() { 3 } else { 4 };

    let message = NotificationMessage {
        title,
        body,
        priority,
        actions: vec![],
    };

    notify
        .send_for_service("health", &message)
        .await
        .context("Failed to send health notification")?;

    Ok(())
}
