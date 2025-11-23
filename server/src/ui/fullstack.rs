//! Fullstack Dioxus serving functions

use axum::{
    extract::Request,
    response::{Html, IntoResponse},
};
use dioxus::prelude::*;
use dioxus_ssr::Renderer;

use super::App;

/// Serve the Dioxus fullstack application
///
/// This function renders the app with SSR and prepares it for client-side hydration.
pub async fn serve_fullstack(request: Request) -> impl IntoResponse {
    let path = request.uri().path();

    tracing::debug!("Fullstack request: {}", path);

    // For now, render the App component with SSR + hydration support
    let mut vdom = VirtualDom::new(App);
    vdom.rebuild_in_place();

    // Create renderer with pre-rendering enabled for hydration
    let mut renderer = Renderer::new();
    renderer.pre_render = true; // Enable element IDs for hydration

    let html_body = renderer.render(&vdom);

    // Wrap in full HTML document with hydration script
    let html = format!(
        r#"<!DOCTYPE html>
<html lang="en" data-theme="dark">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>SvrCtlRS Dashboard</title>
    <script type="module">
        // Hydration will happen here when we build the client bundle
        // For now, this is just SSR
    </script>
</head>
<body>
    <div id="main">
        {html_body}
    </div>
    <!-- Client bundle will be loaded here -->
    <script type="module" src="/assets/dioxus/svrctlrs.js"></script>
</body>
</html>"#
    );

    Html(html)
}
