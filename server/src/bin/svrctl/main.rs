//! `svrctl` — the command line for svrctlrs, built for agents (and fine for people).
//!
//! Remote commands talk to a running server with an API token (`SVRCTLRS_URL`,
//! `SVRCTLRS_TOKEN`): servers, credentials, host keys, diagnose, exec, onboard, rotate-key,
//! approvals. They never prompt, answer one JSON document when stdout is not a terminal (or
//! with `--json`), and exit with codes an agent can branch on (see `out::code`). `backup` and
//! `reset-password` run next to the database instead (`docker exec`); `health` needs no token
//! and is the container's healthcheck.

mod api;
mod local;
mod out;

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use clap::{Args, Parser, Subcommand};
use reqwest::Method;
use serde_json::{json, Value};
use svrctlrs_api::approvals::{Approval, HeldForApproval};
use svrctlrs_api::audit::AuditPage;
use svrctlrs_api::credentials::{CredentialList, GeneratedKey};
use svrctlrs_api::ops::{Diagnosis, ExecResult, OnboardResult, OpStep, RotateKeyResult};
use svrctlrs_api::servers::{
    ConnectionTest, HostKeyList, HostKeyTrusted, HostKeysForgotten, ImportResult, KeyAuthorized,
    Server, ServerList, ServerOne,
};
use svrctlrs_api::system::{Health, Status};
use svrctlrs_api::IdMessage;

use api::Api;
use out::{code, Failure, Out};

const EXAMPLES: &str = "\
Examples:
  export SVRCTLRS_URL=https://svrctlrs.example SVRCTLRS_TOKEN=svr_…
  svrctl servers                                  # what svrctlrs knows
  svrctl diagnose web-01                          # why can't I reach it? (verdict + fix per step)
  svrctl exec web-01 -- systemctl is-active ssh   # one command, its exit status is svrctl's
  svrctl onboard --name lab-02 --host 10.0.0.12 --credential 'lab password'
  svrctl onboard --name lab-02 --host 10.0.0.12 --credential 'lab password' --fingerprint SHA256:…
  svrctl rotate-key web-01 --dry-run
  svrctl import hosts.ssh_config --tag imported
  svrctl tag prod web-01 web-02                   # --remove to take it off
  svrctl server rm old-box --wait                 # waits for the owner's approval if needed

Output: one JSON document when stdout is not a terminal (or with --json); errors are
{\"error\": {code, message, hint}}.
Exit codes: 0 done · 1 ran, but the answer is a failure · 2 bad arguments · 3 waiting on a
human (host key, approval) · 4 not found · 5 not allowed · 6 invalid or conflicting ·
7 svrctlrs unreachable. `exec` exits with the remote command's status (124: timed out).";

/// svrctl — administer svrctlrs from the command line
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None, after_help = EXAMPLES)]
#[command(propagate_version = true)]
struct Cli {
    /// The svrctlrs server
    #[arg(
        short,
        long,
        global = true,
        default_value = "http://localhost:8080",
        env = "SVRCTLRS_URL"
    )]
    url: String,

    /// API token (`svr_…`, from Settings → Access → Agent tokens)
    #[arg(
        short,
        long,
        global = true,
        env = "SVRCTLRS_TOKEN",
        hide_env_values = true
    )]
    token: Option<String>,

    /// Answer in JSON even on a terminal
    #[arg(long, global = true, conflicts_with = "text")]
    json: bool,

    /// Answer in text even when piped
    #[arg(long, global = true)]
    text: bool,

    #[command(subcommand)]
    command: Command,
}

