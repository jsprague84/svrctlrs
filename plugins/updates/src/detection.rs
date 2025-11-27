//! OS update detection module
//!
//! Detects available OS updates using package managers (apt, dnf, pacman)

use serde::{Deserialize, Serialize};
use svrctlrs_core::{Error, RemoteExecutor, Result, Server};
use tracing::{debug, info, instrument};

/// Package manager types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PackageManager {
    Apt,
    Dnf,
    Pacman,
}

impl PackageManager {
    /// Get all supported package managers
    pub fn all() -> Vec<Self> {
        vec![Self::Apt, Self::Dnf, Self::Pacman]
    }

    /// Get the binary name for this package manager
    pub fn binary(&self) -> &str {
        match self {
            Self::Apt => "apt",
            Self::Dnf => "dnf",
            Self::Pacman => "pacman",
        }
    }

    /// Get the display name
    pub fn display_name(&self) -> &str {
        match self {
            Self::Apt => "APT",
            Self::Dnf => "DNF",
            Self::Pacman => "Pacman",
        }
    }
}

/// Update checker trait for different package managers
pub trait UpdateChecker: Send + Sync {
    /// Get the command to check for available updates
    /// Returns: (command, args)
    fn check_command(&self) -> (&str, Vec<&str>);

    /// Parse the output from the check command into a list of package names
    fn parse_updates(&self, output: &str) -> Vec<String>;
}

/// APT package manager checker (Debian, Ubuntu, etc.)
pub struct AptChecker;

impl UpdateChecker for AptChecker {
    fn check_command(&self) -> (&str, Vec<&str>) {
        // apt list --upgradable
        // Use full path for SSH compatibility
        ("/usr/bin/apt", vec!["list", "--upgradable"])
    }

    fn parse_updates(&self, output: &str) -> Vec<String> {
        output
            .lines()
            .skip(1) // Skip "Listing..." header
            .filter(|line| line.contains("[upgradable from:"))
            .map(|line| {
                // Extract package name (everything before the first '/')
                let package_name = line.split('/').next().unwrap_or(line);

                // Check if this is a security update
                let is_security = line.contains("-security");

                if is_security {
                    format!("{} (security)", package_name)
                } else {
                    package_name.to_string()
                }
            })
            .collect()
    }
}

/// DNF package manager checker (Fedora, RHEL 8+, CentOS Stream, etc.)
pub struct DnfChecker;

impl UpdateChecker for DnfChecker {
    fn check_command(&self) -> (&str, Vec<&str>) {
        // dnf check-update returns exit code 100 if updates available
        // Use --cacheonly to avoid refreshing repos (much faster)
        (
            "/usr/bin/dnf",
            vec!["check-update", "--quiet", "--cacheonly"],
        )
    }

    fn parse_updates(&self, output: &str) -> Vec<String> {
        output
            .lines()
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .filter(|line| {
                // Lines with updates have at least 3 parts (package, version, repo)
                line.split_whitespace().count() >= 3
            })
            .map(|line| {
                // Extract package name (first column, before the dot and arch)
                let parts: Vec<&str> = line.split_whitespace().collect();
                if let Some(first) = parts.first() {
                    // Split on '.' to remove arch suffix (e.g., "docker-ce.x86_64" -> "docker-ce")
                    first.split('.').next().unwrap_or(first).to_string()
                } else {
                    line.to_string()
                }
            })
            .collect()
    }
}

/// Pacman package manager checker (Arch Linux, etc.)
pub struct PacmanChecker;

impl UpdateChecker for PacmanChecker {
    fn check_command(&self) -> (&str, Vec<&str>) {
        ("/usr/bin/pacman", vec!["-Qu"])
    }

    fn parse_updates(&self, output: &str) -> Vec<String> {
        output
            .lines()
            .filter(|line| !line.is_empty())
            .map(|line| {
                // Extract package name (first column)
                line.split_whitespace().next().unwrap_or(line).to_string()
            })
            .collect()
    }
}

/// Get the appropriate checker for a package manager
pub fn get_checker(pm: &PackageManager) -> Box<dyn UpdateChecker> {
    match pm {
        PackageManager::Apt => Box::new(AptChecker),
        PackageManager::Dnf => Box::new(DnfChecker),
        PackageManager::Pacman => Box::new(PacmanChecker),
    }
}

/// Information about available updates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateInfo {
    pub package_manager: String,
    pub total_updates: usize,
    pub security_updates: usize,
    pub packages: Vec<String>,
}

/// Update detector
#[derive(Default)]
pub struct UpdateDetector {}

impl UpdateDetector {
    pub fn new() -> Self {
        Self::default()
    }

