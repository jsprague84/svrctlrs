//! Docker monitoring plugin

mod health;

use async_trait::async_trait;
use health::HealthMonitor;
use serde_json::json;
use std::collections::HashMap;
use svrctlrs_core::{Plugin, PluginContext, PluginMetadata, PluginResult, Result, ScheduledTask};
use tracing::{info, instrument};

/// Docker monitoring and management plugin
pub struct DockerPlugin {}

impl DockerPlugin {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for DockerPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Plugin for DockerPlugin {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            id: "docker".to_string(),
            name: "Docker Monitor".to_string(),
            description: "Monitor Docker containers, images, and resources".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            author: "SvrCtlRS".to_string(),
        }
    }

    fn scheduled_tasks(&self) -> Vec<ScheduledTask> {
        vec![
            ScheduledTask {
                id: "docker_health".to_string(),
                schedule: "0 */5 * * * *".to_string(), // Every 5 minutes
                description: "Check Docker container health".to_string(),
                enabled: true,
            },
            ScheduledTask {
                id: "docker_cleanup".to_string(),
                schedule: "0 0 2 * * 0".to_string(), // Sundays at 2 AM
                description: "Analyze Docker cleanup opportunities".to_string(),
                enabled: true,
            },
        ]
    }

    async fn execute(&self, task_id: &str, context: &PluginContext) -> Result<PluginResult> {
        info!(task_id = %task_id, "Executing Docker plugin task");

        match task_id {
            "docker_health" => self.check_health(context).await,
            "docker_cleanup" => self.analyze_cleanup(context).await,
            _ => Ok(PluginResult {
                success: false,
                message: format!("Unknown task: {}", task_id),
                data: None,
                metrics: None,
            }),
        }
    }
}

impl DockerPlugin {
    #[instrument(skip(self, context))]
    async fn check_health(&self, context: &PluginContext) -> Result<PluginResult> {
        info!("Running Docker health check");

        // Create health monitor
        let monitor = HealthMonitor::new().await?;

        // Check health of all containers
        let health_statuses = monitor.check_health(&context.notification_manager).await?;

        // Count containers by status
        let total = health_statuses.len();
        let running = health_statuses.iter().filter(|c| c.running).count();
        let with_issues = health_statuses.iter().filter(|c| !c.issues.is_empty()).count();

        let message = format!(
            "Docker health check complete: {} containers ({} running, {} with issues)",
            total, running, with_issues
        );

        // Prepare structured data
        let data = json!({
            "total_containers": total,
            "running_containers": running,
            "containers_with_issues": with_issues,
            "health_statuses": health_statuses,
        });

        // Prepare metrics
        let mut metrics = HashMap::new();
        metrics.insert("total_containers".to_string(), total as f64);
        metrics.insert("running_containers".to_string(), running as f64);
        metrics.insert("containers_with_issues".to_string(), with_issues as f64);

        Ok(PluginResult {
            success: with_issues == 0,
            message,
            data: Some(data),
            metrics: Some(metrics),
        })
    }

    async fn analyze_cleanup(&self, _context: &PluginContext) -> Result<PluginResult> {
        // TODO: Implement actual Docker cleanup analysis
        Ok(PluginResult {
            success: true,
            message: "Docker cleanup analysis placeholder".to_string(),
            data: None,
            metrics: None,
        })
    }
}
