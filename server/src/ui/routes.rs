//! Route definitions for the Dioxus UI

use dioxus::prelude::*;
use dioxus_router::{Routable, components::{Link, Outlet}};

// Import layout and pages
use crate::ui::layout::AppLayout;
use crate::ui::pages::{
    dashboard::Dashboard,
    servers::{Servers, ServerDetail},
    plugins::{Plugins, PluginDetail},
    tasks::Tasks,
    settings::Settings,
    not_found::NotFound,
};

/// Application routes
#[derive(Routable, Clone, PartialEq)]
#[rustfmt::skip]
pub enum Route {
    // Main layout wraps all routes
    #[layout(AppLayout)]
        // Dashboard
        #[route("/")]
        Dashboard {},

        // Servers
        #[route("/servers")]
        Servers {},

        #[route("/servers/:id")]
        ServerDetail { id: String },

        // Plugins
        #[route("/plugins")]
        Plugins {},

        #[route("/plugins/:id")]
        PluginDetail { id: String },

        // Tasks
        #[route("/tasks")]
        Tasks {},

        // Settings
        #[route("/settings")]
        Settings {},
    #[end_layout]

    // 404 page (outside layout)
    #[route("/:..segments")]
    NotFound { segments: Vec<String> },
}
