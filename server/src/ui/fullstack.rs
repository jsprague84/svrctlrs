//! Fullstack Dioxus configuration with AppState context.
//!
//! This wires Dioxus fullstack into Axum and exposes `AppState` to all
//! `#[server]` functions via the fullstack context.

use dioxus::prelude::*;

#[cfg(feature = "server")]
use {
    crate::state::AppState,
    axum::Router,
    std::sync::Arc,
    // Dioxus re-exports the server crate as `dioxus::server`
    dioxus::server::{DioxusRouterExt, ServeConfig},
};

#[cfg(feature = "server")]
use super::App;

/// Create the Dioxus fullstack Axum router with `AppState` in context.
///
/// This:
/// - Builds a `ServeConfig`
/// - Injects `AppState` into the Dioxus fullstack context
/// - Returns an Axum `Router` with SSR + server functions wired up
#[cfg(feature = "server")]
pub fn create_fullstack_handler(app_state: AppState) -> Router {
    // Clone so we can move `app_state` into the provider closure.
    let state_for_context = app_state.clone();

    // A single context provider that returns `AppState` as `Box<dyn Any + Send + Sync>`.
    //
    // This matches the pattern used in official Dioxus fullstack examples where
    // arbitrary values are provided to the fullstack context via providers.
    let app_state_provider =
        Box::new(move || {
            Box::new(state_for_context.clone()) as Box<dyn std::any::Any + Send + Sync>
        })
        as Box<dyn Fn() -> Box<dyn std::any::Any + Send + Sync> + Send + Sync + 'static>;

    let config = ServeConfig::builder()
        .context_providers(Arc::new(vec![app_state_provider]))
        .build()
        .expect("failed to build Dioxus ServeConfig");

    // This extension method comes from `DioxusRouterExt` in `dioxus::server`.
    Router::new().serve_dioxus_application(config, App)
}
