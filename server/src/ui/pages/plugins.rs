//! Plugins management page - Interactive

use crate::ui::{
    components::Badge,
    server_fns::{list_plugins, toggle_plugin, TogglePluginRequest},
};
use dioxus::prelude::*;

#[component]
pub fn Plugins() -> Element {
    let mut plugins_resource = use_resource(|| async move { list_plugins().await });

    rsx! {
        div {
            div {
                style: "display: flex; align-items: center; justify-content: space-between; margin-bottom: 24px;",
                h1 { "Plugins" }
                p {
                    style: "color: var(--text-secondary);",
                    match &*plugins_resource.read_unchecked() {
                        Some(Ok(data)) => rsx! { "{data.plugins.len()} plugin(s) loaded" },
                        Some(Err(_)) => rsx! { "Error loading plugins" },
                        None => rsx! { "Loading plugins..." },
                    }
                }
            }

            match &*plugins_resource.read_unchecked() {
                None => rsx! {
                    div {
                        class: "card",
                        style: "text-align: center; padding: 48px;",
                        p { style: "color: var(--text-muted);", "Loading plugins..." }
                    }
                },
                Some(Err(e)) => rsx! {
                    div {
                        class: "card",
                        style: "text-align: center; padding: 48px;",
                        p { style: "color: var(--error);", "Failed to load plugins: {e}" }
                        button {
                            class: "btn btn-primary",
                            style: "margin-top: 16px;",
                            onclick: move |_| plugins_resource.restart(),
                            "Retry"
                        }
                    }
                },
                Some(Ok(plugin_list)) => rsx! {
                    div {
                        style: "display: grid; grid-template-columns: repeat(auto-fill, minmax(350px, 1fr)); gap: 16px;",
                        for plugin in &plugin_list.plugins {
                            PluginCard {
                                key: "{plugin.id}",
                                id: plugin.id.clone(),
                                name: plugin.name.clone(),
                                description: plugin.description.clone(),
                                version: plugin.version.clone(),
                                author: plugin.author.clone(),
                                on_refresh: move |_| plugins_resource.restart(),
                            }
                        }
                    }
                },
            }
        }
    }
}

#[component]
fn PluginCard(
    id: String,
    name: String,
    description: String,
    version: String,
    author: String,
    on_refresh: EventHandler<()>,
) -> Element {
    let mut enabled = use_signal(|| true); // Assume active by default
    let mut toggling = use_signal(|| false);

    rsx! {
        div {
            class: "card",

            div {
                style: "display: flex; align-items: center; justify-content: space-between; margin-bottom: 12px;",
                div {
                    style: "display: flex; align-items: center; gap: 12px;",
                    h3 { "{name}" }
                    Badge {
                        variant: if enabled() { "success" } else { "warning" },
                        if enabled() { "Active" } else { "Disabled" }
                    }
                }
                span {
                    style: "color: var(--text-muted); font-size: 0.875rem;",
                    "v{version}"
                }
            }

            p {
                style: "color: var(--text-secondary); margin-bottom: 16px;",
                "{description}"
            }

            div {
                style: "display: flex; align-items: center; justify-content: space-between; padding-top: 16px; border-top: 1px solid var(--border-color);",
                span {
                    style: "color: var(--text-muted); font-size: 0.875rem;",
                    "👤 {author}"
                }
                button {
                    class: if enabled() { "btn btn-warning" } else { "btn btn-success" },
                    style: "font-size: 0.875rem;",
                    disabled: toggling(),
                    onclick: {
                        let plugin_id = id.clone();
                        let new_enabled = !enabled();
                        move |_| {
                            let plugin_id = plugin_id.clone();
                            spawn(async move {
                                toggling.set(true);
                                match toggle_plugin(TogglePluginRequest {
                                    plugin_id,
                                    enabled: new_enabled,
                                }).await {
                                    Ok(_) => {
                                        enabled.set(new_enabled);
                                        on_refresh.call(());
                                    }
                                    Err(e) => {
                                        tracing::error!("Failed to toggle plugin: {}", e);
                                    }
                                }
                                toggling.set(false);
                            });
                        }
                    },
                    if toggling() { "..." } else if enabled() { "Disable" } else { "Enable" }
                }
            }
        }
    }
}

/// Plugin detail page (placeholder)
#[component]
pub fn PluginDetail(id: String) -> Element {
    rsx! {
        div {
            h1 { "Plugin Details: {id}" }
            p { "Full plugin details page coming soon..." }
        }
    }
}
