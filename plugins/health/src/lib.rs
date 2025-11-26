//! System health monitoring plugin

use async_trait::async_trait;
use serde_json::json;
use std::collections::HashMap;
use sysinfo::{System, Disks};
use svrctlrs_core::{
    Error, NotificationMessage, Plugin, PluginContext, PluginMetadata, PluginResult, Result,
    ScheduledTask,
};
use tracing::{info, instrument};

/// System health monitoring plugin (CPU, memory, disk)
pub struct HealthPlugin {
    config: HealthConfig,
}

#[derive(Debug, Clone)]
struct HealthConfig {
    send_summary: bool,
    cpu_warn_pct: f64,
    mem_warn_pct: f64,
    disk_warn_pct: f64,
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

impl HealthPlugin {
    pub fn new() -> Self {
        Self {
            config: HealthConfig::default(),
        }
    }

    pub fn from_config(config: serde_json::Value) -> Result<Self> {
        let send_summary = config
            .get("send_summary")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let cpu_warn_pct = config
            .get("cpu_warn_pct")
            .and_then(|v| v.as_f64())
            .unwrap_or(80.0);

        let mem_warn_pct = config
            .get("mem_warn_pct")
            .and_then(|v| v.as_f64())
            .unwrap_or(80.0);

        let disk_warn_pct = config
            .get("disk_warn_pct")
            .and_then(|v| v.as_f64())
            .unwrap_or(85.0);

        Ok(Self {
            config: HealthConfig {
                send_summary,
                cpu_warn_pct,
                mem_warn_pct,
                disk_warn_pct,
            },
        })
    }
}

impl Default for HealthPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Plugin for HealthPlugin {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            id: "health".to_string(),
            name: "System Health Monitor".to_string(),
            description: "Monitor system resources (CPU, memory, disk)".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            author: "SvrCtlRS".to_string(),
        }
    }

    fn scheduled_tasks(&self) -> Vec<ScheduledTask> {
        vec![ScheduledTask {
            id: "system_metrics".to_string(),
            schedule: "0 */5 * * * *".to_string(), // Every 5 minutes
            description: "Collect system metrics".to_string(),
            enabled: true,
        }]
    }

    async fn execute(&self, task_id: &str, context: &PluginContext) -> Result<PluginResult> {
        info!(task_id = %task_id, "Executing Health plugin task");

        match task_id {
            "system_metrics" => self.collect_metrics(context).await,
            _ => Ok(PluginResult {
                success: false,
                message: format!("Unknown task: {}", task_id),
                data: None,
                metrics: None,
            }),
        }
    }
}

