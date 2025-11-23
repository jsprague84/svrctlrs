//! Plugins management page

use dioxus::prelude::*;
use crate::ui::{components::Badge, DashboardData};

#[component]
pub fn Plugins() -> Element {
    // Get data from context
    let data = use_context::<DashboardData>();

    rsx! {
        div {
            div {
                style: "display: flex; align-items: center; justify-content: space-between; margin-bottom: 24px;",
                h1 { "Plugins" }
                p {
                    style: "color: var(--text-secondary);",
                    if let Some(plugins) = &data.plugins {
                        "{plugins.plugins.len()} plugin(s) loaded"
                    } else {
                        "Loading plugins..."
                    }
                }
            }

            // Plugin list
            if let Some(plugins) = &data.plugins {
                if plugins.plugins.is_empty() {
                    div {
                        class: "card",
                        style: "text-align: center; padding: 48px;",
                        p {
                            style: "color: var(--text-muted);",
                            "No plugins loaded"
                        }
                    }
                } else {
                    div {
                        style: "display: grid; grid-template-columns: repeat(auto-fill, minmax(350px, 1fr)); gap: 16px;",
                        for plugin in &plugins.plugins {
                            div {
                                class: "card",

                                div {
                                    style: "display: flex; align-items: center; justify-content: space-between; margin-bottom: 12px;",
                                    div {
                                        style: "display: flex; align-items: center; gap: 12px;",
                                        h3 { "{plugin.name}" }
                                        Badge {
                                            variant: "success",
                                            "Active"
                                        }
                                    }
                                    span {
                                        style: "color: var(--text-muted); font-size: 0.875rem;",
                                        "v{plugin.version}"
                                    }
                                }

                                p {
                                    style: "color: var(--text-secondary); margin-bottom: 12px; font-size: 0.875rem;",
                                    "{plugin.description}"
                                }

                                div {
                                    style: "display: flex; gap: 8px; padding-top: 12px; border-top: 1px solid var(--border-color);",

                                    div {
                                        style: "flex: 1;",
                                        p {
                                            style: "color: var(--text-muted); font-size: 0.75rem; margin-bottom: 4px;",
                                            "ID"
                                        }
                                        p {
                                            style: "color: var(--text-primary); font-size: 0.875rem; font-family: monospace;",
                                            "{plugin.id}"
                                        }
                                    }

                                    div {
                                        style: "flex: 1;",
                                        p {
                                            style: "color: var(--text-muted); font-size: 0.75rem; margin-bottom: 4px;",
                                            "Author"
                                        }
                                        p {
                                            style: "color: var(--text-primary); font-size: 0.875rem;",
                                            "{plugin.author}"
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
                        "Failed to load plugins"
                    }
                }
            }
        }
    }
}

#[component]
pub fn PluginDetail(id: String) -> Element {
    rsx! {
        div {
            h1 { "Plugin: {id}" }
            p {
                style: "color: var(--text-secondary);",
                "Plugin configuration and tasks"
            }

            div {
                class: "card",
                style: "margin-top: 24px;",
                p { "Plugin details will be implemented here" }
            }
        }
    }
}
