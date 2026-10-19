//! A loopback SSH server for tests: russh's server side on 127.0.0.1 with its own host key,
//! one user who logs in with a password and/or authorized keys, a pretend shell, and
//! `direct-tcpip` so the server can be its own jump host. Everything the client asks for is
//! recorded as an [`Event`] for the test to assert on.
//!
//! The shell echoes what is typed (as a tty in cooked mode would) and answers each line:
//! `exit N` ends the session with exit status N, `big N` prints N bytes, anything else prints
//! `ran: <line>` and a new prompt.

use std::sync::Arc;
use std::time::Duration;

use russh::keys::{PrivateKey, PublicKey};
use russh::server::{self, Auth, Msg, Session};
use russh::{Channel, ChannelId, ChannelOpenFailure};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

/// What the server saw, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    PasswordAuth { user: String, accepted: bool },
    KeyAuth { user: String, accepted: bool },
    Pty { term: String, cols: u32, rows: u32 },
    Env { name: String, value: String },
    Shell,
    Resize { cols: u32, rows: u32 },
    Input(Vec<u8>),
    DirectTcpip { host: String, port: u32 },
}

/// Who may log in, and how.
#[derive(Clone)]
pub struct Options {
    pub user: String,
    pub password: Option<String>,
    pub authorized_keys: Vec<PublicKey>,
}

impl Options {
    pub fn password(user: &str, password: &str) -> Self {
        Self {
            user: user.to_string(),
            password: Some(password.to_string()),
            authorized_keys: Vec::new(),
        }
    }

    pub fn key(user: &str, key: PublicKey) -> Self {
        Self {
            user: user.to_string(),
            password: None,
            authorized_keys: vec![key],
        }
    }
}

struct Shared {
    options: Options,
    events: Mutex<Vec<Event>>,
}

pub struct TestSshServer {
    pub port: u16,
    /// The host key's public half, as `authorized_keys` / `known_hosts` write it
    pub host_key_openssh: String,
    shared: Arc<Shared>,
    accept: JoinHandle<()>,
}

/// A fresh ed25519 key pair, through the same generator the app uses.
pub fn new_key() -> PrivateKey {
    let generated = crate::ssh_keys::generate_ed25519("svrctlrs test").expect("key generation");
    russh::keys::decode_secret_key(&generated.private_openssh, None).expect("decode")
}

/// The OpenSSH private key text of a key, for storing it as a credential.
pub fn private_openssh(key: &PrivateKey) -> String {
    key.to_openssh(russh::keys::ssh_key::LineEnding::LF)
        .expect("encode")
        .to_string()
}

impl TestSshServer {
    pub async fn start(options: Options) -> Self {
        Self::start_with_host_key(options, new_key()).await
    }

