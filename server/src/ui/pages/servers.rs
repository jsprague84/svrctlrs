//! Servers management page - Interactive

use crate::ui::{
    components::Badge,
    server_fns::{get_server_details, list_servers, ServerDetails},
};
use dioxus::prelude::*;

#[component]
pub fn Servers() -> Element {
    // Fetch servers list reactively
    let mut servers_resource = use_resource(|| async move { list_servers().await });

    rsx! {
        div {
            div {
                style: "display: flex; align-items: center; justify-content: space-between; margin-bottom: 24px;",
                h1 { "Servers" }
                p {
                    style: "color: var(--text-secondary);",
                    match &*servers_resource.read_unchecked() {
                        Some(Ok(servers)) => rsx! { "{servers.servers.len()} server(s) configured" },
                        Some(Err(_)) => rsx! { "Error loading servers" },
                        None => rsx! { "Loading servers..." },
                    }
                }
            }

            // Render based on resource state
            match &*servers_resource.read_unchecked() {
                None => rsx! {
                    div {
                        class: "card",
                        style: "text-align: center; padding: 48px;",
                        p {
                            style: "color: var(--text-muted);",
                            "Loading servers..."
                        }
                    }
                },

                Some(Err(e)) => rsx! {
                    div {
                        class: "card",
                        style: "text-align: center; padding: 48px;",
                        p {
                            style: "color: var(--error);",
                            "Failed to load servers: {e}"
                        }
                        button {
                            class: "btn btn-primary",
                            style: "margin-top: 16px;",
                            onclick: move |_| servers_resource.restart(),
                            "Retry"
                        }
                    }
                },

                Some(Ok(server_list)) => {
                    if server_list.servers.is_empty() {
                        rsx! {
                            div {
                                class: "card",
                                style: "text-align: center; padding: 48px;",
                                p {
                                    style: "color: var(--text-muted);",
                                    "No servers configured"
                                }
                                p {
                                    style: "color: var(--text-secondary); margin-top: 8px;",
                                    "Add servers in your config.toml file"
                                }
                            }
                        }
                    } else {
                        rsx! {
                            div {
                                style: "display: grid; gap: 16px;",
                                for server in &server_list.servers {
                                    ServerCard {
                                        key: "{server.name}",
                                        name: server.name.clone(),
                                        ssh_host: server.ssh_host.clone(),
                                        is_local: server.is_local,
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn ServerCard(name: String, ssh_host: String, is_local: bool) -> Element {
    let mut expanded = use_signal(|| false);
    let mut loading = use_signal(|| false);
    let mut details = use_signal(|| None::<ServerDetails>);
    let mut error = use_signal(|| None::<String>);

    rsx! {
        div {
            class: "card",

            // Server header
            div {
                style: "display: flex; align-items: center; justify-content: space-between;",

                div {
                    div {
                        style: "display: flex; align-items: center; gap: 12px; margin-bottom: 8px;",
                        h3 { "{name}" }
                        if is_local {
                            Badge {
                                variant: "info",
                                "Local"
                            }
                        } else {
                            Badge {
                                variant: "success",
                                "Remote"
                            }
                        }
                    }
                    p {
                        style: "color: var(--text-secondary); font-size: 0.875rem;",
                        "🖥️ SSH Host: {ssh_host}"
                    }
                }

                button {
                    class: "btn btn-secondary",
                    disabled: loading(),
                    onclick: {
                        let name = name.clone();
                        move |_| {
                            let name = name.clone();
                            if expanded() {
                                expanded.set(false);
                            } else {
                                spawn(async move {
                                    loading.set(true);
                                    error.set(None);

                                    match get_server_details(name).await {
                                        Ok(server_details) => {
                                            details.set(Some(server_details));
                                            expanded.set(true);
                                        }
                                        Err(e) => {
                                            error.set(Some(e.to_string()));
                                        }
                                    }

                                    loading.set(false);
                                });
                            }
                        }
                    },
                    if loading() {
                        "Loading..."
                    } else if expanded() {
                        "Hide Details"
                    } else {
                        "View Details"
                    }
                }
            }

            // Error message
            if let Some(err) = error() {
                div {
                    style: "margin-top: 16px; padding: 12px; background: var(--bg-secondary); border-radius: 4px; color: var(--error);",
                    "Error: {err}"
                }
            }

            // Expanded details
            if expanded() {
                if let Some(server_details) = details() {
                    div {
                        style: "margin-top: 16px; padding-top: 16px; border-top: 2px solid var(--border-color);",

                        div {
                            style: "display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 16px;",

                            // Status
                            div {
                                div {
                                    style: "color: var(--text-secondary); font-size: 0.875rem; margin-bottom: 4px;",
                                    "Status"
                                }
                                div {
                                    style: "font-weight: bold;",
                                    Badge {
                                        variant: if server_details.status == "online" { "success" } else { "warning" },
                                        "{server_details.status}"
                                    }
                                }
                            }

                            // Uptime
                            if let Some(uptime) = &server_details.uptime {
                                div {
                                    div {
                                        style: "color: var(--text-secondary); font-size: 0.875rem; margin-bottom: 4px;",
                                        "Uptime"
                                    }
                                    div {
                                        style: "font-weight: bold;",
                                        "{uptime}"
                                    }
                                }
                            }

                            // CPU Usage
                            if let Some(cpu) = server_details.cpu_usage {
                                div {
                                    div {
                                        style: "color: var(--text-secondary); font-size: 0.875rem; margin-bottom: 4px;",
                                        "CPU Usage"
                                    }
                                    div {
                                        style: "font-weight: bold;",
                                        "{cpu:.1}%"
                                    }
                                }
                            }

                            // Memory Usage
                            if let Some(memory) = server_details.memory_usage {
                                div {
                                    div {
                                        style: "color: var(--text-secondary); font-size: 0.875rem; margin-bottom: 4px;",
                                        "Memory Usage"
                                    }
                                    div {
                                        style: "font-weight: bold;",
                                        "{memory:.1}%"
                                    }
                                }
                            }

                            // Disk Usage
                            if let Some(disk) = server_details.disk_usage {
                                div {
                                    div {
                                        style: "color: var(--text-secondary); font-size: 0.875rem; margin-bottom: 4px;",
                                        "Disk Usage"
                                    }
                                    div {
                                        style: "font-weight: bold;",
                                        "{disk:.1}%"
                                    }
                                }
                            }
                        }

                        // Note about placeholder data
                        div {
                            style: "margin-top: 16px; padding: 12px; background: var(--bg-secondary); border-radius: 4px; font-size: 0.875rem; color: var(--text-secondary);",
                            "💡 Note: Server metrics will be available once backend endpoints are implemented"
                        }
                    }
                }
            }
        }
    }
}

/// Server detail page (placeholder for future implementation)
#[component]
pub fn ServerDetail(id: String) -> Element {
    rsx! {
        div {
            h1 { "Server Details: {id}" }
            p { "Full server details page coming soon..." }
        }
    }
}
