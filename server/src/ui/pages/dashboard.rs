//! Dashboard page - System overview with real-time data

use dioxus::prelude::*;
use crate::ui::{
    components::{StatusCard, Badge},
    server_fns::{get_status, list_plugins, list_servers, list_tasks},
};

#[component]
pub fn Dashboard() -> Element {
    // Fetch data reactively using use_resource
    let status_resource = use_resource(|| async move { get_status().await });
    let plugins_resource = use_resource(|| async move { list_plugins().await });
    let servers_resource = use_resource(|| async move { list_servers().await });
    let tasks_resource = use_resource(|| async move { list_tasks().await });

    // Extract counts and status
    let (server_count, plugin_count, system_status, scheduler_running) = match &*status_resource.read_unchecked() {
        Some(Ok(status)) => (
            status.servers,
            status.plugins_loaded,
            if status.status == "running" { "healthy" } else { "degraded" },
            status.scheduler_running,
        ),
        _ => (0, 0, "loading", false),
    };

    let task_count = match &*tasks_resource.read_unchecked() {
        Some(Ok(tasks)) => tasks.tasks.len(),
        _ => 0,
    };

    let enabled_tasks_count = match &*tasks_resource.read_unchecked() {
        Some(Ok(tasks)) => tasks.tasks.iter().filter(|t| t.enabled).count(),
        _ => 0,
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
                    icon: if system_status == "healthy" { "✅" } else if system_status == "loading" { "⏳" } else { "⚠️" },
                    variant: if system_status == "healthy" { "success" } else if system_status == "loading" { "info" } else { "warning" }
                }
            }

            // Quick stats
            div {
                style: "display: grid; grid-template-columns: repeat(auto-fit, minmax(300px, 1fr)); gap: 16px; margin: 24px 0;",

                div {
                    class: "card",
                    h3 { "Scheduler Status" }
                    div {
                        style: "margin-top: 12px; display: flex; align-items: center; gap: 12px;",
                        span {
                            style: "font-size: 2rem;",
                            if scheduler_running { "🟢" } else { "🔴" }
                        }
                        div {
                            div {
                                style: "font-size: 1.25rem; font-weight: bold;",
                                if scheduler_running { "Running" } else { "Stopped" }
                            }
                            div {
                                style: "color: var(--text-secondary); font-size: 0.875rem;",
                                "{enabled_tasks_count} / {task_count} tasks enabled"
                            }
                        }
                    }
                }

                div {
                    class: "card",
                    h3 { "Plugin Status" }
                    div {
                        style: "margin-top: 12px;",
                        match &*plugins_resource.read_unchecked() {
                            Some(Ok(plugins)) => rsx! {
                                for plugin in &plugins.plugins {
                                    div {
                                        style: "display: flex; align-items: center; gap: 8px; padding: 8px 0; border-bottom: 1px solid var(--border-color);",
                                        span { "🔌" }
                                        span {
                                            style: "flex: 1;",
                                            "{plugin.name}"
                                        }
                                        Badge {
                                            variant: "success",
                                            "Active"
                                        }
                                    }
                                }
                            },
                            Some(Err(_)) => rsx! {
                                p {
                                    style: "color: var(--text-muted);",
                                    "Failed to load plugins"
                                }
                            },
                            None => rsx! {
                                p {
                                    style: "color: var(--text-muted);",
                                    "Loading plugins..."
                                }
                            },
                        }
                    }
                }

                div {
                    class: "card",
                    h3 { "Server Status" }
                    div {
                        style: "margin-top: 12px;",
                        match &*servers_resource.read_unchecked() {
                            Some(Ok(servers)) => rsx! {
                                for server in &servers.servers {
                                    div {
                                        style: "display: flex; align-items: center; gap: 8px; padding: 8px 0; border-bottom: 1px solid var(--border-color);",
                                        span {
                                            if server.is_local { "🏠" } else { "☁️" }
                                        }
                                        div {
                                            style: "flex: 1;",
                                            div { "{server.name}" }
                                            div {
                                                style: "font-size: 0.75rem; color: var(--text-secondary); font-family: monospace;",
                                                "{server.ssh_host}"
                                            }
                                        }
                                        Badge {
                                            variant: "success",
                                            "Online"
                                        }
                                    }
                                }
                            },
                            Some(Err(_)) => rsx! {
                                p {
                                    style: "color: var(--text-muted);",
                                    "Failed to load servers"
                                }
                            },
                            None => rsx! {
                                p {
                                    style: "color: var(--text-muted);",
                                    "Loading servers..."
                                }
                            },
                        }
                    }
                }
            }

            // Recent tasks
            div {
                style: "margin-top: 32px;",
                h2 { "Scheduled Tasks" }

                div {
                    class: "card",
                    style: "margin-top: 16px;",

                    match &*tasks_resource.read_unchecked() {
                        Some(Ok(tasks)) => {
                            if tasks.tasks.is_empty() {
                                rsx! {
                                    p {
                                        style: "color: var(--text-muted); padding: 24px; text-align: center;",
                                        "No scheduled tasks configured"
                                    }
                                }
                            } else {
                                rsx! {
                                    for (idx, task) in tasks.tasks.iter().take(5).enumerate() {
                                        TaskItem {
                                            key: "{idx}",
                                            name: task.description.clone(),
                                            schedule: task.schedule.clone(),
                                            enabled: task.enabled,
                                        }
                                    }
                                    if tasks.tasks.len() > 5 {
                                        div {
                                            style: "padding: 16px; text-align: center; border-top: 1px solid var(--border-color);",
                                            a {
                                                href: "/tasks",
                                                style: "color: var(--accent-primary); text-decoration: none;",
                                                "View all {tasks.tasks.len()} tasks →"
                                            }
                                        }
                                    }
                                }
                            }
                        },
                        Some(Err(_)) => rsx! {
                            p {
                                style: "color: var(--text-muted); padding: 24px; text-align: center;",
                                "Failed to load tasks"
                            }
                        },
                        None => rsx! {
                            p {
                                style: "color: var(--text-muted); padding: 24px; text-align: center;",
                                "Loading tasks..."
                            }
                        },
                    }
                }
            }
        }
    }
}

/// Task item component
#[component]
fn TaskItem(name: String, schedule: String, enabled: bool) -> Element {
    let (icon, badge_variant, badge_text) = if enabled {
        ("🟢", "success", "Enabled")
    } else {
        ("🔴", "warning", "Disabled")
    };

    rsx! {
        div {
            style: "display: flex; align-items: center; gap: 12px; padding: 12px 0; border-bottom: 1px solid var(--border-color);",
            span {
                style: "font-size: 1.25rem;",
                "{icon}"
            }
            div {
                style: "flex: 1;",
                div {
                    style: "color: var(--text-primary);",
                    "{name}"
                }
                div {
                    style: "font-size: 0.75rem; color: var(--text-secondary); font-family: monospace; margin-top: 4px;",
                    "{schedule}"
                }
            }
            Badge {
                variant: badge_variant,
                "{badge_text}"
            }
        }
    }
}
