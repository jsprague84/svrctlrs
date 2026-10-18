//! The crate-wide error type.
//!
//! Variants are chosen by *what the caller should do about them*: the API maps `NotFound`,
//! `Conflict` and `InvalidInput` to 404/409/400 and everything else to a logged 500.

use thiserror::Error;

/// Result type alias
pub type Result<T> = std::result::Result<T, Error>;

/// Main error type
#[derive(Debug, Error)]
pub enum Error {
    /// The row does not exist, or is not visible to the requesting user. The payload names
    /// it ("server 3", "user 12"); the message reads "server 3 not found".
    #[error("{0} not found")]
    NotFound(String),

    /// A uniqueness rule refused the write (a name already in use).
    #[error("{0}")]
    Conflict(String),

    /// The input was refused by a validation rule (too short a password, a bad URL …). The
    /// message is written for the person who sent it.
    #[error("{0}")]
    InvalidInput(String),

    /// The database refused or failed a statement. The cause chain (context + driver error)
    /// is kept, so logs and `{:#}` show why.
    #[error("Database error: {0}")]
    Database(anyhow::Error),

    /// Encrypting or decrypting a secret failed, or the key is not available.
    #[error("Encryption error: {0}")]
    Encryption(String),

    /// The process is misconfigured (environment, key file, data directory).
    #[error("Configuration error: {0}")]
    Config(String),

    /// A notification could not be delivered.
    #[error("Notification error: {0}")]
    Notification(String),

    /// Anything else, with its cause chain.
    #[error("{0}")]
    Other(anyhow::Error),
}

impl Error {
    /// The full story for a log line or an operator-facing message: the cause chain for the
    /// wrapped `anyhow` variants, the plain message for the rest.
    pub fn detail(&self) -> String {
        match self {
            Error::Database(e) => format!("Database error: {e:#}"),
            Error::Other(e) => format!("{e:#}"),
            e => e.to_string(),
        }
    }
}

/// An `anyhow::Error` that wraps one of ours (a query that used `anyhow::Context` on top of a
/// `NotFound`, say) converts back to that variant, so the API still answers 404 and not 500.
impl From<anyhow::Error> for Error {
    fn from(err: anyhow::Error) -> Self {
        match err.downcast::<Error>() {
            Ok(ours) => ours,
            Err(other) => Error::Other(other),
        }
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Error::Other(anyhow::Error::new(err).context("JSON"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anyhow_wrapping_our_error_unwraps_to_it() {
        let wrapped = anyhow::Error::from(Error::NotFound("server 3".into()))
            .context("Failed to load the server");
        let back = Error::from(wrapped);
        assert!(matches!(back, Error::NotFound(ref what) if what == "server 3"));
        assert_eq!(back.to_string(), "server 3 not found");
    }

    #[test]
    fn foreign_anyhow_becomes_other_with_its_chain() {
        let err: anyhow::Error = anyhow::anyhow!("disk full").context("Failed to write");
        let ours = Error::from(err);
        assert!(matches!(ours, Error::Other(_)));
        assert_eq!(ours.detail(), "Failed to write: disk full");
        assert_eq!(ours.to_string(), "Failed to write");
    }

    #[test]
    fn database_detail_keeps_the_cause() {
        let e = Error::Database(
            anyhow::anyhow!("no such table: servers").context("Failed to list servers"),
        );
        assert_eq!(
            e.detail(),
            "Database error: Failed to list servers: no such table: servers"
        );
    }
}
