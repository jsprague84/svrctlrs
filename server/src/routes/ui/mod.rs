//! UI routes module — serves Svelte SPA and authentication

use axum::{routing::get, Router};
use tower_http::services::{ServeDir, ServeFile};

use crate::state::AppState;

pub mod auth;
pub mod manifest;

/// Create UI router: SPA + auth routes
pub fn ui_routes() -> Router<AppState> {
    let spa_dir = std::env::var("SPA_DIR").unwrap_or_else(|_| "ui/build".to_string());
    let fallback = format!("{}/index.html", spa_dir);

    Router::new()
        // Auth routes (login/logout pages)
        .merge(auth::routes())
        // The installed app's manifest, with the user's recent servers as shortcuts
        .route("/manifest.webmanifest", get(manifest::manifest))
        // Serve Svelte SPA at root with index.html fallback for client-side routing
        .fallback_service(ServeDir::new(&spa_dir).fallback(ServeFile::new(fallback)))
}
