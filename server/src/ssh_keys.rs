//! Loading SSH private keys for the connectors.
//!
//! A credential's `value` is either the key material itself (pasted into the UI, stored
//! encrypted) or a path to a key file readable by the server. Both connectors used to treat
//! it as a path only, so pasted keys could never connect.

use svrctlrs_database::models::Credential;

/// A private key ready to be decoded, with where it came from for log messages.
pub struct KeyCandidate {
    pub label: String,
    pub material: String,
    pub passphrase: Option<String>,
}

impl std::fmt::Debug for KeyCandidate {
    /// Never prints the key or its passphrase.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeyCandidate")
            .field("label", &self.label)
            .field("material", &"[REDACTED]")
            .field(
                "passphrase",
                &self.passphrase.as_ref().map(|_| "[REDACTED]"),
            )
            .finish()
    }
}

/// The key described by an `ssh_key` credential: its material, read from disk when the
/// value is a path, plus the passphrase from its metadata.
pub async fn credential_key(cred: &Credential) -> Result<KeyCandidate, String> {
    use svrctlrs_database::models::{ssh_key_value_kind, SshKeyValueKind};

    let material = match ssh_key_value_kind(&cred.value) {
        SshKeyValueKind::PrivateKey => cred.value.clone(),
        SshKeyValueKind::PublicKey => {
            return Err(format!(
                "Credential '{}' holds a PUBLIC key; SSH needs the private key. Edit the \
                 credential and paste the contents of the key file without .pub.",
                cred.name
            ));
        }
        SshKeyValueKind::Path => tokio::fs::read_to_string(&cred.value).await.map_err(|e| {
            format!(
                "Credential '{}': cannot read the key file {} on the SvrCtlRS server ({e}). \
                 Paths refer to the server's own filesystem, not your machine; paste the \
                 private key into the credential instead.",
                cred.name, cred.value
            )
        })?,
    };
    Ok(KeyCandidate {
        label: if cred.value_is_key_material() {
            format!("credential '{}'", cred.name)
        } else {
            cred.value.clone()
        },
        material,
        passphrase: cred.passphrase(),
    })
}

/// The server's own default keys (`~/.ssh/id_*`), used when a server has no credential.
pub async fn default_keys() -> Vec<KeyCandidate> {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let mut keys = Vec::new();
    for name in ["id_ed25519", "id_rsa", "id_ecdsa", "id_dsa"] {
        let path = format!("{home}/.ssh/{name}");
        if let Ok(material) = tokio::fs::read_to_string(&path).await {
            keys.push(KeyCandidate {
                label: path,
                material,
                passphrase: None,
            });
        }
    }
    keys
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn cred(value: &str, metadata: Option<&str>) -> Credential {
        Credential {
            id: 1,
            name: "k".to_string(),
            credential_type_str: "ssh_key".to_string(),
            description: None,
            value: value.to_string(),
            username: None,
            metadata: metadata.map(str::to_string),
            encrypted: true,
            metadata_encrypted: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn pasted_material_is_used_directly_with_its_passphrase() {
        let c = cred(
            "-----BEGIN OPENSSH PRIVATE KEY-----\nabc\n-----END OPENSSH PRIVATE KEY-----\n",
            Some(r#"{"passphrase":"pw"}"#),
        );
        let key = credential_key(&c).await.unwrap();
        assert!(key.material.starts_with("-----BEGIN"));
        assert_eq!(key.passphrase.as_deref(), Some("pw"));
        assert_eq!(key.label, "credential 'k'");
    }

    #[tokio::test]
    async fn path_values_are_read_from_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("id_test");
        std::fs::write(&path, "-----BEGIN OPENSSH PRIVATE KEY-----\nfile\n").unwrap();
        let c = cred(path.to_str().unwrap(), None);
        let key = credential_key(&c).await.unwrap();
        assert!(key.material.contains("file"));
        assert_eq!(key.passphrase, None);

        let missing = cred("/nonexistent/key", None);
        let err = credential_key(&missing).await.unwrap_err();
        assert!(
            err.contains("/nonexistent/key") && err.contains("paste"),
            "{err}"
        );

        let public = cred("ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIGmO user@t490", None);
        assert!(credential_key(&public)
            .await
            .unwrap_err()
            .contains("PUBLIC"));
    }
}
