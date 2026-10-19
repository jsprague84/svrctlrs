//! The one SSH client. Every connection the server makes — an interactive shell, the
//! connection test, "Authorize key" — goes through [`connect`]: russh, the host key checked
//! against the trusted keys on every handshake (an unconfirmed key is refused until the user
//! confirms its fingerprint; a mismatch is refused outright), and the same
//! authentication order everywhere: a password credential logs in with its password; a key
//! credential tries its key first, then the server's own default keys; no credential tries the
//! default keys alone. A server with a jump host (`via_server_id`) is reached through a
//! `direct-tcpip` channel of the jump host's own connection, which is made the same way (its
//! host key checked, its credential used), so a chain of jumps is just recursion. Every
//! connection sends keepalives (`SSH_KEEPALIVE_SECS`, default 15) so NAT boxes and phone
//! networks do not drop an idle shell. Local execution does not exist: a server is something
//! to SSH into.

use std::sync::Arc;

use russh::client::{self, AuthResult, Handle};
use russh::keys::key::PrivateKeyWithHashAlg;
use russh::keys::{Algorithm, HashAlg};
use russh::ChannelMsg;
use svrctlrs_database::models::{Credential, CredentialType, Server};
use svrctlrs_database::queries;
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
    /// First contact: the key was recorded and waits for the user's confirmation. Names the
    /// server whose key it is, which is the jump host when one is in the way.
    HostKeyUnconfirmed {
        server_id: i64,
        server_name: String,
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
                server_name,
                key_type,
                fingerprint,
                ..
            } => f.write_str(&host_keys::unconfirmed_message(
                server_name,
                key_type,
                fingerprint,
            )),
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
    server_name: String,
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
                    server_id: self.server_id,
                    server_name: self.server_name.clone(),
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
    /// The jump host's connection, kept open for as long as this one lives
    _via: Option<Box<Connection>>,
}

/// Longest chain of jump hosts followed before giving up (also stops a loop in the data).
const MAX_JUMPS: usize = 4;

/// Default `SSH_KEEPALIVE_SECS`.
const DEFAULT_KEEPALIVE_SECS: u64 = 15;

/// The client configuration every connection uses: keepalives (three unanswered close the
/// connection), no Nagle delay on an interactive link, and compression offered first
/// (`zlib@openssh.com`, applied after authentication; falls back to none) so slow links
/// carry less.
pub fn client_config() -> Arc<client::Config> {
    let keepalive = std::env::var("SSH_KEEPALIVE_SECS")
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .unwrap_or(DEFAULT_KEEPALIVE_SECS);
    let preferred = russh::Preferred {
        compression: std::borrow::Cow::Borrowed(&[
            russh::compression::ZLIB_LEGACY,
            russh::compression::ZLIB,
            russh::compression::NONE,
        ]),
        ..russh::Preferred::default()
    };
    Arc::new(client::Config {
        keepalive_interval: (keepalive > 0).then(|| std::time::Duration::from_secs(keepalive)),
        keepalive_max: 3,
        nodelay: true,
        preferred,
        ..client::Config::default()
    })
}

#[cfg(test)]
mod config_tests {
    use super::*;

    #[test]
    fn client_config_offers_compression_and_keepalives() {
        let c = client_config();
        assert_eq!(c.keepalive_max, 3);
        assert!(c.nodelay);
        assert_eq!(
            c.preferred.compression.first(),
            Some(&russh::compression::ZLIB_LEGACY)
        );
        assert!(c.preferred.compression.contains(&russh::compression::NONE));
    }
}

/// Connect and authenticate, through the server's jump host if it has one.
pub async fn connect(
    pool: &Pool<Sqlite>,
    server: &Server,
    credential: Option<&Credential>,
) -> Result<Connection, ConnectError> {
    connect_via(pool, server, credential, &mut Vec::new()).await
}

