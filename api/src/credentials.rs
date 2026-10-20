//! Stored credentials: passwords and SSH keys, encrypted at rest. Values never come back.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// A credential in the list (values redacted).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CredentialSummary {
    pub id: i64,
    pub name: String,
    /// `password`, `ssh_key` or `api_token`
    pub credential_type: String,
    pub description: Option<String>,
    /// The login it comes with, if any
    pub username: Option<String>,
    pub has_value: bool,
    pub has_passphrase: bool,
    /// SSH keys: `key` (material stored here) or `path` (a file on the server)
    pub key_source: Option<String>,
    /// SSH keys: `SHA256:…` of the public half
    pub fingerprint: Option<String>,
    /// Some server uses it (deleting it is refused)
    pub in_use: bool,
    /// The names of the servers that use it
    pub used_by: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CredentialList {
    pub credentials: Vec<CredentialSummary>,
}

/// One credential (value redacted; an SSH key's public half included).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CredentialDetail {
    pub id: i64,
    pub name: String,
    pub credential_type: String,
    pub description: Option<String>,
    pub username: Option<String>,
    pub has_value: bool,
    pub has_passphrase: bool,
    pub key_source: Option<String>,
    pub fingerprint: Option<String>,
    /// SSH keys: the line for `authorized_keys`
    pub public_key: Option<String>,
    pub in_use: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// `POST /credentials/generate-key`: an ed25519 key made on the server; the private half
/// never leaves it.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct GenerateKey {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    /// Comment at the end of the public key line (default `svrctlrs:<name>`)
    #[serde(default)]
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct GeneratedKey {
    pub id: i64,
    /// The line for `authorized_keys`
    pub public_key: String,
    pub message: String,
}

/// `POST /credentials`
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct NewCredential {
    pub name: String,
    /// `password`, `ssh_key` or `api_token`
    pub credential_type: String,
    /// The password, or OpenSSH private key material (or a key path on the server)
    pub value: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub username: Option<String>,
    /// Passphrase of an encrypted SSH key
    #[serde(default)]
    pub passphrase: Option<String>,
}

/// `PUT /credentials/{id}`: only the fields given change
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
pub struct CredentialChanges {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub value: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub username: Option<String>,
    /// Omit to keep the current passphrase; an empty string removes it
    #[serde(default)]
    pub passphrase: Option<String>,
}
