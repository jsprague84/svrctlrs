//! Settings page - Interactive configuration management

use dioxus::prelude::*;
use crate::ui::{
    components::Badge,
    server_fns::{get_status, UpdateSettingsRequest, update_settings},
};
use std::collections::HashMap;

#[component]
pub fn Settings() -> Element {
    // Fetch status data reactively
    let mut status_resource = use_resource(|| async move {
        get_status().await
    });

    // Form state
    let mut auto_refresh_interval = use_signal(|| "30".to_string());
    let mut log_level = use_signal(|| "info".to_string());
    let mut enable_notifications = use_signal(|| true);
    let mut notification_backend = use_signal(|| "gotify".to_string());
    let mut scheduler_enabled = use_signal(|| true);

    // UI state
    let mut saving = use_signal(|| false);
    let mut save_result = use_signal(|| None::<Result<String, String>>);

    rsx! {
        div {
            h1 { "Settings" }
            p {
                style: "color: var(--text-secondary); margin-bottom: 24px;",
                "Application configuration and system information"
            }

            // System Overview (Read-only)
            div {
                class: "card",
                style: "margin-bottom: 24px;",
                h2 {
                    style: "margin-bottom: 16px;",
                    "System Overview"
                }

                match &*status_resource.read_unchecked() {
                    None => rsx! {
                        p { style: "color: var(--text-muted);", "Loading system status..." }
                    },
                    Some(Err(e)) => rsx! {
                        p { style: "color: var(--error);", "Failed to load status: {e}" }
                        button {
                            class: "btn btn-primary",
                            style: "margin-top: 12px;",
                            onclick: move |_| status_resource.restart(),
                            "Retry"
                        }
                    },
                    Some(Ok(status)) => rsx! {
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
                    },
                }
            }

            // Application Settings (Editable)
            div {
                class: "card",
                style: "margin-bottom: 24px;",
                h2 {
                    style: "margin-bottom: 16px;",
                    "Application Settings"
                }

                div {
                    style: "display: grid; gap: 20px;",

                    // Auto-refresh interval
                    div {
                        label {
                            style: "display: block; margin-bottom: 8px; color: var(--text-primary); font-weight: 500;",
                            "Auto-refresh Interval (seconds)"
                        }
                        input {
                            r#type: "number",
                            value: "{auto_refresh_interval}",
                            oninput: move |e| auto_refresh_interval.set(e.value().clone()),
                            min: "5",
                            max: "300",
                            step: "5",
                            style: "width: 100%; padding: 8px; border: 1px solid var(--border-color); border-radius: 4px; background: var(--bg-primary); color: var(--text-primary);",
                        }
                        p {
                            style: "color: var(--text-muted); font-size: 0.875rem; margin-top: 4px;",
                            "How often the dashboard refreshes data automatically"
                        }
                    }

                    // Log level
                    div {
                        label {
                            style: "display: block; margin-bottom: 8px; color: var(--text-primary); font-weight: 500;",
                            "Log Level"
                        }
                        select {
                            value: "{log_level}",
                            onchange: move |e| log_level.set(e.value().clone()),
                            style: "width: 100%; padding: 8px; border: 1px solid var(--border-color); border-radius: 4px; background: var(--bg-primary); color: var(--text-primary);",
                            option { value: "trace", "Trace (Most Verbose)" }
                            option { value: "debug", "Debug" }
                            option { value: "info", selected: true, "Info (Default)" }
                            option { value: "warn", "Warning" }
                            option { value: "error", "Error (Least Verbose)" }
                        }
                        p {
                            style: "color: var(--text-muted); font-size: 0.875rem; margin-top: 4px;",
                            "Minimum log level for system logging"
                        }
                    }

                    // Enable notifications
                    div {
                        label {
                            style: "display: flex; align-items: center; gap: 12px; cursor: pointer;",
                            input {
                                r#type: "checkbox",
                                checked: enable_notifications(),
                                onchange: move |e| enable_notifications.set(e.checked()),
                                style: "width: 20px; height: 20px; cursor: pointer;",
                            }
                            span {
                                style: "color: var(--text-primary); font-weight: 500;",
                                "Enable Notifications"
                            }
                        }
                        p {
                            style: "color: var(--text-muted); font-size: 0.875rem; margin-top: 4px; margin-left: 32px;",
                            "Send notifications for task completions and system events"
                        }
                    }

                    // Notification backend
                    if enable_notifications() {
                        div {
                            label {
                                style: "display: block; margin-bottom: 8px; color: var(--text-primary); font-weight: 500;",
                                "Notification Backend"
                            }
                            select {
                                value: "{notification_backend}",
                                onchange: move |e| notification_backend.set(e.value().clone()),
                                style: "width: 100%; padding: 8px; border: 1px solid var(--border-color); border-radius: 4px; background: var(--bg-primary); color: var(--text-primary);",
                                option { value: "gotify", selected: true, "Gotify" }
                                option { value: "ntfy", "ntfy.sh" }
                                option { value: "both", "Both" }
                            }
                            p {
                                style: "color: var(--text-muted); font-size: 0.875rem; margin-top: 4px;",
                                "Which notification service(s) to use"
                            }
                        }
                    }

                    // Scheduler enable/disable
                    div {
                        label {
                            style: "display: flex; align-items: center; gap: 12px; cursor: pointer;",
                            input {
                                r#type: "checkbox",
                                checked: scheduler_enabled(),
                                onchange: move |e| scheduler_enabled.set(e.checked()),
                                style: "width: 20px; height: 20px; cursor: pointer;",
                            }
                            span {
                                style: "color: var(--text-primary); font-weight: 500;",
                                "Enable Task Scheduler"
                            }
                        }
                        p {
                            style: "color: var(--text-muted); font-size: 0.875rem; margin-top: 4px; margin-left: 32px;",
                            "Automatically run scheduled tasks based on their cron schedules"
                        }
                    }
                }

                // Save button and status
                div {
                    style: "margin-top: 24px; padding-top: 24px; border-top: 1px solid var(--border-color);",

                    button {
                        class: "btn btn-primary",
                        disabled: saving(),
                        onclick: move |_| {
                            spawn(async move {
                                saving.set(true);
                                save_result.set(None);

                                let mut settings = HashMap::new();
                                settings.insert("auto_refresh_interval".to_string(), auto_refresh_interval());
                                settings.insert("log_level".to_string(), log_level());
                                settings.insert("enable_notifications".to_string(), enable_notifications().to_string());
                                settings.insert("notification_backend".to_string(), notification_backend());
                                settings.insert("scheduler_enabled".to_string(), scheduler_enabled().to_string());

                                match update_settings(UpdateSettingsRequest { settings }).await {
                                    Ok(_) => {
                                        save_result.set(Some(Ok("Settings saved successfully!".to_string())));
                                        // Refresh status to reflect changes
                                        status_resource.restart();
                                    }
                                    Err(e) => {
                                        save_result.set(Some(Err(format!("Failed to save settings: {}", e))));
                                    }
                                }

                                saving.set(false);
                            });
                        },
                        if saving() { "Saving..." } else { "Save Settings" }
                    }

                    // Show save result
                    if let Some(result) = save_result() {
                        div {
                            style: match &result {
                                Ok(_) => "margin-top: 12px; padding: 12px; background: var(--success-bg); border: 1px solid var(--success); border-radius: 4px; color: var(--success);",
                                Err(_) => "margin-top: 12px; padding: 12px; background: var(--error-bg); border: 1px solid var(--error); border-radius: 4px; color: var(--error);",
                            },
                            match &result {
                                Ok(msg) => rsx! { "✓ {msg}" },
                                Err(err) => rsx! { "✗ {err}" },
                            }
                        }
                    }
                }
            }

            // Information section
            div {
                class: "card",
                h2 {
                    style: "margin-bottom: 16px;",
                    "About"
                }
                div {
                    style: "display: grid; gap: 12px;",

                    SettingRow {
                        label: "Application",
                        value: rsx! {
                            span { "SvrCtlRS" }
                        }
                    }

                    SettingRow {
                        label: "Version",
                        value: rsx! {
                            span { "1.0.0" }
                        }
                    }

                    SettingRow {
                        label: "Build",
                        value: rsx! {
                            span {
                                style: "font-family: monospace; color: var(--text-muted); font-size: 0.875rem;",
                                "Fullstack (Dioxus 0.7)"
                            }
                        }
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
