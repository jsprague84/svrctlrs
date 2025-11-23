//! Dioxus web UI module

pub mod routes;
pub mod layout;
pub mod theme;
pub mod pages;
pub mod components;
pub mod api;

use axum::{extract::Request, response::{Html, IntoResponse}};
use dioxus::prelude::*;
use crate::ui::routes::Route;

/// Dashboard data fetched from API
#[derive(Clone, Debug, PartialEq)]
pub struct DashboardData {
    pub status: Option<api::StatusResponse>,
    pub plugins: Option<api::PluginListResponse>,
    pub servers: Option<api::ServerListResponse>,
    pub tasks: Option<api::TaskListResponse>,
}

/// Serve the Dioxus UI with server-side rendering
pub async fn serve(request: Request) -> impl IntoResponse {
    let path = request.uri().path();

    // Fetch data for the dashboard
    let api_client = api::ApiClient::default();

    let status = api_client.status().await.ok();
    let plugins = api_client.plugins().await.ok();
    let servers = api_client.servers().await.ok();
    let tasks = api_client.tasks().await.ok();

    let dashboard_data = DashboardData { status, plugins, servers, tasks };

    // Determine which route to render based on the path
    let route = match path {
        "/" => Route::Dashboard {},
        "/servers" => Route::Servers {},
        "/plugins" => Route::Plugins {},
        "/tasks" => Route::Tasks {},
        "/settings" => Route::Settings {},
        _ => {
            // Extract path segments for 404 page
            let segments = path
                .trim_start_matches('/')
                .split('/')
                .filter(|s| !s.is_empty())
                .map(String::from)
                .collect();
            Route::NotFound { segments }
        }
    };

    // Render the specific route component with layout
    let mut vdom = VirtualDom::new_with_props(
        ServerApp,
        ServerAppProps {
            data: dashboard_data,
            route,
        },
    );
    vdom.rebuild_in_place();

    // Render to HTML string
    let html_body = dioxus_ssr::render(&vdom);

    // Wrap in a full HTML document
    let html = format!(
        r#"<!DOCTYPE html>
<html lang="en" data-theme="dark">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>SvrCtlRS Dashboard</title>
    <style>
        /* Note: Inline CSS from theme.rs will be injected by the component */
    </style>
</head>
<body>
    {html_body}
</body>
</html>"#
    );

    Html(html)
}

/// Server-side app component that renders a specific route
#[component]
fn ServerApp(data: DashboardData, route: Route) -> Element {
    use_context_provider(|| data);
    use crate::ui::theme::inject_global_css;
    use crate::ui::pages::*;

    // Render the appropriate page based on the route
    let page_content = match route {
        Route::Dashboard {} => rsx! { dashboard::Dashboard {} },
        Route::Servers {} => rsx! { servers::Servers {} },
        Route::Plugins {} => rsx! { plugins::Plugins {} },
        Route::Tasks {} => rsx! { tasks::Tasks {} },
        Route::Settings {} => rsx! { settings::Settings {} },
        Route::NotFound { segments } => rsx! { not_found::NotFound { segments: segments.clone() } },
        _ => rsx! { not_found::NotFound { segments: vec![] } },
    };

    rsx! {
        // Inject global CSS
        {inject_global_css()}

        div { class: "app-container",
            // Header
            header { class: "header",
                div {
                    style: "display: flex; align-items: center; gap: 12px;",
                    span {
                        style: "font-size: 1.5rem; font-weight: 700; color: var(--accent-primary);",
                        "SvrCtlRS"
                    }
                }
            }

            // Main layout with sidebar and content
            div { class: "main-layout",
                // Sidebar
                nav { class: "sidebar",
                    a { href: "/", class: "nav-link", "📊 Dashboard" }
                    a { href: "/servers", class: "nav-link", "🖥️ Servers" }
                    a { href: "/plugins", class: "nav-link", "🔌 Plugins" }
                    a { href: "/tasks", class: "nav-link", "⚙️ Tasks" }
                    a { href: "/settings", class: "nav-link", "⚙️ Settings" }

                    div {
                        style: "position: absolute; bottom: 16px; left: 24px; font-size: 0.75rem; color: var(--text-muted);",
                        "v1.0.0"
                    }
                }

                // Main content
                main { class: "main-content",
                    {page_content}
                }
            }
        }
    }
}
