//! The one SSH client. Every connection the server makes — an interactive shell, the
//! connection test, "Authorize key" — goes through [`connect`]: russh, the host key checked
//! against the trusted keys on every handshake (an unconfirmed key is refused until the user
//! confirms its fingerprint; a mismatch is refused outright), and the same
//! authentication order everywhere: a password credential logs in with its password; a key
//! credential tries its key first, then the server's own default keys; no credential tries the
//! default keys alone. Local execution does not exist: a server is something to SSH into.

use std::sync::Arc;

use russh::client::{self, AuthResult, Handle};
use russh::keys::key::PrivateKeyWithHashAlg;
use russh::keys::{Algorithm, HashAlg};
use russh::ChannelMsg;
use svrctlrs_database::models::{Credential, CredentialType, Server};
use svrctlrs_database::sqlx::{Pool, Sqlite};
use tokio::sync::Mutex;
use tracing::{debug, error, info};

use crate::host_keys::{self, Verdict};
use crate::ssh_keys::{self, KeyCandidate};

/// Who to log in as, and where.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub hostname: String,
    pub port: u16,
    pub username: String,
}

impl Target {
    pub fn login(&self) -> String {
        format!("{}@{}:{}", self.username, self.hostname, self.port)
    }
}

/// Resolve the address and user for a server: the server's username, else the credential's
/// default, else `root`.
pub fn target(server: &Server, credential: Option<&Credential>) -> Result<Target, String> {
    let hostname = server.hostname.trim();
    if hostname.is_empty() {
        return Err("Server has no hostname configured".to_string());
    }
    let port = u16::try_from(server.port).map_err(|_| "Server port is out of range".to_string())?;
    let username = server
        .username
        .as_deref()
        .or_else(|| credential.and_then(|c| c.username.as_deref()))
        .filter(|u| !u.is_empty())
        .unwrap_or("root");
    Ok(Target {
        hostname: hostname.to_string(),
        port,
        username: username.to_string(),
    })
}

/// How to authenticate, decided before any network traffic.
#[derive(Debug)]
pub enum AuthPlan {
    Password(String),
    /// Tried in order; the credential's own key comes first.
    Keys(Vec<KeyCandidate>),
}

/// The credential's key first, then the defaults it does not duplicate.
pub fn key_order(primary: Option<KeyCandidate>, defaults: Vec<KeyCandidate>) -> Vec<KeyCandidate> {
    let mut keys: Vec<KeyCandidate> = primary.into_iter().collect();
    for key in defaults {
        if !keys.iter().any(|k| k.label == key.label) {
            keys.push(key);
        }
    }
    keys
}

pub async fn plan_auth(credential: Option<&Credential>) -> Result<AuthPlan, String> {
    match credential {
        Some(cred) => match cred.credential_type() {
            Some(CredentialType::Password) => Ok(AuthPlan::Password(cred.value.clone())),
            Some(CredentialType::SshKey) => {
                let own = ssh_keys::credential_key(cred).await?;
                Ok(AuthPlan::Keys(key_order(
                    Some(own),
                    ssh_keys::default_keys().await,
                )))
            }
            Some(other) => Err(format!("Unsupported credential type for SSH: {other:?}")),
            None => Err(format!(
                "Invalid credential type: {}",
                cred.credential_type_str
            )),
        },
        None => {
            let keys = ssh_keys::default_keys().await;
            if keys.is_empty() {
                return Err(
                    "No credential assigned and no default SSH key found; assign a credential \
                     or install a key under ~/.ssh in the container"
                        .to_string(),
                );
            }
            Ok(AuthPlan::Keys(keys))
        }
    }
}

/// Why [`connect`] refused or failed. The host-key variants are structured so a client can
/// offer the right action (confirm the fingerprint, or forget the stored keys); everything
/// else is a sentence meant for the user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectError {
    /// First contact: the key was recorded and waits for the user's confirmation.
    HostKeyUnconfirmed {
        key_type: String,
        fingerprint: String,
    },
    /// The presented key matches none of the trusted ones.
    HostKeyMismatch(String),
    Failed(String),
}

impl std::fmt::Display for ConnectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::HostKeyUnconfirmed {
                key_type,
                fingerprint,
            } => f.write_str(&host_keys::unconfirmed_message(key_type, fingerprint)),
            Self::HostKeyMismatch(msg) | Self::Failed(msg) => f.write_str(msg),
        }
    }
}

impl std::error::Error for ConnectError {}

impl From<String> for ConnectError {
    fn from(msg: String) -> Self {
        Self::Failed(msg)
    }
}

/// Host-key check for russh against `server_host_keys`: trusted keys pass, a first contact is
/// recorded and refused until confirmed, a mismatch or a database error is refused. A refusal
/// leaves its reason in `refusal` so the caller can show it instead of russh's bare
/// "unknown key".
pub struct Handler {
    pool: Pool<Sqlite>,
    server_id: i64,
    refusal: Arc<Mutex<Option<ConnectError>>>,
}

