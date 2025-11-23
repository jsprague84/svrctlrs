//! Error types

use thiserror::Error;

/// Result type alias
pub type Result<T> = std::result::Result<T, Error>;

/// Main error type
#[derive(Debug, Error)]
pub enum Error {
    #[error("Plugin error: {0}")]
    PluginError(String),

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Notification error: {0}")]
    NotificationError(String),

    #[error("Remote execution error: {0}")]
    RemoteExecutionError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Scheduler error: {0}")]
    SchedulerError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("HTTP error: {0}")]
    HttpError(String),

    #[error("Other error: {0}")]
    Other(String),
}

// Convert anyhow::Error to our Error type
impl From<anyhow::Error> for Error {
    fn from(err: anyhow::Error) -> Self {
        Error::Other(err.to_string())
    }
}
