//! Settings page

use dioxus::prelude::*;
use crate::ui::{components::Badge, DashboardData};

#[component]
pub fn Settings() -> Element {
    // Get data from context
    let data = use_context::<DashboardData>();

    rsx! {
        div {
            h1 { "Settings" }
            p {
                style: "color: var(--text-secondary); margin-bottom: 24px;",
                "Application configuration and system information"
            }

            // System Overview
            div {
                class: "card",
                style: "margin-bottom: 24px;",
                h2 {
                    style: "margin-bottom: 16px;",
                    "System Overview"
                }

                if let Some(status) = &data.status {
                    div {
                        style: "display: grid; gap: 12px;",

                        SettingRow {
                            label: "System Status",
                            value: rsx! {
                                Badge {
                                    variant: if status.status == "running" { "success" } else { "warning" },
                                    "{status.status}"
                                }
                            }
                        }

                        SettingRow {
                            label: "Scheduler",
                            value: rsx! {
                                Badge {
                                    variant: if status.scheduler_running { "success" } else { "error" },
                                    if status.scheduler_running { "Running" } else { "Stopped" }
                                }
                            }
                        }

                        SettingRow {
                            label: "Plugins Loaded",
                            value: rsx! {
                                span { "{status.plugins_loaded}" }
                            }
                        }

                        SettingRow {
                            label: "Servers Configured",
                            value: rsx! {
                                span { "{status.servers}" }
                            }
                        }
                    }
                } else {
                    p {
                        style: "color: var(--text-muted);",
                        "Failed to load system status"
                    }
                }
            }

            // Plugin Configuration
            div {
                class: "card",
                style: "margin-bottom: 24px;",
                h2 {
                    style: "margin-bottom: 16px;",
                    "Plugin Configuration"
                }

                if let Some(plugins) = &data.plugins {
                    if plugins.plugins.is_empty() {
                        p {
                            style: "color: var(--text-muted);",
                            "No plugins loaded"
                        }
                    } else {
                        div {
                            style: "display: grid; gap: 12px;",
                            for plugin in &plugins.plugins {
                                SettingRow {
                                    label: "{plugin.name}",
                                    value: rsx! {
                                        div {
                                            style: "display: flex; gap: 8px; align-items: center;",
                                            Badge {
                                                variant: "success",
                                                "Enabled"
                                            }
                                            span {
                                                style: "color: var(--text-muted); font-size: 0.875rem;",
                                                "v{plugin.version}"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                } else {
                    p {
                        style: "color: var(--text-muted);",
                        "Failed to load plugin configuration"
                    }
                }
            }

            // Server Configuration
            div {
                class: "card",
                style: "margin-bottom: 24px;",
                h2 {
                    style: "margin-bottom: 16px;",
                    "Server Configuration"
                }

                if let Some(servers) = &data.servers {
                    if servers.servers.is_empty() {
                        p {
                            style: "color: var(--text-muted);",
                            "No servers configured"
                        }
                    } else {
                        div {
                            style: "display: grid; gap: 12px;",
                            for server in &servers.servers {
                                SettingRow {
                                    label: "{server.name}",
                                    value: rsx! {
                                        div {
                                            style: "display: flex; gap: 8px; align-items: center;",
                                            span {
                                                style: "font-family: monospace; color: var(--text-secondary); font-size: 0.875rem;",
                                                "{server.ssh_host}"
                                            }
                                            if server.is_local {
                                                Badge {
                                                    variant: "info",
                                                    "Local"
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                } else {
                    p {
                        style: "color: var(--text-muted);",
                        "Failed to load server configuration"
                    }
                }
            }

            // Task Statistics
            div {
                class: "card",
                h2 {
                    style: "margin-bottom: 16px;",
                    "Task Statistics"
                }

                if let Some(tasks) = &data.tasks {
                    div {
                        style: "display: grid; gap: 12px;",

                        SettingRow {
                            label: "Total Tasks",
                            value: rsx! {
                                span { "{tasks.tasks.len()}" }
                            }
                        }

                        SettingRow {
                            label: "Enabled Tasks",
                            value: rsx! {
                                span {
                                    "{tasks.tasks.iter().filter(|t| t.enabled).count()}"
                                }
                            }
                        }

                        SettingRow {
                            label: "Disabled Tasks",
                            value: rsx! {
                                span {
                                    style: "color: var(--text-muted);",
                                    "{tasks.tasks.iter().filter(|t| !t.enabled).count()}"
                                }
                            }
                        }
                    }
                } else {
                    p {
                        style: "color: var(--text-muted);",
                        "Failed to load task statistics"
                    }
                }
            }
        }
    }
}

/// Setting row component for key-value pairs
#[component]
fn SettingRow(label: String, value: Element) -> Element {
    rsx! {
        div {
            style: "display: flex; justify-content: space-between; align-items: center; padding: 12px; border-bottom: 1px solid var(--border-color);",

            span {
                style: "color: var(--text-primary); font-weight: 500;",
                "{label}"
            }

            div {
                {value}
            }
        }
    }
}
