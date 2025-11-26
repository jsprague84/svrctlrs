//! Docker monitoring plugin

mod analysis;
mod cleanup;
mod health;

use analysis::AnalysisManager;
use async_trait::async_trait;
use cleanup::CleanupManager;
use health::HealthMonitor;
use serde_json::json;
use std::collections::HashMap;
use svrctlrs_core::{
    Error, Plugin, PluginContext, PluginMetadata, PluginResult, Result, ScheduledTask,
};
use tracing::{info, instrument};

/// Docker monitoring and management plugin
pub struct DockerPlugin {
    config: DockerConfig,
}

#[derive(Debug, Clone)]
struct DockerConfig {
    send_summary: bool,
    cpu_warn_pct: f64,
    mem_warn_pct: f64,
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

impl DockerPlugin {
    pub fn new() -> Self {
        Self {
            config: DockerConfig::default(),
        }
    }

    pub fn from_config(config: serde_json::Value) -> svrctlrs_core::Result<Self> {
        let send_summary = config.get("send_summary")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        
        let cpu_warn_pct = config.get("cpu_warn_pct")
            .and_then(|v| v.as_f64())
            .unwrap_or(80.0);
        
        let mem_warn_pct = config.get("mem_warn_pct")
            .and_then(|v| v.as_f64())
            .unwrap_or(80.0);

        Ok(Self {
            config: DockerConfig {
                send_summary,
                cpu_warn_pct,
                mem_warn_pct,
            },
        })
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
            ScheduledTask {
                id: "docker_analysis".to_string(),
                schedule: "0 0 3 * * 0".to_string(), // Sundays at 3 AM
                description: "Advanced Docker analysis (unused images, logs, layers)".to_string(),
                enabled: true,
            },
        ]
    }

