//! Dioxus web UI module

pub mod routes;
pub mod layout;
pub mod theme;
pub mod pages;
pub mod components;

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

/// Serve the Dioxus UI with server-side rendering
pub async fn serve() -> impl IntoResponse {
    // Render the Dioxus app to HTML
    let mut vdom = VirtualDom::new(App);
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
