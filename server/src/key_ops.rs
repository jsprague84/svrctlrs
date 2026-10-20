//! Installing and removing a key credential's public half on a server, through a way in that
//! already works. Shared by "Authorize key", onboarding and key rotation. Only public keys
//! travel; the private halves stay in the encrypted credentials.

use svrctlrs_database::models::{Credential, Server, UpdateServer};
use svrctlrs_database::queries::servers;
use svrctlrs_database::sqlx::{Pool, Sqlite};

use crate::routes::api::ApiError;

/// What `ssh-copy-id` does: append the public key to `~/.ssh/authorized_keys` with sane
/// permissions, idempotently.
pub fn install_command(public_key: &str) -> String {
    let escaped = public_key.replace('\'', "'\\''");
    format!(
        "umask 077 && mkdir -p ~/.ssh && touch ~/.ssh/authorized_keys && \
         (grep -qxF '{escaped}' ~/.ssh/authorized_keys || printf '%s\\n' '{escaped}' >> ~/.ssh/authorized_keys) && \
         chmod 700 ~/.ssh && chmod 600 ~/.ssh/authorized_keys && echo svrctlrs-key-installed"
    )
}

/// Remove exactly that line from `~/.ssh/authorized_keys`, keeping the rest and the file's
/// permissions; replaces the file in one `mv`, so a failure leaves it as it was.
pub fn removal_command(public_key: &str) -> String {
    let escaped = public_key.replace('\'', "'\\''");
    format!(
        "umask 077 && f=~/.ssh/authorized_keys && t=$(mktemp ~/.ssh/.svrctlrs.XXXXXX) && \
         {{ grep -vxF '{escaped}' \"$f\" > \"$t\" || true; }} && chmod 600 \"$t\" && \
         mv \"$t\" \"$f\" && echo svrctlrs-key-removed"
    )
}

/// The public half of a key credential, or why there is none.
pub fn public_half(key: &Credential) -> Result<String, ApiError> {
    if !key.is_ssh_key() {
        return Err(ApiError::bad_request("The credential must be an SSH key"));
    }
    crate::ssh_keys::public_key_of(key).ok_or_else(|| {
        ApiError::bad_request(
            "Cannot derive the public key: the credential must hold private key material \
             (generate a key, or paste one), not a path",
        )
    })
}

/// Install `key`'s public half on `server` logging in with `way_in`, prove that `key` now
/// logs in, and make it the server's credential.
pub async fn install_and_switch(
    pool: &Pool<Sqlite>,
    server: &Server,
    key: &Credential,
    way_in: &Credential,
) -> Result<(), ApiError> {
    let public_key = public_half(key)?;
    let (stdout, stderr, code) =
        crate::ssh::run(pool, server, Some(way_in), &install_command(&public_key))
            .await
            .map_err(|e| {
                // A host-key refusal keeps its code and details so the client can offer the fix
                let crate::ssh::ConnectError::Failed(e) = e else {
                    return ApiError::from(e);
                };
                let login = format!(
                    "{}@{}:{}",
                    server.username.as_deref().unwrap_or("root"),
                    server.hostname,
                    server.port
                );
                let hint = if e.contains("Password authentication failed") {
                    " Either the password is wrong, or this server does not allow password logins \
                     at all (sshd `PasswordAuthentication no`, the default on hardened hosts). In \
                     that case bootstrap with an existing key credential instead, or copy the \
                     public key into ~/.ssh/authorized_keys by hand (Credentials → Public key)."
                } else {
                    ""
                };
                ApiError::bad_request(format!(
                    "Could not log in as {login} to install the key: {e}.{hint}"
                ))
            })?;
    if code != 0 || !stdout.contains("svrctlrs-key-installed") {
        return Err(ApiError::bad_request(format!(
            "Installing the key failed (exit {code}): {}",
            stderr.trim()
        )));
    }

    // Prove the key works before switching the server over to it.
    let (_, stderr, code) = crate::ssh::run(pool, server, Some(key), "echo svrctlrs-key-ok")
        .await
        .map_err(|e| {
            ApiError::bad_request(format!("Key installed, but logging in with it failed: {e}"))
        })?;
    if code != 0 {
        return Err(ApiError::bad_request(format!(
            "Key installed, but logging in with it failed (exit {code}): {}",
            stderr.trim()
        )));
    }

    let update = UpdateServer {
        credential_id: Some(Some(key.id)),
        ..Default::default()
    };
    servers::update_server(pool, server.id, &update)
        .await
        .map_err(|e| {
            ApiError::internal_error(format!("Key works, but updating the server failed: {e}"))
        })?;
    Ok(())
}

/// Remove `old`'s public half from `server`'s `authorized_keys`, logging in with `way_in`.
pub async fn remove(
    pool: &Pool<Sqlite>,
    server: &Server,
    old: &Credential,
    way_in: &Credential,
) -> Result<(), ApiError> {
    let public_key = public_half(old)?;
    let (stdout, stderr, code) =
        crate::ssh::run(pool, server, Some(way_in), &removal_command(&public_key))
            .await
            .map_err(ApiError::from)?;
    if code != 0 || !stdout.contains("svrctlrs-key-removed") {
        return Err(ApiError::bad_request(format!(
            "Removing the old key failed (exit {code}): {}",
            stderr.trim()
        )));
    }
    Ok(())
}

/// Generate an ed25519 key and store it as a credential named `base` (or `base (2)`, … when
/// that name is taken). The private half never leaves the database.
pub async fn generate_credential(pool: &Pool<Sqlite>, base: &str) -> Result<Credential, ApiError> {
    use svrctlrs_database::models::{CreateCredential, CredentialType};
    use svrctlrs_database::queries::credentials;

    let generated = crate::ssh_keys::generate_ed25519(&format!("svrctlrs:{base}"))
        .map_err(|e| ApiError::internal_error(format!("Failed to generate key: {e}")))?;
    for n in 1..=20 {
        let name = if n == 1 {
            base.to_string()
        } else {
            format!("{base} ({n})")
        };
        let create = CreateCredential {
            name,
            credential_type: CredentialType::SshKey,
            value: generated.private_openssh.clone(),
            description: Some("Generated by svrctlrs".to_string()),
            username: None,
            metadata: None,
        };
        match credentials::create_credential(pool, &create).await {
            Ok(id) => return Ok(credentials::get_credential(pool, id).await?),
            Err(svrctlrs_core::Error::Conflict(_)) => continue,
            Err(e) => return Err(e.into()),
        }
    }
    Err(ApiError::conflict(format!(
        "Too many credentials named like {base}; pick another name"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_command_is_idempotent_and_quotes_the_key() {
        let cmd = install_command("ssh-ed25519 AAAA it's");
        assert!(cmd.contains("grep -qxF 'ssh-ed25519 AAAA it'\\''s'"));
        assert!(cmd.contains("chmod 600 ~/.ssh/authorized_keys"));
        assert!(cmd.ends_with("echo svrctlrs-key-installed"));
    }

    #[test]
    fn removal_command_drops_one_exact_line_atomically() {
        let cmd = removal_command("ssh-ed25519 AAAA old");
        assert!(cmd.contains("grep -vxF 'ssh-ed25519 AAAA old' \"$f\" > \"$t\" || true"));
        assert!(cmd.contains("mv \"$t\" \"$f\""));
        assert!(cmd.ends_with("echo svrctlrs-key-removed"));
    }
}
