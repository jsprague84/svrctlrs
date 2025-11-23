//! Settings page

use dioxus::prelude::*;

#[component]
pub fn Settings() -> Element {
    rsx! {
        div {
            h1 { "Settings" }
            p {
                style: "color: var(--text-secondary);",
                "Application configuration and preferences"
            }

            div {
                class: "card",
                style: "margin-top: 24px;",
                p { "Settings interface will be implemented here" }
            }
        }
    }
}
