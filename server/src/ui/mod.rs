//! Dioxus web UI module

pub mod routes;
pub mod layout;
pub mod theme;
pub mod pages;
pub mod components;
pub mod api;
pub mod server_fns;

use dioxus::prelude::*;
use dioxus_router::Router;
use crate::ui::routes::Route;

/// Main Dioxus App component for fullstack mode
#[component]
pub fn App() -> Element {
    rsx! {
        Router::<Route> {}
    }
}