/// `--wait` for the owner's approval of a held delete
#[derive(Args, Debug, Clone, Copy)]
struct Wait {
    /// If svrctlrs holds the call for approval, wait for the owner's answer
    #[arg(long)]
    wait: bool,
    /// How long to wait, in seconds
    #[arg(long, default_value_t = 3600)]
    wait_timeout: u64,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Is svrctlrs up? (no token; the container healthcheck)
    Health,
    /// Version and number of servers
    Status {
        /// Ignored (older `svrctl status server`)
        #[arg(hide = true)]
        what: Option<String>,
    },
    /// List servers
    #[command(visible_alias = "ls")]
    Servers {
        /// Only servers with this tag
        #[arg(long)]
        tag: Option<String>,
    },
    /// Show, add, change, test or delete a server
    #[command(subcommand)]
    Server(ServerCommand),
    /// Why svrctlrs can or cannot reach a server: DNS, TCP or tunnel, handshake, host key,
    /// login, per hop, with a fix for the step that fails (exit 1 when unreachable)
    Diagnose {
        /// Server id or name
        server: String,
        /// Log in with this stored credential (id or name) instead of the server's own, to
        /// test it without changing the server
        #[arg(long)]
        credential: Option<String>,
    },
    /// Run one command on a server through its stored credential (no terminal); exits with
    /// the command's status, 124 on a timeout. Needs a full-admin token
    Exec {
        /// Server id or name
        server: String,
        /// Seconds the command may run (1–600)
        #[arg(long, default_value_t = 60)]
        timeout: u64,
        /// The command, after `--`
        #[arg(required = true, trailing_var_arg = true, allow_hyphen_values = true)]
        command: Vec<String>,
    },
    /// Add a server and get it onto a key: create it with a stored credential as the way in,
    /// confirm its host key (only with --fingerprint, which you compare on the host), check the
    /// login, then switch it to a key generated on svrctlrs. Run again with --fingerprint after
    /// the first run stops for it (exit 3)
    Onboard {
        #[arg(long)]
        name: String,
        /// Hostname or address
        #[arg(long)]
        host: String,
        /// The way in: a stored credential (id or name) that works there
        #[arg(long)]
        credential: String,
        #[arg(long)]
        port: Option<i32>,
        #[arg(long)]
        user: Option<String>,
        /// Reach it through this server (id or name)
        #[arg(long)]
        via: Option<String>,
        /// Tags (repeat)
        #[arg(long = "tag")]
        tags: Vec<String>,
        /// The `SHA256:` fingerprint you compared on the host
        #[arg(long)]
        fingerprint: Option<String>,
        /// Keep logging in with --credential instead of a new key
        #[arg(long)]
        keep_credential: bool,
        /// Say what would happen; change nothing
        #[arg(long)]
        dry_run: bool,
    },
    /// Replace a server's key: generate, install with the current key, prove, switch, and
    /// remove the old public key from the server
    RotateKey {
        /// Server id or name
        server: String,
        /// Leave the old public key on the server
        #[arg(long)]
        keep_old: bool,
        #[arg(long)]
        dry_run: bool,
    },
    /// A server's recorded host keys, with fingerprints
    HostKeys {
        /// Server id or name
        server: String,
    },
    /// Trust the host key recorded on first contact, by the fingerprint you compared
    Trust {
        /// Server id or name
        server: String,
        #[arg(long)]
        fingerprint: String,
    },
    /// Forget a server's host keys (after you know why its key changed)
    ForgetKeys {
        /// Server id or name
        server: String,
        #[command(flatten)]
        wait: Wait,
    },
    /// Create servers from the Host blocks of an ssh_config
    Import {
        /// The file, or - for stdin
        #[arg(default_value = "-")]
        file: String,
        /// Credential (id or name) for every created server
        #[arg(long)]
        credential: Option<String>,
        /// Tag for every created server (repeat)
        #[arg(long = "tag")]
        tags: Vec<String>,
    },
    /// Add a tag to servers (or take it off with --remove)
    Tag {
        tag: String,
        /// Server ids or names
        #[arg(required = true)]
        servers: Vec<String>,
        #[arg(long)]
        remove: bool,
    },
    /// List credentials (values are never shown)
    #[command(visible_alias = "creds")]
    Credentials,
    /// Generate or install SSH keys
    #[command(subcommand)]
    Key(KeyCommand),
    /// Delete a credential
    #[command(subcommand)]
    Credential(CredentialCommand),
    /// An approval request; with --wait, wait for the owner's answer
    Approval {
        id: String,
        #[command(flatten)]
        wait: Wait,
    },
    /// The audit log, newest first
    Audit {
        #[arg(long, default_value_t = 50)]
        limit: i64,
        /// Action prefixes, e.g. `server.,token.`
        #[arg(long)]
        action: Option<String>,
        /// Text to look for
        #[arg(short, long)]
        query: Option<String>,
    },
    /// Print the API's OpenAPI document
    Openapi,

