//! Badge component for status indicators

use dioxus::prelude::*;

#[component]
pub fn Badge(#[props(default = "info".to_string())] variant: String, children: Element) -> Element {
    let class_name = format!("badge badge-{}", variant);

    rsx! {
        span {
            class: "{class_name}",
            {children}
        }
    }
}
