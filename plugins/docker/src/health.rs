//! Docker container health monitoring
//!
//! Monitors Docker containers for health status, resource usage, and issues.
//! Sends notifications when containers are unhealthy or exceed resource thresholds.

use bollard::container::{InspectContainerOptions, ListContainersOptions, Stats, StatsOptions};
use bollard::Docker;
use futures_util::stream::StreamExt;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use svrctlrs_core::{Error, NotificationManager, NotificationMessage, Result};
use tokio::time::timeout;
use tracing::{debug, info, instrument, warn};

/// Default CPU warning threshold (percentage)
const DEFAULT_CPU_WARN_PCT: f64 = 80.0;

/// Default memory warning threshold (percentage)
const DEFAULT_MEM_WARN_PCT: f64 = 80.0;

/// Stats sampling timeout
const STATS_TIMEOUT_SECS: u64 = 5;

/// Container health status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerHealth {
    pub id: String,
    pub name: String,
    pub running: bool,
    pub health_status: Option<String>,
    pub cpu_percent: Option<f64>,
    pub mem_percent: Option<f64>,
    pub issues: Vec<String>,
}

/// Docker health monitor
pub struct HealthMonitor {
    docker: Docker,
    cpu_warn_pct: f64,
    mem_warn_pct: f64,
    ignore_list: Vec<String>,
}

