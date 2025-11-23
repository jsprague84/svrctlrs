//! Tasks management page

use dioxus::prelude::*;

#[component]
pub fn Tasks() -> Element {
    rsx! {
        div {
            h1 { "Tasks" }
            p {
                style: "color: var(--text-secondary);",
                "View and manage scheduled tasks"
            }

            div {
                class: "card",
                style: "margin-top: 24px;",
                p { "Task scheduler will be implemented here" }
            }
        }
    }
}
