//! Application layout component

use dioxus::prelude::*;
use dioxus_router::components::{Link, Outlet};
use crate::ui::{routes::Route, theme::{ThemeMode, inject_global_css}};

/// Main application layout with header and sidebar
#[component]
pub fn AppLayout() -> Element {
    let mut theme = use_signal(|| ThemeMode::Light);

    // TODO: Set theme attribute on root element using web_sys
    // For now, theme switching is prepared but not fully implemented

    rsx! {
        // Inject global CSS
        {inject_global_css()}

        div { class: "app-container",
            // Header
            Header { theme }

            // Main layout with sidebar and content
            div { class: "main-layout",
                Sidebar {}

                main { class: "main-content",
                    Outlet::<Route> {}
                }
            }
        }
    }
}

/// Header component
#[component]
fn Header(theme: Signal<ThemeMode>) -> Element {
    rsx! {
        header { class: "header",
            // Logo and title
            div {
                style: "display: flex; align-items: center; gap: 12px;",
                span {
                    style: "font-size: 1.5rem; font-weight: 700; color: var(--accent-primary);",
                    "SvrCtlRS"
                }
            }

            // Spacer
            div { style: "flex: 1;" }

            // Theme toggle button
            button {
                class: "btn btn-secondary",
                onclick: move |_| theme.set(theme().toggle()),
                if theme() == ThemeMode::Light {
                    "🌙 Dark"
                } else {
                    "☀️ Light"
                }
            }
        }
    }
}

/// Sidebar navigation component
#[component]
fn Sidebar() -> Element {
    rsx! {
        nav { class: "sidebar",
            Link {
                to: Route::Dashboard {},
                class: "nav-link",
                active_class: "active",
                "📊 Dashboard"
            }

            Link {
                to: Route::Servers {},
                class: "nav-link",
                active_class: "active",
                "🖥️ Servers"
            }

            Link {
                to: Route::Plugins {},
                class: "nav-link",
                active_class: "active",
                "🔌 Plugins"
            }

            Link {
                to: Route::Tasks {},
                class: "nav-link",
                active_class: "active",
                "⚙️ Tasks"
            }

            Link {
                to: Route::Settings {},
                class: "nav-link",
                active_class: "active",
                "⚙️ Settings"
            }

            // Version footer
            div {
                style: "position: absolute; bottom: 16px; left: 24px; font-size: 0.75rem; color: var(--text-muted);",
                "v0.1.0"
            }
        }
    }
}
