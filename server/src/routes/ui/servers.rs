//! Server management routes (updated for new schema)

use askama::Template;
use axum::{
    extract::{Path, State},
    response::{Html, IntoResponse},
    routing::{get, post, put},
    Router,
};
use axum_extra::extract::Form; // Use axum_extra::Form for proper multi-value support
use serde::Deserialize;
use servers_queries::ServerWithDetails;
use svrctlrs_database::{
    models::{CreateServer, UpdateServer},
    queries::{credentials, servers as servers_queries, tags},
};

use super::{get_user_from_session, AppError};
use crate::{state::AppState, templates::*};

/// Detect if we're running inside a Docker/container environment
fn is_containerized() -> bool {
    // Check for /.dockerenv file (Docker-specific)
    if std::path::Path::new("/.dockerenv").exists() {
        return true;
    }
    // Check for container runtime markers in /proc/1/cgroup
    if let Ok(cgroup) = std::fs::read_to_string("/proc/1/cgroup") {
        if cgroup.contains("docker") || cgroup.contains("containerd") || cgroup.contains("lxc") {
            return true;
        }
    }
    // Check for container environment variable
    if std::env::var("container").is_ok() {
        return true;
    }
    false
}

/// Create servers router
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/servers", get(servers_page).post(server_create))
        .route("/servers/new", get(server_form_new))
        .route("/servers/list", get(server_list))
        .route("/servers/test", post(server_test_connection))
        .route("/servers/{id}/edit", get(server_form_edit))
        .route("/servers/{id}", put(server_update).delete(server_delete))
        .route("/servers/{id}/test", post(server_test_by_id))
        .route("/servers/{id}/capabilities", get(server_capabilities))
        .route(
            "/servers/{id}/capabilities/display",
            get(server_capabilities_display),
        )
        .route(
            "/servers/{id}/tags",
            post(server_add_tag).delete(server_remove_tag),
        )
}

/// Servers page handler
async fn servers_page(State(state): State<AppState>) -> Result<Html<String>, AppError> {
    let user = get_user_from_session().await;

    // Load servers, credentials, and tags from database
    let db = state.db().await;
    let servers = servers_queries::list_servers_with_details(db.pool()).await?;
    let credentials_list = credentials::list_credentials(db.pool()).await?;
    let tags_list = tags::get_tags_with_counts(db.pool()).await?;

    // Convert to display models and fetch tags for each server
    let mut servers_display: Vec<ServerDisplay> = Vec::new();
    for server in servers {
        let mut display = server_to_display(&server);

        // Fetch tags for this server
        if let Ok(server_tags) = tags::get_server_tags(db.pool(), server.server.id).await {
            display.tags = server_tags
                .into_iter()
                .map(|t| ServerTagInfo {
                    name: t.name.clone(),
                    color: t.color_or_default(),
                })
                .collect();
        }

        servers_display.push(display);
    }

    let credentials_display: Vec<CredentialDisplay> = credentials_list
        .into_iter()
        .map(|c| credential_to_display(&c))
        .collect();
    let tags_display: Vec<TagDisplay> = tags_list
        .into_iter()
        .map(|t| TagDisplay {
            id: t.tag.id,
            name: t.tag.name.clone(),
            color: t.tag.color_or_default(),
            description: t.tag.description.clone(),
            server_count: t.server_count,
            created_at: t.tag.created_at.to_rfc3339(),
        })
        .collect();

    let template = ServersTemplate {
        user,
        servers: servers_display,
        credentials: credentials_display,
        tags: tags_display,
    };
    Ok(Html(template.render()?))
}

/// Get server list (HTMX component)
async fn server_list(State(state): State<AppState>) -> Result<Html<String>, AppError> {
    let db = state.db().await;
    let servers = servers_queries::list_servers_with_details(db.pool()).await?;

    // Convert servers to display models and fetch tags for each
    let mut servers_display: Vec<ServerDisplay> = Vec::new();
    for server in servers {
        let mut display = server_to_display(&server);

        // Fetch tags for this server
        if let Ok(server_tags) = tags::get_server_tags(db.pool(), server.server.id).await {
            display.tags = server_tags
                .into_iter()
                .map(|t| ServerTagInfo {
                    name: t.name.clone(),
                    color: t.color_or_default(),
                })
                .collect();
        }

        servers_display.push(display);
    }

    let template = ServerListTemplate {
        servers: servers_display,
    };
    Ok(Html(template.render()?))
}

