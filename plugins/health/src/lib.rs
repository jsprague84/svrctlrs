//! System health monitoring plugin

use async_trait::async_trait;
use svrctlrs_core::{Plugin, PluginContext, PluginMetadata, PluginResult, Result, ScheduledTask};
use tracing::info;

/// System health monitoring plugin (CPU, memory, disk, network)
pub struct HealthPlugin {}

impl HealthPlugin {
    pub fn new() -> Self {
        Self {}
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
            description: "Monitor system resources (CPU, memory, disk, network)".to_string(),
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
    async fn collect_metrics(&self, _context: &PluginContext) -> Result<PluginResult> {
        // TODO: Implement actual metrics collection
        Ok(PluginResult {
            success: true,
            message: "Metrics collection placeholder".to_string(),
            data: None,
            metrics: None,
        })
    }
}
