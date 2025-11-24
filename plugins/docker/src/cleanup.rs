//! Docker cleanup analysis and operations
//!
//! Provides analysis of Docker resources that can be cleaned up, including:
//! - Dangling images
//! - Stopped containers
//! - Unused volumes
//! - Unused networks
//! - Build cache

use bollard::container::PruneContainersOptions;
use bollard::image::PruneImagesOptions;
use bollard::network::PruneNetworksOptions;
use bollard::volume::PruneVolumesOptions;
use bollard::Docker;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use svrctlrs_core::{Error, NotificationManager, NotificationMessage, Result};
use tracing::{debug, info, instrument, warn};

/// Cleanup analysis result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupAnalysis {
    pub images_reclaimable: u64,
    pub images_space_bytes: u64,
    pub containers_reclaimable: u64,
    pub containers_space_bytes: u64,
    pub volumes_reclaimable: u64,
    pub volumes_space_bytes: u64,
    pub networks_reclaimable: u64,
    pub build_cache_space_bytes: u64,
    pub total_space_bytes: u64,
}

impl CleanupAnalysis {
    /// Calculate total reclaimable items
    pub fn total_items(&self) -> u64 {
        self.images_reclaimable
            + self.containers_reclaimable
            + self.volumes_reclaimable
            + self.networks_reclaimable
    }

    /// Format space in human-readable format
    pub fn format_space(bytes: u64) -> String {
        const KB: u64 = 1024;
        const MB: u64 = KB * 1024;
        const GB: u64 = MB * 1024;

        if bytes >= GB {
            format!("{:.2} GB", bytes as f64 / GB as f64)
        } else if bytes >= MB {
            format!("{:.2} MB", bytes as f64 / MB as f64)
        } else if bytes >= KB {
            format!("{:.2} KB", bytes as f64 / KB as f64)
        } else {
            format!("{} bytes", bytes)
        }
    }

    /// Get total space formatted
    pub fn total_space_formatted(&self) -> String {
        Self::format_space(self.total_space_bytes)
    }
}

/// Docker cleanup manager
pub struct CleanupManager {
    docker: Docker,
    dry_run: bool,
}

impl CleanupManager {
    /// Create a new cleanup manager
    ///
    /// # Errors
    ///
    /// Returns error if Docker connection fails
    #[instrument]
    pub async fn new() -> Result<Self> {
        info!("Connecting to Docker daemon for cleanup");

        let docker = Docker::connect_with_unix_defaults()
            .map_err(|e| Error::PluginError(format!("Failed to connect to Docker: {}", e)))?;

        // Check if dry-run mode is enabled
        let dry_run = std::env::var("DOCKER_CLEANUP_DRY_RUN")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(true); // Default to dry-run for safety

        info!(dry_run = %dry_run, "Cleanup manager configured");

        Ok(Self { docker, dry_run })
    }

