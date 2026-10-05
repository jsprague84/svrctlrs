//! SSH host-key trust shared by the PTY and command connectors (trust on first use).
//!
//! The decision looks at *every* key stored for a server, not only the one of the presented
//! type: a server that was trusted with an ed25519 key and now offers an unknown RSA key is a
//! mismatch, otherwise an attacker could bypass the check by negotiating a different
//! algorithm. Database failures are errors, and callers refuse the connection on them.

use svrctlrs_database::{models::ServerHostKey, queries::server_host_keys, SqlxPool, SqlxSqlite};
use tracing::{error, info};

/// Outcome of checking a presented host key against what is stored for the server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// Matches a stored key.
    Trusted,
    /// Nothing stored yet; the key has been recorded.
    FirstSeen,
    /// Stored keys exist and none matches. Holds the stored key types for the message.
    Mismatch { stored_types: Vec<String> },
}

/// Pure decision, separated so it can be tested without a database.
pub fn evaluate(stored: &[ServerHostKey], key_type: &str, public_key: &str) -> Verdict {
    if stored.is_empty() {
        Verdict::FirstSeen
    } else if stored
        .iter()
        .any(|k| k.key_type == key_type && k.public_key == public_key)
    {
        Verdict::Trusted
    } else {
        Verdict::Mismatch {
            stored_types: stored.iter().map(|k| k.key_type.clone()).collect(),
        }
    }
}

/// Check a presented key and record the outcome: refresh `last_seen_at` on a match, store the
/// key on first contact. A database error is returned as `Err`; never treat it as trust.
pub async fn verify_and_record(
    pool: &SqlxPool<SqlxSqlite>,
    server_id: i64,
    key_type: &str,
    public_key: &str,
) -> Result<Verdict, String> {
    let stored = server_host_keys::list_host_keys_for_server(pool, server_id)
        .await
        .map_err(|e| format!("Failed to read stored host keys: {e}"))?;

    let verdict = evaluate(&stored, key_type, public_key);
    match &verdict {
        Verdict::Trusted => {
            info!(server_id, key_type, "Host key verification successful");
            server_host_keys::update_host_key_last_seen(pool, server_id, key_type)
                .await
                .ok();
        }
        Verdict::FirstSeen => {
            info!(
                server_id,
                key_type, "First connection - storing host key (TOFU)"
            );
            server_host_keys::store_host_key(pool, server_id, key_type, public_key)
                .await
                .map_err(|e| format!("Failed to store host key: {e}"))?;
        }
        Verdict::Mismatch { stored_types } => {
            error!(
                server_id,
                presented_key_type = key_type,
                stored_key_types = ?stored_types,
                "HOST KEY MISMATCH - possible MITM attack!"
            );
        }
    }
    Ok(verdict)
}

/// The OpenSSH text of what the server presented: its host key, or the key embedded in its
/// host certificate. There is no CA trust store, so a certificate is pinned by that key like
/// any other host key.
pub fn presented_key_openssh(
    presented: &russh::keys::PublicKeyOrCertificate,
) -> Result<String, russh::Error> {
    use russh::keys::PublicKeyOrCertificate;
    let text = match presented {
        PublicKeyOrCertificate::PublicKey { key, .. } => key.to_openssh(),
        PublicKeyOrCertificate::Certificate(cert) => {
            russh::keys::PublicKey::from(cert.public_key().clone()).to_openssh()
        }
    };
    text.map_err(|_| russh::Error::CouldNotReadKey)
}

/// The warning shown in the terminal when a key does not match.
pub fn mismatch_message(stored_types: &[String], presented_type: &str) -> String {
    format!(
        "\u{26a0}\u{fe0f}  WARNING: SSH HOST KEY MISMATCH for this server!\r\n\
         The server presented a key that does not match any key stored at first contact.\r\n\
         This could indicate a man-in-the-middle attack.\r\n\
         Stored key type(s): {}\r\n\
         Presented key type: {}\r\n\
         Connection refused. If the server's key legitimately changed, an administrator must \
         remove the stored key(s) for this server before reconnecting.",
        stored_types.join(", "),
        presented_type
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn stored(key_type: &str, public_key: &str) -> ServerHostKey {
        ServerHostKey {
            id: 1,
            server_id: 1,
            key_type: key_type.to_string(),
            public_key: public_key.to_string(),
            first_seen_at: Utc::now(),
            last_seen_at: Utc::now(),
        }
    }

    #[test]
    fn first_contact_when_nothing_is_stored() {
        assert_eq!(
            evaluate(&[], "ssh-ed25519", "ssh-ed25519 AAAA1"),
            Verdict::FirstSeen
        );
    }

    #[test]
    fn exact_match_is_trusted() {
        let keys = [stored("ssh-ed25519", "ssh-ed25519 AAAA1")];
        assert_eq!(
            evaluate(&keys, "ssh-ed25519", "ssh-ed25519 AAAA1"),
            Verdict::Trusted
        );
    }

    #[test]
    fn different_key_of_same_type_is_a_mismatch() {
        let keys = [stored("ssh-ed25519", "ssh-ed25519 AAAA1")];
        assert_eq!(
            evaluate(&keys, "ssh-ed25519", "ssh-ed25519 BBBB2"),
            Verdict::Mismatch {
                stored_types: vec!["ssh-ed25519".to_string()]
            }
        );
    }

    #[test]
    fn unknown_key_type_is_a_mismatch_not_first_contact() {
        // This was the bypass: the old check looked up (server, key_type) and treated an
        // unseen type as a first contact.
        let keys = [stored("ssh-ed25519", "ssh-ed25519 AAAA1")];
        assert_eq!(
            evaluate(&keys, "ssh-rsa", "ssh-rsa CCCC3"),
            Verdict::Mismatch {
                stored_types: vec!["ssh-ed25519".to_string()]
            }
        );
    }

    #[test]
    fn any_stored_key_may_match() {
        let keys = [
            stored("ssh-ed25519", "ssh-ed25519 AAAA1"),
            stored("ssh-rsa", "ssh-rsa CCCC3"),
        ];
        assert_eq!(
            evaluate(&keys, "ssh-rsa", "ssh-rsa CCCC3"),
            Verdict::Trusted
        );
    }
}
