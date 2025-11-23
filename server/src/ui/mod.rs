//! Dioxus web UI module

pub mod routes;
pub mod layout;
pub mod theme;
pub mod pages;
pub mod components;
pub mod api;

use axum::response::{Html, IntoResponse};
use dioxus::prelude::*;
use dioxus_router::Router;
use crate::ui::routes::Route;

/// Main app component
#[allow(non_snake_case)]
pub fn App() -> Element {
    rsx! {
        Router::<Route> {}
    }
}

/// Dashboard data fetched from API
#[derive(Clone, Debug, PartialEq)]
pub struct DashboardData {
    pub status: Option<api::StatusResponse>,
    pub plugins: Option<api::PluginListResponse>,
    pub servers: Option<api::ServerListResponse>,
    pub tasks: Option<api::TaskListResponse>,
}

/// App component with data context
#[component]
fn AppWithData(data: DashboardData) -> Element {
    use_context_provider(|| data);
    rsx! { Router::<Route> {} }
}

/// Serve the Dioxus UI with server-side rendering
pub async fn serve() -> impl IntoResponse {
    // Fetch data for the dashboard
    let api_client = api::ApiClient::default();

    let status = api_client.status().await.ok();
    let plugins = api_client.plugins().await.ok();
    let servers = api_client.servers().await.ok();
    let tasks = api_client.tasks().await.ok();

    let dashboard_data = DashboardData { status, plugins, servers, tasks };

    // Render the Dioxus app to HTML
    let mut vdom = VirtualDom::new_with_props(AppWithData, AppWithDataProps { data: dashboard_data });
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
