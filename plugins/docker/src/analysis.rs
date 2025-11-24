//! Docker analysis features for advanced monitoring
//!
//! Provides detailed analysis of Docker resources:
//! - Unused images (not referenced by any containers)
//! - Container logs (large log files)
//! - Image layers (sharing and efficiency)

use bollard::container::ListContainersOptions;
use bollard::image::ListImagesOptions;
use bollard::Docker;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use svrctlrs_core::{Error, Result};
use tracing::{debug, info, instrument};

/// Unused images analysis result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnusedImagesAnalysis {
    pub images: Vec<UnusedImageInfo>,
    pub total_count: usize,
    pub total_size_bytes: u64,
}

/// Information about an unused image
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnusedImageInfo {
    pub repository: String,
    pub tag: String,
    pub image_id: String,
    pub size_bytes: u64,
    pub created_timestamp: i64,
    pub age_days: i64,
}

/// Container logs analysis result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerLogsAnalysis {
    pub containers: Vec<ContainerLogInfo>,
    pub total_size_bytes: u64,
    pub containers_over_threshold: usize,
}

/// Information about container logs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerLogInfo {
    pub container_name: String,
    pub container_id: String,
    pub log_size_bytes: u64,
    pub has_rotation: bool,
}

/// Image layers analysis result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayersAnalysis {
    pub shared_layers: Vec<SharedLayerInfo>,
    pub total_shared_bytes: u64,
    pub total_unique_bytes: u64,
    pub efficiency_percent: f64,
}

/// Information about a shared layer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedLayerInfo {
    pub layer_id: String,
    pub size_bytes: u64,
    pub shared_by_count: usize,
    pub images_using: Vec<String>,
}

/// Docker analysis manager
pub struct AnalysisManager {
    docker: Docker,
}

impl AnalysisManager {
    /// Create a new analysis manager
    ///
    /// # Errors
    ///
    /// Returns error if Docker connection fails
    #[instrument]
    pub async fn new() -> Result<Self> {
        info!("Connecting to Docker daemon for analysis");

        let docker = Docker::connect_with_unix_defaults()
            .map_err(|e| Error::PluginError(format!("Failed to connect to Docker: {}", e)))?;

        Ok(Self { docker })
    }