    async fn execute(&self, task_id: &str, context: &PluginContext) -> Result<PluginResult> {
        info!(task_id = %task_id, "Executing Docker plugin task");

        match task_id {
            "docker_health" => self.check_health(context).await,
            "docker_cleanup" => self.analyze_cleanup(context).await,
            "docker_analysis" => self.advanced_analysis(context).await,
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

        // Create health monitor with configuration
        let mut monitor = HealthMonitor::new().await?;
        monitor.set_thresholds(self.config.cpu_warn_pct, self.config.mem_warn_pct);

        // Check health of all containers
        let health_statuses = monitor.check_health(&context.notification_manager, self.config.send_summary).await?;

        // Count containers by status
        let total = health_statuses.len();
        let running = health_statuses.iter().filter(|c| c.running).count();
        let with_issues = health_statuses
            .iter()
            .filter(|c| !c.issues.is_empty())
            .count();

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

    #[instrument(skip(self, context))]
    async fn analyze_cleanup(&self, context: &PluginContext) -> Result<PluginResult> {
        info!("Running Docker cleanup analysis");

        // Create cleanup manager
        let manager = CleanupManager::new().await?;

        // Analyze cleanup opportunities
        let analysis = manager.analyze(&context.notification_manager).await?;

        let message = format!(
            "Docker cleanup analysis: {} reclaimable ({})",
            analysis.total_items(),
            analysis.total_space_formatted()
        );

        // Prepare structured data
        let data = json!({
            "images_reclaimable": analysis.images_reclaimable,
            "images_space_bytes": analysis.images_space_bytes,
            "containers_reclaimable": analysis.containers_reclaimable,
            "containers_space_bytes": analysis.containers_space_bytes,
            "volumes_reclaimable": analysis.volumes_reclaimable,
            "volumes_space_bytes": analysis.volumes_space_bytes,
            "networks_reclaimable": analysis.networks_reclaimable,
            "build_cache_space_bytes": analysis.build_cache_space_bytes,
            "total_space_bytes": analysis.total_space_bytes,
        });

        // Prepare metrics
        let mut metrics = HashMap::new();
        metrics.insert(
            "images_reclaimable".to_string(),
            analysis.images_reclaimable as f64,
        );
        metrics.insert(
            "images_space_mb".to_string(),
            analysis.images_space_bytes as f64 / 1024.0 / 1024.0,
        );
        metrics.insert(
            "containers_reclaimable".to_string(),
            analysis.containers_reclaimable as f64,
        );
        metrics.insert(
            "volumes_reclaimable".to_string(),
            analysis.volumes_reclaimable as f64,
        );
        metrics.insert(
            "networks_reclaimable".to_string(),
            analysis.networks_reclaimable as f64,
        );
        metrics.insert(
            "total_space_mb".to_string(),
            analysis.total_space_bytes as f64 / 1024.0 / 1024.0,
        );

        Ok(PluginResult {
            success: true,
            message,
            data: Some(data),
            metrics: Some(metrics),
        })
    }

    #[instrument(skip(self, context))]
    async fn advanced_analysis(&self, context: &PluginContext) -> Result<PluginResult> {
        info!("Running advanced Docker analysis");

        // Create analysis manager
        let manager = AnalysisManager::new().await?;

        // Perform all analyses
        let unused_images = manager.analyze_unused_images().await?;
        let container_logs = manager.analyze_container_logs().await?;
        let image_layers = manager.analyze_image_layers().await?;

        let message = format!(
            "Docker analysis: {} unused images ({:.1} MB), {} large logs ({:.1} MB), {:.1}% layer efficiency",
            unused_images.total_count,
            unused_images.total_size_bytes as f64 / 1024.0 / 1024.0,
            container_logs.containers_over_threshold,
            container_logs.total_size_bytes as f64 / 1024.0 / 1024.0,
            image_layers.efficiency_percent
        );

        // Prepare structured data
        let data = json!({
            "unused_images": {
                "count": unused_images.total_count,
                "total_size_bytes": unused_images.total_size_bytes,
                "images": unused_images.images,
            },
            "container_logs": {
                "total_size_bytes": container_logs.total_size_bytes,
                "containers_over_threshold": container_logs.containers_over_threshold,
                "containers": container_logs.containers,
            },
            "image_layers": {
                "total_shared_bytes": image_layers.total_shared_bytes,
                "total_unique_bytes": image_layers.total_unique_bytes,
                "efficiency_percent": image_layers.efficiency_percent,
                "shared_layers_count": image_layers.shared_layers.len(),
            },
        });

        // Prepare metrics
        let mut metrics = HashMap::new();
        metrics.insert(
            "unused_images_count".to_string(),
            unused_images.total_count as f64,
        );
        metrics.insert(
            "unused_images_mb".to_string(),
            unused_images.total_size_bytes as f64 / 1024.0 / 1024.0,
        );
        metrics.insert(
            "large_logs_count".to_string(),
            container_logs.containers_over_threshold as f64,
        );
        metrics.insert(
            "total_logs_mb".to_string(),
            container_logs.total_size_bytes as f64 / 1024.0 / 1024.0,
        );
        metrics.insert(
            "layer_efficiency_percent".to_string(),
            image_layers.efficiency_percent,
        );
        metrics.insert(
            "shared_layers_count".to_string(),
            image_layers.shared_layers.len() as f64,
        );

        // Send notification with summary
        self.send_analysis_notification(
            &context.notification_manager,
            &unused_images,
            &container_logs,
            &image_layers,
        )
        .await?;

        Ok(PluginResult {
            success: true,
            message,
            data: Some(data),
            metrics: Some(metrics),
        })
    }

    async fn send_analysis_notification(
        &self,
        notify_mgr: &svrctlrs_core::NotificationManager,
        unused_images: &analysis::UnusedImagesAnalysis,
        container_logs: &analysis::ContainerLogsAnalysis,
        image_layers: &analysis::LayersAnalysis,
    ) -> Result<()> {
        let title = "Docker Advanced Analysis Report".to_string();

        let mut body = String::new();
        body.push_str("## Docker Resource Analysis\n\n");

        // Unused images
        if unused_images.total_count > 0 {
            body.push_str(&format!(
                "📦 **Unused Images**: {} images ({:.1} MB)\n",
                unused_images.total_count,
                unused_images.total_size_bytes as f64 / 1024.0 / 1024.0
            ));
            body.push_str("  Images not used by any containers (older than threshold)\n\n");
        }

        // Large logs
        if container_logs.containers_over_threshold > 0 {
            body.push_str(&format!(
                "📝 **Large Logs**: {} containers ({:.1} MB total)\n",
                container_logs.containers_over_threshold,
                container_logs.total_size_bytes as f64 / 1024.0 / 1024.0
            ));
            for (i, log) in container_logs.containers.iter().take(5).enumerate() {
                body.push_str(&format!(
                    "  {}. {} - {:.1} MB {}\n",
                    i + 1,
                    log.container_name,
                    log.log_size_bytes as f64 / 1024.0 / 1024.0,
                    if log.has_rotation {
                        "✓"
                    } else {
                        "⚠️ no rotation"
                    }
                ));
            }
            body.push('\n');
        }

        // Layer efficiency
        body.push_str(&format!(
            "🔗 **Layer Sharing**: {:.1}% efficient\n",
            image_layers.efficiency_percent
        ));
        body.push_str(&format!(
            "  {} shared layers ({:.1} MB)\n",
            image_layers.shared_layers.len(),
            image_layers.total_shared_bytes as f64 / 1024.0 / 1024.0
        ));
        body.push_str(&format!(
            "  {:.1} MB unique layers\n",
            image_layers.total_unique_bytes as f64 / 1024.0 / 1024.0
        ));

        let message = svrctlrs_core::NotificationMessage {
            title,
            body,
            priority: 3,
            actions: vec![],
        };

        notify_mgr
            .send_for_service("docker", &message)
            .await
            .map_err(|e| Error::PluginError(format!("Failed to send notification: {}", e)))?;

        Ok(())
    }
}