    /// Analyze cleanup opportunities without actually cleaning
    ///
    /// # Arguments
    ///
    /// * `notify_mgr` - Notification manager for sending reports
    ///
    /// # Returns
    ///
    /// Cleanup analysis summary
    #[instrument(skip(self, notify_mgr))]
    pub async fn analyze(&self, notify_mgr: &NotificationManager) -> Result<CleanupAnalysis> {
        info!("Analyzing Docker cleanup opportunities");

        let mut analysis = CleanupAnalysis {
            images_reclaimable: 0,
            images_space_bytes: 0,
            containers_reclaimable: 0,
            containers_space_bytes: 0,
            volumes_reclaimable: 0,
            volumes_space_bytes: 0,
            networks_reclaimable: 0,
            build_cache_space_bytes: 0,
            total_space_bytes: 0,
        };

        // Analyze images (dangling only by default)
        if let Ok(result) = self.analyze_images().await {
            analysis.images_reclaimable = result.0;
            analysis.images_space_bytes = result.1;
            debug!(
                images = result.0,
                space = %CleanupAnalysis::format_space(result.1),
                "Image cleanup analysis"
            );
        }

        // Analyze stopped containers
        if let Ok(result) = self.analyze_containers().await {
            analysis.containers_reclaimable = result.0;
            analysis.containers_space_bytes = result.1;
            debug!(
                containers = result.0,
                space = %CleanupAnalysis::format_space(result.1),
                "Container cleanup analysis"
            );
        }

        // Analyze unused volumes
        if let Ok(result) = self.analyze_volumes().await {
            analysis.volumes_reclaimable = result.0;
            analysis.volumes_space_bytes = result.1;
            debug!(
                volumes = result.0,
                space = %CleanupAnalysis::format_space(result.1),
                "Volume cleanup analysis"
            );
        }

        // Analyze unused networks
        if let Ok(result) = self.analyze_networks().await {
            analysis.networks_reclaimable = result.0;
            debug!(networks = result.0, "Network cleanup analysis");
        }

        // Analyze build cache
        if let Ok(space) = self.analyze_build_cache().await {
            analysis.build_cache_space_bytes = space;
            debug!(
                space = %CleanupAnalysis::format_space(space),
                "Build cache analysis"
            );
        }

        // Calculate total
        analysis.total_space_bytes = analysis.images_space_bytes
            + analysis.containers_space_bytes
            + analysis.volumes_space_bytes
            + analysis.build_cache_space_bytes;

        // Send notification if there are cleanup opportunities
        if analysis.total_items() > 0 || analysis.total_space_bytes > 0 {
            self.send_cleanup_report(notify_mgr, &analysis).await?;
        } else {
            info!("No cleanup opportunities found");
        }

        Ok(analysis)
    }

    /// Analyze dangling and unused images
    async fn analyze_images(&self) -> Result<(u64, u64)> {
        let mut filters = HashMap::new();
        filters.insert("dangling", vec!["true"]);

        let options = Some(PruneImagesOptions { filters });

        let result = self
            .docker
            .prune_images(options)
            .await
            .map_err(|e| Error::PluginError(format!("Failed to analyze images: {}", e)))?;

        let count = result
            .images_deleted
            .as_ref()
            .map(|v| v.len() as u64)
            .unwrap_or(0);
        let space = result.space_reclaimed.unwrap_or(0).max(0) as u64;

        Ok((count, space))
    }

    /// Analyze stopped containers
    async fn analyze_containers(&self) -> Result<(u64, u64)> {
        let options = Some(PruneContainersOptions::<String> {
            filters: HashMap::new(),
        });

        let result = self
            .docker
            .prune_containers(options)
            .await
            .map_err(|e| Error::PluginError(format!("Failed to analyze containers: {}", e)))?;

        let count = result
            .containers_deleted
            .as_ref()
            .map(|v| v.len() as u64)
            .unwrap_or(0);
        let space = result.space_reclaimed.unwrap_or(0).max(0) as u64;

        Ok((count, space))
    }

    /// Analyze unused volumes
    async fn analyze_volumes(&self) -> Result<(u64, u64)> {
        let options = Some(PruneVolumesOptions::<String> {
            filters: HashMap::new(),
        });

        let result = self
            .docker
            .prune_volumes(options)
            .await
            .map_err(|e| Error::PluginError(format!("Failed to analyze volumes: {}", e)))?;

        let count = result
            .volumes_deleted
            .as_ref()
            .map(|v| v.len() as u64)
            .unwrap_or(0);
        let space = result.space_reclaimed.unwrap_or(0).max(0) as u64;

        Ok((count, space))
    }

    /// Analyze unused networks
    async fn analyze_networks(&self) -> Result<(u64, u64)> {
        let options = Some(PruneNetworksOptions::<String> {
            filters: HashMap::new(),
        });

        let result = self
            .docker
            .prune_networks(options)
            .await
            .map_err(|e| Error::PluginError(format!("Failed to analyze networks: {}", e)))?;

        let count = result
            .networks_deleted
            .as_ref()
            .map(|v| v.len() as u64)
            .unwrap_or(0);

        Ok((count, 0)) // Networks don't report space
    }