    /// Analyze unused images
    ///
    /// Finds images that are not referenced by any containers (running or stopped).
    /// Only includes images older than the configured threshold.
    ///
    /// # Returns
    ///
    /// Analysis of unused images
    #[instrument(skip(self))]
    pub async fn analyze_unused_images(&self) -> Result<UnusedImagesAnalysis> {
        info!("Analyzing unused images");

        // Get all images
        let all_images = self
            .docker
            .list_images(None::<ListImagesOptions<String>>)
            .await
            .map_err(|e| Error::PluginError(format!("Failed to list images: {}", e)))?;

        // Get all containers (including stopped)
        let containers = self
            .docker
            .list_containers(Some(ListContainersOptions::<String> {
                all: true,
                ..Default::default()
            }))
            .await
            .map_err(|e| Error::PluginError(format!("Failed to list containers: {}", e)))?;

        // Build set of images in use
        let mut images_in_use = HashSet::new();
        for container in containers {
            if let Some(image) = container.image {
                images_in_use.insert(image);
            }
            if let Some(image_id) = container.image_id {
                images_in_use.insert(image_id);
            }
        }

        let image_age_threshold_days = std::env::var("DOCKER_IMAGE_AGE_DAYS")
            .ok()
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(90);

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let mut unused_images = Vec::new();
        let mut total_size_bytes = 0u64;

        for image in all_images {
            // Skip dangling images (handled by cleanup)
            let repo_tags = &image.repo_tags;
            if repo_tags.is_empty() || repo_tags.iter().any(|t| t.contains("<none>")) {
                continue;
            }

            // Check if image is in use
            let image_id = image.id.clone();
            let is_in_use = images_in_use.contains(&image_id)
                || repo_tags.iter().any(|tag| images_in_use.contains(tag));

            if is_in_use {
                continue;
            }

            // Check age threshold
            let age_days = (now - image.created) / 86400;
            if age_days < image_age_threshold_days {
                continue; // Too recent, skip
            }

            let size = image.size.max(0) as u64;
            total_size_bytes += size;

            let (repo, tag) = if let Some(first_tag) = repo_tags.first() {
                if let Some((r, t)) = first_tag.split_once(':') {
                    (r.to_string(), t.to_string())
                } else {
                    (first_tag.clone(), "latest".to_string())
                }
            } else {
                ("<none>".to_string(), "<none>".to_string())
            };

            unused_images.push(UnusedImageInfo {
                repository: repo,
                tag,
                image_id: image_id.clone(),
                size_bytes: size,
                created_timestamp: image.created,
                age_days,
            });
        }

        // Sort by size descending
        unused_images.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes));

        debug!(
            count = unused_images.len(),
            size_mb = total_size_bytes / 1024 / 1024,
            "Unused images analysis complete"
        );

        Ok(UnusedImagesAnalysis {
            total_count: unused_images.len(),
            total_size_bytes,
            images: unused_images,
        })
    }

    /// Analyze container logs
    ///
    /// Finds containers with large log files and checks if log rotation is configured.
    ///
    /// # Returns
    ///
    /// Analysis of container logs
    #[instrument(skip(self))]
    pub async fn analyze_container_logs(&self) -> Result<ContainerLogsAnalysis> {
        info!("Analyzing container logs");

        let containers = self
            .docker
            .list_containers(Some(ListContainersOptions::<String> {
                all: true,
                ..Default::default()
            }))
            .await
            .map_err(|e| Error::PluginError(format!("Failed to list containers: {}", e)))?;

        // Get threshold from env (default 100MB)
        let threshold_bytes = parse_size_threshold(
            &std::env::var("DOCKER_LOG_SIZE_THRESHOLD").unwrap_or_else(|_| "100M".to_string()),
        )
        .unwrap_or(100 * 1024 * 1024);

        let mut container_logs = Vec::new();
        let mut total_size_bytes = 0u64;
        let mut containers_over_threshold = 0usize;

        for container in containers {
            let id = container.id.clone().unwrap_or_default();
            let name = container
                .names
                .as_ref()
                .and_then(|v| v.first())
                .map(|s| s.trim_start_matches('/').to_string())
                .unwrap_or_else(|| id.chars().take(12).collect());

            // Get container details to find log path
            let inspect = match self.docker.inspect_container(&id, None).await {
                Ok(i) => i,
                Err(_) => continue,
            };

            // Check if log rotation is configured
            let has_rotation = inspect
                .host_config
                .as_ref()
                .and_then(|hc| hc.log_config.as_ref())
                .and_then(|lc| lc.config.as_ref())
                .map(|config| config.contains_key("max-size") || config.contains_key("max-file"))
                .unwrap_or(false);

            // Get log path
            let log_path = inspect.log_path.unwrap_or_default();
            if log_path.is_empty() {
                continue;
            }

            // Get log file size
            let log_size = match get_file_size(&log_path) {
                Ok(size) => size,
                Err(_) => continue,
            };

            total_size_bytes += log_size;

            // Only include in report if over threshold
            if log_size >= threshold_bytes {
                containers_over_threshold += 1;
                container_logs.push(ContainerLogInfo {
                    container_name: name,
                    container_id: id,
                    log_size_bytes: log_size,
                    has_rotation,
                });
            }
        }

        // Sort by size descending
        container_logs.sort_by(|a, b| b.log_size_bytes.cmp(&a.log_size_bytes));

        debug!(
            total_containers = container_logs.len(),
            over_threshold = containers_over_threshold,
            total_size_mb = total_size_bytes / 1024 / 1024,
            "Container logs analysis complete"
        );

        Ok(ContainerLogsAnalysis {
            containers: container_logs,
            total_size_bytes,
            containers_over_threshold,
        })
    }

    /// Analyze image layers
    ///
    /// Analyzes image layer sharing to show storage efficiency.
    ///
    /// # Returns
    ///
    /// Analysis of layer sharing
    #[instrument(skip(self))]
    pub async fn analyze_image_layers(&self) -> Result<LayersAnalysis> {
        info!("Analyzing image layers");

        let images = self
            .docker
            .list_images(None::<ListImagesOptions<String>>)
            .await
            .map_err(|e| Error::PluginError(format!("Failed to list images: {}", e)))?;

        // Map: layer_id -> (size, Vec<image_names>)
        let mut layer_usage: HashMap<String, (i64, Vec<String>)> = HashMap::new();
        let mut total_image_sizes = 0i64;

        for image in &images {
            let image_name = image
                .repo_tags
                .first()
                .cloned()
                .unwrap_or_else(|| format!("<none>:{}", &image.id[7..19.min(image.id.len())]));

            // Skip dangling images for layer analysis
            if image_name.contains("<none>") {
                continue;
            }

            total_image_sizes += image.size;

            // Inspect image to get layer information
            if let Ok(inspect) = self.docker.inspect_image(&image.id).await {
                if let Some(root_fs) = inspect.root_fs {
                    if let Some(ref layers) = root_fs.layers {
                        let layer_count = layers.len();
                        for layer_id in layers {
                            let entry = layer_usage
                                .entry(layer_id.clone())
                                .or_insert((0, Vec::new()));
                            entry.1.push(image_name.clone());

                            // Size estimation - divide image size by number of layers
                            if layer_count > 0 {
                                entry.0 = image.size / layer_count as i64;
                            }
                        }
                    }
                }
            }
        }

        // Calculate shared layers (used by 2+ images)
        let mut shared_layers = Vec::new();
        let mut total_shared_bytes = 0u64;
        let mut total_unique_bytes = 0u64;

        for (layer_id, (size, images_using)) in layer_usage {
            let size_bytes = size.max(0) as u64;

            if images_using.len() > 1 {
                // Shared layer
                total_shared_bytes += size_bytes;
                let short_id = if layer_id.len() > 19 {
                    layer_id[7..19].to_string()
                } else {
                    layer_id.clone()
                };
                shared_layers.push(SharedLayerInfo {
                    layer_id: short_id,
                    size_bytes,
                    shared_by_count: images_using.len(),
                    images_using: images_using.clone(),
                });
            } else {
                // Unique layer
                total_unique_bytes += size_bytes;
            }
        }

        // Sort shared layers by size descending
        shared_layers.sort_by(|a, b| {
            b.size_bytes
                .cmp(&a.size_bytes)
                .then(b.shared_by_count.cmp(&a.shared_by_count))
        });

        // Calculate efficiency
        let total_actual = total_shared_bytes + total_unique_bytes;
        let efficiency_percent = if total_actual > 0 {
            (1.0 - (total_actual as f64 / total_image_sizes.max(1) as f64)) * 100.0
        } else {
            0.0
        };

        debug!(
            shared_layers = shared_layers.len(),
            efficiency = %format!("{:.1}%", efficiency_percent),
            "Image layers analysis complete"
        );

        Ok(LayersAnalysis {
            shared_layers,
            total_shared_bytes,
            total_unique_bytes,
            efficiency_percent,
        })
    }
}

