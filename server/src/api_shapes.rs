//! The JSON shape of every API response the UI and agents read, kept in
//! `server/testdata/api-shapes.json`: each response with its values replaced by their JSON
//! types (`string`, `integer`, `number`, `boolean`, `null`; arrays by the union of their
//! elements, keys sorted). Moving handlers to typed responses must not change a single key or
//! type, and this test is how that is checked. After a deliberate change, regenerate with
//! `UPDATE_API_SHAPES=1 cargo test -p server api_shapes`.

use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};

use crate::test_support::app::{eventually, AppHarness};
use crate::test_support::ssh_server::{Options, TestSshServer};
use svrctlrs_database::models::NewRecording;
use svrctlrs_database::queries;

const GOLDEN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/api-shapes.json");

/// A value's shape: its type, recursively.
fn shape(v: &Value) -> Value {
    match v {
        Value::Null => json!("null"),
        Value::Bool(_) => json!("boolean"),
        Value::Number(n) if n.is_i64() || n.is_u64() => json!("integer"),
        Value::Number(_) => json!("number"),
        Value::String(_) => json!("string"),
        Value::Array(items) => {
            let mut merged: Option<Value> = None;
            for item in items {
                let s = shape(item);
                merged = Some(match merged {
                    None => s,
                    Some(m) => union(m, s),
                });
            }
            json!([merged.unwrap_or(json!("empty"))])
        }
        Value::Object(map) => {
            let sorted: BTreeMap<&String, Value> = map.iter().map(|(k, v)| (k, shape(v))).collect();
            Value::Object(sorted.into_iter().map(|(k, v)| (k.clone(), v)).collect())
        }
    }
}

/// Two shapes as one: objects key by key (a key missing on one side is `absent`), scalars as
/// `a|b`.
fn union(a: Value, b: Value) -> Value {
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            let keys: BTreeSet<String> = x.keys().chain(y.keys()).cloned().collect();
            let mut out = Map::new();
            for k in keys {
                let l = x.get(&k).cloned().unwrap_or(json!("absent"));
                let r = y.get(&k).cloned().unwrap_or(json!("absent"));
                out.insert(k, union(l, r));
            }
            Value::Object(out)
        }
        (Value::Array(x), Value::Array(y)) => {
            let l = x.into_iter().next().unwrap_or(json!("empty"));
            let r = y.into_iter().next().unwrap_or(json!("empty"));
            if l == json!("empty") {
                json!([r])
            } else if r == json!("empty") {
                json!([l])
            } else {
                json!([union(l, r)])
            }
        }
        (x, y) if x == y => x,
        (x, y) => {
            let mut parts: BTreeSet<String> = BTreeSet::new();
            for v in [x, y] {
                match v {
                    Value::String(s) => parts.extend(s.split('|').map(String::from)),
                    other => {
                        parts.insert(other.to_string());
                    }
                }
            }
            json!(parts.into_iter().collect::<Vec<_>>().join("|"))
        }
    }
}

struct Recorder {
    shapes: BTreeMap<String, Value>,
}

impl Recorder {
    fn keep(&mut self, name: &str, status: axum::http::StatusCode, body: &Value) {
        self.shapes.insert(
            name.to_string(),
            json!({ "status": status.as_u16(), "body": shape(body) }),
        );
    }
}

