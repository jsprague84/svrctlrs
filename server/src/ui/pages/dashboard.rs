//! Dashboard page - System overview

use dioxus::prelude::*;
use crate::ui::{components::{StatusCard, Badge}, DashboardData};

#[component]
pub fn Dashboard() -> Element {
    // Get data from context (provided by server)
    let data = use_context::<DashboardData>();

    // Extract values from API data
    let (server_count, plugin_count, system_status) = if let Some(status) = &data.status {
        (
            status.servers,
            status.plugins_loaded,
            if status.status == "running" { "healthy" } else { "degraded" }
        )
    } else {
        (0, 0, "unknown")
    };

    // Get real task count from API
    let task_count = if let Some(tasks) = &data.tasks {
        tasks.tasks.len()
    } else {
        0
    };

    rsx! {
        div {
            h1 { "Dashboard" }

            // Status cards grid
            div {
                style: "display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 16px; margin: 24px 0;",

                StatusCard {
                    title: "Servers",
                    value: "{server_count}",
                    icon: "🖥️",
                    variant: "info"
                }

                StatusCard {
                    title: "Plugins",
                    value: "{plugin_count}",
                    icon: "🔌",
                    variant: "primary"
                }

                StatusCard {
                    title: "Tasks",
                    value: "{task_count}",
                    icon: "⚙️",
                    variant: "info"
                }

                StatusCard {
                    title: "Status",
                    value: "{system_status}",
                    icon: if system_status == "healthy" { "✅" } else { "⚠️" },
                    variant: if system_status == "healthy" { "success" } else { "warning" }
                }
            }

            // Active tasks section
            div {
                style: "margin-top: 32px;",
                h2 { "Active Tasks" }

                div {
                    style: "display: grid; grid-template-columns: repeat(auto-fit, minmax(300px, 1fr)); gap: 24px; margin-top: 16px;",

                    // Running now
                    div {
                        class: "card",
                        h3 { "Running Now" }
                        div {
                            style: "margin-top: 12px;",
                            TaskItem {
                                name: "Docker Health Check",
                                status: "running"
                            }
                            TaskItem {
                                name: "Speed Test",
                                status: "running"
                            }
                        }
                    }

                    // Recent results
                    div {
                        class: "card",
                        h3 { "Recent Results" }
                        div {
                            style: "margin-top: 12px;",
                            TaskItem {
                                name: "Updates Check",
                                status: "success"
                            }
                            TaskItem {
                                name: "Weather Forecast",
                                status: "success"
                            }
                        }
                    }
                }
            }

            // Recent notifications
            div {
                style: "margin-top: 32px;",
                h2 { "Recent Notifications" }

                div {
                    class: "card",
                    style: "margin-top: 16px;",

                    NotificationItem {
                        icon: "🟢",
                        message: "Docker: All containers healthy",
                        variant: "success"
                    }

                    NotificationItem {
                        icon: "🟡",
                        message: "Updates: 5 updates available",
                        variant: "warning"
                    }

                    NotificationItem {
                        icon: "🔵",
                        message: "Weather: 72°F, Sunny",
                        variant: "info"
                    }
                }
            }
        }
    }
}

/// Task item component
#[component]
fn TaskItem(name: String, status: String) -> Element {
    let (icon, badge_variant) = match status.as_str() {
        "running" => ("🔄", "info"),
        "success" => ("✓", "success"),
        "failed" => ("✗", "error"),
        _ => ("•", "info"),
    };

    rsx! {
        div {
            style: "display: flex; align-items: center; gap: 12px; padding: 8px 0; border-bottom: 1px solid var(--border-color);",
            span {
                style: "font-size: 1.25rem;",
                "{icon}"
            }
            span {
                style: "flex: 1;",
                "{name}"
            }
            Badge {
                variant: badge_variant,
                "{status}"
            }
        }
    }
}

/// Notification item component
#[component]
fn NotificationItem(icon: String, message: String, variant: String) -> Element {
    rsx! {
        div {
            style: "display: flex; align-items: center; gap: 12px; padding: 12px 0; border-bottom: 1px solid var(--border-color);",
            span {
                style: "font-size: 1.25rem;",
                "{icon}"
            }
            span {
                style: "flex: 1; color: var(--text-primary);",
                "{message}"
            }
            Badge {
                variant: "{variant}",
                "{variant}"
            }
        }
    }
}
