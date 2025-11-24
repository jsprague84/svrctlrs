//! System updates monitoring and management plugin
//!
//! Provides OS update detection, execution, and cleanup operations

mod cleanup;
mod detection;
mod execution;

use async_trait::async_trait;
use cleanup::CleanupExecutor;
use detection::UpdateDetector;
use execution::UpdateExecutor;
use serde_json::json;
use std::collections::HashMap;
use svrctlrs_core::{
    Error, Plugin, PluginContext, PluginMetadata, PluginResult, Result, ScheduledTask,
};
use tracing::{info, instrument};

/// System and package updates monitoring plugin
pub struct UpdatesPlugin {}

impl UpdatesPlugin {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for UpdatesPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Plugin for UpdatesPlugin {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            id: "updates".to_string(),
            name: "Updates Manager".to_string(),
            description: "Monitor and apply OS updates, manage system cleanup".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            author: "SvrCtlRS".to_string(),
        }
    }

    fn scheduled_tasks(&self) -> Vec<ScheduledTask> {
        vec![
            ScheduledTask {
                id: "updates_check".to_string(),
                schedule: "0 0 */6 * * *".to_string(), // Every 6 hours
                description: "Check for available OS updates".to_string(),
                enabled: true,
            },
            ScheduledTask {
                id: "updates_apply".to_string(),
                schedule: "0 0 3 * * 0".to_string(), // Sundays at 3 AM
                description: "Apply OS updates (if enabled)".to_string(),
                enabled: false, // Disabled by default for safety
            },
            ScheduledTask {
                id: "os_cleanup".to_string(),
                schedule: "0 0 4 * * 0".to_string(), // Sundays at 4 AM
                description: "Clean OS package cache and old packages".to_string(),
                enabled: false, // Disabled by default for safety
            },
        ]
    }

    async fn execute(&self, task_id: &str, context: &PluginContext) -> Result<PluginResult> {
        info!(task_id = %task_id, "Executing Updates plugin task");

        match task_id {
            "updates_check" => self.check_updates(context).await,
            "updates_apply" => self.apply_updates(context).await,
            "os_cleanup" => self.cleanup_os(context).await,
            _ => Ok(PluginResult {
                success: false,
                message: format!("Unknown task: {}", task_id),
                data: None,
                metrics: None,
            }),
        }
    }
}

impl UpdatesPlugin {
    #[instrument(skip(self, context))]
    async fn check_updates(&self, context: &PluginContext) -> Result<PluginResult> {
        info!("Checking for OS updates");

        // Get server configuration from context
        let server_name =
            std::env::var("UPDATES_SERVER_NAME").unwrap_or_else(|_| "localhost".to_string());
        let ssh_enabled = std::env::var("UPDATES_SSH_ENABLED")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

        let detector = UpdateDetector::new();

        let update_info = if ssh_enabled {
            // Remote update check via SSH
            let ssh_host = std::env::var("UPDATES_SSH_HOST")
                .map_err(|_| Error::PluginError("UPDATES_SSH_HOST not set".to_string()))?;
            let ssh_user = std::env::var("UPDATES_SSH_USER").ok();
            let ssh_key = std::env::var("UPDATES_SSH_KEY").ok();

            detector
                .check_remote_updates(
                    &server_name,
                    &ssh_host,
                    ssh_user.as_deref(),
                    ssh_key.as_deref(),
                )
                .await?
        } else {
            // Local update check
            detector.check_local_updates().await?
        };

        let message = format!(
            "Updates check: {} packages available on {}",
            update_info.total_updates, server_name
        );

        // Prepare structured data
        let data = json!({
            "server_name": server_name,
            "package_manager": update_info.package_manager,
            "total_updates": update_info.total_updates,
            "security_updates": update_info.security_updates,
            "packages": update_info.packages,
        });

        // Prepare metrics
        let mut metrics = HashMap::new();
        metrics.insert(
            "total_updates".to_string(),
            update_info.total_updates as f64,
        );
        metrics.insert(
            "security_updates".to_string(),
            update_info.security_updates as f64,
        );

        // Send notification if updates available
        if update_info.total_updates > 0 {
            self.send_update_notification(
                &context.notification_manager,
                &update_info,
                &server_name,
            )
            .await?;
        }

        Ok(PluginResult {
            success: true,
            message,
            data: Some(data),
            metrics: Some(metrics),
        })
    }

    #[instrument(skip(self, context))]
    async fn apply_updates(&self, context: &PluginContext) -> Result<PluginResult> {
        info!("Applying OS updates");

        let server_name =
            std::env::var("UPDATES_SERVER_NAME").unwrap_or_else(|_| "localhost".to_string());
        let ssh_enabled = std::env::var("UPDATES_SSH_ENABLED")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

        let executor = UpdateExecutor::new();

        let result = if ssh_enabled {
            let ssh_host = std::env::var("UPDATES_SSH_HOST")
                .map_err(|_| Error::PluginError("UPDATES_SSH_HOST not set".to_string()))?;
            let ssh_user = std::env::var("UPDATES_SSH_USER").ok();
            let ssh_key = std::env::var("UPDATES_SSH_KEY").ok();

            executor
                .apply_remote_updates(
                    &server_name,
                    &ssh_host,
                    ssh_user.as_deref(),
                    ssh_key.as_deref(),
                )
                .await?
        } else {
            executor.apply_local_updates().await?
        };

        let message = format!("Updates applied on {}: {}", server_name, result.summary);

        // Prepare metrics
        let mut metrics = HashMap::new();
        metrics.insert(
            "packages_updated".to_string(),
            result.packages_updated as f64,
        );
        metrics.insert(
            "success".to_string(),
            if result.success { 1.0 } else { 0.0 },
        );

        // Send notification with results
        self.send_execution_notification(&context.notification_manager, &result, &server_name)
            .await?;

        Ok(PluginResult {
            success: result.success,
            message,
            data: None,
            metrics: Some(metrics),
        })
    }

