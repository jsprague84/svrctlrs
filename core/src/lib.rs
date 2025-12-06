//! Core library for SvrCtlRS
//!
//! This crate defines shared types, notification system, and job execution
//! used across all SvrCtlRS components.

pub mod error;
#[cfg(feature = "executor")]
pub mod executor;
pub mod notifications;
pub mod remote;
pub mod types;

// Re-exports
pub use error::{Error, Result};
#[cfg(feature = "executor")]
pub use executor::{JobExecutor, DEFAULT_MAX_CONCURRENT_JOBS, DEFAULT_TIMEOUT_SECONDS};
pub use notifications::{
    mask_token, GotifyBackend, NotificationAction, NotificationBackend, NotificationContext,
    NotificationManager, NotificationMessage, NtfyBackend,
};
pub use remote::RemoteExecutor;
pub use types::{MetricValue, Server, ServerStatus};