/// Get file size in bytes
fn get_file_size(path: &str) -> Result<u64> {
    let metadata = std::fs::metadata(Path::new(path))
        .map_err(|e| Error::PluginError(format!("Failed to get file size: {}", e)))?;
    Ok(metadata.len())
}

/// Parse size string like "100M", "1G", "500K" to bytes
fn parse_size_threshold(s: &str) -> Result<u64> {
    let s = s.trim().to_uppercase();
    let (num_str, suffix) = if s.ends_with('G') {
        (&s[..s.len() - 1], 1024 * 1024 * 1024)
    } else if s.ends_with('M') {
        (&s[..s.len() - 1], 1024 * 1024)
    } else if s.ends_with('K') {
        (&s[..s.len() - 1], 1024)
    } else {
        (s.as_str(), 1)
    };

    let num: u64 = num_str
        .parse()
        .map_err(|e| Error::PluginError(format!("Failed to parse size: {}", e)))?;
    Ok(num * suffix)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_size_threshold_bytes() {
        assert_eq!(parse_size_threshold("100").unwrap(), 100);
    }

    #[test]
    fn test_parse_size_threshold_kb() {
        assert_eq!(parse_size_threshold("100K").unwrap(), 100 * 1024);
        assert_eq!(parse_size_threshold("100k").unwrap(), 100 * 1024);
    }

    #[test]
    fn test_parse_size_threshold_mb() {
        assert_eq!(parse_size_threshold("100M").unwrap(), 100 * 1024 * 1024);
        assert_eq!(parse_size_threshold("100m").unwrap(), 100 * 1024 * 1024);
    }

    #[test]
    fn test_parse_size_threshold_gb() {
        assert_eq!(parse_size_threshold("1G").unwrap(), 1024 * 1024 * 1024);
        assert_eq!(parse_size_threshold("1g").unwrap(), 1024 * 1024 * 1024);
    }
}