    /// Detect package manager on a remote server
    #[instrument(skip(executor))]
    async fn detect_package_manager(executor: &RemoteExecutor) -> Result<PackageManager> {
        for pm in PackageManager::all() {
            let binary = pm.binary();

            // Check if the binary exists using full path
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

    /// Check for updates on a remote server via SSH
    #[instrument(skip(self, ssh_key))]
    pub async fn check_remote_updates(
        &self,
        server_name: &str,
        ssh_host: &str,
        ssh_user: Option<&str>,
        ssh_key: Option<&str>,
    ) -> Result<UpdateInfo> {
        info!(
            server = %server_name,
            host = %ssh_host,
            "Checking remote updates via SSH"
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

        // Get appropriate checker
        let checker = get_checker(&pm);
        let (cmd, args) = checker.check_command();

        // If this is DNF with --cacheonly, refresh the cache in the background
        if matches!(pm, PackageManager::Dnf) && args.contains(&"--cacheonly") {
            let server_clone = server.clone();
            let ssh_key_clone = ssh_key.map(|s| s.to_string());
            tokio::spawn(async move {
                debug!("Refreshing DNF cache in background");
                let bg_executor = RemoteExecutor::for_server(server_clone, ssh_key_clone);
                let _ = bg_executor
                    .execute_command("/usr/bin/dnf", &["makecache", "--quiet"])
                    .await;
            });
        }

        // Execute check command
        let output = executor
            .execute_command(cmd, &args)
            .await
            .map_err(|e| Error::PluginError(format!("Failed to check updates: {}", e)))?;

        // Parse updates
        let packages = checker.parse_updates(&output);
        let total_updates = packages.len();
        let security_updates = packages.iter().filter(|p| p.contains("(security)")).count();

        info!(
            total = total_updates,
            security = security_updates,
            "Updates found"
        );

        Ok(UpdateInfo {
            package_manager: pm.display_name().to_string(),
            total_updates,
            security_updates,
            packages,
        })
    }

    /// Check for updates on the local system
    #[instrument(skip(self))]
    pub async fn check_local_updates(&self) -> Result<UpdateInfo> {
        info!("Checking local system updates");

        // Detect package manager locally
        let pm = self.detect_local_package_manager().await?;
        debug!(package_manager = %pm.display_name(), "Package manager detected");

        // Get appropriate checker
        let checker = get_checker(&pm);
        let (cmd, args) = checker.check_command();

        // Execute command locally using tokio Command
        use tokio::process::Command;

        let output = Command::new(cmd)
            .args(&args)
            .output()
            .await
            .map_err(|e| Error::PluginError(format!("Failed to execute command: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout);

        // Parse updates
        let packages = checker.parse_updates(&stdout);
        let total_updates = packages.len();
        let security_updates = packages.iter().filter(|p| p.contains("(security)")).count();

        info!(
            total = total_updates,
            security = security_updates,
            "Updates found"
        );

        Ok(UpdateInfo {
            package_manager: pm.display_name().to_string(),
            total_updates,
            security_updates,
            packages,
        })
    }

    /// Detect package manager on local system
    async fn detect_local_package_manager(&self) -> Result<PackageManager> {
        use tokio::process::Command;

        for pm in PackageManager::all() {
            let binary = format!("/usr/bin/{}", pm.binary());

            // Check if binary exists and is executable
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apt_parse_updates() {
        let checker = AptChecker;
        let output = r#"Listing...
docker-ce/jammy 5:25.0.0-1~ubuntu.22.04~jammy amd64 [upgradable from: 5:24.0.7-1~ubuntu.22.04~jammy]
linux-image-generic/jammy-security 5.15.0.91.89 amd64 [upgradable from: 5.15.0.89.87]
vim/jammy 2:8.2.3995-1ubuntu2.15 amd64 [upgradable from: 2:8.2.3995-1ubuntu2.14]
"#;

        let updates = checker.parse_updates(output);

        assert_eq!(updates.len(), 3);
        assert_eq!(updates[0], "docker-ce");
        assert_eq!(updates[1], "linux-image-generic (security)");
        assert_eq!(updates[2], "vim");
    }

    #[test]
    fn test_dnf_parse_updates() {
        let checker = DnfChecker;
        let output = r#"docker-ce.x86_64                    3:25.0.0-1.fc39                    docker-ce-stable
kernel.x86_64                       6.6.8-200.fc39                     updates
vim-enhanced.x86_64                 2:9.0.2120-1.fc39                  updates
"#;

        let updates = checker.parse_updates(output);

        assert_eq!(updates.len(), 3);
        assert_eq!(updates[0], "docker-ce");
        assert_eq!(updates[1], "kernel");
        assert_eq!(updates[2], "vim-enhanced");
    }

    #[test]
    fn test_pacman_parse_updates() {
        let checker = PacmanChecker;
        let output = r#"linux 6.6.8.arch1-1 -> 6.6.9.arch1-1
vim 9.0.2120-1 -> 9.0.2121-1
"#;

        let updates = checker.parse_updates(output);

        assert_eq!(updates.len(), 2);
        assert_eq!(updates[0], "linux");
        assert_eq!(updates[1], "vim");
    }
}
