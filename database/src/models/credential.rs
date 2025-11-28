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
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Credential {
    pub id: i64,
    pub name: String,
    #[sqlx(rename = "credential_type")]
    pub credential_type_str: String,
    pub description: Option<String>,

    /// Credential value (SSH key path, token value, password, cert path)
    /// WARNING: This should be encrypted in production
    pub value: String,

    /// Username (for password type credentials)
    pub username: Option<String>,

    /// Additional metadata as JSON string
    /// Examples: {"key_passphrase": "...", "port": 2222}
    pub metadata: Option<String>,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
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
    fn test_credential_type_checks() {
        let cred = Credential {
            id: 1,
            name: "test-key".to_string(),
            credential_type_str: "ssh_key".to_string(),
            description: None,
            value: "/path/to/key".to_string(),
            username: None,
            metadata: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        assert!(cred.is_ssh_key());
        assert!(!cred.is_api_token());
        assert!(!cred.is_password());
        assert!(!cred.is_certificate());
    }
}
