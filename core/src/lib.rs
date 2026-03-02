//! Core library for SvrCtlRS
//!
//! This crate defines shared types, encryption, and error handling
//! used across all SvrCtlRS components.

pub mod encryption;
pub mod error;
pub mod types;

// Re-exports
pub use error::{Error, Result};
pub use types::{MetricValue, Server, ServerStatus};
