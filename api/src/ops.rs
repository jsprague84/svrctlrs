//! Task-level operations on one server, run by svrctlrs itself: diagnose, exec, onboard,
//! rotate a key. Private keys and passwords stay on the server; none of these take or return
//! one.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// `POST /servers/{id}/diagnose`: why svrctlrs can or cannot reach a server, step by step,
/// from where svrctlrs runs (its container, address, DNS and credentials).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Diagnosis {
    pub server_id: i64,
    pub server_name: String,
    /// Every step passed: a shell would open
    pub ok: bool,
    /// One sentence: what is wrong, or that all is well
    pub verdict: String,
    /// The jump hosts in order, then the server itself
    pub hops: Vec<DiagnosisHop>,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct DiagnosisHop {
    pub server_id: i64,
    pub server_name: String,
    /// `user@host:port`
    pub address: String,
    /// Reached through this jump host
    pub via_server_id: Option<i64>,
    pub steps: Vec<DiagnosisStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct DiagnosisStep {
    /// `config`, `dns`, `tcp`, `jump_host`, `tunnel`, `handshake`, `host_key` or `login`
    pub step: String,
    /// `ok`, `failed` or `skipped`
    pub status: String,
    pub detail: String,
    /// What to do about a failure
    pub fix: Option<String>,
    pub duration_ms: u64,
}

/// `POST /servers/{id}/exec`: one command, no terminal, through the stored credential.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ExecRequest {
    pub command: String,
    /// 1–600 seconds, default 60
    #[serde(default)]
    pub timeout_secs: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ExecResult {
    pub server_id: i64,
    /// Null when the command was still running at the deadline
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    /// More than 1 MiB came out on that stream; the rest was dropped
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
    pub timed_out: bool,
    pub duration_ms: u64,
}

/// One step of an onboarding or key rotation.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct OpStep {
    pub step: String,
    /// `ok`, `failed`, `skipped`, `waiting` (on you) or `planned` (a dry run)
    pub status: String,
    pub detail: String,
}

/// `POST /servers/onboard`: add a server, confirm its host key, check the login, and switch
/// it to a key generated on svrctlrs. Run it again with `fingerprint` after comparing the key;
/// a server of the same name and hostname is picked up where it stopped.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct OnboardRequest {
    pub name: String,
    pub hostname: String,
    /// Default 22
    #[serde(default)]
    pub port: Option<i32>,
    #[serde(default)]
    pub username: Option<String>,
    /// The way in: a credential stored in svrctlrs that already works there (a password or
    /// a key). Passwords and private keys never travel in this request.
    pub credential_id: i64,
    #[serde(default)]
    pub via_server_id: Option<i64>,
    #[serde(default)]
    pub tags: Vec<String>,
    /// The `SHA256:` fingerprint you compared on the host; it is trusted only if it matches
    #[serde(default)]
    pub fingerprint: Option<String>,
    /// Generate a key here, install it with the way in, and switch to it (default true)
    #[serde(default)]
    pub new_key: Option<bool>,
    /// Say what would happen, change nothing
    #[serde(default)]
    pub dry_run: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct OnboardResult {
    /// Null in a dry run, or when nothing was created
    pub server_id: Option<i64>,
    /// Every step passed
    pub done: bool,
    pub dry_run: bool,
    pub steps: Vec<OpStep>,
    /// What to do next, when not done
    pub next: Option<String>,
}

/// `POST /servers/{id}/rotate-key`: generate a new key, install it with the current one,
/// prove it, switch, and remove the old public key from the server.
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
pub struct RotateKeyRequest {
    /// Remove the old public key from `authorized_keys` (default true)
    #[serde(default)]
    pub remove_old: Option<bool>,
    #[serde(default)]
    pub dry_run: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RotateKeyResult {
    pub server_id: i64,
    pub done: bool,
    pub dry_run: bool,
    pub old_credential_id: Option<i64>,
    pub new_credential_id: Option<i64>,
    pub steps: Vec<OpStep>,
    pub next: Option<String>,
}
