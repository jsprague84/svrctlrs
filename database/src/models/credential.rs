use std::fmt;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::FromRow;

/// Credential types supported by the system
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialType {
    SshKey,
    ApiToken,
    Password,
    Certificate,
}

impl CredentialType {
    /// Parse credential type from string
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "ssh_key" => Some(Self::SshKey),
            "api_token" => Some(Self::ApiToken),
            "password" => Some(Self::Password),
            "certificate" => Some(Self::Certificate),
            _ => None,
        }
    }

    /// Convert credential type to string
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::SshKey => "ssh_key",
            Self::ApiToken => "api_token",
            Self::Password => "password",
            Self::Certificate => "certificate",
        }
    }
}

/// Credential model - stores SSH keys, API tokens, passwords, certificates
#[derive(Clone, Serialize, Deserialize, FromRow)]
pub struct Credential {
    pub id: i64,
    pub name: String,
    #[sqlx(rename = "credential_type")]
    pub credential_type_str: String,
    pub description: Option<String>,

    /// Credential value: a password, token, SSH key *material* (`-----BEGIN ...`) or a path
    /// to a key file readable by the server. Encrypted at rest.
    pub value: String,

    /// Username (for password type credentials)
    pub username: Option<String>,

    /// Additional metadata as a JSON object string, e.g. `{"passphrase": "..."}` for an
    /// encrypted SSH key. Encrypted at rest.
    pub metadata: Option<String>,

    /// Whether `value` is encrypted at rest
    pub encrypted: bool,

    /// Whether `metadata` is encrypted at rest
    pub metadata_encrypted: bool,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl fmt::Debug for Credential {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Credential")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("credential_type_str", &self.credential_type_str)
            .field("description", &self.description)
            .field("value", &"[REDACTED]")
            .field("username", &self.username)
            .field("metadata", &self.metadata.as_ref().map(|_| "[REDACTED]"))
            .field("encrypted", &self.encrypted)
            .field("metadata_encrypted", &self.metadata_encrypted)
            .field("created_at", &self.created_at)
            .field("updated_at", &self.updated_at)
            .finish()
    }
}

impl Credential {
    /// Get the credential type as an enum
    pub fn credential_type(&self) -> Option<CredentialType> {
        CredentialType::from_str(&self.credential_type_str)
    }

    /// Get metadata as JSON value
    pub fn get_metadata(&self) -> JsonValue {
        self.metadata
            .as_ref()
            .and_then(|m| serde_json::from_str(m).ok())
            .unwrap_or(JsonValue::Object(serde_json::Map::new()))
    }

    /// The SSH key passphrase stored in metadata, if any.
    pub fn passphrase(&self) -> Option<String> {
        self.get_metadata()
            .get("passphrase")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    }

    /// Whether `value` holds key material rather than a path to a key file.
    pub fn value_is_key_material(&self) -> bool {
        ssh_key_value_kind(&self.value) == SshKeyValueKind::PrivateKey
    }

    /// Check if this is an SSH key credential
    pub fn is_ssh_key(&self) -> bool {
        self.credential_type() == Some(CredentialType::SshKey)
    }

    /// Check if this is an API token credential
    pub fn is_api_token(&self) -> bool {
        self.credential_type() == Some(CredentialType::ApiToken)
    }

    /// Check if this is a password credential
    pub fn is_password(&self) -> bool {
        self.credential_type() == Some(CredentialType::Password)
    }

    /// Check if this is a certificate credential
    pub fn is_certificate(&self) -> bool {
        self.credential_type() == Some(CredentialType::Certificate)
    }
}

/// What an `ssh_key` credential's `value` looks like.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SshKeyValueKind {
    /// Private key material (`-----BEGIN ... PRIVATE KEY-----`): what SSH authentication needs.
    PrivateKey,
    /// A *public* key (`ssh-ed25519 AAAA...`, `ssh-rsa ...`): useless for authenticating.
    PublicKey,
    /// Anything else is taken as a path on the server's filesystem.
    Path,
}

/// Classify an `ssh_key` credential value.
pub fn ssh_key_value_kind(value: &str) -> SshKeyValueKind {
    let v = value.trim_start();
    if v.starts_with("-----BEGIN") {
        SshKeyValueKind::PrivateKey
    } else if [
        "ssh-ed25519 ",
        "ssh-rsa ",
        "ssh-dss ",
        "ecdsa-sha2-",
        "sk-ssh-ed25519@",
        "sk-ecdsa-",
    ]
    .iter()
    .any(|p| v.starts_with(p))
    {
        SshKeyValueKind::PublicKey
    } else {
        SshKeyValueKind::Path
    }
}

/// Why an `ssh_key` value cannot work, in words for the person who typed it; `None` when it
/// is acceptable (private key material, or an absolute path for the server to read).
pub fn ssh_key_value_problem(value: &str) -> Option<String> {
    let v = value.trim();
    match ssh_key_value_kind(v) {
        SshKeyValueKind::PrivateKey => None,
        SshKeyValueKind::PublicKey => Some(
            "That is a public key (it starts with ssh-ed25519/ssh-rsa). SSH needs the PRIVATE \
             key: paste the contents of the file without .pub, which starts with \
             -----BEGIN OPENSSH PRIVATE KEY-----."
                .to_string(),
        ),
        SshKeyValueKind::Path if v.ends_with(".pub") => Some(
            "That path is a public key (.pub). Give the private key file instead, or paste its \
             contents."
                .to_string(),
        ),
        SshKeyValueKind::Path if !v.starts_with('/') => Some(
            "A key path must be absolute and refer to the SvrCtlRS server's own filesystem (inside \
             its container, e.g. /home/svrctlrs/.ssh/id_ed25519). To use a key from your machine, \
             paste the private key's contents instead."
                .to_string(),
        ),
        SshKeyValueKind::Path => None,
    }
}