impl client::Handler for Handler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &russh::keys::PublicKeyOrCertificate,
    ) -> Result<bool, Self::Error> {
        let openssh_key = host_keys::presented_key_openssh(server_public_key)?;
        let key_type = openssh_key
            .split_whitespace()
            .next()
            .unwrap_or("unknown")
            .to_string();
        let refusal =
            match host_keys::verify_and_record(&self.pool, self.server_id, &key_type, &openssh_key)
                .await
            {
                Ok(Verdict::Trusted) => return Ok(true),
                Ok(Verdict::Unconfirmed) => ConnectError::HostKeyUnconfirmed {
                    fingerprint: host_keys::fingerprint(&openssh_key),
                    key_type,
                },
                Ok(Verdict::Mismatch { stored_types }) => ConnectError::HostKeyMismatch(
                    host_keys::mismatch_message(&stored_types, &key_type),
                ),
                Err(e) => {
                    error!(server_id = self.server_id, error = %e,
                    "Host key check failed; refusing connection");
                    ConnectError::Failed(format!("Could not verify the host key: {e}"))
                }
            };
        *self.refusal.lock().await = Some(refusal);
        Ok(false)
    }
}

/// An authenticated session.
pub struct Connection {
    pub handle: Handle<Handler>,
    pub target: Target,
    /// `password`, or the label of the key that was accepted
    pub auth: String,
}

/// Connect and authenticate.
pub async fn connect(
    pool: &Pool<Sqlite>,
    server: &Server,
    credential: Option<&Credential>,
) -> Result<Connection, ConnectError> {
    let target = target(server, credential)?;
    let plan = plan_auth(credential).await?;
    let login = target.login();

    let refusal = Arc::new(Mutex::new(None));
    let handler = Handler {
        pool: pool.clone(),
        server_id: server.id,
        refusal: refusal.clone(),
    };
    let config = Arc::new(client::Config::default());
    let mut handle =
        match client::connect(config, (target.hostname.as_str(), target.port), handler).await {
            Ok(h) => h,
            Err(e) => {
                let why = refusal.lock().await.take();
                return Err(why.unwrap_or_else(|| {
                    ConnectError::Failed(format!("SSH connection to {login} failed: {e}"))
                }));
            }
        };

    let auth = match plan {
        AuthPlan::Password(password) => match handle
            .authenticate_password(target.username.clone(), password)
            .await
        {
            Ok(AuthResult::Success) => "password".to_string(),
            Ok(AuthResult::Failure { .. }) => {
                return Err(ConnectError::Failed(format!(
                    "Password authentication failed for {login}"
                )));
            }
            Err(e) => {
                return Err(ConnectError::Failed(format!(
                    "Password authentication failed for {login}: {e}"
                )))
            }
        },
        AuthPlan::Keys(keys) => {
            let mut tried = Vec::with_capacity(keys.len());
            let mut last_error = String::from("no key could be used");
            let mut accepted = None;
            for key in &keys {
                tried.push(key.label.clone());
                let key_pair = match russh::keys::decode_secret_key(
                    &key.material,
                    key.passphrase.as_deref(),
                ) {
                    Ok(k) => k,
                    Err(e) => {
                        debug!(key = %key.label, error = %e, "Could not decode SSH key");
                        last_error = format!("could not decode {}: {e}", key.label);
                        continue;
                    }
                };
                // RSA needs an explicit SHA-2 signature; legacy ssh-rsa (SHA-1) is refused
                // by modern servers. Other algorithms carry their own.
                let hash_alg = match key_pair.algorithm() {
                    Algorithm::Rsa { .. } => Some(HashAlg::Sha256),
                    _ => None,
                };
                match handle
                    .authenticate_publickey(
                        target.username.clone(),
                        PrivateKeyWithHashAlg::new(Arc::new(key_pair), hash_alg),
                    )
                    .await
                {
                    Ok(AuthResult::Success) => {
                        accepted = Some(key.label.clone());
                        break;
                    }
                    Ok(AuthResult::Failure { .. }) => {
                        debug!(key = %key.label, "SSH key rejected");
                        last_error = format!("{} was rejected", key.label);
                    }
                    Err(e) => {
                        debug!(key = %key.label, error = %e, "SSH authentication error");
                        last_error = format!("{}: {e}", key.label);
                    }
                }
            }
            match accepted {
                Some(label) => label,
                None => {
                    return Err(ConnectError::Failed(format!(
                        "SSH authentication failed for {login}. Tried keys: {}. Last error: {last_error}",
                        tried.join(", ")
                    )));
                }
            }
        }
    };

    info!(login = %login, auth = %auth, "SSH authenticated");
    Ok(Connection {
        handle,
        target,
        auth,
    })
}

/// What one command printed and how it ended. Bytes are kept until the end so a multibyte
/// character split across two channel messages is still decoded whole.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct CommandOutput {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    exit_code: Option<i32>,
}