impl HealthPlugin {
    #[instrument(skip(self, context))]
    async fn collect_metrics(&self, context: &PluginContext) -> Result<PluginResult> {
        info!("Collecting system health metrics");

        // Initialize system info
        let mut sys = System::new_all();
        sys.refresh_all();

        // Get CPU usage (average across all cores)
        let cpu_usage = sys.global_cpu_info().cpu_usage();

        // Get memory usage
        let total_memory = sys.total_memory();
        let used_memory = sys.used_memory();
        let mem_usage_pct = if total_memory > 0 {
            (used_memory as f64 / total_memory as f64) * 100.0
        } else {
            0.0
        };

        // Get disk usage
        let disks = Disks::new_with_refreshed_list();
        let mut disk_issues = Vec::new();
        let mut total_disk_space = 0u64;
        let mut used_disk_space = 0u64;

        for disk in &disks {
            let total = disk.total_space();
            let available = disk.available_space();
            let used = total - available;
            let usage_pct = if total > 0 {
                (used as f64 / total as f64) * 100.0
            } else {
                0.0
            };

            total_disk_space += total;
            used_disk_space += used;

            if usage_pct > self.config.disk_warn_pct {
                disk_issues.push(format!(
                    "{}: {:.1}% used ({:.1} GB / {:.1} GB)",
                    disk.mount_point().display(),
                    usage_pct,
                    used as f64 / 1024.0 / 1024.0 / 1024.0,
                    total as f64 / 1024.0 / 1024.0 / 1024.0
                ));
            }
        }

        let disk_usage_pct = if total_disk_space > 0 {
            (used_disk_space as f64 / total_disk_space as f64) * 100.0
        } else {
            0.0
        };

        // Check for issues
        let mut issues = Vec::new();
        if cpu_usage > self.config.cpu_warn_pct as f32 {
            issues.push(format!("High CPU usage: {:.1}%", cpu_usage));
        }
        if mem_usage_pct > self.config.mem_warn_pct {
            issues.push(format!("High memory usage: {:.1}%", mem_usage_pct));
        }
        if !disk_issues.is_empty() {
            issues.extend(disk_issues);
        }

        // Send notification
        if !issues.is_empty() {
            self.send_health_alert(&context.notification_manager, &issues, cpu_usage, mem_usage_pct, disk_usage_pct)
                .await?;
        } else if self.config.send_summary {
            self.send_health_summary(&context.notification_manager, cpu_usage, mem_usage_pct, disk_usage_pct)
                .await?;
        }

        // Prepare result
        let message = if !issues.is_empty() {
            format!("System health check: {} issue(s) detected", issues.len())
        } else {
            "System health check: All systems normal".to_string()
        };

        let data = json!({
            "cpu_usage_pct": cpu_usage,
            "memory_usage_pct": mem_usage_pct,
            "memory_used_mb": used_memory / 1024 / 1024,
            "memory_total_mb": total_memory / 1024 / 1024,
            "disk_usage_pct": disk_usage_pct,
            "disk_used_gb": used_disk_space as f64 / 1024.0 / 1024.0 / 1024.0,
            "disk_total_gb": total_disk_space as f64 / 1024.0 / 1024.0 / 1024.0,
            "issues": issues,
        });

        let mut metrics = HashMap::new();
        metrics.insert("cpu_usage_pct".to_string(), cpu_usage as f64);
        metrics.insert("memory_usage_pct".to_string(), mem_usage_pct);
        metrics.insert("disk_usage_pct".to_string(), disk_usage_pct);

        Ok(PluginResult {
            success: issues.is_empty(),
            message,
            data: Some(data),
            metrics: Some(metrics),
        })
    }

    async fn send_health_alert(
        &self,
        notify_mgr: &svrctlrs_core::NotificationManager,
        issues: &[String],
        cpu_usage: f32,
        mem_usage_pct: f64,
        disk_usage_pct: f64,
    ) -> Result<()> {
        let title = format!("System Health Alert: {} issue(s)", issues.len());

        let mut body = String::new();
        body.push_str("⚠️ **System Health Issues Detected**\n\n");

        for issue in issues {
            body.push_str(&format!("• {}\n", issue));
        }

        body.push_str(&format!(
            "\n**Current Status**:\n\
            CPU: {:.1}%\n\
            Memory: {:.1}%\n\
            Disk: {:.1}%\n",
            cpu_usage, mem_usage_pct, disk_usage_pct
        ));

        let message = NotificationMessage {
            title,
            body,
            priority: 4, // High priority
            actions: vec![],
        };

        notify_mgr
            .send_for_service("health", &message)
            .await
            .map_err(|e| Error::PluginError(format!("Failed to send notification: {}", e)))?;

        info!("Health alert sent");
        Ok(())
    }

    async fn send_health_summary(
        &self,
        notify_mgr: &svrctlrs_core::NotificationManager,
        cpu_usage: f32,
        mem_usage_pct: f64,
        disk_usage_pct: f64,
    ) -> Result<()> {
        let title = "System Health Summary".to_string();

        let body = format!(
            "📊 **System Status** ✓\n\n\
            CPU Usage: {:.1}%\n\
            Memory Usage: {:.1}%\n\
            Disk Usage: {:.1}%\n\n\
            All systems within normal parameters.",
            cpu_usage, mem_usage_pct, disk_usage_pct
        );

        let message = NotificationMessage {
            title,
            body,
            priority: 3, // Normal priority
            actions: vec![],
        };

        notify_mgr
            .send_for_service("health", &message)
            .await
            .map_err(|e| Error::PluginError(format!("Failed to send notification: {}", e)))?;

        info!("Health summary sent");
        Ok(())
    }
}
