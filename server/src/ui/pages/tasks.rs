//! Tasks management page

use dioxus::prelude::*;
use crate::ui::{components::Badge, DashboardData};

#[component]
pub fn Tasks() -> Element {
    // Get data from context
    let data = use_context::<DashboardData>();

    rsx! {
        div {
            div {
                style: "display: flex; align-items: center; justify-content: space-between; margin-bottom: 24px;",
                h1 { "Scheduled Tasks" }
                p {
                    style: "color: var(--text-secondary);",
                    if let Some(tasks) = &data.tasks {
                        "{tasks.tasks.len()} task(s) configured"
                    } else {
                        "Loading tasks..."
                    }
                }
            }

            // Task list
            if let Some(tasks) = &data.tasks {
                if tasks.tasks.is_empty() {
                    div {
                        class: "card",
                        style: "text-align: center; padding: 48px;",
                        p {
                            style: "color: var(--text-muted);",
                            "No scheduled tasks configured"
                        }
                    }
                } else {
                    div {
                        class: "card",

                        // Table header
                        div {
                            style: "display: grid; grid-template-columns: 2fr 3fr 2fr 1fr; gap: 16px; padding: 16px; border-bottom: 2px solid var(--border-color); font-weight: bold;",
                            div { "Plugin" }
                            div { "Description" }
                            div { "Schedule" }
                            div { "Status" }
                        }

                        // Task rows
                        for task in &tasks.tasks {
                            div {
                                style: "display: grid; grid-template-columns: 2fr 3fr 2fr 1fr; gap: 16px; padding: 16px; border-bottom: 1px solid var(--border-color); align-items: center;",

                                // Plugin ID
                                div {
                                    style: "font-family: monospace; color: var(--text-secondary); font-size: 0.875rem;",
                                    "{task.plugin_id}"
                                }

                                // Description
                                div {
                                    style: "color: var(--text-primary);",
                                    "{task.description}"
                                }

                                // Schedule
                                div {
                                    style: "font-family: monospace; color: var(--text-secondary); font-size: 0.875rem;",
                                    "{task.schedule}"
                                }

                                // Status badge
                                div {
                                    if task.enabled {
                                        Badge {
                                            variant: "success",
                                            "Enabled"
                                        }
                                    } else {
                                        Badge {
                                            variant: "warning",
                                            "Disabled"
                                        }
                                    }
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
                        "Failed to load tasks"
                    }
                }
            }
        }
    }
}
