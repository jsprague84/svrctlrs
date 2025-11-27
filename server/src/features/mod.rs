// ! Feature modules for server monitoring and management
//!
//! This module contains built-in features that can run on both local and remote servers
//! via the RemoteExecutor. Features replace the previous plugin architecture with a
//! simpler, more direct approach.
//!
//! Each feature module exports functions that take Server, RemoteExecutor, and
//! NotificationManager as parameters, allowing uniform execution across all servers.

pub mod docker;
pub mod health;
pub mod updates;

/// Feature execution result
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct FeatureResult {
    /// Whether execution was successful
    pub success: bool,
    /// Human-readable message
    pub message: String,
    /// Structured data (optional)
    pub data: Option<serde_json::Value>,
    /// Metrics collected (optional)
    pub metrics: Option<std::collections::HashMap<String, f64>>,
}

impl FeatureResult {
    /// Create a successful result
    pub fn success(message: impl Into<String>) -> Self {
        Self {
            success: true,
            message: message.into(),
            data: None,
            metrics: None,
        }
    }

    /// Create a successful result with data
    pub fn success_with_data(
        message: impl Into<String>,
        data: serde_json::Value,
        metrics: Option<std::collections::HashMap<String, f64>>,
    ) -> Self {
        Self {
            success: true,
            message: message.into(),
            data: Some(data),
            metrics,
        }
    }

    /// Create an error result
    #[allow(dead_code)]
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            success: false,
            message: message.into(),
            data: None,
            metrics: None,
        }
    }
}