impl CommandOutput {
    /// Fold one channel message in. `false` once the channel has closed.
    fn absorb(&mut self, msg: &ChannelMsg) -> bool {
        match msg {
            ChannelMsg::Data { data } => self.stdout.extend_from_slice(data),
            ChannelMsg::ExtendedData { data, ext: 1 } => self.stderr.extend_from_slice(data),
            ChannelMsg::ExitStatus { exit_status } => {
                self.exit_code = Some(i32::try_from(*exit_status).unwrap_or(i32::MAX));
            }
            ChannelMsg::Close => return false,
            _ => {}
        }
        true
    }

    /// `(stdout, stderr, exit code)`; the code is -1 when the server never reported one.
    pub fn finish(self) -> (String, String, i32) {
        (
            String::from_utf8_lossy(&self.stdout).into_owned(),
            String::from_utf8_lossy(&self.stderr).into_owned(),
            self.exit_code.unwrap_or(-1),
        )
    }
}

/// Run one command without a PTY and collect its output.
pub async fn run(
    pool: &Pool<Sqlite>,
    server: &Server,
    credential: Option<&Credential>,
    command: &str,
) -> Result<(String, String, i32), ConnectError> {
    let conn = connect(pool, server, credential).await?;
    let mut channel = conn
        .handle
        .channel_open_session()
        .await
        .map_err(|e| ConnectError::Failed(format!("Failed to open channel: {e}")))?;
    channel
        .exec(true, command)
        .await
        .map_err(|e| ConnectError::Failed(format!("Failed to run command: {e}")))?;
    let mut output = CommandOutput::default();
    while let Some(msg) = channel.wait().await {
        if !output.absorb(&msg) {
            break;
        }
    }
    let _ = conn
        .handle
        .disconnect(russh::Disconnect::ByApplication, "", "English")
        .await;
    Ok(output.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server(hostname: &str, username: Option<&str>) -> Server {
        Server {
            id: 1,
            name: "web".into(),
            hostname: hostname.into(),
            port: 22,
            username: username.map(String::from),
            credential_id: None,
            description: None,
            enabled: true,
            last_seen_at: None,
            last_error: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    fn credential(username: Option<&str>) -> Credential {
        Credential {
            id: 2,
            name: "deploy".into(),
            credential_type_str: "ssh_key".into(),
            description: None,
            value: String::new(),
            username: username.map(String::from),
            metadata: None,
            encrypted: true,
            metadata_encrypted: true,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    fn key(label: &str) -> KeyCandidate {
        KeyCandidate {
            label: label.into(),
            material: String::new(),
            passphrase: None,
        }
    }

    #[test]
    fn target_prefers_the_servers_user_then_the_credentials_then_root() {
        let s = server(" host.example ", Some("ops"));
        let t = target(&s, Some(&credential(Some("deploy")))).unwrap();
        assert_eq!(t.login(), "ops@host.example:22");

        let s = server("host.example", None);
        assert_eq!(
            target(&s, Some(&credential(Some("deploy"))))
                .unwrap()
                .username,
            "deploy"
        );
        assert_eq!(target(&s, None).unwrap().username, "root");
        assert_eq!(
            target(&s, Some(&credential(Some("")))).unwrap().username,
            "root"
        );
    }

    #[test]
    fn target_refuses_missing_hostnames_and_bad_ports() {
        assert!(target(&server("", None), None).is_err());
        assert!(target(&server("  ", None), None).is_err());
        let mut s = server("x", None);
        s.port = 70000;
        assert!(target(&s, None).unwrap_err().contains("port"));
    }

    #[test]
    fn credential_key_goes_first_and_defaults_are_not_repeated() {
        let order = key_order(
            Some(key("~/.ssh/deploy")),
            vec![key("~/.ssh/id_ed25519"), key("~/.ssh/deploy")],
        );
        let labels: Vec<_> = order.iter().map(|k| k.label.as_str()).collect();
        assert_eq!(labels, ["~/.ssh/deploy", "~/.ssh/id_ed25519"]);
        assert_eq!(key_order(None, vec![key("a")]).len(), 1);
    }

    #[test]
    fn command_output_separates_streams_and_joins_split_characters() {
        let mut out = CommandOutput::default();
        // "✓" is E2 9C 93; the server may split it between two Data messages
        assert!(out.absorb(&ChannelMsg::Data {
            data: (&[b'o', b'k', 0xE2, 0x9C][..]).into()
        }));
        assert!(out.absorb(&ChannelMsg::Data {
            data: (&[0x93, b'\n'][..]).into()
        }));
        assert!(out.absorb(&ChannelMsg::ExtendedData {
            data: (&b"warn\n"[..]).into(),
            ext: 1
        }));
        assert!(out.absorb(&ChannelMsg::ExtendedData {
            data: (&b"ignored"[..]).into(),
            ext: 7
        }));
        assert!(out.absorb(&ChannelMsg::ExitStatus { exit_status: 3 }));
        assert!(out.absorb(&ChannelMsg::Eof));
        assert!(!out.absorb(&ChannelMsg::Close), "close ends the read loop");
        assert_eq!(out.finish(), ("ok✓\n".to_string(), "warn\n".to_string(), 3));

        assert_eq!(
            CommandOutput::default().finish().2,
            -1,
            "no exit status reported"
        );
    }
}