    pub async fn start_with_host_key(options: Options, host_key: PrivateKey) -> Self {
        let host_key_openssh = host_key.public_key().to_openssh().expect("public key");
        let config = Arc::new(server::Config {
            keys: vec![host_key],
            auth_rejection_time: Duration::from_millis(5),
            auth_rejection_time_initial: Some(Duration::ZERO),
            inactivity_timeout: Some(Duration::from_secs(60)),
            ..Default::default()
        });
        let shared = Arc::new(Shared {
            options,
            events: Mutex::new(Vec::new()),
        });
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let port = listener.local_addr().expect("address").port();
        let accept_shared = shared.clone();
        let accept = tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                let handler = Connection {
                    shared: accept_shared.clone(),
                    shells: Vec::new(),
                    line: Vec::new(),
                };
                let config = config.clone();
                tokio::spawn(async move {
                    if let Ok(running) = server::run_stream(config, stream, handler).await {
                        let _ = running.await;
                    }
                });
            }
        });
        Self {
            port,
            host_key_openssh,
            shared,
            accept,
        }
    }

    /// `SHA256:…` of the host key, as the client reports it
    pub fn fingerprint(&self) -> String {
        crate::host_keys::fingerprint(&self.host_key_openssh)
    }

    pub async fn events(&self) -> Vec<Event> {
        self.shared.events.lock().await.clone()
    }

    /// Wait (up to 5 s) until an event matching `want` has been recorded.
    pub async fn wait_for(&self, want: impl Fn(&Event) -> bool) -> bool {
        for _ in 0..500 {
            if self.shared.events.lock().await.iter().any(&want) {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        false
    }
}

impl Drop for TestSshServer {
    fn drop(&mut self) {
        self.accept.abort();
    }
}

/// One client connection.
struct Connection {
    shared: Arc<Shared>,
    /// Channels that asked for a shell. russh hands `data` for every channel to the handler,
    /// tunnels included, and a tunnel's bytes must not reach the pretend shell.
    shells: Vec<ChannelId>,
    /// The shell's current input line
    line: Vec<u8>,
}

impl Connection {
    async fn record(&self, event: Event) {
        self.shared.events.lock().await.push(event);
    }

    /// The pretend shell's answer to one line of input.
    fn run_line(&self, channel: ChannelId, line: &str, session: &mut Session) {
        let line = line.trim();
        if let Some(code) = line.strip_prefix("exit") {
            let code = code.trim().parse::<u32>().unwrap_or(0);
            let _ = session.data(channel, &b"\r\nlogout\r\n"[..]);
            let _ = session.exit_status_request(channel, code);
            let _ = session.eof(channel);
            let _ = session.close(channel);
        } else if let Some(n) = line.strip_prefix("big ") {
            let n = n.trim().parse::<usize>().unwrap_or(0);
            for chunk in vec![b'x'; n].chunks(16 * 1024) {
                let _ = session.data(channel, chunk.to_vec());
            }
            let _ = session.data(channel, &b"\r\n$ "[..]);
        } else {
            let _ = session.data(channel, format!("\r\nran: {line}\r\n$ ").into_bytes());
        }
    }
}

impl server::Handler for Connection {
    type Error = russh::Error;

    async fn auth_password(&mut self, user: &str, password: &str) -> Result<Auth, Self::Error> {
        let options = &self.shared.options;
        let accepted = user == options.user && options.password.as_deref() == Some(password);
        self.record(Event::PasswordAuth {
            user: user.to_string(),
            accepted,
        })
        .await;
        Ok(if accepted {
            Auth::Accept
        } else {
            Auth::reject()
        })
    }

    async fn auth_publickey(&mut self, user: &str, key: &PublicKey) -> Result<Auth, Self::Error> {
        let options = &self.shared.options;
        let accepted = user == options.user
            && options
                .authorized_keys
                .iter()
                .any(|k| k.key_data() == key.key_data());
        self.record(Event::KeyAuth {
            user: user.to_string(),
            accepted,
        })
        .await;
        Ok(if accepted {
            Auth::Accept
        } else {
            Auth::reject()
        })
    }

    async fn channel_open_session(
        &mut self,
        _channel: Channel<Msg>,
        reply: server::ChannelOpenHandle,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        reply.accept().await;
        Ok(())
    }

    async fn channel_open_direct_tcpip(
        &mut self,
        channel: Channel<Msg>,
        host_to_connect: &str,
        port_to_connect: u32,
        _originator_address: &str,
        _originator_port: u32,
        reply: server::ChannelOpenHandle,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        self.record(Event::DirectTcpip {
            host: host_to_connect.to_string(),
            port: port_to_connect,
        })
        .await;
        let Ok(port) = u16::try_from(port_to_connect) else {
            reply.reject(ChannelOpenFailure::ConnectFailed).await;
            return Ok(());
        };
        match TcpStream::connect((host_to_connect, port)).await {
            Ok(mut tcp) => {
                reply.accept().await;
                tokio::spawn(async move {
                    let mut stream = channel.into_stream();
                    let _ = tokio::io::copy_bidirectional(&mut stream, &mut tcp).await;
                });
            }
            Err(_) => reply.reject(ChannelOpenFailure::ConnectFailed).await,
        }
        Ok(())
    }

    async fn pty_request(
        &mut self,
        _channel: ChannelId,
        term: &str,
        col_width: u32,
        row_height: u32,
        _pix_width: u32,
        _pix_height: u32,
        _modes: &[(russh::Pty, u32)],
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        self.record(Event::Pty {
            term: term.to_string(),
            cols: col_width,
            rows: row_height,
        })
        .await;
        Ok(())
    }

    async fn env_request(
        &mut self,
        _channel: ChannelId,
        variable_name: &str,
        variable_value: &str,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        self.record(Event::Env {
            name: variable_name.to_string(),
            value: variable_value.to_string(),
        })
        .await;
        Ok(())
    }

    async fn shell_request(
        &mut self,
        channel: ChannelId,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        self.record(Event::Shell).await;
        self.shells.push(channel);
        let _ = session.data(channel, &b"$ "[..]);
        Ok(())
    }

    async fn window_change_request(
        &mut self,
        _channel: ChannelId,
        col_width: u32,
        row_height: u32,
        _pix_width: u32,
        _pix_height: u32,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        self.record(Event::Resize {
            cols: col_width,
            rows: row_height,
        })
        .await;
        Ok(())
    }

    async fn data(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        if !self.shells.contains(&channel) {
            return Ok(());
        }
        self.record(Event::Input(data.to_vec())).await;
        // Echo, as a tty in cooked mode does
        let _ = session.data(channel, data.to_vec());
        for &b in data {
            if b == b'\n' || b == b'\r' {
                let line = String::from_utf8_lossy(&self.line).into_owned();
                self.line.clear();
                self.run_line(channel, &line, session);
            } else {
                self.line.push(b);
            }
        }
        Ok(())
    }
}
