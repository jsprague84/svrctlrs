//! Servers (SSH targets), their host keys, the connection test, key installation and
//! `ssh_config` import.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::double_option;

/// An SSH target.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Server {
    pub id: i64,
    pub name: String,
    /// Hostname or IP address
    pub hostname: String,
    pub port: i32,
    /// SSH user; null means the credential's user, else `root`
    pub username: Option<String>,
    pub credential_id: Option<i64>,
    /// Reached through this server's SSH connection (a jump host)
    pub via_server_id: Option<i64>,
    /// Labels for filtering: trimmed, unique ignoring case, sorted
    pub tags: Vec<String>,
    /// Typed into every new shell right after it starts (one line)
    pub startup_command: Option<String>,
    /// Shells opened here are recorded for the user who opens them
    pub record_sessions: bool,
    pub description: Option<String>,
    pub enabled: bool,
    /// The last successful contact
    pub last_seen_at: Option<DateTime<Utc>>,
    /// Why the last attempt failed
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ServerList {
    pub servers: Vec<Server>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ServerOne {
    pub server: Server,
}

/// `POST /servers`
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct NewServer {
    pub name: String,
    #[serde(default)]
    pub hostname: Option<String>,
    /// Default 22
    #[serde(default = "default_port")]
    pub port: i32,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub credential_id: Option<i64>,
    /// Reach this server through another server's SSH connection (jump host)
    #[serde(default)]
    pub via_server_id: Option<i64>,
    #[serde(default)]
    pub tags: Vec<String>,
    /// One line typed into every new shell, e.g. `tmux new -A -s main`
    #[serde(default)]
    pub startup_command: Option<String>,
    #[serde(default)]
    pub record_sessions: bool,
    #[serde(default)]
    pub description: Option<String>,
    /// Retired with command mode; `true` is refused
    #[serde(default)]
    pub is_local: bool,
    /// Default true
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_port() -> i32 {
    22
}

fn default_true() -> bool {
    true
}

/// `PUT /servers/{id}`: only the fields given change
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
pub struct ServerChanges {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub hostname: Option<String>,
    #[serde(default)]
    pub port: Option<i32>,
    #[serde(default)]
    pub username: Option<String>,
    /// Absent keeps the credential, `null` unassigns it, a number sets it
    #[serde(default, deserialize_with = "double_option")]
    pub credential_id: Option<Option<i64>>,
    /// Absent keeps it, `null` makes the server direct again
    #[serde(default, deserialize_with = "double_option")]
    pub via_server_id: Option<Option<i64>>,
    /// Replaces the whole list
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    /// An empty string clears it
    #[serde(default)]
    pub startup_command: Option<String>,
    #[serde(default)]
    pub record_sessions: Option<bool>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub enabled: Option<bool>,
}

/// A host key recorded for a server (no key material).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct HostKey {
    pub id: i64,
    /// e.g. `ssh-ed25519`
    pub key_type: String,
    /// `SHA256:…`, as `ssh-keygen -lf` prints it
    pub fingerprint: String,
    pub first_seen_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
    /// Null for the key seen on first contact, waiting for confirmation
    pub trusted_at: Option<DateTime<Utc>>,
    pub trusted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct HostKeyList {
    pub server_id: i64,
    pub host_keys: Vec<HostKey>,
}

/// `POST /servers/{id}/host-keys/trust`
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TrustHostKey {
    /// The fingerprint you compared with `ssh-keygen -lf` on the host
    pub fingerprint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct HostKeyTrusted {
    pub server_id: i64,
    pub host_key: HostKey,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct HostKeysForgotten {
    pub server_id: i64,
    pub removed: u64,
}

/// `POST /servers/{id}/test`. Always 200: `success` says how it went.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ConnectionTest {
    pub success: bool,
    pub message: String,
    /// On failure only: `HOST_KEY_UNCONFIRMED`, `HOST_KEY_MISMATCH` or `BAD_REQUEST`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    /// On failure only (null when there are none): what a client can act on, such as the
    /// `fingerprint` to confirm
    #[serde(
        default,
        deserialize_with = "double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub details: Option<Option<serde_json::Value>>,
    pub server_id: i64,
    pub hostname: String,
    pub port: i32,
    pub latency_ms: u64,
}

/// `POST /servers/{id}/authorize-key`: install a key credential's public half with one-time
/// access, verify it, and switch the server to it
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AuthorizeKey {
    /// The `ssh_key` credential to install
    pub credential_id: i64,
    /// One-time bootstrap: the account's password (never stored) …
    #[serde(default)]
    pub password: Option<String>,
    /// … or a credential that already works on the server
    #[serde(default)]
    pub bootstrap_credential_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct KeyAuthorized {
    pub success: bool,
    pub server_id: i64,
    pub credential_id: i64,
    pub message: String,
}

/// `POST /servers/import`
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ImportServers {
    /// The pasted `ssh_config` (at most 256 KiB)
    pub config: String,
    /// Given to every created server
    #[serde(default)]
    pub credential_id: Option<i64>,
    /// Given to every created server
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ImportResult {
    pub created: Vec<ImportedServer>,
    pub skipped: Vec<ImportSkipped>,
    /// Jump hosts set, and ones that could not be
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ImportedServer {
    pub id: i64,
    pub name: String,
    pub hostname: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ImportSkipped {
    pub name: String,
    pub reason: String,
}
