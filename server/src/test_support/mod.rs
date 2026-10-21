//! Shared fixtures for the server crate's tests: a migrated in-memory database, the process'
//! encryption key, and a loopback SSH server ([`ssh_server`]) for the client, host-key and PTY
//! code paths that otherwise only the e2e run against a real host exercised.

pub mod app;
pub mod oidc_provider;
pub mod ssh_server;

use svrctlrs_database::models::{CreateCredential, CreateServer, CredentialType};
use svrctlrs_database::queries;
use svrctlrs_database::sqlx::{sqlite::SqlitePoolOptions, Pool, Sqlite};

/// The encryption key is process-wide and settable once; every test that stores a credential
/// calls this, and whichever runs first sets it.
pub fn ensure_encryption() {
    if !svrctlrs_core::encryption::is_initialized() {
        let _ = svrctlrs_core::encryption::init_with_hex_key(&"ab".repeat(32));
    }
}

/// A fresh, fully migrated database. One connection: each pooled connection to `:memory:`
/// would otherwise get its own empty database.
pub async fn database() -> Pool<Sqlite> {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("in-memory database");
    svrctlrs_database::run_migrations(&pool)
        .await
        .expect("migrations");
    pool
}

/// A server row pointing at `127.0.0.1:port`, optionally behind a jump host.
pub async fn add_server(
    pool: &Pool<Sqlite>,
    name: &str,
    port: u16,
    username: &str,
    credential_id: Option<i64>,
    via_server_id: Option<i64>,
) -> i64 {
    queries::servers::create_server(
        pool,
        &CreateServer {
            name: name.to_string(),
            hostname: "127.0.0.1".to_string(),
            port: i32::from(port),
            username: Some(username.to_string()),
            credential_id,
            via_server_id,
            tags: vec![],
            startup_command: None,
            record_sessions: false,
            persistent_shell: false,
            description: None,
            enabled: true,
        },
    )
    .await
    .expect("server row")
}

/// A stored (encrypted) credential: a password, or OpenSSH private key material.
pub async fn add_credential(
    pool: &Pool<Sqlite>,
    name: &str,
    kind: CredentialType,
    value: &str,
) -> i64 {
    ensure_encryption();
    queries::credentials::create_credential(
        pool,
        &CreateCredential {
            name: name.to_string(),
            credential_type: kind,
            description: None,
            value: value.to_string(),
            username: None,
            metadata: None,
        },
    )
    .await
    .expect("credential row")
}

/// Confirm every key recorded for `server_id` on first contact, as the user's "Trust and
/// connect" does.
pub async fn trust_recorded_keys(pool: &Pool<Sqlite>, server_id: i64) {
    for key in queries::server_host_keys::list_host_keys_for_server(pool, server_id)
        .await
        .expect("host keys")
        .into_iter()
        .filter(|k| k.trusted_at.is_none())
    {
        queries::server_host_keys::trust_host_key(pool, server_id, key.id)
            .await
            .expect("trust");
    }
}
