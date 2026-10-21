//! `svrctl` against a real `server` process on a temporary database: JSON on a pipe, exit
//! codes, errors with hints, token presets, and a delete held for approval that `--wait`
//! sees through once the owner approves. And the local commands: a backup, changed data, a
//! restore staged with `svrctl restore` and put in place by the next start.

use std::process::{Child, Command, Stdio};
use std::time::Duration;

use serde_json::{json, Value};

struct Server {
    child: Child,
    url: String,
    session: String,
    dir: Option<tempfile::TempDir>,
}

/// The test servers' credential key.
fn test_key() -> String {
    (0..32)
        .map(|i| format!("{:02x}", (i * 7 + 3) as u8))
        .collect()
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

async fn start() -> Server {
    start_in(tempfile::tempdir().unwrap()).await
}

/// A server on the database in `dir` (a fresh one, or one a stopped server left).
async fn start_in(dir: tempfile::TempDir) -> Server {
    let port = free_port();
    let url = format!("http://127.0.0.1:{port}");
    let key = test_key();
    let child = Command::new(env!("CARGO_BIN_EXE_server"))
        .args(["--addr", &format!("127.0.0.1:{port}")])
        .env(
            "DATABASE_URL",
            format!("sqlite:{}", dir.path().join("t.db").display()),
        )
        .env("ENCRYPTION_KEY", key)
        .env("BACKUP_DIR", dir.path().join("backups"))
        .env("ADMIN_USERNAME", "admin")
        .env("ADMIN_PASSWORD", "svrctl-test-password")
        .env("SESSION_SECURE", "false")
        .env("SPA_DIR", dir.path())
        .env("RUST_LOG", "warn")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("server binary");
    let http = reqwest::Client::new();
    let mut up = false;
    for _ in 0..100 {
        if http
            .get(format!("{url}/api/v1/health"))
            .send()
            .await
            .is_ok()
        {
            up = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(up, "the server did not start");
    let login: Value = http
        .post(format!("{url}/api/v1/auth/login"))
        .json(&json!({ "username": "admin", "password": "svrctl-test-password" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    Server {
        child,
        url,
        session: login["session_id"].as_str().unwrap().to_string(),
        dir: Some(dir),
    }
}

impl Server {
    fn path(&self) -> &std::path::Path {
        self.dir.as_ref().unwrap().path()
    }

    /// Stop the server and hand back its folder, database included.
    fn stop(mut self) -> tempfile::TempDir {
        let _ = self.child.kill();
        let _ = self.child.wait();
        self.dir.take().unwrap()
    }

    /// Run a local svrctl command (backup, restore) next to this server's database with
    /// `key` as ENCRYPTION_KEY; (exit code, parsed stdout)
    fn local(&self, key: &str, args: &[&str]) -> (i32, Value) {
        let output = Command::new(env!("CARGO_BIN_EXE_svrctl"))
            .args(args)
            .env(
                "DATABASE_URL",
                format!("sqlite:{}", self.path().join("t.db").display()),
            )
            .env("ENCRYPTION_KEY", key)
            .env("BACKUP_DIR", self.path().join("backups"))
            .env_remove("SVRCTLRS_TOKEN")
            .output()
            .unwrap();
        let text = String::from_utf8_lossy(&output.stdout).into_owned();
        (
            output.status.code().unwrap_or(-1),
            serde_json::from_str(&text).unwrap_or(Value::Null),
        )
    }

    async fn owner(&self, method: reqwest::Method, path: &str, body: Value) -> Value {
        reqwest::Client::new()
            .request(method, format!("{}/api/v1{path}", self.url))
            .bearer_auth(&self.session)
            .json(&body)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap_or(Value::Null)
    }

    async fn token(&self, name: &str, preset: &str, ask_first: bool) -> String {
        let made = self
            .owner(
                reqwest::Method::POST,
                "/tokens",
                json!({ "name": name, "preset": preset, "expires_in_days": null, "require_approval": ask_first }),
            )
            .await;
        made["secret"].as_str().unwrap().to_string()
    }

    /// Run svrctl with `token`; (exit code, parsed stdout, stdout text)
    fn svrctl(
        &self,
        token: Option<&str>,
        args: &[&str],
        stdin: Option<&str>,
    ) -> (i32, Value, String) {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_svrctl"));
        cmd.args(args)
            .env("SVRCTLRS_URL", &self.url)
            .env_remove("SVRCTLRS_TOKEN")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(if stdin.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            });
        if let Some(t) = token {
            cmd.env("SVRCTLRS_TOKEN", t);
        }
        let mut child = cmd.spawn().unwrap();
        if let Some(input) = stdin {
            use std::io::Write;
            child
                .stdin
                .take()
                .unwrap()
                .write_all(input.as_bytes())
                .unwrap();
        }
        let output = child.wait_with_output().unwrap();
        let text = String::from_utf8_lossy(&output.stdout).into_owned();
        let parsed = serde_json::from_str(&text).unwrap_or(Value::Null);
        (output.status.code().unwrap_or(-1), parsed, text)
    }
}

#[tokio::test]
async fn svrctl_speaks_json_with_stable_exit_codes() {
    let s = start().await;
    let admin = s.token("cli", "full_admin", false).await;
    let reader = s.token("reader", "read_only", false).await;
    let asker = s.token("asker", "servers_keys", true).await;
    let closed = free_port().to_string();

    // No token needed for health; a piped stdout gets JSON
    let (code, out, _) = s.svrctl(None, &["health"], None);
    assert_eq!((code, out["status"].as_str()), (0, Some("ok")));

    let (code, out, _) = s.svrctl(None, &["servers"], None);
    assert_eq!(code, 5, "no token");
    assert_eq!(out["error"]["code"], "NO_TOKEN");
    assert!(out["error"]["hint"]
        .as_str()
        .unwrap()
        .contains("SVRCTLRS_TOKEN"));

    // A stored password, so the connection gets as far as TCP wherever the test runs (with
    // no credential, a machine without a default SSH key stops earlier)
    s.owner(
        reqwest::Method::POST,
        "/credentials",
        json!({ "name": "web password", "credential_type": "password", "value": "x" }),
    )
    .await;
    let (code, out, _) = s.svrctl(
        Some(&admin),
        &[
            "server",
            "add",
            "--name",
            "web",
            "--host",
            "127.0.0.1",
            "--port",
            &closed,
            "--tag",
            "lab",
            "--credential",
            "WEB PASSWORD",
        ],
        None,
    );
    assert_eq!(code, 0, "{out}");
    assert_eq!(out["server"]["name"], "web");
    assert!(
        out["server"]["credential_id"].is_number(),
        "credential found by name: {out}"
    );

    let (code, out, _) = s.svrctl(Some(&admin), &["tag", "prod", "web"], None);
    assert_eq!(code, 0);
    assert_eq!(out["servers"][0]["tags"], json!(["lab", "prod"]));
    let (_, out, _) = s.svrctl(Some(&admin), &["servers", "--tag", "PROD"], None);
    assert_eq!(out["servers"].as_array().unwrap().len(), 1);

    // Unreachable: the diagnosis runs (exit 1) and says where it stops
    let (code, out, _) = s.svrctl(Some(&admin), &["diagnose", "web"], None);
    assert_eq!(code, 1, "{out}");
    assert_eq!(out["ok"], false);
    let tcp = out["hops"][0]["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["step"] == "tcp")
        .unwrap()
        .clone();
    assert_eq!(tcp["status"], "failed");

    // exec where nothing listens: svrctlrs refuses (6), with the reason
    let (code, out, _) = s.svrctl(Some(&admin), &["exec", "web", "--", "echo", "hi"], None);
    assert_eq!(code, 6, "{out}");
    assert!(
        out["error"]["message"]
            .as_str()
            .unwrap()
            .contains("refused"),
        "{out}"
    );

    let (code, out, _) = s.svrctl(Some(&admin), &["server", "show", "nope"], None);
    assert_eq!(
        (code, out["error"]["code"].as_str()),
        (4, Some("NOT_FOUND"))
    );

    // A read-only token cannot add, and says why
    let (code, out, _) = s.svrctl(
        Some(&reader),
        &["server", "add", "--name", "x", "--host", "h"],
        None,
    );
    assert_eq!(
        (code, out["error"]["code"].as_str()),
        (5, Some("TOKEN_SCOPE"))
    );
    let (code, _, _) = s.svrctl(Some(&reader), &["servers"], None);
    assert_eq!(code, 0);

    // ssh_config from stdin, keys, credentials
    let (code, out, _) = s.svrctl(
        Some(&admin),
        &["import", "-", "--tag", "imported"],
        Some("Host db\n  HostName 10.0.0.9\n  User ops\n"),
    );
    assert_eq!(code, 0, "{out}");
    assert_eq!(out["created"][0]["name"], "db");
    let (code, out, _) = s.svrctl(
        Some(&admin),
        &["key", "generate", "--name", "agent key"],
        None,
    );
    assert_eq!(code, 0);
    assert!(out["public_key"]
        .as_str()
        .unwrap()
        .starts_with("ssh-ed25519 "));
    let (_, out, _) = s.svrctl(Some(&admin), &["credentials"], None);
    assert!(out["credentials"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c["name"] == "agent key"));

    // Text when asked for, even on a pipe
    let (_, _, text) = s.svrctl(Some(&admin), &["--text", "servers"], None);
    assert!(text.contains("web") && text.contains("db"), "{text}");

    // A delete held for approval: 3 without --wait; with --wait, done once the owner approves
    let (code, out, _) = s.svrctl(Some(&asker), &["server", "rm", "web"], None);
    assert_eq!(code, 3, "{out}");
    let id = out["approval"]["id"].as_str().unwrap().to_string();
    let url = s.url.clone();
    let token = asker.clone();
    let waiter = std::thread::spawn(move || {
        let output = Command::new(env!("CARGO_BIN_EXE_svrctl"))
            .args(["server", "rm", "web", "--wait"])
            .env("SVRCTLRS_URL", url)
            .env("SVRCTLRS_TOKEN", token)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
            .unwrap();
        (
            output.status.code().unwrap_or(-1),
            serde_json::from_slice::<Value>(&output.stdout).unwrap_or(Value::Null),
        )
    });
    tokio::time::sleep(Duration::from_millis(800)).await;
    let decided = s
        .owner(
            reqwest::Method::POST,
            &format!("/approvals/{id}/approve"),
            json!({}),
        )
        .await;
    assert_eq!(decided["status"], "done", "{decided}");
    let (code, out) = waiter.join().unwrap();
    assert_eq!((code, out["status"].as_str()), (0, Some("done")), "{out}");
    let (_, out, _) = s.svrctl(Some(&admin), &["servers"], None);
    assert!(out["servers"]
        .as_array()
        .unwrap()
        .iter()
        .all(|x| x["name"] != "web"));

    // The audit names the tokens; the OpenAPI document comes back whole
    let (_, out, _) = s.svrctl(Some(&admin), &["audit", "--action", "server.deleted"], None);
    assert_eq!(out["entries"][0]["token_name"], "asker", "{out}");
    let (code, out, _) = s.svrctl(Some(&admin), &["openapi"], None);
    assert_eq!(code, 0);
    assert!(out["paths"]["/api/v1/servers/{id}/diagnose"].is_object());
}

#[tokio::test]
async fn backup_then_restore_brings_the_data_back() {
    let s = start().await;
    let made = s
        .owner(
            reqwest::Method::POST,
            "/credentials",
            json!({ "name": "restore me", "credential_type": "password", "value": "s3cret value",
                    "username": "root" }),
        )
        .await;
    let cred_id = made["id"]
        .as_i64()
        .or_else(|| made["data"]["id"].as_i64())
        .unwrap();

    // The same copy the server makes at night, from the command line
    let (code, out) = s.local(&test_key(), &["backup"]);
    assert_eq!(code, 0, "{out}");
    let file = out["backup"].as_str().unwrap().to_string();
    assert!(file.contains("/backups/svrctlrs-"), "{file}");
    assert!(out["key"].as_str().unwrap().contains("ENCRYPTION_KEY"));
    // It is in the server's list and history
    let status = s.owner(reqwest::Method::GET, "/backups", Value::Null).await;
    assert_eq!(status["files"].as_array().unwrap().len(), 1, "{status}");
    assert_eq!(status["last_run"]["trigger"], "svrctl");

    // Then the data changes
    s.owner(
        reqwest::Method::DELETE,
        &format!("/credentials/{cred_id}"),
        Value::Null,
    )
    .await;
    let creds = s
        .owner(reqwest::Method::GET, "/credentials", Value::Null)
        .await;
    assert!(!creds.to_string().contains("restore me"));

    // A key that is not the one the copy was made with is refused, and nothing is staged
    let wrong: String = "ab".repeat(32);
    let (code, out) = s.local(&wrong, &["restore", &file]);
    assert_ne!(code, 0);
    assert!(
        out["error"]["message"]
            .as_str()
            .unwrap()
            .contains("another encryption key"),
        "{out}"
    );
    assert!(!s.path().join("t.db.restore").exists());
    // As is something that is not a copy
    let junk = s.path().join("junk.db");
    std::fs::write(&junk, b"not a database").unwrap();
    let (code, _) = s.local(&test_key(), &["restore", junk.to_str().unwrap()]);
    assert_ne!(code, 0);

    // The right key stages it; the running server is untouched until it restarts
    let (code, out) = s.local(&test_key(), &["restore", &file]);
    assert_eq!(code, 0, "{out}");
    assert_eq!(out["credentials_checked"], 1);
    assert!(s.path().join("t.db.restore").is_file());
    let old_session = s.session.clone();

    let dir = s.stop();
    let s = start_in(dir).await;
    assert!(!s.path().join("t.db.restore").exists());
    let kept_aside = std::fs::read_dir(s.path()).unwrap().flatten().any(|e| {
        e.file_name()
            .to_string_lossy()
            .starts_with("before-restore-")
    });
    assert!(kept_aside, "the replaced database is kept");

    let creds = s
        .owner(reqwest::Method::GET, "/credentials", Value::Null)
        .await;
    assert!(creds.to_string().contains("restore me"), "{creds}");
    // Sign-ins do not travel in a backup: the session from before is gone
    let status = reqwest::Client::new()
        .get(format!("{}/api/v1/credentials", s.url))
        .bearer_auth(&old_session)
        .send()
        .await
        .unwrap()
        .status();
    assert_eq!(status, reqwest::StatusCode::UNAUTHORIZED);
    // And the restore is in the audit log
    let audit = s.owner(reqwest::Method::GET, "/audit", Value::Null).await;
    assert!(audit.to_string().contains("backup.restored"), "{audit}");
}
