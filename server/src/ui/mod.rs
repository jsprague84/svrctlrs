//! Dioxus web UI module

pub mod api;
pub mod components;
pub mod layout;
pub mod pages;
pub mod routes;
pub mod server_fns;
pub mod theme;

use crate::ui::routes::Route;
use dioxus::prelude::*;
use dioxus_router::Router;

/// Main Dioxus App component for fullstack mode
#[component]
pub fn App() -> Element {
    rsx! {
        Router::<Route> {}
    }
}
