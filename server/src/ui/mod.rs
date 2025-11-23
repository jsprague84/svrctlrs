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

/// Serve the Dioxus UI
pub async fn serve() -> impl IntoResponse {
    // For now, serve a simple HTML page that shows the UI is being worked on
    // TODO: Implement proper SSR or WASM bundling
    Html(r#"
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>SvrCtlRS Dashboard</title>
    <style>
        body {
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
            background: #2e3440;
            color: #eceff4;
            display: flex;
            align-items: center;
            justify-content: center;
            min-height: 100vh;
            margin: 0;
        }
        .container {
            text-align: center;
            padding: 48px;
        }
        h1 {
            font-size: 3rem;
            margin-bottom: 16px;
            color: #81a1c1;
        }
        p {
            font-size: 1.25rem;
            color: #d8dee9;
            margin-bottom: 32px;
        }
        .status {
            background: #3b4252;
            border-radius: 12px;
            padding: 24px;
            margin-top: 32px;
        }
        .api-link {
            color: #88c0d0;
            text-decoration: none;
            font-weight: 600;
        }
        .api-link:hover {
            text-decoration: underline;
        }
    </style>
</head>
<body>
    <div class="container">
        <h1>🚀 SvrCtlRS</h1>
        <p>Server Control & Monitoring Platform</p>

        <div class="status">
            <h2>Status: UI In Development</h2>
            <p>The Dioxus web interface is currently being implemented.</p>
            <p>In the meantime, you can use:</p>
            <ul style="list-style: none; padding: 0;">
                <li style="margin: 12px 0;">
                    <a href="/api/v1/health" class="api-link">REST API</a> - HTTP JSON API
                </li>
                <li style="margin: 12px 0;">
                    CLI Tool - <code>svrctl</code> command-line interface
                </li>
            </ul>
        </div>
    </div>
</body>
</html>
    "#)
}
