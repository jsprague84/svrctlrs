//! OS update execution module
//!
//! Applies OS updates using package managers

use crate::detection::{get_checker, PackageManager, UpdateChecker};
use serde::{Deserialize, Serialize};
use svrctlrs_core::{Error, RemoteExecutor, Result, Server};
use tracing::{debug, info, instrument, warn};

/// Result of update execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub success: bool,
    pub summary: String,
    pub packages_updated: usize,
    pub errors: Vec<String>,
}

/// Update executor
pub struct UpdateExecutor {}

impl UpdateExecutor {
    pub fn new() -> Self {
        Self {}
    }

    /// Detect package manager on a remote server
    #[instrument(skip(executor))]
    async fn detect_package_manager(executor: &RemoteExecutor) -> Result<PackageManager> {
        for pm in PackageManager::all() {
            let binary = pm.binary();

            let check_cmd = format!("test -x /usr/bin/{} && echo found", binary);
            match executor.execute_command("sh", &["-c", &check_cmd]).await {
                Ok(output) if output.trim() == "found" => {
                    info!("Detected package manager: {:?}", pm);
                    return Ok(pm);
                }
                _ => continue,
            }
        }

        Err(Error::PluginError(format!(
            "No supported package manager found on {}",
            executor.server().name
        )))
    }

