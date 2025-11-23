//! Status card component for displaying metrics

use dioxus::prelude::*;

#[component]
pub fn StatusCard(
    title: String,
    value: String,
    icon: String,
    #[props(default = "info".to_string())] variant: String,
) -> Element {
    let color = match variant.as_str() {
        "success" => "var(--accent-success)",
        "warning" => "var(--accent-warning)",
        "error" => "var(--accent-error)",
        "primary" => "var(--accent-primary)",
        _ => "var(--accent-info)",
    };

    rsx! {
        div {
            class: "card",
            style: "text-align: center; position: relative; overflow: hidden;",

            // Icon background (subtle)
            div {
                style: "position: absolute; top: -20px; right: -20px; font-size: 6rem; opacity: 0.1;",
                "{icon}"
            }

            // Content
            div {
                style: "position: relative; z-index: 1;",

                div {
                    style: "font-size: 2.5rem; margin-bottom: 8px;",
                    "{icon}"
                }

                div {
                    style: "font-size: 2rem; font-weight: 700; color: {color}; margin-bottom: 4px;",
                    "{value}"
                }

                div {
                    style: "font-size: 0.875rem; color: var(--text-muted); text-transform: uppercase; letter-spacing: 0.05em;",
                    "{title}"
                }
            }
        }
    }
}