    /// [local] Write a consistent copy of the database (and the key file, when the key is
    /// not in the environment) into a directory, keeping only the newest copies
    Backup {
        #[arg(long, default_value = "data/backups")]
        out: PathBuf,
        #[arg(long, default_value_t = 7)]
        keep: usize,
        #[arg(long, env = "DATABASE_URL", default_value = "sqlite:data/svrctlrs.db")]
        database_url: String,
    },
    /// [local] Set a new password for an account when nobody can sign in. Asks twice on a
    /// terminal unless --password-stdin or --generate, and signs the account out everywhere.
    /// --clear-totp also turns two-factor sign-in off (a lost authenticator and no recovery
    /// codes left)
    ResetPassword {
        username: String,
        #[arg(long, conflicts_with = "generate")]
        password_stdin: bool,
        #[arg(long)]
        generate: bool,
        /// Leave the account's sign-ins (browser and native-app sessions) alone; API tokens
        /// are never touched
        #[arg(long)]
        keep_sessions: bool,
        /// Also turn the account's two-factor sign-in (TOTP and recovery codes) off
        #[arg(long)]
        clear_totp: bool,
        #[arg(long, env = "DATABASE_URL", default_value = "sqlite:data/svrctlrs.db")]
        database_url: String,
    },
}

#[derive(Subcommand, Debug)]
enum ServerCommand {
    /// One server
    Show { server: String },
    /// Add a server (see also `onboard`)
    Add {
        #[arg(long)]
        name: String,
        #[arg(long)]
        host: String,
        #[arg(long)]
        port: Option<i32>,
        #[arg(long)]
        user: Option<String>,
        /// Credential id or name
        #[arg(long)]
        credential: Option<String>,
        /// Jump host id or name
        #[arg(long)]
        via: Option<String>,
        #[arg(long = "tag")]
        tags: Vec<String>,
        #[arg(long)]
        description: Option<String>,
        /// Keep shells in tmux on the server, so they outlive a restart of SvrCtlRS
        #[arg(long)]
        persistent_shell: bool,
    },
    /// Change a server; only what you pass changes
    Edit {
        server: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        host: Option<String>,
        #[arg(long)]
        port: Option<i32>,
        #[arg(long)]
        user: Option<String>,
        /// Credential id or name, or `none`
        #[arg(long)]
        credential: Option<String>,
        /// Jump host id or name, or `none`
        #[arg(long)]
        via: Option<String>,
        /// Replace all tags (comma-separated; empty clears)
        #[arg(long)]
        tags: Option<String>,
        #[arg(long)]
        description: Option<String>,
        /// Keep shells in tmux on the server (`true`/`false`)
        #[arg(long)]
        persistent_shell: Option<bool>,
        #[arg(long, conflicts_with = "disable")]
        enable: bool,
        #[arg(long)]
        disable: bool,
    },
    /// Delete a server (may wait for the owner's approval)
    Rm {
        server: String,
        #[command(flatten)]
        wait: Wait,
    },
    /// The connection test (exit 1 when it fails, 3 when a host key waits)
    Test { server: String },
}

#[derive(Subcommand, Debug)]
enum KeyCommand {
    /// Generate an ed25519 key on svrctlrs; prints its public half
    Generate {
        #[arg(long)]
        name: String,
        #[arg(long)]
        comment: Option<String>,
    },
    /// Install a key credential on a server, logging in once with another credential that
    /// works there, and switch the server to the key
    Authorize {
        server: String,
        /// The key credential to install (id or name)
        #[arg(long)]
        key: String,
        /// The credential that works there now (id or name)
        #[arg(long = "with")]
        with: String,
    },
}

