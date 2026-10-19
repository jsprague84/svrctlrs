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

/// A freshly generated ed25519 key pair in OpenSSH text form.
pub struct GeneratedKey {
    pub private_openssh: String,
    pub public_openssh: String,
}

/// Generate an ed25519 key pair. The seed comes from the OS RNG; the key never leaves the
/// server except as the public half.
pub fn generate_ed25519(comment: &str) -> Result<GeneratedKey, String> {
    use rand::RngCore;
    use russh::keys::ssh_key::private::{Ed25519Keypair, KeypairData};
    use russh::keys::ssh_key::LineEnding;

    let mut seed = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut seed);
    let keypair = Ed25519Keypair::from_seed(&seed);
    let key = russh::keys::PrivateKey::new(KeypairData::Ed25519(keypair), comment)
        .map_err(|e| format!("Failed to build key: {e}"))?;
    let private_openssh = key
        .to_openssh(LineEnding::LF)
        .map_err(|e| format!("Failed to encode private key: {e}"))?
        .to_string();
    let public_openssh = key
        .public_key()
        .to_openssh()
        .map_err(|e| format!("Failed to encode public key: {e}"))?;
    Ok(GeneratedKey {
        private_openssh,
        public_openssh,
    })
}

/// The public half of an `ssh_key` credential whose value is key material, in the form that
/// goes into `authorized_keys`. `None` for path credentials or unparsable material.
pub fn public_key_of(cred: &Credential) -> Option<String> {
    if !cred.value_is_key_material() {
        return None;
    }
    let passphrase = cred.passphrase();
    let key = russh::keys::decode_secret_key(&cred.value, passphrase.as_deref()).ok()?;
    key.public_key().to_openssh().ok()
}

/// What the credential list shows about an SSH key instead of its value.
#[derive(Debug, PartialEq, Eq)]
pub struct KeySummary {
    /// `key`: pasted or generated material stored in the database; `path`: a file on the server
    pub source: &'static str,
    pub has_passphrase: bool,
    /// `SHA256:…` of the public half; `None` for a path or material that does not decode
    pub fingerprint: Option<String>,
}

/// Summary of an `ssh_key` credential. It needs the decrypted credential: the list query
/// redacts `value` and `metadata` in SQL, and a redacted value looks like neither a key nor a
/// readable path. `None` for other credential types.
pub fn key_summary(cred: &Credential) -> Option<KeySummary> {
    if !cred.is_ssh_key() {
        return None;
    }
    Some(KeySummary {
        source: if cred.value_is_key_material() {
            "key"
        } else {
            "path"
        },
        has_passphrase: cred.passphrase().is_some(),
        fingerprint: public_key_of(cred)
            .as_deref()
            .map(crate::host_keys::fingerprint),
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

    #[test]
    fn generated_keys_round_trip_and_expose_their_public_half() {
        let g = generate_ed25519("svrctlrs test").unwrap();
        assert!(g
            .private_openssh
            .starts_with("-----BEGIN OPENSSH PRIVATE KEY-----"));
        assert!(g.public_openssh.starts_with("ssh-ed25519 "));
        assert!(g.public_openssh.ends_with("svrctlrs test"));

        let c = cred(&g.private_openssh, None);
        assert_eq!(
            public_key_of(&c).as_deref(),
            Some(g.public_openssh.as_str())
        );
        assert_ne!(
            generate_ed25519("x").unwrap().public_openssh,
            g.public_openssh
        );
        assert_eq!(public_key_of(&cred("/some/path", None)), None);
    }

    #[test]
    fn key_summary_describes_material_paths_and_passphrases() {
        let g = generate_ed25519("svrctlrs test").unwrap();
        let s = key_summary(&cred(&g.private_openssh, None)).unwrap();
        assert_eq!(s.source, "key");
        assert!(!s.has_passphrase);
        assert_eq!(
            s.fingerprint.as_deref(),
            Some(crate::host_keys::fingerprint(&g.public_openssh).as_str())
        );
        assert!(s.fingerprint.unwrap().starts_with("SHA256:"));

        let s = key_summary(&cred(
            "/home/svrctlrs/.ssh/id_ed25519",
            Some(r#"{"passphrase":"pw"}"#),
        ))
        .unwrap();
        assert_eq!(
            s,
            KeySummary {
                source: "path",
                has_passphrase: true,
                fingerprint: None
            }
        );

        // What the list query returns before decryption: must not be taken for a path
        // that someone typed, which is why the list reads the decrypted row.
        let redacted = cred("[REDACTED]", Some(r#"{"redacted":true}"#));
        assert_eq!(key_summary(&redacted).unwrap().source, "path");

        let mut password = cred("hunter2", None);
        password.credential_type_str = "password".to_string();
        assert_eq!(key_summary(&password), None);
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