    /// Apply updates on a remote server via SSH
    #[instrument(skip(self, ssh_key))]
    pub async fn apply_remote_updates(
        &self,
        server_name: &str,
        ssh_host: &str,
        ssh_user: Option<&str>,
        ssh_key: Option<&str>,
    ) -> Result<ExecutionResult> {
        info!(
            server = %server_name,
            host = %ssh_host,
            "Applying remote updates via SSH"
        );

        // Build SSH connection string (user@host or just host)
        let ssh_connection = if let Some(user) = ssh_user {
            format!("{}@{}", user, ssh_host)
        } else {
            ssh_host.to_string()
        };

        // Build server config
        let server = Server::remote(server_name, &ssh_connection);

        // Create remote executor
        let executor = RemoteExecutor::for_server(server.clone(), ssh_key.map(|s| s.to_string()));

        // Detect package manager
        let pm = Self::detect_package_manager(&executor).await?;
        debug!(package_manager = %pm.display_name(), "Package manager detected");

        // Get checker to verify updates before applying
        let checker = get_checker(&pm);
        let updates_before = self.check_updates(&executor, checker.as_ref()).await?;

        if updates_before.is_empty() {
            return Ok(ExecutionResult {
                success: true,
                summary: "No updates available".to_string(),
                packages_updated: 0,
                errors: vec![],
            });
        }

        info!(count = updates_before.len(), "Applying updates");

        // Apply updates based on package manager
        let mut errors = Vec::new();

        match pm {
            PackageManager::Apt => {
                // Update package lists
                match executor
                    .execute_command(
                        "sh",
                        &["-c", r#"if [ "$(id -u)" = "0" ]; then apt-get update -qq; else sudo apt-get update -qq; fi"#],
                    )
                    .await
                {
                    Ok(_) => debug!("Package lists updated"),
                    Err(e) => {
                        errors.push(format!("Failed to update package lists: {}", e));
                        return Ok(ExecutionResult {
                            success: false,
                            summary: "Failed to update package lists".to_string(),
                            packages_updated: 0,
                            errors,
                        });
                    }
                }

                // Full upgrade
                match executor
                    .execute_command(
                        "sh",
                        &["-c", r#"if [ "$(id -u)" = "0" ]; then DEBIAN_FRONTEND=noninteractive apt-get full-upgrade -y; else DEBIAN_FRONTEND=noninteractive sudo apt-get full-upgrade -y; fi"#],
                    )
                    .await
                {
                    Ok(output) => debug!(output = %output, "Updates applied"),
                    Err(e) => {
                        errors.push(format!("Failed to apply updates: {}", e));
                        return Ok(ExecutionResult {
                            success: false,
                            summary: "Failed to apply updates".to_string(),
                            packages_updated: 0,
                            errors,
                        });
                    }
                }
            }
            PackageManager::Dnf => {
                match executor
                    .execute_command(
                        "sh",
                        &["-c", r#"if [ "$(id -u)" = "0" ]; then dnf upgrade -y; else sudo dnf upgrade -y; fi"#],
                    )
                    .await
                {
                    Ok(output) => debug!(output = %output, "Updates applied"),
                    Err(e) => {
                        errors.push(format!("Failed to apply updates: {}", e));
                        return Ok(ExecutionResult {
                            success: false,
                            summary: "Failed to apply updates".to_string(),
                            packages_updated: 0,
                            errors,
                        });
                    }
                }
            }
            PackageManager::Pacman => {
                match executor
                    .execute_command(
                        "sh",
                        &["-c", r#"if [ "$(id -u)" = "0" ]; then pacman -Syu --noconfirm; else sudo pacman -Syu --noconfirm; fi"#],
                    )
                    .await
                {
                    Ok(output) => debug!(output = %output, "Updates applied"),
                    Err(e) => {
                        errors.push(format!("Failed to apply updates: {}", e));
                        return Ok(ExecutionResult {
                            success: false,
                            summary: "Failed to apply updates".to_string(),
                            packages_updated: 0,
                            errors,
                        });
                    }
                }
            }
        }

        // Verify by checking for remaining updates
        let updates_after = self.check_updates(&executor, checker.as_ref()).await?;

        let packages_updated = updates_before.len().saturating_sub(updates_after.len());

        if updates_after.is_empty() {
            Ok(ExecutionResult {
                success: true,
                summary: "Up to date".to_string(),
                packages_updated,
                errors,
            })
        } else {
            warn!(
                remaining = updates_after.len(),
                "Some updates still available"
            );
            Ok(ExecutionResult {
                success: true,
                summary: format!(
                    "{} updates still available (may require reboot or manual intervention)",
                    updates_after.len()
                ),
                packages_updated,
                errors,
            })
        }
    }

    /// Apply updates on the local system
    #[instrument(skip(self))]
    pub async fn apply_local_updates(&self) -> Result<ExecutionResult> {
        info!("Applying local system updates");

        // Detect package manager locally
        let pm = self.detect_local_package_manager().await?;
        debug!(package_manager = %pm.display_name(), "Package manager detected");

        // Get checker to verify updates before applying
        let checker = get_checker(&pm);

        use tokio::process::Command;

        // Check updates before
        let (cmd, args) = checker.check_command();
        let output_before = Command::new(cmd)
            .args(&args)
            .output()
            .await
            .map_err(|e| Error::PluginError(format!("Failed to check updates: {}", e)))?;

        let stdout_before = String::from_utf8_lossy(&output_before.stdout);
        let updates_before = checker.parse_updates(&stdout_before);

        if updates_before.is_empty() {
            return Ok(ExecutionResult {
                success: true,
                summary: "No updates available".to_string(),
                packages_updated: 0,
                errors: vec![],
            });
        }

        info!(count = updates_before.len(), "Applying updates");

        let mut errors = Vec::new();

        // Apply updates based on package manager
        match pm {
            PackageManager::Apt => {
                // Update package lists
                let update_result = Command::new("sudo")
                    .args(["apt-get", "update", "-qq"])
                    .status()
                    .await;

                if let Err(e) = update_result {
                    errors.push(format!("Failed to update package lists: {}", e));
                    return Ok(ExecutionResult {
                        success: false,
                        summary: "Failed to update package lists".to_string(),
                        packages_updated: 0,
                        errors,
                    });
                }

                // Full upgrade
                let upgrade_result = Command::new("sudo")
                    .env("DEBIAN_FRONTEND", "noninteractive")
                    .args(["apt-get", "full-upgrade", "-y"])
                    .status()
                    .await;

                if let Err(e) = upgrade_result {
                    errors.push(format!("Failed to apply updates: {}", e));
                    return Ok(ExecutionResult {
                        success: false,
                        summary: "Failed to apply updates".to_string(),
                        packages_updated: 0,
                        errors,
                    });
                }
            }
            PackageManager::Dnf => {
                let result = Command::new("sudo")
                    .args(["dnf", "upgrade", "-y"])
                    .status()
                    .await;

                if let Err(e) = result {
                    errors.push(format!("Failed to apply updates: {}", e));
                    return Ok(ExecutionResult {
                        success: false,
                        summary: "Failed to apply updates".to_string(),
                        packages_updated: 0,
                        errors,
                    });
                }
            }
            PackageManager::Pacman => {
                let result = Command::new("sudo")
                    .args(["pacman", "-Syu", "--noconfirm"])
                    .status()
                    .await;

                if let Err(e) = result {
                    errors.push(format!("Failed to apply updates: {}", e));
                    return Ok(ExecutionResult {
                        success: false,
                        summary: "Failed to apply updates".to_string(),
                        packages_updated: 0,
                        errors,
                    });
                }
            }
        }

        // Verify by checking for remaining updates
        let output_after = Command::new(cmd)
            .args(&args)
            .output()
            .await
            .map_err(|e| Error::PluginError(format!("Failed to check updates: {}", e)))?;

        let stdout_after = String::from_utf8_lossy(&output_after.stdout);
        let updates_after = checker.parse_updates(&stdout_after);

        let packages_updated = updates_before.len().saturating_sub(updates_after.len());

        if updates_after.is_empty() {
            Ok(ExecutionResult {
                success: true,
                summary: "Up to date".to_string(),
                packages_updated,
                errors,
            })
        } else {
            warn!(
                remaining = updates_after.len(),
                "Some updates still available"
            );
            Ok(ExecutionResult {
                success: true,
                summary: format!(
                    "{} updates still available (may require reboot or manual intervention)",
                    updates_after.len()
                ),
                packages_updated,
                errors,
            })
        }
    }

    /// Check for updates using the given checker
    async fn check_updates(
        &self,
        executor: &RemoteExecutor,
        checker: &dyn UpdateChecker,
    ) -> Result<Vec<String>> {
        let (cmd, args) = checker.check_command();

        let output = executor
            .execute_command(cmd, &args)
            .await
            .map_err(|e| Error::PluginError(format!("Failed to check updates: {}", e)))?;

        Ok(checker.parse_updates(&output))
    }

    /// Detect package manager on local system
    async fn detect_local_package_manager(&self) -> Result<PackageManager> {
        use tokio::process::Command;

        for pm in PackageManager::all() {
            let binary = format!("/usr/bin/{}", pm.binary());

            let check = Command::new("test").arg("-x").arg(&binary).status().await;

            if let Ok(status) = check {
                if status.success() {
                    info!("Detected package manager: {:?}", pm);
                    return Ok(pm);
                }
            }
        }

        Err(Error::PluginError(
            "No supported package manager found on local system".to_string(),
        ))
    }
}