#[derive(Subcommand, Debug)]
enum CredentialCommand {
    /// Delete a credential (refused while a server uses it; may wait for approval)
    Rm {
        credential: String,
        #[command(flatten)]
        wait: Wait,
    },
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    let out = Out::new(cli.json, cli.text);
    let api = Api::new(&cli.url, cli.token.clone());
    let result = match cli.command {
        Command::Backup {
            out: dir,
            keep,
            database_url,
        } => local_result(&out, local::handle_backup(&database_url, &dir, keep).await),
        Command::ResetPassword {
            username,
            password_stdin,
            generate,
            keep_sessions,
            clear_totp,
            database_url,
        } => local_result(
            &out,
            local::handle_reset_password(
                &database_url,
                &username,
                password_stdin,
                generate,
                !keep_sessions,
                clear_totp,
            )
            .await,
        ),
        command => match run(&api, &out, command).await {
            Ok(code) => code,
            Err(f) => out.fail(&f),
        },
    };
    ExitCode::from(result)
}

fn local_result(out: &Out, r: anyhow::Result<()>) -> u8 {
    match r {
        Ok(()) => code::OK,
        Err(e) => out.fail(&Failure::new("FAILED", e.to_string())),
    }
}

async fn run(api: &Api, out: &Out, command: Command) -> Result<u8, Failure> {
    match command {
        Command::Health => {
            let reply = api
                .send(Method::GET, "/health", None, false, Duration::from_secs(10))
                .await?;
            let health: Health = api::decode(reply.body)?;
            out.emit(&health, |h| {
                format!(
                    "{} {} ({}), database {}",
                    h.service, h.version, h.status, h.database
                )
            });
            Ok(if health.status == "ok" {
                code::OK
            } else {
                code::FAILED
            })
        }
        Command::Status { .. } => {
            let s: Status = api.get("/status").await?;
            out.emit(&s, |s| {
                format!(
                    "svrctlrs {} {}, {} servers",
                    s.version, s.status, s.resources.servers
                )
            });
            Ok(code::OK)
        }
        Command::Servers { tag } => {
            let mut list: ServerList = api.get("/servers").await?;
            if let Some(tag) = tag {
                list.servers
                    .retain(|s| s.tags.iter().any(|t| t.eq_ignore_ascii_case(&tag)));
            }
            out.emit(&list, |l| servers_text(&l.servers));
            Ok(code::OK)
        }
        Command::Server(cmd) => server_command(api, out, cmd).await,
        Command::Diagnose { server, credential } => {
            let s = api.server(&server).await?;
            let body = match credential {
                Some(c) => Some(json!({ "credential_id": api.credential(&c).await?.id })),
                None => None,
            };
            let reply = api
                .send(
                    Method::POST,
                    &format!("/servers/{}/diagnose", s.id),
                    body.as_ref(),
                    true,
                    Duration::from_secs(180),
                )
                .await?;
            let d: Diagnosis = api::decode(reply.body)?;
            out.emit(&d, diagnosis_text);
            Ok(if d.ok { code::OK } else { code::FAILED })
        }
        Command::Exec {
            server,
            timeout,
            command,
        } => {
            let s = api.server(&server).await?;
            let body = json!({ "command": command.join(" "), "timeout_secs": timeout });
            let reply = api
                .send(
                    Method::POST,
                    &format!("/servers/{}/exec", s.id),
                    Some(&body),
                    true,
                    Duration::from_secs(timeout + 60),
                )
                .await?;
            let r: ExecResult = api::decode(reply.body)?;
            if out.json {
                out.emit(&r, |_| String::new());
            } else {
                print!("{}", r.stdout);
                eprint!("{}", r.stderr);
                if r.timed_out {
                    eprintln!("svrctl: the command was still running after {timeout} s");
                }
            }
            Ok(if r.timed_out {
                code::TIMED_OUT
            } else {
                r.exit_code
                    .map(|c| u8::try_from(c.clamp(0, 255)).unwrap_or(1))
                    .unwrap_or(code::FAILED)
            })
        }
        Command::Onboard {
            name,
            host,
            credential,
            port,
            user,
            via,
            tags,
            fingerprint,
            keep_credential,
            dry_run,
        } => {
            let cred = api.credential(&credential).await?;
            let via = match via {
                Some(v) => Some(api.server(&v).await?.id),
                None => None,
            };
            let body = json!({
                "name": name, "hostname": host, "port": port, "username": user,
                "credential_id": cred.id, "via_server_id": via, "tags": tags,
                "fingerprint": fingerprint, "new_key": !keep_credential, "dry_run": dry_run,
            });
            let reply = api
                .send(
                    Method::POST,
                    "/servers/onboard",
                    Some(&body),
                    true,
                    Duration::from_secs(180),
                )
                .await?;
            let r: OnboardResult = api::decode(reply.body)?;
            out.emit(&r, |r| steps_text(&r.steps, r.next.as_deref()));
            Ok(if r.done || r.dry_run {
                code::OK
            } else if r.steps.iter().any(|s| s.status == "waiting") {
                code::NEEDS_HUMAN
            } else {
                code::FAILED
            })
        }
        Command::RotateKey {
            server,
            keep_old,
            dry_run,
        } => {
            let s = api.server(&server).await?;
            let body = json!({ "remove_old": !keep_old, "dry_run": dry_run });
            let reply = api
                .send(
                    Method::POST,
                    &format!("/servers/{}/rotate-key", s.id),
                    Some(&body),
                    true,
                    Duration::from_secs(180),
                )
                .await?;
            let r: RotateKeyResult = api::decode(reply.body)?;
            out.emit(&r, |r| steps_text(&r.steps, r.next.as_deref()));
            Ok(if r.done || r.dry_run {
                code::OK
            } else {
                code::FAILED
            })
        }
        Command::HostKeys { server } => {
            let s = api.server(&server).await?;
            let keys: HostKeyList = api.get(&format!("/servers/{}/host-keys", s.id)).await?;
            out.emit(&keys, |k| {
                if k.host_keys.is_empty() {
                    return "No host keys recorded yet; connect once (svrctl server test) to record one".into();
                }
                k.host_keys
                    .iter()
                    .map(|h| format!("{} {} {}", if h.trusted { "trusted    " } else { "UNCONFIRMED" }, h.key_type, h.fingerprint))
                    .collect::<Vec<_>>()
                    .join("\n")
            });
            Ok(code::OK)
        }
        Command::Trust {
            server,
            fingerprint,
        } => {
            let s = api.server(&server).await?;
            let r: HostKeyTrusted = api
                .call(
                    Method::POST,
                    &format!("/servers/{}/host-keys/trust", s.id),
                    Some(&json!({ "fingerprint": fingerprint })),
                )
                .await?;
            out.emit(&r, |r| {
                format!(
                    "Trusted {} {} for {}",
                    r.host_key.key_type, r.host_key.fingerprint, s.name
                )
            });
            Ok(code::OK)
        }
        Command::ForgetKeys { server, wait } => {
            let s = api.server(&server).await?;
            gated::<HostKeysForgotten>(
                api,
                out,
                Method::DELETE,
                &format!("/servers/{}/host-keys", s.id),
                wait,
                |r| {
                    format!(
                        "Forgot {} host key(s); the next connection asks for confirmation again",
                        r.removed
                    )
                },
            )
            .await
        }
        Command::Import {
            file,
            credential,
            tags,
        } => {
            let config = if file == "-" {
                let mut s = String::new();
                std::io::Read::read_to_string(&mut std::io::stdin(), &mut s)
                    .map_err(|e| Failure::new("USAGE", format!("Could not read stdin: {e}")))?;
                s
            } else {
                std::fs::read_to_string(&file)
                    .map_err(|e| Failure::new("USAGE", format!("Could not read {file}: {e}")))?
            };
            let cred = match credential {
                Some(c) => Some(api.credential(&c).await?.id),
                None => None,
            };
            let r: ImportResult = api
                .call(
                    Method::POST,
                    "/servers/import",
                    Some(&json!({ "config": config, "credential_id": cred, "tags": tags })),
                )
                .await?;
            out.emit(&r, |r| {
                let mut lines: Vec<String> = r
                    .created
                    .iter()
                    .map(|c| format!("created {} ({}) id {}", c.name, c.hostname, c.id))
                    .collect();
                lines.extend(
                    r.skipped
                        .iter()
                        .map(|s| format!("skipped {}: {}", s.name, s.reason)),
                );
                lines.extend(r.notes.iter().cloned());
                lines.join("\n")
            });
            Ok(code::OK)
        }
        Command::Tag {
            tag,
            servers,
            remove,
        } => {
            let mut changed = Vec::new();
            for reference in servers {
                let s = api.server(&reference).await?;
                let mut tags = s.tags.clone();
                if remove {
                    tags.retain(|t| !t.eq_ignore_ascii_case(&tag));
                } else if !tags.iter().any(|t| t.eq_ignore_ascii_case(&tag)) {
                    tags.push(tag.clone());
                }
                let _: IdMessage = api
                    .call(
                        Method::PUT,
                        &format!("/servers/{}", s.id),
                        Some(&json!({ "tags": tags })),
                    )
                    .await?;
                let after: ServerOne = api.get(&format!("/servers/{}", s.id)).await?;
                changed.push(after.server);
            }
            let list = ServerList { servers: changed };
            out.emit(&list, |l| servers_text(&l.servers));
            Ok(code::OK)
        }
        Command::Credentials => {
            let list: CredentialList = api.get("/credentials").await?;
            out.emit(&list, |l| {
                l.credentials
                    .iter()
                    .map(|c| {
                        format!(
                            "{:>4}  {:<24} {:<9} {}{}",
                            c.id,
                            c.name,
                            c.credential_type,
                            c.fingerprint.as_deref().unwrap_or(""),
                            if c.used_by.is_empty() {
                                String::new()
                            } else {
                                format!("  used by {}", c.used_by.join(", "))
                            }
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            });
            Ok(code::OK)
        }
        Command::Key(KeyCommand::Generate { name, comment }) => {
            let r: GeneratedKey = api
                .call(
                    Method::POST,
                    "/credentials/generate-key",
                    Some(&json!({ "name": name, "comment": comment })),
                )
                .await?;
            out.emit(&r, |r| {
                format!("Generated credential {} ({name})\n{}", r.id, r.public_key)
            });
            Ok(code::OK)
        }
        Command::Key(KeyCommand::Authorize { server, key, with }) => {
            let s = api.server(&server).await?;
            let key = api.credential(&key).await?;
            let with = api.credential(&with).await?;
            let reply = api
                .send(
                    Method::POST,
                    &format!("/servers/{}/authorize-key", s.id),
                    Some(&json!({ "credential_id": key.id, "bootstrap_credential_id": with.id })),
                    true,
                    Duration::from_secs(120),
                )
                .await?;
            let r: KeyAuthorized = api::decode(reply.body)?;
            out.emit(&r, |r| r.message.clone());
            Ok(code::OK)
        }
        Command::Credential(CredentialCommand::Rm { credential, wait }) => {
            let c = api.credential(&credential).await?;
            gated::<IdMessage>(
                api,
                out,
                Method::DELETE,
                &format!("/credentials/{}", c.id),
                wait,
                |r| r.message.clone(),
            )
            .await
        }
        Command::Approval { id, wait } => {
            let first: Approval = api.get(&format!("/approvals/{id}")).await?;
            let a = if wait.wait {
                wait_for(api, first, wait).await?
            } else {
                first
            };
            out.emit(&a, approval_text);
            Ok(approval_code(&a))
        }
        Command::Audit {
            limit,
            action,
            query,
        } => {
            let mut path = format!("/audit?limit={}", limit.clamp(1, 500));
            if let Some(a) = action {
                path.push_str(&format!("&action={}", urlencode(&a)));
            }
            if let Some(q) = query {
                path.push_str(&format!("&q={}", urlencode(&q)));
            }
            let page: AuditPage = api.get(&path).await?;
            out.emit(&page, |p| {
                p.entries
                    .iter()
                    .map(|e| {
                        format!(
                            "{}  {:<10} {:<26} {}{}",
                            e.at.format("%Y-%m-%d %H:%M"),
                            e.username.as_deref().unwrap_or("-"),
                            e.action,
                            e.target_name.as_deref().unwrap_or(""),
                            e.token_name
                                .as_deref()
                                .map(|t| format!("  (via {t})"))
                                .unwrap_or_default()
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            });
            Ok(code::OK)
        }
        Command::Openapi => {
            let doc: Value = api.get("/openapi.json").await?;
            println!("{}", serde_json::to_string_pretty(&doc).unwrap_or_default());
            Ok(code::OK)
        }
        Command::Backup { .. } | Command::ResetPassword { .. } => unreachable!("handled in main"),
    }
}

async fn server_command(api: &Api, out: &Out, cmd: ServerCommand) -> Result<u8, Failure> {
    match cmd {
        ServerCommand::Show { server } => {
            let s = api.server(&server).await?;
            let one = ServerOne { server: s };
            out.emit(&one, |o| servers_text(std::slice::from_ref(&o.server)));
            Ok(code::OK)
        }
        ServerCommand::Add {
            name,
            host,
            port,
            user,
            credential,
            via,
            tags,
            description,
            persistent_shell,
        } => {
            let credential_id = match credential {
                Some(c) => Some(api.credential(&c).await?.id),
                None => None,
            };
            let via_server_id = match via {
                Some(v) => Some(api.server(&v).await?.id),
                None => None,
            };
            let mut body = json!({
                "name": name, "hostname": host, "username": user, "credential_id": credential_id,
                "via_server_id": via_server_id, "tags": tags, "description": description,
                "persistent_shell": persistent_shell,
            });
            if let Some(p) = port {
                body["port"] = json!(p);
            }
            let r: IdMessage = api.call(Method::POST, "/servers", Some(&body)).await?;
            let s: ServerOne = api.get(&format!("/servers/{}", r.id)).await?;
            out.emit(&s, |o| servers_text(std::slice::from_ref(&o.server)));
            Ok(code::OK)
        }
        ServerCommand::Edit {
            server,
            name,
            host,
            port,
            user,
            credential,
            via,
            tags,
            description,
            persistent_shell,
            enable,
            disable,
        } => {
            let s = api.server(&server).await?;
            let mut body = serde_json::Map::new();
            if let Some(v) = name {
                body.insert("name".into(), json!(v));
            }
            if let Some(v) = host {
                body.insert("hostname".into(), json!(v));
            }
            if let Some(v) = port {
                body.insert("port".into(), json!(v));
            }
            if let Some(v) = user {
                body.insert("username".into(), json!(v));
            }
            if let Some(v) = credential {
                let id = if v.eq_ignore_ascii_case("none") {
                    Value::Null
                } else {
                    json!(api.credential(&v).await?.id)
                };
                body.insert("credential_id".into(), id);
            }
            if let Some(v) = via {
                let id = if v.eq_ignore_ascii_case("none") {
                    Value::Null
                } else {
                    json!(api.server(&v).await?.id)
                };
                body.insert("via_server_id".into(), id);
            }
            if let Some(v) = tags {
                let list: Vec<&str> = v
                    .split(',')
                    .map(str::trim)
                    .filter(|t| !t.is_empty())
                    .collect();
                body.insert("tags".into(), json!(list));
            }
            if let Some(v) = description {
                body.insert("description".into(), json!(v));
            }
            if let Some(v) = persistent_shell {
                body.insert("persistent_shell".into(), json!(v));
            }
            if enable || disable {
                body.insert("enabled".into(), json!(enable));
            }
            let _: IdMessage = api
                .call(
                    Method::PUT,
                    &format!("/servers/{}", s.id),
                    Some(&Value::Object(body)),
                )
                .await?;
            let after: ServerOne = api.get(&format!("/servers/{}", s.id)).await?;
            out.emit(&after, |o| servers_text(std::slice::from_ref(&o.server)));
            Ok(code::OK)
        }
        ServerCommand::Rm { server, wait } => {
            let s = api.server(&server).await?;
            gated::<IdMessage>(
                api,
                out,
                Method::DELETE,
                &format!("/servers/{}", s.id),
                wait,
                |r| r.message.clone(),
            )
            .await
        }
        ServerCommand::Test { server } => {
            let s = api.server(&server).await?;
            let reply = api
                .send(
                    Method::POST,
                    &format!("/servers/{}/test", s.id),
                    None,
                    true,
                    Duration::from_secs(90),
                )
                .await?;
            let t: ConnectionTest = api::decode(reply.body)?;
            out.emit(&t, |t| t.message.clone());
            Ok(if t.success {
                code::OK
            } else if t.code.as_deref() == Some("HOST_KEY_UNCONFIRMED") {
                code::NEEDS_HUMAN
            } else {
                code::FAILED
            })
        }
    }
}

/// A call that svrctlrs may hold for the owner's approval (202). Without --wait the held
/// request is printed and the exit code says a human is needed; with it, svrctl waits for the
/// answer and prints the decided request (whose `result` is what the call answered).
async fn gated<T: serde::de::DeserializeOwned + serde::Serialize>(
    api: &Api,
    out: &Out,
    method: Method,
    path: &str,
    wait: Wait,
    human: impl FnOnce(&T) -> String,
) -> Result<u8, Failure> {
    let reply = api
        .send(method, path, None, true, Duration::from_secs(90))
        .await?;
    if reply.status != 202 {
        let r: T = api::decode(reply.body)?;
        out.emit(&r, human);
        return Ok(code::OK);
    }
    let held: HeldForApproval = api::decode(reply.body)?;
    if !wait.wait {
        out.emit(&held, |h| {
            format!(
                "Waiting for approval: {}. The owner approves or denies it at {}\nRun again with --wait to wait for the answer.",
                h.approval.summary, h.approval.url
            )
        });
        return Ok(code::NEEDS_HUMAN);
    }
    if !out.json {
        eprintln!(
            "Waiting for the owner's approval of \"{}\" ({})…",
            held.approval.summary, held.approval.url
        );
    }
    let decided = wait_for(api, held.approval, wait).await?;
    out.emit(&decided, approval_text);
    Ok(approval_code(&decided))
}

/// Poll an approval (each poll waits up to 25 s on the server) until it is decided or the
/// wait runs out.
async fn wait_for(api: &Api, mut a: Approval, wait: Wait) -> Result<Approval, Failure> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(wait.wait_timeout);
    while matches!(a.status.as_str(), "pending" | "approved")
        && tokio::time::Instant::now() < deadline
    {
        a = api.get(&format!("/approvals/{}?wait=25", a.id)).await?;
    }
    Ok(a)
}

fn approval_code(a: &Approval) -> u8 {
    match a.status.as_str() {
        "done" => code::OK,
        "pending" | "approved" => code::NEEDS_HUMAN,
        _ => code::FAILED,
    }
}

fn approval_text(a: &Approval) -> String {
    match a.status.as_str() {
        "done" => format!("Approved and done: {}", a.summary),
        "denied" => format!("Denied: {}. Nothing was done.", a.summary),
        "expired" => format!(
            "Expired without an answer: {}. Nothing was done.",
            a.summary
        ),
        "failed" => format!(
            "Approved, but the call failed ({}): {}",
            a.result_status.unwrap_or(0),
            a.summary
        ),
        _ => format!("Still waiting for the owner: {} ({})", a.summary, a.url),
    }
}

fn servers_text(servers: &[Server]) -> String {
    if servers.is_empty() {
        return "No servers".into();
    }
    servers
        .iter()
        .map(|s| {
            let state = if !s.enabled {
                "disabled".to_string()
            } else if let Some(e) = &s.last_error {
                format!("error: {}", e.chars().take(60).collect::<String>())
            } else if s.last_seen_at.is_some() {
                "ok".into()
            } else {
                "not checked".into()
            };
            format!(
                "{:>4}  {:<20} {}@{}:{}  [{}]  {}",
                s.id,
                s.name,
                s.username.as_deref().unwrap_or("-"),
                s.hostname,
                s.port,
                s.tags.join(","),
                state
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn diagnosis_text(d: &Diagnosis) -> String {
    let mut lines = vec![d.verdict.clone()];
    for hop in &d.hops {
        lines.push(format!("{} ({})", hop.server_name, hop.address));
        for s in &hop.steps {
            let mark = match s.status.as_str() {
                "ok" => "ok  ",
                "failed" => "FAIL",
                _ => "--  ",
            };
            lines.push(format!("  {mark} {:<10} {}", s.step, s.detail));
            if let Some(fix) = &s.fix {
                lines.push(format!("       fix: {fix}"));
            }
        }
    }
    lines.join("\n")
}

fn steps_text(steps: &[OpStep], next: Option<&str>) -> String {
    let mut lines: Vec<String> = steps
        .iter()
        .map(|s| format!("{:<8} {:<11} {}", s.status, s.step, s.detail))
        .collect();
    if let Some(n) = next {
        lines.push(format!("next: {n}"));
    }
    lines.join("\n")
}

fn urlencode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}
