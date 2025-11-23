//! Servers management page

use dioxus::prelude::*;

#[component]
pub fn Servers() -> Element {
    rsx! {
        div {
            h1 { "Servers" }
            p {
                style: "color: var(--text-secondary);",
                "Manage and monitor your servers"
            }

            div {
                class: "card",
                style: "margin-top: 24px;",
                p { "Server list will be implemented here" }
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
