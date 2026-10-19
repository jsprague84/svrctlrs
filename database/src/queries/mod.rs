//! Database query modules

pub mod audit;
pub mod credentials;
pub mod quick_commands;
pub mod recordings;
pub mod server_host_keys;
pub mod servers;
pub mod settings;
pub mod terminal_profiles;
pub mod users;

use svrctlrs_core::Error;

/// Map a write's `sqlx` error. A UNIQUE violation (the `name` columns of `credentials` and
/// `servers`) becomes [`Error::Conflict`], which the API turns into 409 with a message the user
/// can act on; anything else keeps the driver's cause in the text, which `anyhow::Context`
/// alone would hide behind the context line.
pub(crate) fn write_error(
    entity: &'static str,
    action: &'static str,
) -> impl Fn(sqlx::Error) -> Error {
    move |e| match &e {
        sqlx::Error::Database(db) if db.is_unique_violation() => {
            Error::Conflict(format!("A {entity} with that name already exists"))
        }
        _ => Error::Database(anyhow::Error::new(e).context(format!("Failed to {action} {entity}"))),
    }
}

// Re-export commonly used functions for convenience
pub use audit::*;
pub use credentials::*;
pub use quick_commands::*;
pub use server_host_keys::*;
pub use servers::*;
pub use settings::*;
pub use terminal_profiles::*;
pub use users::*;
