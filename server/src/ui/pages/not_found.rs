//! 404 Not Found page

use dioxus::prelude::*;
use dioxus_router::components::Link;
use crate::ui::routes::Route;

#[component]
pub fn NotFound(segments: Vec<String>) -> Element {
    let path = segments.join("/");

    rsx! {
        div {
            style: "text-align: center; padding: 64px;",
            h1 { "404 - Page Not Found" }
            p {
                style: "color: var(--text-secondary); margin: 16px 0;",
                "The page \"/{path}\" could not be found."
            }
            Link {
                to: Route::Dashboard {},
                class: "btn btn-primary",
                "← Back to Dashboard"
            }
        }
    }
}
