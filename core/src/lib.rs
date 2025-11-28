//! Core library for SvrCtlRS
//!
//! This crate defines the plugin system, shared types, and traits
//! used across all SvrCtlRS components.

pub mod error;
#[cfg(feature = "executor")]
pub mod executor;
pub mod notifications;
pub mod plugin;
pub mod remote;
pub mod types;

// Re-exports
pub use error::{Error, Result};
#[cfg(feature = "executor")]
pub use executor::{JobExecutor, DEFAULT_MAX_CONCURRENT_JOBS, DEFAULT_TIMEOUT_SECONDS};
pub use notifications::{
    mask_token, GotifyBackend, NotificationAction, NotificationBackend, NotificationManager,
    NotificationMessage, NtfyBackend,
};
pub use plugin::{
    Plugin, PluginContext, PluginInfo, PluginMetadata, PluginRegistry, PluginResult, ScheduledTask,
};
pub use remote::RemoteExecutor;
pub use types::{MetricValue, Server, ServerStatus};