#[tokio::test]
async fn api_shapes_are_unchanged() {
    let h = AppHarness::new().await;
    let pool = h.state.pool.clone();
    let mut r = Recorder {
        shapes: BTreeMap::new(),
    };
    macro_rules! owner {
        ($name:expr, $method:expr, $uri:expr) => {
            owner!($name, $method, $uri, None)
        };
        ($name:expr, $method:expr, $uri:expr, $body:expr) => {{
            let (status, body) = h.owner($method, &$uri, $body).await;
            r.keep($name, status, &body);
            body
        }};
    }

    // ── Health and status
    let (status, body) =
        crate::test_support::app::call(&h.app, "GET", "/api/v1/health", None, None).await;
    r.keep("GET /health", status, &body);
    owner!("GET /status", "GET", "/api/v1/status");

    // ── Credentials
    let pw = owner!(
        "POST /credentials",
        "POST",
        "/api/v1/credentials",
        Some(
            json!({ "name": "ops password", "credential_type": "password", "value": "pw",
                     "username": "alice", "description": "lab" })
        )
    );
    let pw_id = pw["id"]
        .as_i64()
        .or_else(|| pw["data"]["id"].as_i64())
        .unwrap();
    let key = owner!(
        "POST /credentials/generate-key",
        "POST",
        "/api/v1/credentials/generate-key",
        Some(json!({ "name": "deploy key" }))
    );
    let key_id = key["id"].as_i64().unwrap();
    owner!("GET /credentials", "GET", "/api/v1/credentials");
    owner!(
        "GET /credentials/{id} (password)",
        "GET",
        format!("/api/v1/credentials/{pw_id}")
    );
    owner!(
        "GET /credentials/{id} (key)",
        "GET",
        format!("/api/v1/credentials/{key_id}")
    );
    owner!(
        "PUT /credentials/{id}",
        "PUT",
        format!("/api/v1/credentials/{pw_id}"),
        Some(json!({ "description": "lab login" }))
    );
    owner!(
        "POST /credentials (taken name)",
        "POST",
        "/api/v1/credentials",
        Some(json!({ "name": "ops password", "credential_type": "password", "value": "x" }))
    );

    // ── Servers, against the loopback SSH server
    let ssh = TestSshServer::start(Options::password("alice", "pw")).await;
    let srv = owner!(
        "POST /servers",
        "POST",
        "/api/v1/servers",
        Some(
            json!({ "name": "loop", "hostname": "127.0.0.1", "port": ssh.port,
                     "username": "alice", "credential_id": pw_id, "tags": ["Lab", "prod"],
                     "description": "loopback" })
        )
    );
    let sid = srv["id"]
        .as_i64()
        .or_else(|| srv["data"]["id"].as_i64())
        .unwrap();
    let hop = owner!(
        "POST /servers (via)",
        "POST",
        "/api/v1/servers",
        Some(
            json!({ "name": "inner", "hostname": "127.0.0.1", "port": ssh.port,
                     "credential_id": pw_id, "via_server_id": sid,
                     "startup_command": "cd /tmp", "record_sessions": true })
        )
    );
    let hop_id = hop["id"]
        .as_i64()
        .or_else(|| hop["data"]["id"].as_i64())
        .unwrap();
    owner!("GET /servers", "GET", "/api/v1/servers");
    owner!("GET /servers/{id}", "GET", format!("/api/v1/servers/{sid}"));
    owner!(
        "GET /servers/{id} (via)",
        "GET",
        format!("/api/v1/servers/{hop_id}")
    );
    owner!(
        "PUT /servers/{id}",
        "PUT",
        format!("/api/v1/servers/{sid}"),
        Some(json!({ "description": "the loopback server" }))
    );
    let test = owner!(
        "POST /servers/{id}/test (unconfirmed)",
        "POST",
        format!("/api/v1/servers/{sid}/test")
    );
    owner!(
        "GET /servers/{id}/host-keys",
        "GET",
        format!("/api/v1/servers/{sid}/host-keys")
    );
    let fingerprint = test["details"]["fingerprint"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    owner!(
        "POST /servers/{id}/host-keys/trust (wrong)",
        "POST",
        format!("/api/v1/servers/{sid}/host-keys/trust"),
        Some(json!({ "fingerprint": "SHA256:nope" }))
    );
    owner!(
        "POST /servers/{id}/host-keys/trust",
        "POST",
        format!("/api/v1/servers/{sid}/host-keys/trust"),
        Some(json!({ "fingerprint": fingerprint }))
    );
    owner!(
        "POST /servers/{id}/test",
        "POST",
        format!("/api/v1/servers/{sid}/test")
    );
    owner!(
        "POST /servers/import",
        "POST",
        "/api/v1/servers/import",
        Some(
            json!({ "config": "Host web\n  HostName 10.0.0.5\n  User ops\nHost *\n  User x\n",
                     "tags": ["imported"] })
        )
    );
    owner!(
        "POST /servers (taken name)",
        "POST",
        "/api/v1/servers",
        Some(json!({ "name": "loop", "hostname": "x" }))
    );
    owner!("GET /servers/{id} (missing)", "GET", "/api/v1/servers/999");

    // ── Settings
    owner!("GET /settings", "GET", "/api/v1/settings");
    owner!(
        "GET /settings/{key}",
        "GET",
        "/api/v1/settings/notification.url"
    );
    owner!(
        "PUT /settings",
        "PUT",
        "/api/v1/settings",
        Some(json!({ "values": { "notification.url": "https://push.example" } }))
    );
    owner!(
        "PUT /settings/{key}",
        "PUT",
        "/api/v1/settings/recording.retention_days",
        Some(json!({ "value": "14" }))
    );
    owner!(
        "POST /settings",
        "POST",
        "/api/v1/settings",
        Some(json!({ "key": "app.extra", "value": "1", "value_type": "string" }))
    );
    owner!(
        "POST /notifications/test (unset)",
        "POST",
        "/api/v1/notifications/test"
    );

    // ── Profiles and quick commands
    let profile = owner!(
        "POST /profiles",
        "POST",
        "/api/v1/profiles",
        Some(json!({ "name": "two panes", "layout": "split-h",
                     "pane_configs": [{ "server_id": sid }] }))
    );
    let profile_id = profile["id"]
        .as_i64()
        .or_else(|| profile["data"]["id"].as_i64())
        .unwrap();
    owner!("GET /profiles", "GET", "/api/v1/profiles");
    owner!(
        "GET /profiles/{id}",
        "GET",
        format!("/api/v1/profiles/{profile_id}")
    );
    owner!(
        "PUT /profiles/{id}",
        "PUT",
        format!("/api/v1/profiles/{profile_id}"),
        Some(json!({ "is_default": true }))
    );
    let qc = owner!(
        "POST /quick-commands",
        "POST",
        "/api/v1/quick-commands",
        Some(json!({ "name": "disk", "command": "df -h", "server_id": sid, "category": "ops" }))
    );
    let qc_id = qc["id"]
        .as_i64()
        .or_else(|| qc["data"]["id"].as_i64())
        .unwrap();
    owner!("GET /quick-commands", "GET", "/api/v1/quick-commands");
    owner!(
        "GET /quick-commands/{id}",
        "GET",
        format!("/api/v1/quick-commands/{qc_id}")
    );
    owner!(
        "PUT /quick-commands/{id}",
        "PUT",
        format!("/api/v1/quick-commands/{qc_id}"),
        Some(json!({ "sort_order": 2 }))
    );

    // ── Users and preferences
    let bob = owner!(
        "POST /users",
        "POST",
        "/api/v1/users",
        Some(json!({ "username": "bob", "password": "another long password" }))
    );
    owner!("GET /users", "GET", "/api/v1/users");
    owner!(
        "GET /users/me/preferences/terminal (unset)",
        "GET",
        "/api/v1/users/me/preferences/terminal"
    );
    owner!(
        "PUT /users/me/preferences/terminal",
        "PUT",
        "/api/v1/users/me/preferences/terminal",
        Some(json!({ "value": { "fontSize": 13 } }))
    );
    owner!("GET /users/me/sign-in", "GET", "/api/v1/users/me/sign-in");
    owner!("GET /users/me/devices", "GET", "/api/v1/users/me/devices");
    owner!(
        "POST /users/me/devices/sign-out-others",
        "POST",
        "/api/v1/users/me/devices/sign-out-others"
    );
    owner!(
        "PUT /users/me/password (wrong current)",
        "PUT",
        "/api/v1/users/me/password",
        Some(json!({ "current_password": "nope", "new_password": "a brand new password" }))
    );
    owner!(
        "DELETE /users/{id}",
        "DELETE",
        format!("/api/v1/users/{}", bob["id"])
    );

    // ── Recordings (a finished row; the file itself is not read here)
    let rec = queries::recordings::create(
        &pool,
        &NewRecording {
            user_id: 1,
            server_id: hop_id,
            server_name: "inner".into(),
            session_id: "s1".into(),
            file_name: "x.cast.enc".into(),
            cols: 80,
            rows: 24,
        },
    )
    .await
    .unwrap();
    owner!("GET /recordings", "GET", "/api/v1/recordings");
    owner!(
        "GET /recordings/{id}",
        "GET",
        format!("/api/v1/recordings/{rec}")
    );
    owner!("GET /sessions", "GET", "/api/v1/sessions");
    owner!(
        "PUT /sessions/keep",
        "PUT",
        "/api/v1/sessions/keep",
        Some(json!({ "detach_ttl_secs": 3600 }))
    );
    owner!(
        "PUT /sessions/keep (refused)",
        "PUT",
        "/api/v1/sessions/keep",
        Some(json!({ "detach_ttl_secs": 61 }))
    );
    owner!(
        "DELETE /sessions/{id} (missing)",
        "DELETE",
        "/api/v1/sessions/nope"
    );

    // ── Backups
    owner!("POST /backups", "POST", "/api/v1/backups");
    owner!("GET /backups", "GET", "/api/v1/backups");

    // ── Tokens and approvals
    let made = owner!(
        "POST /tokens",
        "POST",
        "/api/v1/tokens",
        Some(json!({ "name": "hermes", "preset": "servers_keys", "expires_in_days": 30 }))
    );
    let tid = made["token"]["id"].as_i64().unwrap();
    let secret = made["secret"].as_str().unwrap().to_string();
    owner!("GET /tokens", "GET", "/api/v1/tokens");
    owner!(
        "PUT /tokens/{id}",
        "PUT",
        format!("/api/v1/tokens/{tid}"),
        Some(json!({ "require_approval": true }))
    );
    let (status, held) = h
        .with(
            &secret,
            "DELETE",
            &format!("/api/v1/servers/{hop_id}"),
            None,
        )
        .await;
    r.keep("DELETE /servers/{id} (held for approval)", status, &held);
    let (status, body) = h.with(&secret, "GET", "/api/v1/tokens", None).await;
    r.keep("GET /tokens (as a token)", status, &body);
    let aid = held["approval"]["id"].as_str().unwrap().to_string();
    owner!("GET /approvals", "GET", "/api/v1/approvals");
    owner!(
        "GET /approvals/{id}",
        "GET",
        format!("/api/v1/approvals/{aid}")
    );
    owner!(
        "POST /approvals/{id}/approve",
        "POST",
        format!("/api/v1/approvals/{aid}/approve")
    );
    owner!(
        "POST /approvals/{id}/deny (decided)",
        "POST",
        format!("/api/v1/approvals/{aid}/deny")
    );
    owner!(
        "POST /tokens/{id}/revoke",
        "POST",
        format!("/api/v1/tokens/{tid}/revoke")
    );
    owner!(
        "DELETE /tokens/{id}",
        "DELETE",
        format!("/api/v1/tokens/{tid}")
    );

    // ── Deletes
    owner!(
        "DELETE /servers/{id}/host-keys",
        "DELETE",
        format!("/api/v1/servers/{sid}/host-keys")
    );
    owner!(
        "DELETE /quick-commands/{id}",
        "DELETE",
        format!("/api/v1/quick-commands/{qc_id}")
    );
    owner!(
        "DELETE /profiles/{id}",
        "DELETE",
        format!("/api/v1/profiles/{profile_id}")
    );
    owner!(
        "DELETE /servers/{id}",
        "DELETE",
        format!("/api/v1/servers/{sid}")
    );
    owner!(
        "DELETE /credentials/{id}",
        "DELETE",
        format!("/api/v1/credentials/{key_id}")
    );
    owner!("POST /auth/ws-ticket", "POST", "/api/v1/auth/ws-ticket");

    // The audit log last, once every entry above has landed
    assert!(
        eventually(|| async {
            queries::audit::list(&pool, 200, None)
                .await
                .unwrap()
                .iter()
                .any(|e| e.action == "credential.deleted")
        })
        .await
    );
    owner!("GET /audit", "GET", "/api/v1/audit?limit=200");

    let got = serde_json::to_value(&r.shapes).unwrap();
    if std::env::var("UPDATE_API_SHAPES").is_ok() {
        std::fs::create_dir_all(std::path::Path::new(GOLDEN).parent().unwrap()).unwrap();
        std::fs::write(GOLDEN, serde_json::to_string_pretty(&got).unwrap() + "\n").unwrap();
        return;
    }
    let want: Value = serde_json::from_str(
        &std::fs::read_to_string(GOLDEN).expect("testdata/api-shapes.json (UPDATE_API_SHAPES=1)"),
    )
    .unwrap();
    let mut diffs = Vec::new();
    let names: BTreeSet<&String> = want
        .as_object()
        .unwrap()
        .keys()
        .chain(got.as_object().unwrap().keys())
        .collect();
    for name in names {
        if want.get(name) != got.get(name) {
            diffs.push(format!(
                "{name}:\n  want {}\n  got  {}",
                want.get(name).map_or("(none)".into(), Value::to_string),
                got.get(name).map_or("(none)".into(), Value::to_string)
            ));
        }
    }
    assert!(
        diffs.is_empty(),
        "API shapes changed:\n{}",
        diffs.join("\n")
    );
}

#[test]
fn shapes_union_array_elements() {
    let v = json!([{ "a": 1, "b": null }, { "a": 2, "b": "x", "c": true }]);
    assert_eq!(
        shape(&v),
        json!([{ "a": "integer", "b": "null|string", "c": "absent|boolean" }])
    );
    assert_eq!(shape(&json!([])), json!(["empty"]));
}
