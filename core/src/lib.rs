//! Core library for SvrCtlRS
//!
//! This crate defines the plugin system, shared types, and traits
//! used across all SvrCtlRS components.

pub mod plugin;
pub mod types;
pub mod error;
pub mod notifications;
pub mod remote;

// Re-exports
pub use plugin::{Plugin, PluginInfo, PluginMetadata, PluginContext, PluginResult, ScheduledTask, PluginRegistry};
pub use types::{Server, ServerStatus, MetricValue};
pub use error::{Error, Result};
pub use notifications::{
    NotificationBackend, NotificationMessage, NotificationAction,
    GotifyBackend, NtfyBackend, NotificationManager,
};
pub use remote::RemoteExecutor;
