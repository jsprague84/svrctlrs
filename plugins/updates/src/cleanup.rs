//! OS cleanup operations module
//!
//! Cleans package manager cache and removes old packages

use crate::detection::PackageManager;
use serde::{Deserialize, Serialize};
use svrctlrs_core::{Error, RemoteExecutor, Result, Server};
use tracing::{debug, info, instrument};

/// Result of cleanup execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupResult {
    pub success: bool,
    pub summary: String,
    pub space_freed_bytes: u64,
    pub errors: Vec<String>,
}

/// Cleanup executor
pub struct CleanupExecutor {}

impl CleanupExecutor {
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

    /// Perform cleanup on a remote server via SSH
    #[instrument(skip(self, ssh_key))]
    pub async fn cleanup_remote(
        &self,
        server_name: &str,
        ssh_host: &str,
        ssh_user: Option<&str>,
        ssh_key: Option<&str>,
    ) -> Result<CleanupResult> {
        info!(
            server = %server_name,
            host = %ssh_host,
            "Performing remote OS cleanup via SSH"
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

        let mut errors = Vec::new();
        let mut operations = Vec::new();

        match pm {
            PackageManager::Apt => {
                // Clean package cache
                match executor
                    .execute_command(
                        "sh",
                        &["-c", r#"if [ "$(id -u)" = "0" ]; then apt clean all; else sudo apt clean all; fi"#],
                    )
                    .await
                {
                    Ok(_) => {
                        operations.push("cleaned package cache");
                        debug!("Package cache cleaned");
                    }
                    Err(e) => {
                        errors.push(format!("Failed to clean package cache: {}", e));
                    }
                }

                // Remove unused packages
                match executor
                    .execute_command(
                        "sh",
                        &["-c", r#"if [ "$(id -u)" = "0" ]; then apt autoremove -y; else sudo apt autoremove -y; fi"#],
                    )
                    .await
                {
                    Ok(_) => {
                        operations.push("removed unused packages");
                        debug!("Unused packages removed");
                    }
                    Err(e) => {
                        errors.push(format!("Failed to autoremove: {}", e));
                    }
                }
            }
            PackageManager::Dnf => {
                // Clean package cache
                match executor
                    .execute_command(
                        "sh",
                        &["-c", r#"if [ "$(id -u)" = "0" ]; then dnf clean all; else sudo dnf clean all; fi"#],
                    )
                    .await
                {
                    Ok(_) => {
                        operations.push("cleaned package cache");
                        debug!("Package cache cleaned");
                    }
                    Err(e) => {
                        errors.push(format!("Failed to clean package cache: {}", e));
                    }
                }

                // Remove unused packages
                match executor
                    .execute_command(
                        "sh",
                        &["-c", r#"if [ "$(id -u)" = "0" ]; then dnf autoremove -y; else sudo dnf autoremove -y; fi"#],
                    )
                    .await
                {
                    Ok(_) => {
                        operations.push("removed unused packages");
                        debug!("Unused packages removed");
                    }
                    Err(e) => {
                        errors.push(format!("Failed to autoremove: {}", e));
                    }
                }
            }
            PackageManager::Pacman => {
                // Clean package cache
                match executor
                    .execute_command(
                        "sh",
                        &["-c", r#"if [ "$(id -u)" = "0" ]; then pacman -Sc --noconfirm; else sudo pacman -Sc --noconfirm; fi"#],
                    )
                    .await
                {
                    Ok(_) => {
                        operations.push("cleaned package cache");
                        debug!("Package cache cleaned");
                    }
                    Err(e) => {
                        errors.push(format!("Failed to clean package cache: {}", e));
                    }
                }

                // Remove orphaned packages
                match executor
                    .execute_command(
                        "sh",
                        &["-c", r#"if [ "$(id -u)" = "0" ]; then pacman -Qdtq | pacman -Rs - --noconfirm || true; else pacman -Qdtq | sudo pacman -Rs - --noconfirm || true; fi"#],
                    )
                    .await
                {
                    Ok(_) => {
                        operations.push("removed orphaned packages");
                        debug!("Orphaned packages removed");
                    }
                    Err(e) => {
                        errors.push(format!("Failed to remove orphaned packages: {}", e));
                    }
                }
            }
        }

        let summary = if operations.is_empty() {
            "No cleanup performed".to_string()
        } else {
            operations.join(", ")
        };

        let success = errors.is_empty();

        Ok(CleanupResult {
            success,
            summary,
            space_freed_bytes: 0, // TODO: Calculate space freed
            errors,
        })
    }

    /// Perform cleanup on the local system
    #[instrument(skip(self))]
    pub async fn cleanup_local(&self) -> Result<CleanupResult> {
        info!("Performing local OS cleanup");

        // Detect package manager locally
        let pm = self.detect_local_package_manager().await?;
        debug!(package_manager = %pm.display_name(), "Package manager detected");

        use tokio::process::Command;

        let mut errors = Vec::new();
        let mut operations = Vec::new();

        match pm {
            PackageManager::Apt => {
                // Clean package cache
                let clean_result = Command::new("sudo")
                    .args(&["apt", "clean", "all"])
                    .status()
                    .await;

                match clean_result {
                    Ok(status) if status.success() => {
                        operations.push("cleaned package cache");
                        debug!("Package cache cleaned");
                    }
                    Ok(_) | Err(_) => {
                        errors.push("Failed to clean package cache".to_string());
                    }
                }

                // Remove unused packages
                let autoremove_result = Command::new("sudo")
                    .args(&["apt", "autoremove", "-y"])
                    .status()
                    .await;

                match autoremove_result {
                    Ok(status) if status.success() => {
                        operations.push("removed unused packages");
                        debug!("Unused packages removed");
                    }
                    Ok(_) | Err(_) => {
                        errors.push("Failed to autoremove".to_string());
                    }
                }
            }
            PackageManager::Dnf => {
                // Clean package cache
                let clean_result = Command::new("sudo")
                    .args(&["dnf", "clean", "all"])
                    .status()
                    .await;

                match clean_result {
                    Ok(status) if status.success() => {
                        operations.push("cleaned package cache");
                        debug!("Package cache cleaned");
                    }
                    Ok(_) | Err(_) => {
                        errors.push("Failed to clean package cache".to_string());
                    }
                }

                // Remove unused packages
                let autoremove_result = Command::new("sudo")
                    .args(&["dnf", "autoremove", "-y"])
                    .status()
                    .await;

                match autoremove_result {
                    Ok(status) if status.success() => {
                        operations.push("removed unused packages");
                        debug!("Unused packages removed");
                    }
                    Ok(_) | Err(_) => {
                        errors.push("Failed to autoremove".to_string());
                    }
                }
            }
            PackageManager::Pacman => {
                // Clean package cache
                let clean_result = Command::new("sudo")
                    .args(&["pacman", "-Sc", "--noconfirm"])
                    .status()
                    .await;

                match clean_result {
                    Ok(status) if status.success() => {
                        operations.push("cleaned package cache");
                        debug!("Package cache cleaned");
                    }
                    Ok(_) | Err(_) => {
                        errors.push("Failed to clean package cache".to_string());
                    }
                }

                // Remove orphaned packages
                let orphan_result = Command::new("sh")
                    .arg("-c")
                    .arg("pacman -Qdtq | sudo pacman -Rs - --noconfirm || true")
                    .status()
                    .await;

                match orphan_result {
                    Ok(status) if status.success() => {
                        operations.push("removed orphaned packages");
                        debug!("Orphaned packages removed");
                    }
                    Ok(_) | Err(_) => {
                        errors.push("Failed to remove orphaned packages".to_string());
                    }
                }
            }
        }

        let summary = if operations.is_empty() {
            "No cleanup performed".to_string()
        } else {
            operations.join(", ")
        };

        let success = errors.is_empty();

        Ok(CleanupResult {
            success,
            summary,
            space_freed_bytes: 0, // TODO: Calculate space freed
            errors,
        })
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
