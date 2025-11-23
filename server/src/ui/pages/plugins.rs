//! Plugins management page

use dioxus::prelude::*;

#[component]
pub fn Plugins() -> Element {
    rsx! {
        div {
            h1 { "Plugins" }
            p {
                style: "color: var(--text-secondary);",
                "Manage and configure plugins"
            }

            div {
                class: "card",
                style: "margin-top: 24px;",
                p { "Plugin list will be implemented here" }
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