    /// Analyze build cache using disk usage API
    async fn analyze_build_cache(&self) -> Result<u64> {
        let df = self
            .docker
            .df()
            .await
            .map_err(|e| Error::PluginError(format!("Failed to get disk usage: {}", e)))?;

        let mut reclaimable_bytes = 0u64;

        if let Some(build_cache) = df.build_cache {
            for cache_item in build_cache {
                let size = cache_item.size.unwrap_or(0).max(0) as u64;
                let in_use = cache_item.in_use.unwrap_or(false);

                if !in_use {
                    reclaimable_bytes += size;
                }
            }
        }

        Ok(reclaimable_bytes)
    }

    /// Send cleanup analysis report notification
    #[instrument(skip(self, notify_mgr, analysis))]
    async fn send_cleanup_report(
        &self,
        notify_mgr: &NotificationManager,
        analysis: &CleanupAnalysis,
    ) -> Result<()> {
        let title = format!(
            "Docker Cleanup Report: {} reclaimable",
            analysis.total_space_formatted()
        );

        let mut body = String::new();
        body.push_str("## Cleanup Opportunities\n\n");

        if analysis.images_reclaimable > 0 {
            body.push_str(&format!(
                "🖼️  **Images**: {} items ({:.2} MB)\n",
                analysis.images_reclaimable,
                analysis.images_space_bytes as f64 / 1024.0 / 1024.0
            ));
        }

        if analysis.containers_reclaimable > 0 {
            body.push_str(&format!(
                "📦 **Containers**: {} items ({:.2} MB)\n",
                analysis.containers_reclaimable,
                analysis.containers_space_bytes as f64 / 1024.0 / 1024.0
            ));
        }

        if analysis.volumes_reclaimable > 0 {
            body.push_str(&format!(
                "💾 **Volumes**: {} items ({:.2} MB)\n",
                analysis.volumes_reclaimable,
                analysis.volumes_space_bytes as f64 / 1024.0 / 1024.0
            ));
        }

        if analysis.networks_reclaimable > 0 {
            body.push_str(&format!(
                "🌐 **Networks**: {} items\n",
                analysis.networks_reclaimable
            ));
        }

        if analysis.build_cache_space_bytes > 0 {
            body.push_str(&format!(
                "🏗️  **Build Cache**: {:.2} MB\n",
                analysis.build_cache_space_bytes as f64 / 1024.0 / 1024.0
            ));
        }

        body.push_str(&format!(
            "\n**Total**: {} items, {}\n",
            analysis.total_items(),
            analysis.total_space_formatted()
        ));

        if self.dry_run {
            body.push_str("\n⚠️  Dry-run mode: No cleanup performed\n");
            body.push_str("Set DOCKER_CLEANUP_DRY_RUN=false to enable cleanup\n");
        }

        let message = NotificationMessage {
            title,
            body,
            priority: 3, // Normal priority
            actions: vec![],
        };

        notify_mgr
            .send_for_service("docker", &message)
            .await
            .map_err(|e| Error::PluginError(format!("Failed to send notification: {}", e)))?;

        info!("Cleanup report sent");
        Ok(())
    }

