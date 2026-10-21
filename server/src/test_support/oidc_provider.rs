//! A small OpenID Connect provider on a loopback port, for driving the sign-in routes end to
//! end: discovery, keys (one Ed25519 key), and a token endpoint that checks the client, the
//! redirect URI and the PKCE verifier before it hands out an ID token. The test plays the
//! user at the authorization step: it reads the parameters the app sent and calls
//! [`FakeProvider::approve`] for a code.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Form, Json, Router,
};
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use base64::Engine;
use russh::keys::signature::Signer;
use russh::keys::ssh_key::private::Ed25519Keypair;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::sign_in::oidc::OidcConfig;

pub const CLIENT_ID: &str = "svrctlrs-test";
pub const CLIENT_SECRET: &str = "test secret/with:odd chars";

struct Grant {
    challenge: String,
    redirect_uri: String,
    claims: Value,
}

struct Shared {
    issuer: String,
    key: Ed25519Keypair,
    grants: Mutex<HashMap<String, Grant>>,
}

pub struct FakeProvider {
    pub issuer: String,
    shared: Arc<Shared>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for FakeProvider {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// The query parameters of a URL
pub fn query_of(url: &str) -> HashMap<String, String> {
    reqwest::Url::parse(url)
        .expect("a URL")
        .query_pairs()
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect()
}

impl FakeProvider {
    pub async fn start() -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let issuer = format!("http://{}", listener.local_addr().unwrap());
        let shared = Arc::new(Shared {
            issuer: issuer.clone(),
            key: Ed25519Keypair::from_seed(&[7u8; 32]),
            grants: Mutex::new(HashMap::new()),
        });
        let app = Router::new()
            .route("/.well-known/openid-configuration", get(discovery))
            .route("/jwks", get(jwks))
            .route("/token", post(token))
            .with_state(shared.clone());
        let task = tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        Self {
            issuer,
            shared,
            task,
        }
    }

    /// The configuration svrctlrs would build for this provider
    pub fn config(&self, allowed_groups: &[&str]) -> OidcConfig {
        OidcConfig {
            name: "Test ID".into(),
            issuer: self.issuer.clone(),
            client_id: CLIENT_ID.into(),
            client_secret: Some(CLIENT_SECRET.into()),
            redirect_uri: "http://localhost/auth/oidc/callback".into(),
            scopes: "openid profile email groups".into(),
            allowed_groups: allowed_groups.iter().map(|s| s.to_string()).collect(),
        }
    }

    /// The user signs in at the provider as `subject`: a code for the authorization request
    /// `authorize_url`, whose ID token carries these claims (plus `extra`).
    pub fn approve(&self, authorize_url: &str, subject: &str, extra: Value) -> String {
        let q = query_of(authorize_url);
        assert_eq!(q["response_type"], "code");
        assert_eq!(q["client_id"], CLIENT_ID);
        assert_eq!(q["code_challenge_method"], "S256");
        let now = chrono::Utc::now().timestamp();
        let mut claims = json!({
            "iss": self.issuer,
            "aud": CLIENT_ID,
            "sub": subject,
            "iat": now,
            "exp": now + 300,
            "nonce": q["nonce"],
            "email": format!("{subject}@example.com"),
            "name": subject,
            "groups": ["owner"],
        });
        for (k, v) in extra.as_object().cloned().unwrap_or_default() {
            claims[k] = v;
        }
        let code = format!("code-{}", rand::random::<u64>());
        self.shared.grants.lock().unwrap().insert(
            code.clone(),
            Grant {
                challenge: q["code_challenge"].clone(),
                redirect_uri: q["redirect_uri"].clone(),
                claims,
            },
        );
        code
    }
}

async fn discovery(State(s): State<Arc<Shared>>) -> Json<Value> {
    Json(json!({
        "issuer": s.issuer,
        "authorization_endpoint": format!("{}/authorize", s.issuer),
        "token_endpoint": format!("{}/token", s.issuer),
        "jwks_uri": format!("{}/jwks", s.issuer),
    }))
}

async fn jwks(State(s): State<Arc<Shared>>) -> Json<Value> {
    Json(json!({ "keys": [{
        "kty": "OKP",
        "crv": "Ed25519",
        "kid": "k1",
        "alg": "EdDSA",
        "use": "sig",
        "x": URL_SAFE_NO_PAD.encode(s.key.public.0),
    }]}))
}

fn form_encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'*' => {
                (b as char).to_string()
            }
            b' ' => "+".into(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

async fn token(
    State(s): State<Arc<Shared>>,
    headers: HeaderMap,
    Form(form): Form<HashMap<String, String>>,
) -> (StatusCode, Json<Value>) {
    let refuse = |e: &str| (StatusCode::BAD_REQUEST, Json(json!({ "error": e })));
    let basic = format!(
        "Basic {}",
        STANDARD.encode(format!(
            "{}:{}",
            form_encode(CLIENT_ID),
            form_encode(CLIENT_SECRET)
        ))
    );
    if headers.get("authorization").and_then(|v| v.to_str().ok()) != Some(basic.as_str()) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "invalid_client" })),
        );
    }
    if form.get("grant_type").map(String::as_str) != Some("authorization_code") {
        return refuse("unsupported_grant_type");
    }
    // A code works once
    let Some(grant) = form
        .get("code")
        .and_then(|c| s.grants.lock().unwrap().remove(c))
    else {
        return refuse("invalid_grant");
    };
    if form.get("redirect_uri") != Some(&grant.redirect_uri) {
        return refuse("invalid_grant");
    }
    let verifier = form.get("code_verifier").cloned().unwrap_or_default();
    if URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes())) != grant.challenge {
        return refuse("invalid_grant");
    }
    let header = URL_SAFE_NO_PAD.encode(br#"{"alg":"EdDSA","kid":"k1","typ":"JWT"}"#);
    let payload = URL_SAFE_NO_PAD.encode(grant.claims.to_string());
    let input = format!("{header}.{payload}");
    let signature: russh::keys::ssh_key::Signature = s.key.try_sign(input.as_bytes()).unwrap();
    let id_token = format!("{input}.{}", URL_SAFE_NO_PAD.encode(signature.as_bytes()));
    (
        StatusCode::OK,
        Json(json!({ "id_token": id_token, "access_token": "at", "token_type": "Bearer" })),
    )
}
