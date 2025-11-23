//! Servers management page

use dioxus::prelude::*;
use crate::ui::{components::Badge, DashboardData};

#[component]
pub fn Servers() -> Element {
    // Get data from context
    let data = use_context::<DashboardData>();

    rsx! {
        div {
            div {
                style: "display: flex; align-items: center; justify-content: space-between; margin-bottom: 24px;",
                h1 { "Servers" }
                p {
                    style: "color: var(--text-secondary);",
                    if let Some(servers) = &data.servers {
                        "{servers.servers.len()} server(s) configured"
                    } else {
                        "Loading servers..."
                    }
                }
            }

            // Server list
            if let Some(servers) = &data.servers {
                if servers.servers.is_empty() {
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
                } else {
                    div {
                        style: "display: grid; gap: 16px;",
                        for server in &servers.servers {
                            div {
                                class: "card",
                                style: "display: flex; align-items: center; justify-content: space-between;",

                                div {
                                    div {
                                        style: "display: flex; align-items: center; gap: 12px; margin-bottom: 8px;",
                                        h3 { "{server.name}" }
                                        if server.is_local {
                                            Badge {
                                                variant: "info",
                                                "Local"
                                            }
                                        }
                                    }
                                    p {
                                        style: "color: var(--text-secondary); font-size: 0.875rem;",
                                        "🖥️ SSH Host: {server.ssh_host}"
                                    }
                                }

                                button {
                                    class: "btn btn-secondary",
                                    "View Details"
                                }
                            }
                        }
                    }
                }
            } else {
                div {
                    class: "card",
                    style: "text-align: center; padding: 48px;",
                    p {
                        style: "color: var(--text-muted);",
                        "Failed to load servers"
                    }
                }
            }
        }
    }
}

#[component]
pub fn ServerDetail(id: String) -> Element {
    rsx! {
        div {
            h1 { "Server: {id}" }
            p {
                style: "color: var(--text-secondary);",
                "Server details and metrics"
            }

            div {
                class: "card",
                style: "margin-top: 24px;",
                p { "Server details will be implemented here" }
            }
        }
    }
}