impl HealthMonitor {
    /// Create a new health monitor
    ///
    /// # Errors
    ///
    /// Returns error if Docker connection fails
    #[instrument]
    pub async fn new() -> Result<Self> {
        info!("Connecting to Docker daemon");

        let docker = Docker::connect_with_unix_defaults()
            .map_err(|e| Error::PluginError(format!("Failed to connect to Docker: {}", e)))?;

        // Load configuration from environment
        let cpu_warn_pct = std::env::var("DOCKER_CPU_WARN_PCT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT_CPU_WARN_PCT);

        let mem_warn_pct = std::env::var("DOCKER_MEM_WARN_PCT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT_MEM_WARN_PCT);

        let ignore_list: Vec<String> = std::env::var("DOCKER_IGNORE_LIST")
            .ok()
            .map(|s| s.split(',').map(|v| v.trim().to_string()).collect())
            .unwrap_or_default();

        info!(
            cpu_warn = %cpu_warn_pct,
            mem_warn = %mem_warn_pct,
            ignore_count = ignore_list.len(),
            "Health monitor configured"
        );

        Ok(Self {
            docker,
            cpu_warn_pct,
            mem_warn_pct,
            ignore_list,
        })
    }

    /// Set warning thresholds
    pub fn set_thresholds(&mut self, cpu_warn_pct: f64, mem_warn_pct: f64) {
        self.cpu_warn_pct = cpu_warn_pct;
        self.mem_warn_pct = mem_warn_pct;
    }

    /// Check health of all containers
    ///
    /// # Arguments
    ///
    /// * `notify_mgr` - Notification manager for sending alerts
    /// * `send_summary` - Whether to send summary when all healthy
    ///
    /// # Returns
    ///
    /// List of container health statuses
    #[instrument(skip(self, notify_mgr))]
    pub async fn check_health(
        &self,
        notify_mgr: &NotificationManager,
        send_summary: bool,
    ) -> Result<Vec<ContainerHealth>> {
        info!("Starting Docker health check");

        // List all containers (including stopped)
        let options = Some(ListContainersOptions::<String> {
            all: true,
            ..Default::default()
        });

        let containers = self
            .docker
            .list_containers(options)
            .await
            .map_err(|e| Error::PluginError(format!("Failed to list containers: {}", e)))?;

        info!(count = containers.len(), "Found containers");

        let mut health_statuses = Vec::new();
        let mut bad_containers = Vec::new();

        for container in containers {
            let id = container
                .id
                .as_ref()
                .ok_or_else(|| Error::PluginError("Container has no ID".to_string()))?;

            let name = container
                .names
                .as_ref()
                .and_then(|names| names.first())
                .map(|n| n.trim_start_matches('/').to_string())
                .unwrap_or_else(|| id.clone());

            // Check if container is in ignore list
            if self.is_ignored(&name) {
                debug!(container = %name, "Skipping ignored container");
                continue;
            }

            // Inspect container for state and health
            let health = self.inspect_container(id, &name).await?;

            // Track bad containers for notification
            if !health.issues.is_empty() {
                bad_containers.push(health.clone());
            }

            health_statuses.push(health);
        }

        // Send notification if there are issues
        if !bad_containers.is_empty() {
            self.send_health_alert(notify_mgr, &bad_containers).await?;
        } else if send_summary {
            // Send summary even when all healthy
            self.send_health_summary(notify_mgr, &health_statuses).await?;
        } else {
            info!("All containers healthy");
        }

        Ok(health_statuses)
    }

    /// Inspect a single container
    #[instrument(skip(self))]
    async fn inspect_container(&self, id: &str, name: &str) -> Result<ContainerHealth> {
        debug!(container = %name, "Inspecting container");

        // Inspect container
        let inspect = self
            .docker
            .inspect_container(id, None::<InspectContainerOptions>)
            .await
            .map_err(|e| Error::PluginError(format!("Failed to inspect container: {}", e)))?;

        // Extract state and health
        let state = inspect
            .state
            .ok_or_else(|| Error::PluginError(format!("Container {} has no state", name)))?;

        let running = state.running.unwrap_or(false);
        let health_status = state
            .health
            .and_then(|h| h.status)
            .map(|s| format!("{:?}", s));

        let mut issues = Vec::new();

        // Check if container is stopped
        if !running {
            issues.push("Container is stopped".to_string());
        }

        // Check health status
        if let Some(ref status) = health_status {
            if status.contains("unhealthy") {
                issues.push(format!("Health check failed: {}", status));
            }
        }

        // Sample stats for running containers
        let (cpu_percent, mem_percent) = if running {
            match self.sample_stats(id).await {
                Ok(stats) => stats,
                Err(e) => {
                    warn!(container = %name, error = %e, "Failed to sample stats");
                    (None, None)
                }
            }
        } else {
            (None, None)
        };

        // Check resource thresholds
        if let Some(cpu) = cpu_percent {
            if cpu > self.cpu_warn_pct {
                issues.push(format!("High CPU usage: {:.1}%", cpu));
            }
        }

        if let Some(mem) = mem_percent {
            if mem > self.mem_warn_pct {
                issues.push(format!("High memory usage: {:.1}%", mem));
            }
        }

        Ok(ContainerHealth {
            id: id.to_string(),
            name: name.to_string(),
            running,
            health_status,
            cpu_percent,
            mem_percent,
            issues,
        })
    }

    /// Sample container stats
    #[instrument(skip(self))]
    async fn sample_stats(&self, id: &str) -> Result<(Option<f64>, Option<f64>)> {
        let options = Some(StatsOptions {
            stream: false,
            one_shot: true,
        });

        // Stream stats with timeout
        let mut stream = self.docker.stats(id, options);

        let stats = timeout(Duration::from_secs(STATS_TIMEOUT_SECS), stream.next())
            .await
            .map_err(|_| Error::PluginError("Stats sampling timed out".to_string()))?
            .ok_or_else(|| Error::PluginError("No stats available".to_string()))?
            .map_err(|e| Error::PluginError(format!("Failed to get stats: {}", e)))?;

        // Calculate CPU percentage
        let cpu_percent = calculate_cpu_percent(&stats);

        // Calculate memory percentage
        let mem_percent = calculate_mem_percent(&stats);

        Ok((cpu_percent, mem_percent))
    }

    /// Check if container should be ignored
    fn is_ignored(&self, name: &str) -> bool {
        self.ignore_list.iter().any(|pattern| {
            if pattern.contains('*') {
                // Simple wildcard matching
                let pattern_start = pattern.trim_end_matches('*');
                name.starts_with(pattern_start)
            } else {
                name == pattern
            }
        })
    }

    /// Send health alert notification
    #[instrument(skip(self, notify_mgr, containers))]
    async fn send_health_alert(
        &self,
        notify_mgr: &NotificationManager,
        containers: &[ContainerHealth],
    ) -> Result<()> {
        let title = format!("Docker Health Alert: {} issue(s)", containers.len());

        let mut body = String::new();
        for container in containers {
            body.push_str(&format!("\n🐳 {}\n", container.name));
            for issue in &container.issues {
                body.push_str(&format!("  ⚠️  {}\n", issue));
            }

            if let Some(cpu) = container.cpu_percent {
                body.push_str(&format!("  CPU: {:.1}%\n", cpu));
            }
            if let Some(mem) = container.mem_percent {
                body.push_str(&format!("  Memory: {:.1}%\n", mem));
            }
        }

        let message = NotificationMessage {
            title,
            body,
            priority: 4, // High priority
            actions: vec![],
        };

        notify_mgr
            .send_for_service("docker", &message)
            .await
            .map_err(|e| Error::PluginError(format!("Failed to send notification: {}", e)))?;

        info!("Health alert sent");
        Ok(())
    }

    /// Send health summary notification (all containers healthy)
    #[instrument(skip(self, notify_mgr, containers))]
    pub async fn send_health_summary(
        &self,
        notify_mgr: &NotificationManager,
        containers: &[ContainerHealth],
    ) -> Result<()> {
        let total = containers.len();
        let running = containers.iter().filter(|c| c.running).count();
        let stopped = total - running;
        
        // Calculate average resource usage
        let cpu_values: Vec<f64> = containers.iter().filter_map(|c| c.cpu_percent).collect();
        let mem_values: Vec<f64> = containers.iter().filter_map(|c| c.mem_percent).collect();
        
        let avg_cpu = if !cpu_values.is_empty() {
            cpu_values.iter().sum::<f64>() / cpu_values.len() as f64
        } else {
            0.0
        };
        
        let avg_mem = if !mem_values.is_empty() {
            mem_values.iter().sum::<f64>() / mem_values.len() as f64
        } else {
            0.0
        };

        let title = "Docker Health Summary".to_string();
        let body = format!(
            "📊 All containers healthy ✓\n\n\
            Containers: {} total, {} running, {} stopped\n\
            Average CPU: {:.1}%\n\
            Average Memory: {:.1}%\n\n\
            All systems operational.",
            total, running, stopped, avg_cpu, avg_mem
        );

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

        info!("Health summary sent");
        Ok(())
    }
}

/// Calculate CPU percentage from stats
fn calculate_cpu_percent(stats: &Stats) -> Option<f64> {
    let cpu_stats = &stats.cpu_stats;
    let precpu_stats = &stats.precpu_stats;

    let cpu_delta =
        cpu_stats.cpu_usage.total_usage as f64 - precpu_stats.cpu_usage.total_usage as f64;
    let system_delta = cpu_stats.system_cpu_usage? as f64 - precpu_stats.system_cpu_usage? as f64;

    if system_delta > 0.0 && cpu_delta > 0.0 {
        let num_cpus = cpu_stats.online_cpus.unwrap_or(1) as f64;
        Some((cpu_delta / system_delta) * num_cpus * 100.0)
    } else {
        None
    }
}

/// Calculate memory percentage from stats
fn calculate_mem_percent(stats: &Stats) -> Option<f64> {
    let mem_stats = &stats.memory_stats;
    let usage = mem_stats.usage? as f64;
    let limit = mem_stats.limit? as f64;

    if limit > 0.0 {
        Some((usage / limit) * 100.0)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_is_ignored_exact_match() {
        let ignore_list = vec!["test-container".to_string()];

        // Simulate is_ignored logic
        let is_ignored = |name: &str| {
            ignore_list.iter().any(|pattern| {
                if pattern.contains('*') {
                    let pattern_start = pattern.trim_end_matches('*');
                    name.starts_with(pattern_start)
                } else {
                    name == pattern
                }
            })
        };

        assert!(is_ignored("test-container"));
        assert!(!is_ignored("other-container"));
    }

    #[test]
    fn test_is_ignored_wildcard() {
        let ignore_list = vec!["test-*".to_string()];

        // Simulate is_ignored logic
        let is_ignored = |name: &str| {
            ignore_list.iter().any(|pattern| {
                if pattern.contains('*') {
                    let pattern_start = pattern.trim_end_matches('*');
                    name.starts_with(pattern_start)
                } else {
                    name == pattern
                }
            })
        };

        assert!(is_ignored("test-container"));
        assert!(is_ignored("test-another"));
        assert!(!is_ignored("other-container"));
    }
}