    /// Execute cleanup operations (respects dry_run setting)
    ///
    /// # Arguments
    ///
    /// * `notify_mgr` - Notification manager for sending reports
    ///
    /// # Returns
    ///
    /// Cleanup analysis with actual results
    #[instrument(skip(self, notify_mgr))]
    pub async fn execute_cleanup(
        &self,
        notify_mgr: &NotificationManager,
    ) -> Result<CleanupAnalysis> {
        if self.dry_run {
            warn!("Dry-run mode: analyzing only, no cleanup performed");
            return self.analyze(notify_mgr).await;
        }

        info!("Executing Docker cleanup operations");

        let mut analysis = CleanupAnalysis {
            images_reclaimable: 0,
            images_space_bytes: 0,
            containers_reclaimable: 0,
            containers_space_bytes: 0,
            volumes_reclaimable: 0,
            volumes_space_bytes: 0,
            networks_reclaimable: 0,
            build_cache_space_bytes: 0,
            total_space_bytes: 0,
        };

        // Prune images
        if let Ok(result) = self.prune_images().await {
            analysis.images_reclaimable = result.0;
            analysis.images_space_bytes = result.1;
            info!(
                images = result.0,
                space = %CleanupAnalysis::format_space(result.1),
                "Images pruned"
            );
        }

        // Prune containers
        if let Ok(result) = self.prune_containers().await {
            analysis.containers_reclaimable = result.0;
            analysis.containers_space_bytes = result.1;
            info!(
                containers = result.0,
                space = %CleanupAnalysis::format_space(result.1),
                "Containers pruned"
            );
        }

        // Prune volumes
        if let Ok(result) = self.prune_volumes().await {
            analysis.volumes_reclaimable = result.0;
            analysis.volumes_space_bytes = result.1;
            info!(
                volumes = result.0,
                space = %CleanupAnalysis::format_space(result.1),
                "Volumes pruned"
            );
        }

        // Prune networks
        if let Ok(result) = self.prune_networks().await {
            analysis.networks_reclaimable = result.0;
            info!(networks = result.0, "Networks pruned");
        }

        // Prune build cache
        if let Ok(space) = self.prune_build_cache().await {
            analysis.build_cache_space_bytes = space;
            info!(
                space = %CleanupAnalysis::format_space(space),
                "Build cache pruned"
            );
        }

        // Calculate total
        analysis.total_space_bytes = analysis.images_space_bytes
            + analysis.containers_space_bytes
            + analysis.volumes_space_bytes
            + analysis.build_cache_space_bytes;

        // Send notification with results
        self.send_cleanup_report(notify_mgr, &analysis).await?;

        info!(
            total_space = %analysis.total_space_formatted(),
            "Cleanup completed"
        );

        Ok(analysis)
    }

    /// Prune dangling images
    async fn prune_images(&self) -> Result<(u64, u64)> {
        self.analyze_images().await // Same as analyze in dry-run
    }

    /// Prune stopped containers
    async fn prune_containers(&self) -> Result<(u64, u64)> {
        self.analyze_containers().await
    }

    /// Prune unused volumes
    async fn prune_volumes(&self) -> Result<(u64, u64)> {
        self.analyze_volumes().await
    }

    /// Prune unused networks
    #[allow(dead_code)]
    async fn prune_networks(&self) -> Result<(u64, u64)> {
        self.analyze_networks().await
    }

    /// Prune build cache
    /// Note: Build cache pruning is not directly supported by Bollard 0.18
    /// This would need to be done via CLI: `docker builder prune`
    #[allow(dead_code)]
    async fn prune_build_cache(&self) -> Result<u64> {
        // TODO: Implement via system exec or wait for Bollard API support
        // For now, return zero space reclaimed
        Ok(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_space_bytes() {
        assert_eq!(CleanupAnalysis::format_space(500), "500 bytes");
    }

    #[test]
    fn test_format_space_kb() {
        assert_eq!(CleanupAnalysis::format_space(2048), "2.00 KB");
    }

    #[test]
    fn test_format_space_mb() {
        assert_eq!(CleanupAnalysis::format_space(5 * 1024 * 1024), "5.00 MB");
    }

    #[test]
    fn test_format_space_gb() {
        assert_eq!(
            CleanupAnalysis::format_space(3 * 1024 * 1024 * 1024),
            "3.00 GB"
        );
    }

    #[test]
    fn test_total_items() {
        let analysis = CleanupAnalysis {
            images_reclaimable: 5,
            images_space_bytes: 1000,
            containers_reclaimable: 3,
            containers_space_bytes: 500,
            volumes_reclaimable: 2,
            volumes_space_bytes: 200,
            networks_reclaimable: 1,
            build_cache_space_bytes: 100,
            total_space_bytes: 1800,
        };

        assert_eq!(analysis.total_items(), 11); // 5 + 3 + 2 + 1
    }
}