/// `visited` holds the ids on the way down, so a loop in `via_server_id` is an error rather
/// than a stack overflow.
fn connect_via<'a>(
    pool: &'a Pool<Sqlite>,
    server: &'a Server,
    credential: Option<&'a Credential>,
    visited: &'a mut Vec<i64>,
) -> std::pin::Pin<
    Box<dyn std::future::Future<Output = Result<Connection, ConnectError>> + Send + 'a>,
> {
    Box::pin(async move {
        let target = target(server, credential)?;
        let plan = plan_auth(credential).await?;
        let login = target.login();

        let via = match server.via_server_id {
            None => None,
            Some(via_id) => {
                if visited.contains(&via_id) || via_id == server.id || visited.len() >= MAX_JUMPS {
                    return Err(ConnectError::Failed(format!(
                        "Jump hosts for {} form a loop or a chain longer than {MAX_JUMPS}",
                        server.name
                    )));
                }
                visited.push(server.id);
                let jump = queries::servers::get_server(pool, via_id)
                    .await
                    .map_err(|e| format!("Jump host of {}: {e}", server.name))?;
                let jump_credential = match jump.credential_id {
                    Some(id) => Some(
                        queries::credentials::get_credential(pool, id)
                            .await
                            .map_err(|e| format!("Credential of jump host {}: {e}", jump.name))?,
                    ),
                    None => None,
                };
                let conn = connect_via(pool, &jump, jump_credential.as_ref(), visited)
                    .await
                    .map_err(|e| match e {
                        ConnectError::Failed(msg) => {
                            ConnectError::Failed(format!("Jump host {}: {msg}", jump.name))
                        }
                        other => other,
                    })?;
                Some(Box::new(conn))
            }
        };

        let refusal = Arc::new(Mutex::new(None));
        let handler = Handler {
            pool: pool.clone(),
            server_id: server.id,
            server_name: server.name.clone(),
            refusal: refusal.clone(),
        };
        let config = client_config();
        let connected = match &via {
            None => client::connect(config, (target.hostname.as_str(), target.port), handler).await,
            Some(jump) => {
                // A TCP connection opened by the jump host, carried as an SSH channel
                let channel = jump
                    .handle
                    .channel_open_direct_tcpip(
                        target.hostname.as_str(),
                        u32::from(target.port),
                        "127.0.0.1",
                        0,
                    )
                    .await
                    .map_err(|e| {
                        ConnectError::Failed(format!(
                            "Jump host {} could not reach {}:{}: {e}",
                            jump.target.hostname, target.hostname, target.port
                        ))
                    })?;
                client::connect_stream(config, channel.into_stream(), handler).await
            }
        };
        let mut handle = match connected {
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
            _via: via,
        })
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
            via_server_id: None,
            tags: svrctlrs_database::sqlx::types::Json(vec![]),
            startup_command: None,
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

/// The client against an in-process SSH server (`test_support::ssh_server`): host-key first
/// contact, trust and mismatch; password and key logins; a jump host. What used to be
/// covered only by `scripts/dev/run-e2e.sh` against a real machine, now in CI.
#[cfg(test)]
mod loopback_tests {
    use super::*;
    use crate::test_support::ssh_server::{
        new_key, private_openssh, Event, Options, TestSshServer,
    };
    use crate::test_support::{add_credential, add_server, database, trust_recorded_keys};
    use svrctlrs_database::models::UpdateServer;

    /// The server row and its decrypted credential, as the routes load them
    async fn load(pool: &Pool<Sqlite>, server_id: i64) -> (Server, Option<Credential>) {
        let server = queries::servers::get_server(pool, server_id).await.unwrap();
        let credential = match server.credential_id {
            Some(id) => Some(
                queries::credentials::get_credential(pool, id)
                    .await
                    .unwrap(),
            ),
            None => None,
        };
        (server, credential)
    }

    async fn try_connect(pool: &Pool<Sqlite>, server_id: i64) -> Result<Connection, ConnectError> {
        let (server, credential) = load(pool, server_id).await;
        connect(pool, &server, credential.as_ref()).await
    }

    #[tokio::test]
    async fn first_contact_is_refused_until_the_key_is_trusted() {
        let ssh = TestSshServer::start(Options::password("alice", "s3cret")).await;
        let pool = database().await;
        let cred = add_credential(&pool, "pw", CredentialType::Password, "s3cret").await;
        let id = add_server(&pool, "loop", ssh.port, "alice", Some(cred), None).await;

        let Err(ConnectError::HostKeyUnconfirmed {
            server_id,
            server_name,
            key_type,
            fingerprint,
        }) = try_connect(&pool, id).await
        else {
            panic!("first contact must ask for confirmation");
        };
        assert_eq!((server_id, server_name.as_str()), (id, "loop"));
        assert_eq!(key_type, "ssh-ed25519");
        assert_eq!(fingerprint, ssh.fingerprint());
        // Refused during the handshake: no credential was offered to an unconfirmed host
        assert!(!ssh
            .events()
            .await
            .iter()
            .any(|e| matches!(e, Event::PasswordAuth { .. })));
        let keys = queries::server_host_keys::list_host_keys_for_server(&pool, id)
            .await
            .unwrap();
        assert_eq!(keys.len(), 1);
        assert!(keys[0].trusted_at.is_none());

        // Still unconfirmed on a second try, and still one row
        assert!(matches!(
            try_connect(&pool, id).await,
            Err(ConnectError::HostKeyUnconfirmed { .. })
        ));
        assert_eq!(
            queries::server_host_keys::list_host_keys_for_server(&pool, id)
                .await
                .unwrap()
                .len(),
            1
        );

        trust_recorded_keys(&pool, id).await;
        let conn = try_connect(&pool, id).await.expect("trusted key connects");
        assert_eq!(conn.auth, "password");
        assert_eq!(conn.target.login(), format!("alice@127.0.0.1:{}", ssh.port));
        assert!(ssh.events().await.contains(&Event::PasswordAuth {
            user: "alice".into(),
            accepted: true
        }));
    }

    #[tokio::test]
    async fn a_changed_host_key_is_refused_before_authentication() {
        let original = TestSshServer::start(Options::password("alice", "pw")).await;
        let impostor = TestSshServer::start(Options::password("alice", "pw")).await;
        let pool = database().await;
        let cred = add_credential(&pool, "pw", CredentialType::Password, "pw").await;
        let id = add_server(&pool, "web", original.port, "alice", Some(cred), None).await;
        let _ = try_connect(&pool, id).await;
        trust_recorded_keys(&pool, id).await;
        assert!(try_connect(&pool, id).await.is_ok());

        // Same name and address book entry, different machine behind it
        queries::servers::update_server(
            &pool,
            id,
            &UpdateServer {
                port: Some(i32::from(impostor.port)),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        match try_connect(&pool, id).await {
            Err(ConnectError::HostKeyMismatch(msg)) => {
                assert!(
                    msg.contains("MISMATCH") && msg.contains("ssh-ed25519"),
                    "{msg}"
                )
            }
            Err(e) => panic!("expected a mismatch, got {e}"),
            Ok(_) => panic!("expected a mismatch, got a connection"),
        }
        assert!(
            impostor.events().await.is_empty(),
            "the password never reached the impostor"
        );
        // The impostor's key was not recorded as a second key for the server
        assert_eq!(
            queries::server_host_keys::list_host_keys_for_server(&pool, id)
                .await
                .unwrap()
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn a_wrong_password_is_reported_as_such() {
        let ssh = TestSshServer::start(Options::password("alice", "right")).await;
        let pool = database().await;
        let cred = add_credential(&pool, "pw", CredentialType::Password, "wrong").await;
        let id = add_server(&pool, "loop", ssh.port, "alice", Some(cred), None).await;
        let _ = try_connect(&pool, id).await;
        trust_recorded_keys(&pool, id).await;
        match try_connect(&pool, id).await {
            Err(ConnectError::Failed(msg)) => assert!(
                msg.contains(&format!(
                    "Password authentication failed for alice@127.0.0.1:{}",
                    ssh.port
                )),
                "{msg}"
            ),
            Err(e) => panic!("unexpected error: {e}"),
            Ok(_) => panic!("a wrong password logged in"),
        }
    }

    #[tokio::test]
    async fn key_credentials_log_in_and_unknown_keys_are_rejected() {
        let authorized = new_key();
        let ssh = TestSshServer::start(Options::key("bob", authorized.public_key().clone())).await;
        let pool = database().await;
        let good = add_credential(
            &pool,
            "deploy",
            CredentialType::SshKey,
            &private_openssh(&authorized),
        )
        .await;
        let id = add_server(&pool, "loop", ssh.port, "bob", Some(good), None).await;
        let _ = try_connect(&pool, id).await;
        trust_recorded_keys(&pool, id).await;
        let conn = try_connect(&pool, id)
            .await
            .expect("authorized key logs in");
        assert_eq!(conn.auth, "credential 'deploy'");

        let stranger = add_credential(
            &pool,
            "stranger",
            CredentialType::SshKey,
            &private_openssh(&new_key()),
        )
        .await;
        queries::servers::update_server(
            &pool,
            id,
            &UpdateServer {
                credential_id: Some(Some(stranger)),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        match try_connect(&pool, id).await {
            Err(ConnectError::Failed(msg)) => {
                assert!(msg.contains("SSH authentication failed for bob@"), "{msg}");
                assert!(msg.contains("credential 'stranger'"), "{msg}");
            }
            Err(e) => panic!("unexpected error: {e}"),
            Ok(_) => panic!("an unknown key logged in"),
        }
        assert!(ssh.events().await.contains(&Event::KeyAuth {
            user: "bob".into(),
            accepted: false
        }));
    }

    #[tokio::test]
    async fn a_jump_host_carries_the_connection_and_each_hop_is_confirmed() {
        // One loopback server plays both parts: the jump host, and the target it forwards to
        let ssh = TestSshServer::start(Options::password("alice", "pw")).await;
        let pool = database().await;
        let cred = add_credential(&pool, "pw", CredentialType::Password, "pw").await;
        let jump = add_server(&pool, "bastion", ssh.port, "alice", Some(cred), None).await;
        let target = add_server(&pool, "inner", ssh.port, "alice", Some(cred), Some(jump)).await;

        // The jump host's key is asked about first, under its own name
        match try_connect(&pool, target).await {
            Err(ConnectError::HostKeyUnconfirmed { server_id, .. }) => assert_eq!(server_id, jump),
            other => panic!("expected the jump host's key, got {:?}", other.err()),
        }
        trust_recorded_keys(&pool, jump).await;
        // Then the target's, seen through the tunnel and recorded for the target's row
        match try_connect(&pool, target).await {
            Err(ConnectError::HostKeyUnconfirmed { server_id, .. }) => {
                assert_eq!(server_id, target)
            }
            other => panic!("expected the target's key, got {:?}", other.err()),
        }
        trust_recorded_keys(&pool, target).await;

        let conn = try_connect(&pool, target)
            .await
            .expect("through the jump host");
        assert_eq!(conn.auth, "password");
        assert!(
            ssh.wait_for(|e| *e
                == Event::DirectTcpip {
                    host: "127.0.0.1".into(),
                    port: u32::from(ssh.port)
                })
            .await
        );
        let logins = ssh
            .events()
            .await
            .iter()
            .filter(|e| matches!(e, Event::PasswordAuth { accepted: true, .. }))
            .count();
        assert!(logins >= 2, "one login per hop, got {logins}");

        // A loop in the data is refused, not followed
        queries::servers::update_server(
            &pool,
            jump,
            &UpdateServer {
                via_server_id: Some(Some(target)),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        match try_connect(&pool, target).await {
            Err(ConnectError::Failed(msg)) => assert!(msg.contains("loop"), "{msg}"),
            Err(e) => panic!("unexpected error: {e}"),
            Ok(_) => panic!("a jump loop connected"),
        }
    }
}
