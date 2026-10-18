//! Core library for SvrCtlRS
//!
//! Credential encryption, the shared error type and notification senders used by the
//! server and database crates.

pub mod encryption;
pub mod error;
pub mod notifications;

// Re-exports
pub use error::{Error, Result};
