//! Speed test monitoring plugin
//!
//! Optional add-on plugin that runs Ookla speedtest and sends notifications
//! with download/upload speeds and latency.
//!
//! ## Configuration
//!
//! Optional environment variables:
//! - `SPEEDTEST_MIN_DOWN` - Minimum acceptable download speed in Mbps
//! - `SPEEDTEST_MIN_UP` - Minimum acceptable upload speed in Mbps
//! - `SPEEDTEST_SERVER_ID` - Specific server ID to test against
//! - `SPEEDTEST_SCHEDULE` - Cron schedule (default: "0 */4 * * *" - every 4 hours)

use async_trait::async_trait;
use serde::Deserialize;
use svrctlrs_core::{
    Error, NotificationMessage, Plugin, PluginContext, PluginMetadata,
    PluginResult, Result, ScheduledTask,
};
use tokio::process::Command;
use tracing::{info, warn};

/// Speed test monitoring plugin
pub struct SpeedTestPlugin {
    min_down: Option<f64>,
    min_up: Option<f64>,
    server_id: Option<u32>,
    schedule: String,
}

#[derive(Debug, Deserialize)]
struct OoklaResult {
    ping: Ping,
    download: Transfer,
    upload: Transfer,
    isp: Option<String>,
    server: Option<Server>,
}

#[derive(Debug, Deserialize)]
struct Ping {
    latency: f64,
}

#[derive(Debug, Deserialize)]
struct Transfer {
    bandwidth: f64, // bits per second
}

#[derive(Debug, Deserialize)]
struct Server {
    id: Option<u32>,
    name: Option<String>,
    location: Option<String>,
}

impl SpeedTestPlugin {
    /// Create a new speed test plugin
    pub fn new() -> Self {
        let min_down = std::env::var("SPEEDTEST_MIN_DOWN")
            .ok()
            .and_then(|v| v.parse().ok());
        let min_up = std::env::var("SPEEDTEST_MIN_UP")
            .ok()
            .and_then(|v| v.parse().ok());
        let server_id = std::env::var("SPEEDTEST_SERVER_ID")
            .ok()
            .and_then(|v| v.parse().ok());
        let schedule = std::env::var("SPEEDTEST_SCHEDULE")
            .unwrap_or_else(|_| "0 */4 * * *".to_string()); // Every 4 hours

        Self {
            min_down,
            min_up,
            server_id,
            schedule,
        }
    }

    /// Run speedtest and send notification
    async fn run_speedtest(&self, notify_mgr: &svrctlrs_core::NotificationManager) -> Result<String> {
        info!("Running Ookla speedtest");

        // Build command
        let mut cmd = Command::new("speedtest");
        cmd.arg("--format=json");
        cmd.arg("--accept-license");
        cmd.arg("--accept-gdpr");

        if let Some(server) = self.server_id {
            cmd.arg(format!("--server-id={}", server));
        }

        // Execute speedtest
        let output = cmd.output()
            .await
            .map_err(|e| Error::PluginError(format!("Failed to run speedtest: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::PluginError(format!("Speedtest failed: {}", stderr)));
        }

        // Parse JSON result
        let result: OoklaResult = serde_json::from_slice(&output.stdout)
            .map_err(|e| Error::PluginError(format!("Failed to parse speedtest output: {}", e)))?;

        // Convert bandwidth to Mbps
        let down_mbps = result.download.bandwidth / 1_000_000.0;
        let up_mbps = result.upload.bandwidth / 1_000_000.0;
        let ping_ms = result.ping.latency;

        // Check thresholds
        let mut warnings = Vec::new();
        if let Some(min) = self.min_down {
            if down_mbps < min {
                warnings.push(format!("Download speed {:.1} Mbps is below threshold {:.1} Mbps", down_mbps, min));
            }
        }
        if let Some(min) = self.min_up {
            if up_mbps < min {
                warnings.push(format!("Upload speed {:.1} Mbps is below threshold {:.1} Mbps", up_mbps, min));
            }
        }

        // Build message
        let isp = result.isp.as_deref().unwrap_or("Unknown ISP");
        let server_name = result.server.as_ref()
            .and_then(|s| s.name.as_deref())
            .unwrap_or("Unknown");
        let server_location = result.server.as_ref()
            .and_then(|s| s.location.as_deref())
            .unwrap_or("");

        let mut message_lines = vec![
            format!("ISP: {}", isp),
            format!("Server: {} {}", server_name, server_location),
            format!("Download: {:.1} Mbps", down_mbps),
            format!("Upload: {:.1} Mbps", up_mbps),
            format!("Ping: {:.1} ms", ping_ms),
        ];

        if !warnings.is_empty() {
            message_lines.push(String::new());
            message_lines.push("⚠️ Warnings:".to_string());
            message_lines.extend(warnings.iter().cloned());
        }

        let detailed_message = message_lines.join("\n");
        let summary = format!(
            "↓{:.1} Mbps ↑{:.1} Mbps • Ping: {:.1}ms",
            down_mbps, up_mbps, ping_ms
        );

        // Determine priority based on warnings
        let priority = if warnings.is_empty() {
            3 // Normal
        } else {
            5 // High
        };

        // Send notification
        let notification = NotificationMessage {
            title: "Speed Test Results".to_string(),
            body: detailed_message.clone(),
            priority,
            actions: vec![],
        };

        notify_mgr.send_for_service("speedtest", &notification).await?;

        Ok(summary)
    }
}

impl Default for SpeedTestPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Plugin for SpeedTestPlugin {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            id: "speedtest".to_string(),
            name: "Speed Test Monitoring".to_string(),
            description: "Monitors internet speed using Ookla speedtest CLI".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            author: "SvrCtlRS".to_string(),
        }
    }

    fn scheduled_tasks(&self) -> Vec<ScheduledTask> {
        vec![ScheduledTask {
            id: "speedtest_run".to_string(),
            description: "Run speed test and send notifications".to_string(),
            schedule: self.schedule.clone(),
            enabled: true,
        }]
    }

    async fn init(&mut self) -> Result<()> {
        info!("Initializing speed test plugin");

        // Check if speedtest command is available
        match Command::new("speedtest").arg("--version").output().await {
            Ok(output) if output.status.success() => {
                let version = String::from_utf8_lossy(&output.stdout);
                info!("Found Ookla speedtest CLI: {}", version.trim());
            }
            _ => {
                warn!("Ookla speedtest CLI not found - speed test plugin will fail on execution");
                warn!("Install from: https://www.speedtest.net/apps/cli");
            }
        }

        Ok(())
    }

    async fn execute(&self, task_id: &str, context: &PluginContext) -> Result<PluginResult> {
        match task_id {
            "speedtest_run" => {
                info!("Executing speed test");
                match self.run_speedtest(&context.notification_manager).await {
                    Ok(summary) => Ok(PluginResult {
                        success: true,
                        message: format!("Speed test completed: {}", summary),
                        data: None,
                        metrics: None,
                    }),
                    Err(e) => Ok(PluginResult {
                        success: false,
                        message: format!("Speed test failed: {}", e),
                        data: None,
                        metrics: None,
                    }),
                }
            }
            _ => Err(Error::PluginError(format!("Unknown task: {}", task_id))),
        }
    }

    async fn shutdown(&mut self) -> Result<()> {
        info!("Shutting down speed test plugin");
        Ok(())
    }
}
