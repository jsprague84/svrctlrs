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
    /// Matches a trusted key.
    Trusted,
    /// No trusted key yet. The presented key is recorded, unconfirmed, and the connection is
    /// refused until the user confirms its fingerprint.
    Unconfirmed,
    /// Trusted keys exist and none matches. Holds the trusted key types for the message.
    Mismatch { stored_types: Vec<String> },
}

/// Pure decision, separated so it can be tested without a database. Only *trusted* keys count:
/// an unconfirmed key recorded on an earlier attempt neither matches nor mismatches.
pub fn evaluate(stored: &[ServerHostKey], key_type: &str, public_key: &str) -> Verdict {
    let trusted: Vec<&ServerHostKey> = stored.iter().filter(|k| k.is_trusted()).collect();
    if trusted.is_empty() {
        Verdict::Unconfirmed
    } else if trusted
        .iter()
        .any(|k| k.key_type == key_type && k.public_key == public_key)
    {
        Verdict::Trusted
    } else {
        Verdict::Mismatch {
            stored_types: trusted.iter().map(|k| k.key_type.clone()).collect(),
        }
    }
}

/// Check a presented key and record the outcome: refresh `last_seen_at` on a match, record the
/// key unconfirmed on first contact. A database error is returned as `Err`; never treat it as
/// trust.
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
        Verdict::Unconfirmed => {
            info!(
                server_id,
                key_type,
                fingerprint = %fingerprint(public_key),
                "First contact: host key recorded, waiting for confirmation"
            );
            server_host_keys::record_unconfirmed_host_key(pool, server_id, key_type, public_key)
                .await
                .map_err(|e| format!("Failed to record host key: {e}"))?;
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

/// OpenSSH-style fingerprint (`SHA256:<base64>`) of a key given in OpenSSH text form.
/// Falls back to hashing the whole text when the key is not base64, so it never panics.
pub fn fingerprint(openssh_key: &str) -> String {
    use base64::Engine;
    use sha2::{Digest, Sha256};
    let blob_b64 = openssh_key.split_whitespace().nth(1).unwrap_or(openssh_key);
    let blob = base64::engine::general_purpose::STANDARD
        .decode(blob_b64)
        .unwrap_or_else(|_| openssh_key.as_bytes().to_vec());
    let digest = Sha256::digest(&blob);
    format!(
        "SHA256:{}",
        base64::engine::general_purpose::STANDARD_NO_PAD.encode(digest)
    )
}

/// What the user reads when a server's key is not confirmed yet.
pub fn unconfirmed_message(key_type: &str, fingerprint: &str) -> String {
    format!(
        "This server's host key is not confirmed yet.\r\n\
         {key_type} {fingerprint}\r\n\
         Compare it with `ssh-keygen -lf /etc/ssh/ssh_host_*_key.pub` on the host, then trust it \
         here or under Servers \u{2192} Keys and connect again."
    )
}

/// The warning shown in the terminal when a key does not match.
pub fn mismatch_message(stored_types: &[String], presented_type: &str) -> String {
    format!(
        "\u{26a0}\u{fe0f}  WARNING: SSH HOST KEY MISMATCH for this server!\r\n\
         The server presented a key that does not match any key stored at first contact.\r\n\
         This could indicate a man-in-the-middle attack.\r\n\
         Stored key type(s): {}\r\n\
         Presented key type: {}\r\n\
         Connection refused. If the server's key legitimately changed, forget its stored keys \
         under Servers \u{2192} Keys and connect again.",
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
            trusted_at: Some(Utc::now()),
        }
    }

    fn unconfirmed(key_type: &str, public_key: &str) -> ServerHostKey {
        ServerHostKey {
            trusted_at: None,
            ..stored(key_type, public_key)
        }
    }

    #[test]
    fn fingerprint_is_openssh_style_and_key_specific() {
        // ssh-keygen -lf of this ed25519 key prints SHA256:gfEy1b7DhSz9yu4aS8wK3y3UY8AMz5yY12pjTX5f0X8
        let key =
            "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIGmOh7GDvBC5lBrP6AQ4J2NMRi5PXAqGC9io/BPO4He8";
        let fp = fingerprint(key);
        assert!(fp.starts_with("SHA256:"), "{fp}");
        assert_eq!(fp.len(), "SHA256:".len() + 43);
        assert_ne!(
            fp,
            fingerprint(
                "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
            )
        );
        assert!(fingerprint("not base64 at all").starts_with("SHA256:"));
    }

    #[test]
    fn first_contact_when_nothing_is_trusted() {
        assert_eq!(
            evaluate(&[], "ssh-ed25519", "ssh-ed25519 AAAA1"),
            Verdict::Unconfirmed
        );
        // A key recorded on an earlier attempt but never confirmed is not trust, even when it
        // is the very key presented again.
        let waiting = [unconfirmed("ssh-ed25519", "ssh-ed25519 AAAA1")];
        assert_eq!(
            evaluate(&waiting, "ssh-ed25519", "ssh-ed25519 AAAA1"),
            Verdict::Unconfirmed
        );
        assert_eq!(
            evaluate(&waiting, "ssh-rsa", "ssh-rsa CCCC3"),
            Verdict::Unconfirmed
        );
    }

    #[test]
    fn unconfirmed_keys_do_not_soften_a_mismatch() {
        let keys = [
            stored("ssh-ed25519", "ssh-ed25519 AAAA1"),
            unconfirmed("ssh-rsa", "ssh-rsa CCCC3"),
        ];
        assert_eq!(
            evaluate(&keys, "ssh-rsa", "ssh-rsa CCCC3"),
            Verdict::Mismatch {
                stored_types: vec!["ssh-ed25519".to_string()]
            }
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