    #[instrument(skip(self, context))]
    async fn cleanup_os(&self, context: &PluginContext) -> Result<PluginResult> {
        info!("Running OS cleanup");

        let server_name =
            std::env::var("UPDATES_SERVER_NAME").unwrap_or_else(|_| "localhost".to_string());
        let ssh_enabled = std::env::var("UPDATES_SSH_ENABLED")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

        let cleanup_executor = CleanupExecutor::new();

        let result = if ssh_enabled {
            let ssh_host = std::env::var("UPDATES_SSH_HOST")
                .map_err(|_| Error::PluginError("UPDATES_SSH_HOST not set".to_string()))?;
            let ssh_user = std::env::var("UPDATES_SSH_USER").ok();
            let ssh_key = std::env::var("UPDATES_SSH_KEY").ok();

            cleanup_executor
                .cleanup_remote(
                    &server_name,
                    &ssh_host,
                    ssh_user.as_deref(),
                    ssh_key.as_deref(),
                )
                .await?
        } else {
            cleanup_executor.cleanup_local().await?
        };

        let message = format!("OS cleanup on {}: {}", server_name, result.summary);

        // Prepare metrics
        let mut metrics = HashMap::new();
        metrics.insert(
            "space_freed_mb".to_string(),
            result.space_freed_bytes as f64 / 1024.0 / 1024.0,
        );

        // Send notification with results
        self.send_cleanup_notification(&context.notification_manager, &result, &server_name)
            .await?;

        Ok(PluginResult {
            success: result.success,
            message,
            data: None,
            metrics: Some(metrics),
        })
    }

    async fn send_update_notification(
        &self,
        notify_mgr: &svrctlrs_core::NotificationManager,
        update_info: &detection::UpdateInfo,
        server_name: &str,
    ) -> Result<()> {
        let title = format!("Updates Available: {}", server_name);

        let mut body = String::new();
        body.push_str(&format!(
            "## {} Updates Available\n\n",
            update_info.total_updates
        ));
        body.push_str(&format!("**Server**: {}\n", server_name));
        body.push_str(&format!(
            "**Package Manager**: {}\n\n",
            update_info.package_manager
        ));

        if update_info.security_updates > 0 {
            body.push_str(&format!(
                "🔒 **Security Updates**: {}\n\n",
                update_info.security_updates
            ));
        }

        // List first 10 packages
        if !update_info.packages.is_empty() {
            body.push_str("**Available Packages**:\n");
            for (i, pkg) in update_info.packages.iter().take(10).enumerate() {
                body.push_str(&format!("{}. {}\n", i + 1, pkg));
            }
            if update_info.packages.len() > 10 {
                body.push_str(&format!(
                    "\n...and {} more\n",
                    update_info.packages.len() - 10
                ));
            }
        }

        let message = svrctlrs_core::NotificationMessage {
            title,
            body,
            priority: if update_info.security_updates > 0 {
                4
            } else {
                3
            },
            actions: vec![],
        };

        notify_mgr
            .send_for_service("updates", &message)
            .await
            .map_err(|e| Error::PluginError(format!("Failed to send notification: {}", e)))?;

        Ok(())
    }

    async fn send_execution_notification(
        &self,
        notify_mgr: &svrctlrs_core::NotificationManager,
        result: &execution::ExecutionResult,
        server_name: &str,
    ) -> Result<()> {
        let title = if result.success {
            format!("✅ Updates Applied: {}", server_name)
        } else {
            format!("❌ Update Failed: {}", server_name)
        };

        let mut body = String::new();
        body.push_str(&format!("**Server**: {}\n", server_name));
        body.push_str(&format!("**Summary**: {}\n\n", result.summary));

        if result.packages_updated > 0 {
            body.push_str(&format!(
                "📦 **Packages Updated**: {}\n",
                result.packages_updated
            ));
        }

        if !result.errors.is_empty() {
            body.push_str("\n**Errors**:\n");
            for error in &result.errors {
                body.push_str(&format!("  - {}\n", error));
            }
        }

        let message = svrctlrs_core::NotificationMessage {
            title,
            body,
            priority: if result.success { 3 } else { 4 },
            actions: vec![],
        };

        notify_mgr
            .send_for_service("updates", &message)
            .await
            .map_err(|e| Error::PluginError(format!("Failed to send notification: {}", e)))?;

        Ok(())
    }

    async fn send_cleanup_notification(
        &self,
        notify_mgr: &svrctlrs_core::NotificationManager,
        result: &cleanup::CleanupResult,
        server_name: &str,
    ) -> Result<()> {
        let title = format!("OS Cleanup: {}", server_name);

        let mut body = String::new();
        body.push_str("## Cleanup Complete\n\n");
        body.push_str(&format!("**Server**: {}\n", server_name));
        body.push_str(&format!(
            "**Space Freed**: {:.2} MB\n\n",
            result.space_freed_bytes as f64 / 1024.0 / 1024.0
        ));
        body.push_str(&format!("**Summary**: {}\n", result.summary));

        let message = svrctlrs_core::NotificationMessage {
            title,
            body,
            priority: 3,
            actions: vec![],
        };

        notify_mgr
            .send_for_service("updates", &message)
            .await
            .map_err(|e| Error::PluginError(format!("Failed to send notification: {}", e)))?;

        Ok(())
    }
}
