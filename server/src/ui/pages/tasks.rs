//! Tasks management page - Fully interactive

use crate::ui::{
    components::Badge,
    server_fns::{execute_task, list_tasks, toggle_task, ExecuteTaskRequest, ToggleTaskRequest},
};
use dioxus::prelude::*;

#[component]
pub fn Tasks() -> Element {
    // Fetch tasks data reactively using use_resource
    let mut tasks_resource = use_resource(|| async move { list_tasks().await });

    rsx! {
        div {
            div {
                style: "display: flex; align-items: center; justify-content: space-between; margin-bottom: 24px;",
                h1 { "Scheduled Tasks" }
                p {
                    style: "color: var(--text-secondary);",
                    match &*tasks_resource.read_unchecked() {
                        Some(Ok(data)) => rsx! { "{data.tasks.len()} task(s) configured" },
                        Some(Err(_)) => rsx! { "Error loading tasks" },
                        None => rsx! { "Loading tasks..." },
                    }
                }
            }

            // Render based on resource state
            match &*tasks_resource.read_unchecked() {
                // Loading state
                None => rsx! {
                    div {
                        class: "card",
                        style: "text-align: center; padding: 48px;",
                        p {
                            style: "color: var(--text-muted);",
                            "Loading tasks..."
                        }
                    }
                },

                // Error state
                Some(Err(e)) => rsx! {
                    div {
                        class: "card",
                        style: "text-align: center; padding: 48px;",
                        p {
                            style: "color: var(--error);",
                            "Failed to load tasks: {e}"
                        }
                        button {
                            class: "btn btn-primary",
                            style: "margin-top: 16px;",
                            onclick: move |_| tasks_resource.restart(),
                            "Retry"
                        }
                    }
                },

                // Success state
                Some(Ok(task_list)) => {
                    if task_list.tasks.is_empty() {
                        rsx! {
                            div {
                                class: "card",
                                style: "text-align: center; padding: 48px;",
                                p {
                                    style: "color: var(--text-muted);",
                                    "No scheduled tasks configured"
                                }
                            }
                        }
                    } else {
                        rsx! {
                            div {
                                class: "card",

                                // Table header
                                div {
                                    style: "display: grid; grid-template-columns: 2fr 3fr 2fr 1fr 2fr; gap: 16px; padding: 16px; border-bottom: 2px solid var(--border-color); font-weight: bold;",
                                    div { "Plugin" }
                                    div { "Description" }
                                    div { "Schedule" }
                                    div { "Status" }
                                    div { "Actions" }
                                }

                                // Task rows
                                for task in &task_list.tasks {
                                    TaskRow {
                                        key: "{task.plugin_id}-{task.task_id}",
                                        plugin_id: task.plugin_id.clone(),
                                        task_id: task.task_id.clone(),
                                        description: task.description.clone(),
                                        schedule: task.schedule.clone(),
                                        enabled: task.enabled,
                                        on_refresh: move |_| tasks_resource.restart(),
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
fn TaskRow(
    plugin_id: String,
    task_id: String,
    description: String,
    schedule: String,
    enabled: bool,
    on_refresh: EventHandler<()>,
) -> Element {
    let mut executing = use_signal(|| false);
    let mut execute_result = use_signal(|| None::<String>);
    let mut toggling = use_signal(|| false);

    rsx! {
        div {
            style: "display: grid; grid-template-columns: 2fr 3fr 2fr 1fr 2fr; gap: 16px; padding: 16px; border-bottom: 1px solid var(--border-color); align-items: center;",

            // Plugin ID
            div {
                style: "font-family: monospace; color: var(--text-secondary); font-size: 0.875rem;",
                "{plugin_id}"
            }

            // Description
            div {
                style: "color: var(--text-primary);",
                "{description}"
            }

            // Schedule
            div {
                style: "font-family: monospace; color: var(--text-secondary); font-size: 0.875rem;",
                "{schedule}"
            }

            // Status badge
            div {
                if enabled {
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

            // Action buttons
            div {
                style: "display: flex; gap: 8px;",

                // Execute button
                button {
                    class: "btn btn-primary",
                    style: "font-size: 0.875rem; padding: 6px 12px;",
                    disabled: executing(),
                    onclick: {
                        let plugin_id = plugin_id.clone();
                        let task_id = task_id.clone();
                        move |_| {
                            let plugin_id = plugin_id.clone();
                            let task_id = task_id.clone();
                            spawn(async move {
                                executing.set(true);
                                execute_result.set(None);

                                match execute_task(ExecuteTaskRequest {
                                    plugin_id,
                                    task_id,
                                }).await {
                                    Ok(result) => {
                                        if result.success {
                                            execute_result.set(Some(format!("✓ {}", result.message)));
                                        } else {
                                            execute_result.set(Some(format!("✗ {}", result.message)));
                                        }
                                    }
                                    Err(e) => {
                                        execute_result.set(Some(format!("Error: {}", e)));
                                    }
                                }

                                executing.set(false);
                            });
                        }
                    },
                    if executing() { "Executing..." } else { "Execute" }
                }

                // Toggle enable/disable button
                button {
                    class: if enabled { "btn btn-warning" } else { "btn btn-success" },
                    style: "font-size: 0.875rem; padding: 6px 12px;",
                    disabled: toggling(),
                    onclick: {
                        let plugin_id = plugin_id.clone();
                        let task_id = task_id.clone();
                        let new_enabled = !enabled;
                        move |_| {
                            let plugin_id = plugin_id.clone();
                            let task_id = task_id.clone();
                            spawn(async move {
                                toggling.set(true);

                                match toggle_task(ToggleTaskRequest {
                                    plugin_id,
                                    task_id,
                                    enabled: new_enabled,
                                }).await {
                                    Ok(_) => {
                                        // Refresh the tasks list
                                        on_refresh.call(());
                                    }
                                    Err(e) => {
                                        tracing::error!("Failed to toggle task: {}", e);
                                    }
                                }

                                toggling.set(false);
                            });
                        }
                    },
                    if toggling() {
                        "..."
                    } else if enabled {
                        "Disable"
                    } else {
                        "Enable"
                    }
                }
            }
        }

        // Show execution result below the row if present
        if let Some(result) = execute_result() {
            div {
                style: "grid-column: 1 / -1; padding: 12px; background: var(--bg-secondary); border-radius: 4px; margin: 0 16px 16px 16px; font-size: 0.875rem;",
                "{result}"
            }
        }
    }
}