/// New server form
async fn server_form_new(State(state): State<AppState>) -> Result<Html<String>, AppError> {
    let db = state.db().await;
    let credentials_list = credentials::list_credentials(db.pool()).await?;
    let tags_list = tags::get_tags_with_counts(db.pool()).await?;

    let credentials_display: Vec<CredentialDisplay> = credentials_list
        .into_iter()
        .map(|c| credential_to_display(&c))
        .collect();
    let tags_display: Vec<TagDisplay> = tags_list
        .into_iter()
        .map(|t| TagDisplay {
            id: t.tag.id,
            name: t.tag.name.clone(),
            color: t.tag.color_or_default(),
            description: t.tag.description.clone(),
            server_count: t.server_count,
            created_at: t.tag.created_at.to_rfc3339(),
        })
        .collect();

    let template = ServerFormTemplate {
        server: None,
        credentials: credentials_display,
        tags: tags_display,
        selected_tags: vec![],
        error: None,
    };
    Ok(Html(template.render()?))
}

/// Edit server form
async fn server_form_edit(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Html<String>, AppError> {
    let db = state.db().await;
    let server_result = servers_queries::get_server_with_details(db.pool(), id).await;
    let credentials_list = credentials::list_credentials(db.pool()).await?;
    let tags_list = tags::get_tags_with_counts(db.pool()).await?;
    let server_tags = tags::get_server_tags(db.pool(), id)
        .await
        .unwrap_or_default();

    let credentials_display: Vec<CredentialDisplay> = credentials_list
        .into_iter()
        .map(|c| credential_to_display(&c))
        .collect();
    let tags_display: Vec<TagDisplay> = tags_list
        .into_iter()
        .map(|t| TagDisplay {
            id: t.tag.id,
            name: t.tag.name.clone(),
            color: t.tag.color_or_default(),
            description: t.tag.description.clone(),
            server_count: t.server_count,
            created_at: t.tag.created_at.to_rfc3339(),
        })
        .collect();
    let selected_tags: Vec<i64> = server_tags.iter().map(|t| t.id).collect();

    let (server, error) = match server_result {
        Ok(s) => (Some(server_to_display(&s)), None),
        Err(e) => {
            tracing::warn!("Failed to load server {}: {}", id, e);
            (None, Some(format!("Server with ID {} not found", id)))
        }
    };

    let template = ServerFormTemplate {
        server,
        credentials: credentials_display,
        tags: tags_display,
        selected_tags,
        error,
    };
    Ok(Html(template.render()?))
}

/// Create server input (from form)
#[derive(Debug, Deserialize)]
struct CreateServerInput {
    name: String,
    hostname: Option<String>,
    port: Option<i32>,
    username: Option<String>,
    credential_id: Option<String>, // Can be "none" or numeric ID
    description: Option<String>,
    is_local: Option<String>, // checkbox "on" or None
    enabled: Option<String>,  // checkbox "on" or None
    #[serde(default)] // Empty vec when no checkboxes selected
    tag_ids: Vec<i64>, // Multi-select tag IDs (matches form field name)
}

/// Create server handler
async fn server_create(
    State(state): State<AppState>,
    Form(input): Form<CreateServerInput>,
) -> Result<Html<String>, AppError> {
    // Parse credential ID
    let credential_id = match input.credential_id.as_deref() {
        Some("none") | Some("") | None => None,
        Some(id_str) => id_str.parse::<i64>().ok(),
    };

    // Parse checkboxes
    let is_local = input.is_local.is_some();
    let enabled = input.enabled.unwrap_or_else(|| "on".to_string()) == "on";

    // Create server
    let create_server = CreateServer {
        name: input.name.clone(),
        hostname: input.hostname.clone(),
        port: input.port.unwrap_or(22),
        username: input.username.clone(),
        credential_id,
        description: input.description.clone(),
        is_local,
        enabled,
        metadata: None,
    };

    // Validate
    if let Err(e) = create_server.validate() {
        let db = state.db().await;
        let _credentials_list = credentials::list_credentials(db.pool()).await?;
        let _tags_list = tags::list_tags(db.pool()).await?;

        return Ok(Html(format!(
            r#"<div class="alert alert-error alert-auto-dismiss">Validation error: {}</div>"#,
            e
        )));
    }

    // Save to database
    tracing::info!(
        "Creating server: {} @ {:?}",
        create_server.name,
        create_server.hostname
    );
    let db = state.db().await;

    match servers_queries::create_server(db.pool(), &create_server).await {
        Ok(server_id) => {
            // Add tags if any were selected
            for tag_id in &input.tag_ids {
                let _ = tags::add_server_tag(db.pool(), server_id, *tag_id).await;
            }

            // Success - return updated list with success message
            let servers = servers_queries::list_servers_with_details(db.pool()).await?;

            // Convert servers to display models and fetch tags for each
            let mut servers_display: Vec<ServerDisplay> = Vec::new();
            for server in servers {
                let mut display = server_to_display(&server);

                // Fetch tags for this server
                if let Ok(server_tags) = tags::get_server_tags(db.pool(), server.server.id).await {
                    display.tags = server_tags
                        .into_iter()
                        .map(|t| ServerTagInfo {
                            name: t.name.clone(),
                            color: t.color_or_default(),
                        })
                        .collect();
                }

                servers_display.push(display);
            }

            let template = ServerListTemplate {
                servers: servers_display,
            };
            let list_html = template.render()?;

            Ok(Html(format!(
                r#"<div class="alert alert-success alert-auto-dismiss">Server '{}' created successfully!</div>{}"#,
                create_server.name, list_html
            )))
        }
        Err(e) => {
            // Check for duplicate name error
            let error_msg = e.to_string();
            if error_msg.contains("UNIQUE constraint") && error_msg.contains("servers.name") {
                Ok(Html(format!(
                    r#"<div class="alert alert-error alert-auto-dismiss">A server with the name '{}' already exists. Please use a different name.</div>"#,
                    create_server.name
                )))
            } else {
                Err(e.into())
            }
        }
    }
}

/// Update server input (from form)
#[derive(Debug, Deserialize)]
struct UpdateServerInput {
    name: Option<String>,
    hostname: Option<String>,
    port: Option<i32>,
    username: Option<String>,
    credential_id: Option<String>,
    description: Option<String>,
    enabled: Option<String>,
    #[serde(default)] // Empty vec when no checkboxes selected
    tag_ids: Vec<i64>, // Multi-select tag IDs (matches form field name)
}

/// Update server handler
async fn server_update(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Form(input): Form<UpdateServerInput>,
) -> Result<Html<String>, AppError> {
    let db = state.db().await;

    // Parse credential ID
    let credential_id = match input.credential_id.as_deref() {
        Some("none") | Some("") => None,
        Some(id_str) => id_str.parse::<i64>().ok(),
        None => None,
    };

    // Get server name for success message
    let server_name = if let Some(ref name) = input.name {
        name.clone()
    } else {
        servers_queries::get_server_with_details(db.pool(), id)
            .await?
            .server
            .name
    };

    let update_server = UpdateServer {
        name: input.name,
        hostname: input.hostname,
        port: input.port,
        username: input.username,
        credential_id,
        description: input.description,
        enabled: Some(input.enabled.is_some() && input.enabled.as_deref() == Some("on")), // Checkbox: Some("on") = true, None = false
        os_type: None,
        os_distro: None,
        package_manager: None,
        docker_available: None,
        systemd_available: None,
        metadata: None,
        last_error: None,
    };

    match servers_queries::update_server(db.pool(), id, &update_server).await {
        Ok(_) => {
            // Update tags (always, even if empty to clear all tags)
            let new_tag_ids = &input.tag_ids;

            // Get current tags
            let current_tags = tags::get_server_tags(db.pool(), id)
                .await
                .unwrap_or_default();
            let current_tag_ids: Vec<i64> = current_tags.iter().map(|t| t.id).collect();

            // Remove tags that are no longer selected
            for current_tag_id in &current_tag_ids {
                if !new_tag_ids.contains(current_tag_id) {
                    let _ = tags::remove_server_tag(db.pool(), id, *current_tag_id).await;
                }
            }

            // Add new tags
            for new_tag_id in new_tag_ids {
                if !current_tag_ids.contains(new_tag_id) {
                    let _ = tags::add_server_tag(db.pool(), id, *new_tag_id).await;
                }
            }

            // Success - return updated list with tags loaded
            let servers = servers_queries::list_servers_with_details(db.pool()).await?;

            // Convert servers to display models and fetch tags for each
            let mut servers_display: Vec<ServerDisplay> = Vec::new();
            for server in servers {
                let mut display = server_to_display(&server);

                // Fetch tags for this server
                if let Ok(server_tags) = tags::get_server_tags(db.pool(), server.server.id).await {
                    display.tags = server_tags
                        .into_iter()
                        .map(|t| ServerTagInfo {
                            name: t.name.clone(),
                            color: t.color_or_default(),
                        })
                        .collect();
                }

                servers_display.push(display);
            }

            let template = ServerListTemplate {
                servers: servers_display,
            };
            let list_html = template.render()?;

            Ok(Html(format!(
                r#"<div class="alert alert-success alert-auto-dismiss">Server '{}' updated successfully!</div>{}"#,
                server_name, list_html
            )))
        }
        Err(e) => {
            let error_msg = e.to_string();
            if error_msg.contains("UNIQUE constraint") && error_msg.contains("servers.name") {
                Ok(Html(
                    r#"<div class="alert alert-error alert-auto-dismiss">A server with that name already exists. Please use a different name.</div>"#.to_string()
                ))
            } else {
                Err(e.into())
            }
        }
    }
}

/// Delete server handler
async fn server_delete(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, AppError> {
    let db = state.db().await;
    let server_name = servers_queries::get_server_with_details(db.pool(), id)
        .await
        .map(|s| s.server.name)
        .unwrap_or_else(|_| format!("Server {}", id));

    // Delete from database
    tracing::info!("Deleting server {}", id);
    servers_queries::delete_server(db.pool(), id).await?;

    Ok(Html(format!(
        r#"<div class="alert alert-success alert-auto-dismiss">Server '{}' deleted successfully!</div>"#,
        server_name
    )))
}

/// Test connection input
#[derive(Debug, Deserialize)]
struct TestConnectionInput {
    hostname: String,
    port: Option<i32>,
    username: Option<String>,
    credential_id: Option<String>, // Can be empty string or numeric ID
}

/// Test SSH connection handler
async fn server_test_connection(
    State(state): State<AppState>,
    Form(input): Form<TestConnectionInput>,
) -> Result<Html<String>, AppError> {
    use crate::ssh::{test_connection, SshConfig};
    use std::time::Duration;

    let port = input.port.unwrap_or(22);
    let username = input.username.unwrap_or_else(|| "root".to_string());

    tracing::info!(
        "Testing SSH connection to {}@{}:{}",
        username,
        input.hostname,
        port
    );

    // Load credential if specified
    let db = state.db().await;
    let key_path = match input.credential_id.as_deref() {
        Some(id_str) if !id_str.is_empty() => {
            if let Ok(cred_id) = id_str.parse::<i64>() {
                match credentials::get_credential(db.pool(), cred_id).await {
                    Ok(cred) => {
                        tracing::info!("Using credential '{}' for test connection", cred.name);
                        Some(cred.value)
                    }
                    Err(e) => {
                        tracing::warn!("Failed to load credential {}: {}", cred_id, e);
                        None
                    }
                }
            } else {
                None
            }
        }
        _ => None,
    };

    let config = SshConfig {
        host: input.hostname.clone(),
        port: port as u16,
        username: username.clone(),
        key_path,
        timeout: Duration::from_secs(10),
    };

    match test_connection(&config).await {
        Ok(output) => Ok(Html(format!(
            r#"<div class="alert alert-success alert-auto-dismiss">✓ Connection successful to {}@{}:{}<br><small>{}</small></div>"#,
            username, input.hostname, port, output
        ))),
        Err(e) => Ok(Html(format!(
            r#"<div class="alert alert-error alert-auto-dismiss">✗ Connection failed to {}@{}:{}<br><small>{}</small></div>"#,
            username, input.hostname, port, e
        ))),
    }
}

/// Test connection for existing server
async fn server_test_by_id(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Html<String>, AppError> {
    use crate::ssh::{test_connection, SshConfig};
    use std::time::Duration;

    let db = state.db().await;
    let server = servers_queries::get_server_with_details(db.pool(), id).await?;

    tracing::info!("Testing connection for server: {}", server.server.name);

    // Handle local server
    if server.server.is_local {
        return Ok(Html(
            r#"<div class="alert alert-success alert-auto-dismiss">✅ Local server - no SSH connection needed</div>"#
                .to_string(),
        ));
    }

    // Validate server configuration
    let hostname = match &server.server.hostname {
        Some(h) if !h.is_empty() => h.clone(),
        _ => {
            return Ok(Html(
                r#"<div class="alert alert-error alert-auto-dismiss">❌ No hostname configured</div>"#.to_string(),
            ));
        }
    };

    let username = match &server.server.username {
        Some(u) if !u.is_empty() => u.clone(),
        _ => {
            return Ok(Html(
                r#"<div class="alert alert-error alert-auto-dismiss">❌ No username configured</div>"#.to_string(),
            ));
        }
    };

    // Get credential if specified
    let key_path = if let Some(cred_id) = server.server.credential_id {
        match credentials::get_credential(db.pool(), cred_id).await {
            Ok(cred) => Some(cred.value),
            Err(e) => {
                tracing::warn!("Failed to load credential {}: {}", cred_id, e);
                None
            }
        }
    } else {
        None
    };

    // Create SSH config
    let ssh_config = SshConfig {
        host: hostname.clone(),
        port: server.server.port as u16,
        username: username.clone(),
        key_path,
        timeout: Duration::from_secs(10),
    };

    // Test connection
    match test_connection(&ssh_config).await {
        Ok(output) => {
            // Update server status (success - no error)
            let _ = servers_queries::update_server_status(db.pool(), id, None).await;

            Ok(Html(format!(
                r#"<div class="alert alert-success alert-auto-dismiss"
                        data-refresh-target="server-{}-capabilities"
                        data-refresh-url="/servers/{}/capabilities/display">
                    ✅ Connection successful!
                    <br><small class="text-secondary">{}</small>
                </div>"#,
                id, id, output
            )))
        }
        Err(e) => {
            tracing::warn!(
                "SSH connection test failed for {}: {}",
                server.server.name,
                e
            );

            // Update server status (failure - with error)
            let _ = servers_queries::update_server_status(db.pool(), id, Some(e.to_string())).await;

            Ok(Html(format!(
                r#"<div class="alert alert-error alert-auto-dismiss"
                        data-refresh-target="server-{}-capabilities"
                        data-refresh-url="/servers/{}/capabilities/display">
                    ❌ Connection failed: {}
                    <br><small class="text-secondary">Check hostname, port, username, and SSH key</small>
                </div>"#,
                id, id, e
            )))
        }
    }
}

/// Get server capabilities (detect and update)
async fn server_capabilities(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Html<String>, AppError> {
    use crate::ssh::{execute_command, SshConfig};
    use std::time::Duration;

    let db = state.db().await;
    let server = servers_queries::get_server_with_details(db.pool(), id).await?;

    tracing::info!("Detecting capabilities for server: {}", server.server.name);

    // Detection script - used for both local and remote servers
    let detection_script = r#"
#!/bin/bash
echo "OS_RELEASE_START"
cat /etc/os-release 2>/dev/null || echo "unknown"
echo "OS_RELEASE_END"

echo "DOCKER=$(command -v docker >/dev/null 2>&1 && echo '1' || echo '0')"
echo "SYSTEMD=$(command -v systemctl >/dev/null 2>&1 && echo '1' || echo '0')"
echo "APT=$(command -v apt >/dev/null 2>&1 && echo '1' || echo '0')"
echo "DNF=$(command -v dnf >/dev/null 2>&1 && echo '1' || echo '0')"
echo "PACMAN=$(command -v pacman >/dev/null 2>&1 && echo '1' || echo '0')"
echo "ZFS=$(command -v zpool >/dev/null 2>&1 && echo '1' || echo '0')"
echo "LVM=$(command -v lvm >/dev/null 2>&1 && echo '1' || echo '0')"
echo "PYTHON3=$(command -v python3 >/dev/null 2>&1 && echo '1' || echo '0')"
echo "NODE=$(command -v node >/dev/null 2>&1 && echo '1' || echo '0')"
echo "GIT=$(command -v git >/dev/null 2>&1 && echo '1' || echo '0')"
echo "PODMAN=$(command -v podman >/dev/null 2>&1 && echo '1' || echo '0')"
echo "SNAP=$(command -v snap >/dev/null 2>&1 && echo '1' || echo '0')"
echo "FLATPAK=$(command -v flatpak >/dev/null 2>&1 && echo '1' || echo '0')"
echo "PROXMOX=$(command -v pveversion >/dev/null 2>&1 && echo '1' || echo '0')"
if command -v pveversion >/dev/null 2>&1; then
    PROXMOX_VER=$(pveversion 2>/dev/null | head -1 | sed 's/pve-manager\///' | cut -d'/' -f1)
    echo "PROXMOX_VERSION=$PROXMOX_VER"
fi
"#;

    // Handle local server - use tokio::process::Command
    if server.server.is_local {
        use tokio::process::Command;

        tracing::debug!("Running local capability detection script");

        let output = Command::new("bash")
            .arg("-c")
            .arg(detection_script)
            .output()
            .await
            .map_err(|e| {
                tracing::error!("Failed to execute local detection script: {}", e);
                AppError::InternalError(format!("Failed to run detection script: {}", e))
            })?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        tracing::debug!("Local capability detection output: {}", stdout);

        // Parse capabilities from output
        let mut detected_caps = Vec::new();
        let mut proxmox_version: Option<String> = None;

        // First pass: collect Proxmox version if present
        for line in stdout.lines() {
            if line.starts_with("PROXMOX_VERSION=") {
                proxmox_version = Some(line.trim_start_matches("PROXMOX_VERSION=").to_string());
            }
        }

        // Second pass: detect capabilities
        for line in stdout.lines() {
            let (cap_name, cap_available, version) = if line.starts_with("DOCKER=1") {
                ("docker", true, None)
            } else if line.starts_with("SYSTEMD=1") {
                ("systemd", true, None)
            } else if line.starts_with("APT=1") {
                ("apt", true, None)
            } else if line.starts_with("DNF=1") {
                ("dnf", true, None)
            } else if line.starts_with("PACMAN=1") {
                ("pacman", true, None)
            } else if line.starts_with("ZFS=1") {
                ("zfs", true, None)
            } else if line.starts_with("LVM=1") {
                ("lvm", true, None)
            } else if line.starts_with("PYTHON3=1") {
                ("python3", true, None)
            } else if line.starts_with("NODE=1") {
                ("node", true, None)
            } else if line.starts_with("GIT=1") {
                ("git", true, None)
            } else if line.starts_with("PODMAN=1") {
                ("podman", true, None)
            } else if line.starts_with("SNAP=1") {
                ("snap", true, None)
            } else if line.starts_with("FLATPAK=1") {
                ("flatpak", true, None)
            } else if line.starts_with("PROXMOX=1") {
                ("proxmox", true, proxmox_version.clone())
            } else {
                continue;
            };

            detected_caps.push(cap_name);
            servers_queries::set_server_capability(
                db.pool(),
                id,
                cap_name,
                cap_available,
                version.as_deref(),
            )
            .await?;
        }

        // Parse OS info - prefer PRETTY_NAME for fuller version string
        if let Some(os_start) = stdout.find("OS_RELEASE_START") {
            if let Some(os_end) = stdout.find("OS_RELEASE_END") {
                let os_info = &stdout[os_start + 16..os_end];
                // Try PRETTY_NAME first (e.g., "Ubuntu 24.04 LTS")
                let os_distro = os_info
                    .lines()
                    .find(|l| l.starts_with("PRETTY_NAME="))
                    .map(|l| l.trim_start_matches("PRETTY_NAME=").trim_matches('"'))
                    // Fall back to ID if PRETTY_NAME not found
                    .or_else(|| {
                        os_info
                            .lines()
                            .find(|l| l.starts_with("ID="))
                            .map(|l| l.trim_start_matches("ID=").trim_matches('"'))
                    });

                if let Some(distro) = os_distro {
                    let update = UpdateServer {
                        os_distro: Some(distro.to_string()),
                        ..Default::default()
                    };
                    let _ = servers_queries::update_server(db.pool(), id, &update).await;
                }
            }
        }

        tracing::info!(
            "Detected {} capabilities for local server '{}'",
            detected_caps.len(),
            server.server.name
        );

        let capabilities = servers_queries::get_server_capabilities(db.pool(), id).await?;
        let template = ServerCapabilitiesTemplate {
            server_id: id,
            server: server_to_display(&server),
            capabilities: capabilities
                .into_iter()
                .map(|c| ServerCapabilityDisplay {
                    capability: c.capability,
                    available: c.available,
                    version: c.version,
                    detected_at: c
                        .detected_at
                        .with_timezone(&chrono::Local)
                        .format("%Y-%m-%d %H:%M")
                        .to_string(),
                })
                .collect(),
            should_expand: true, // Detect clicked - show expanded and auto-collapse
            is_containerized: is_containerized(),
            is_local_server: server.server.is_local,
        };
        return Ok(Html(template.render()?));
    }

    // Remote server - detect via SSH
    let hostname = match &server.server.hostname {
        Some(h) if !h.is_empty() => h.clone(),
        _ => {
            return Ok(Html(
                r#"<div class="alert alert-error alert-auto-dismiss">❌ No hostname configured</div>"#.to_string(),
            ));
        }
    };

    let username = match &server.server.username {
        Some(u) if !u.is_empty() => u.clone(),
        _ => {
            return Ok(Html(
                r#"<div class="alert alert-error alert-auto-dismiss">❌ No username configured</div>"#.to_string(),
            ));
        }
    };

    // Get credential if specified
    let key_path = if let Some(cred_id) = server.server.credential_id {
        match credentials::get_credential(db.pool(), cred_id).await {
            Ok(cred) => Some(cred.value),
            Err(e) => {
                tracing::warn!("Failed to load credential {}: {}", cred_id, e);
                None
            }
        }
    } else {
        None
    };

    // Create SSH config
    let ssh_config = SshConfig {
        host: hostname.clone(),
        port: server.server.port as u16,
        username: username.clone(),
        key_path,
        timeout: Duration::from_secs(30),
    };

    // Use the same detection script defined at the top of the function

    match execute_command(&ssh_config, detection_script).await {
        Ok(result) => {
            let output = result.stdout;
            tracing::debug!("Capability detection output: {}", output);

            // Parse capabilities from output
            let mut detected_caps = Vec::new();
            let mut proxmox_version: Option<String> = None;

            // First pass: collect Proxmox version if present
            for line in output.lines() {
                if line.starts_with("PROXMOX_VERSION=") {
                    proxmox_version = Some(line.trim_start_matches("PROXMOX_VERSION=").to_string());
                }
            }

            // Second pass: detect capabilities
            for line in output.lines() {
                let (cap_name, cap_available, version) = if line.starts_with("DOCKER=1") {
                    ("docker", true, None)
                } else if line.starts_with("SYSTEMD=1") {
                    ("systemd", true, None)
                } else if line.starts_with("APT=1") {
                    ("apt", true, None)
                } else if line.starts_with("DNF=1") {
                    ("dnf", true, None)
                } else if line.starts_with("PACMAN=1") {
                    ("pacman", true, None)
                } else if line.starts_with("ZFS=1") {
                    ("zfs", true, None)
                } else if line.starts_with("LVM=1") {
                    ("lvm", true, None)
                } else if line.starts_with("PYTHON3=1") {
                    ("python3", true, None)
                } else if line.starts_with("NODE=1") {
                    ("node", true, None)
                } else if line.starts_with("GIT=1") {
                    ("git", true, None)
                } else if line.starts_with("PODMAN=1") {
                    ("podman", true, None)
                } else if line.starts_with("SNAP=1") {
                    ("snap", true, None)
                } else if line.starts_with("FLATPAK=1") {
                    ("flatpak", true, None)
                } else if line.starts_with("PROXMOX=1") {
                    ("proxmox", true, proxmox_version.clone())
                } else {
                    continue;
                };

                detected_caps.push(cap_name);
                servers_queries::set_server_capability(
                    db.pool(),
                    id,
                    cap_name,
                    cap_available,
                    version.as_deref(),
                )
                .await?;
            }

            // Parse OS info - prefer PRETTY_NAME for fuller version string
            if let Some(os_start) = output.find("OS_RELEASE_START") {
                if let Some(os_end) = output.find("OS_RELEASE_END") {
                    let os_info = &output[os_start + 16..os_end];
                    // Try PRETTY_NAME first (e.g., "Ubuntu 24.04 LTS")
                    let os_distro = os_info
                        .lines()
                        .find(|l| l.starts_with("PRETTY_NAME="))
                        .map(|l| l.trim_start_matches("PRETTY_NAME=").trim_matches('"'))
                        // Fall back to ID if PRETTY_NAME not found
                        .or_else(|| {
                            os_info
                                .lines()
                                .find(|l| l.starts_with("ID="))
                                .map(|l| l.trim_start_matches("ID=").trim_matches('"'))
                        });

                    if let Some(distro) = os_distro {
                        let update = UpdateServer {
                            os_distro: Some(distro.to_string()),
                            ..Default::default()
                        };
                        let _ = servers_queries::update_server(db.pool(), id, &update).await;
                    }
                }
            }

            // Update last seen (success - no error)
            let _ = servers_queries::update_server_status(db.pool(), id, None).await;

            tracing::info!(
                "Detected {} capabilities for server '{}'",
                detected_caps.len(),
                server.server.name
            );

            // Return updated capabilities
            let capabilities = servers_queries::get_server_capabilities(db.pool(), id).await?;
            let template = ServerCapabilitiesTemplate {
                server_id: id,
                server: server_to_display(&server),
                capabilities: capabilities
                    .into_iter()
                    .map(|c| ServerCapabilityDisplay {
                        capability: c.capability,
                        available: c.available,
                        version: c.version,
                        detected_at: c
                            .detected_at
                            .with_timezone(&chrono::Local)
                            .format("%Y-%m-%d %H:%M")
                            .to_string(),
                    })
                    .collect(),
                should_expand: true, // Detect clicked - show expanded and auto-collapse
                is_containerized: is_containerized(),
                is_local_server: server.server.is_local,
            };
            Ok(Html(template.render()?))
        }
        Err(e) => {
            tracing::warn!(
                "Capability detection failed for {}: {}",
                server.server.name,
                e
            );
            Ok(Html(format!(
                r#"<div class="alert alert-error alert-auto-dismiss">
                    ❌ Capability detection failed: {}
                    <br><small class="text-secondary">Ensure SSH connection is working</small>
                </div>"#,
                e
            )))
        }
    }
}

/// Display existing server capabilities (without re-detecting)
async fn server_capabilities_display(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Html<String>, AppError> {
    let db = state.db().await;
    let server = servers_queries::get_server_with_details(db.pool(), id).await?;

    // Just fetch existing capabilities from database without re-detecting
    let capabilities = servers_queries::get_server_capabilities(db.pool(), id).await?;

    let template = ServerCapabilitiesTemplate {
        server_id: id,
        server: server_to_display(&server),
        capabilities: capabilities
            .into_iter()
            .map(|c| ServerCapabilityDisplay {
                capability: c.capability,
                available: c.available,
                version: c.version,
                detected_at: c
                    .detected_at
                    .with_timezone(&chrono::Local)
                    .format("%Y-%m-%d %H:%M")
                    .to_string(),
            })
            .collect(),
        should_expand: false, // Page load - start collapsed
        is_containerized: is_containerized(),
        is_local_server: server.server.is_local,
    };
    Ok(Html(template.render()?))
}

/// Add tag to server input
#[derive(Debug, Deserialize)]
struct AddTagInput {
    tag_id: i64,
}

/// Add tag to server
async fn server_add_tag(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Form(input): Form<AddTagInput>,
) -> Result<Html<String>, AppError> {
    let db = state.db().await;
    tags::add_server_tag(db.pool(), id, input.tag_id).await?;

    Ok(Html(
        r#"<div class="alert alert-success alert-auto-dismiss">Tag added successfully!</div>"#
            .to_string(),
    ))
}

/// Remove tag from server input
#[derive(Debug, Deserialize)]
struct RemoveTagInput {
    tag_id: i64,
}

/// Remove tag from server
async fn server_remove_tag(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Form(input): Form<RemoveTagInput>,
) -> Result<Html<String>, AppError> {
    let db = state.db().await;
    tags::remove_server_tag(db.pool(), id, input.tag_id).await?;

    Ok(Html(
        r#"<div class="alert alert-success alert-auto-dismiss">Tag removed successfully!</div>"#
            .to_string(),
    ))
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Convert DB server to UI display model
fn server_to_display(server_with_details: &ServerWithDetails) -> ServerDisplay {
    let server = &server_with_details.server;
    ServerDisplay {
        id: server.id,
        name: server.name.clone(),
        hostname: server.hostname.clone().unwrap_or_default(), // Convert Option → String
        host: server.hostname.clone().unwrap_or_default(),     // Alias for hostname
        port: server.port,
        username: server.username.clone().unwrap_or_default(), // Convert Option → String
        credential_id: server.credential_id,
        credential_name: String::new(), // TODO: Fetch from join
        description: server.description.clone().unwrap_or_default(), // Convert Option → String
        connection_type: if server.is_local {
            "local".to_string()
        } else {
            "ssh".to_string()
        },
        connection_string: String::new(), // TODO: Build from server fields
        is_local: server.is_local,
        enabled: server.enabled,
        os_type: server.os_type.clone().unwrap_or_default(), // Convert Option → String
        os_distro: server.os_distro.clone().unwrap_or_default(), // Convert Option → String
        os_version: String::new(), // TODO: Extract from os_distro or metadata
        package_manager: server.package_manager.clone().unwrap_or_default(), // Convert Option → String
        docker_available: server.docker_available,
        systemd_available: server.systemd_available,
        last_seen_at: server
            .last_seen_at
            .map(|t| t.to_rfc3339())
            .unwrap_or_default(), // Convert Option → String
        tag_ids: vec![],      // Will be filled by join query if needed
        tags: vec![],         // Will be filled by join query if needed
        capabilities: vec![], // TODO: Fetch from server_capabilities table
        created_at: server.created_at.to_rfc3339(),
    }
}

/// Convert DB credential to UI display model
fn credential_to_display(cred: &svrctlrs_database::models::Credential) -> CredentialDisplay {
    let type_display = match cred.credential_type_str.as_str() {
        "ssh_key" => "SSH Key",
        "api_token" => "API Token",
        "password" => "Password",
        "certificate" => "Certificate",
        _ => "Unknown",
    };

    let value_preview = if cred.is_ssh_key() {
        cred.value.clone() // Show path
    } else {
        format!("{}...", &cred.value.chars().take(8).collect::<String>())
    };

    CredentialDisplay {
        id: cred.id,
        name: cred.name.clone(),
        credential_type: cred.credential_type_str.clone(),
        credential_type_display: type_display.to_string(),
        auth_type: cred.credential_type_str.clone(), // Alias for templates
        description: cred.description.clone().unwrap_or_default(), // Convert Option → String
        value_preview,
        username: cred.username.clone().unwrap_or_default(), // Convert Option → String
        server_count: 0, // TODO: Query actual count from database
        created_at: cred.created_at.to_rfc3339(),
        updated_at: cred.updated_at.to_rfc3339(),
    }
}