/// Input for creating a new credential
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCredential {
    pub name: String,
    #[serde(rename = "type")]
    pub credential_type: CredentialType,
    pub description: Option<String>,
    pub value: String,
    pub username: Option<String>,
    pub metadata: Option<JsonValue>,
}

impl CreateCredential {
    /// Convert metadata to JSON string for database storage
    pub fn metadata_string(&self) -> Option<String> {
        self.metadata
            .as_ref()
            .and_then(|m| serde_json::to_string(m).ok())
    }
}

/// Input for updating an existing credential
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateCredential {
    pub name: Option<String>,
    pub description: Option<String>,
    pub value: Option<String>,
    pub username: Option<String>,
    pub metadata: Option<JsonValue>,
}

impl UpdateCredential {
    /// Convert metadata to JSON string for database storage
    pub fn metadata_string(&self) -> Option<String> {
        self.metadata
            .as_ref()
            .and_then(|m| serde_json::to_string(m).ok())
    }

    /// Check if this update contains any changes
    pub fn has_changes(&self) -> bool {
        self.name.is_some()
            || self.description.is_some()
            || self.value.is_some()
            || self.username.is_some()
            || self.metadata.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_credential_type_conversion() {
        assert_eq!(
            CredentialType::from_str("ssh_key"),
            Some(CredentialType::SshKey)
        );
        assert_eq!(
            CredentialType::from_str("api_token"),
            Some(CredentialType::ApiToken)
        );
        assert_eq!(
            CredentialType::from_str("password"),
            Some(CredentialType::Password)
        );
        assert_eq!(
            CredentialType::from_str("certificate"),
            Some(CredentialType::Certificate)
        );
        assert_eq!(CredentialType::from_str("invalid"), None);

        assert_eq!(CredentialType::SshKey.as_str(), "ssh_key");
        assert_eq!(CredentialType::ApiToken.as_str(), "api_token");
        assert_eq!(CredentialType::Password.as_str(), "password");
        assert_eq!(CredentialType::Certificate.as_str(), "certificate");
    }

    #[test]
    fn passphrase_and_key_material_helpers() {
        let mut cred = Credential {
            id: 1,
            name: "k".to_string(),
            credential_type_str: "ssh_key".to_string(),
            description: None,
            value: "/home/svrctlrs/.ssh/id_ed25519".to_string(),
            username: None,
            metadata: Some(r#"{"passphrase": "open sesame"}"#.to_string()),
            encrypted: true,
            metadata_encrypted: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        assert_eq!(cred.passphrase().as_deref(), Some("open sesame"));
        assert!(!cred.value_is_key_material());
        assert!(!format!("{cred:?}").contains("open sesame"));

        cred.value = "-----BEGIN OPENSSH PRIVATE KEY-----\nabc\n".to_string();
        cred.metadata = Some(r#"{"passphrase": ""}"#.to_string());
        assert!(cred.value_is_key_material());
        assert_eq!(cred.passphrase(), None);
    }

    #[test]
    fn ssh_key_values_are_classified_and_explained() {
        assert_eq!(
            ssh_key_value_kind("-----BEGIN OPENSSH PRIVATE KEY-----\nabc"),
            SshKeyValueKind::PrivateKey
        );
        assert_eq!(
            ssh_key_value_kind("ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIGmO user@t490"),
            SshKeyValueKind::PublicKey
        );
        assert_eq!(
            ssh_key_value_kind("/home/svrctlrs/.ssh/id_ed25519"),
            SshKeyValueKind::Path
        );

        assert!(ssh_key_value_problem("-----BEGIN OPENSSH PRIVATE KEY-----\nabc").is_none());
        assert!(ssh_key_value_problem("/home/svrctlrs/.ssh/id_ed25519").is_none());
        assert!(ssh_key_value_problem("ssh-ed25519 AAAA x")
            .unwrap()
            .contains("PRIVATE"));
        assert!(ssh_key_value_problem("/home/me/.ssh/id_ed25519.pub")
            .unwrap()
            .contains(".pub"));
        assert!(ssh_key_value_problem("~/.ssh/id_ed25519")
            .unwrap()
            .contains("absolute"));
    }

    #[test]
    fn test_credential_type_checks() {
        let cred = Credential {
            id: 1,
            name: "test-key".to_string(),
            credential_type_str: "ssh_key".to_string(),
            description: None,
            value: "/path/to/key".to_string(),
            username: None,
            metadata: None,
            encrypted: false,
            metadata_encrypted: false,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        assert!(cred.is_ssh_key());
        assert!(!cred.is_api_token());
        assert!(!cred.is_password());
        assert!(!cred.is_certificate());
    }
}
