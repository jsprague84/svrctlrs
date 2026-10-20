//! `svrctl` against a real `server` process on a temporary database: JSON on a pipe, exit
//! codes, errors with hints, token presets, and a delete held for approval that `--wait`
//! sees through once the owner approves.

use std::process::{Child, Command, Stdio};
use std::time::Duration;

use serde_json::{json, Value};

struct Server {
    child: Child,
    url: String,
    session: String,
    _dir: tempfile::TempDir,
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
    let dir = tempfile::tempdir().unwrap();
    let port = free_port();
    let url = format!("http://127.0.0.1:{port}");
    let key: String = (0..32)
        .map(|i| format!("{:02x}", (i * 7 + 3) as u8))
        .collect();
    let child = Command::new(env!("CARGO_BIN_EXE_server"))
        .args(["--addr", &format!("127.0.0.1:{port}")])
        .env(
            "DATABASE_URL",
            format!("sqlite:{}", dir.path().join("t.db").display()),
        )
        .env("ENCRYPTION_KEY", key)
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
        _dir: dir,
    }
}

impl Server {
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
