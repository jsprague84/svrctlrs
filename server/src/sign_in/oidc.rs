//! Sign-in with an OpenID Connect provider (any: Pocket ID, Authelia, Kanidm, Keycloak, …):
//! the authorization code flow with PKCE, `state` and `nonce`, as a confidential client (or a
//! public one when no secret is configured).
//!
//! The provider is found through its discovery document, fetched when first needed and kept
//! for an hour, so the server starts while the provider is down. Its keys are refetched when a
//! token names a key it does not know (at most once a minute). A flow in progress lives in
//! memory for ten minutes, keyed by its `state`; the browser that started it holds a separate
//! random binding in a cookie, so a callback only completes in that browser.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use rand::RngCore;
use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::jwt::{self, Jwk, JwkSet, JwtError};

/// How long a sign-in at the provider may take
pub const FLOW_TTL: Duration = Duration::from_secs(10 * 60);
/// Flows in progress at once; more are refused rather than grow without bound
const MAX_FLOWS: usize = 256;
const DISCOVERY_TTL: Duration = Duration::from_secs(60 * 60);
const KEYS_REFRESH_MIN: Duration = Duration::from_secs(60);
/// Clock difference tolerated on `exp` and `iat`
const LEEWAY_SECS: i64 = 60;

/// What the environment configured
#[derive(Clone)]
pub struct OidcConfig {
    /// What the sign-in button says
    pub name: String,
    pub issuer: String,
    pub client_id: String,
    pub client_secret: Option<String>,
    pub redirect_uri: String,
    pub scopes: String,
    /// When set, the ID token's `groups` must name one of these
    pub allowed_groups: Vec<String>,
}

impl std::fmt::Debug for OidcConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OidcConfig")
            .field("name", &self.name)
            .field("issuer", &self.issuer)
            .field("client_id", &self.client_id)
            .field(
                "client_secret",
                &self.client_secret.as_ref().map(|_| "[REDACTED]"),
            )
            .field("redirect_uri", &self.redirect_uri)
            .field("scopes", &self.scopes)
            .field("allowed_groups", &self.allowed_groups)
            .finish()
    }
}

/// What a sign-in is for
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Purpose {
    /// Sign in, then go to `next` (a local path)
    Login { next: String },
    /// Link the identity to this (signed-in) account
    Link { user_id: i64 },
}

/// A sign-in in progress
#[derive(Debug, Clone)]
pub struct Flow {
    binding: String,
    nonce: String,
    verifier: String,
    pub purpose: Purpose,
    created: Instant,
}

/// The identity the provider vouched for
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    pub issuer: String,
    pub subject: String,
    pub email: Option<String>,
    pub name: Option<String>,
    pub groups: Vec<String>,
}

#[derive(Debug)]
pub enum OidcError {
    /// The provider could not be reached or answered something unusable
    Provider(String),
    /// No such flow in progress, a different browser, or it took too long
    InvalidFlow,
    /// The ID token failed a check
    Token(String),
    /// Valid, but not in an allowed group
    NotInGroup,
    /// Too many sign-ins in progress
    Busy,
}

impl std::fmt::Display for OidcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OidcError::Provider(m) => write!(f, "the identity provider could not be used: {m}"),
            OidcError::InvalidFlow => {
                write!(f, "that sign-in has expired or was started elsewhere")
            }
            OidcError::Token(m) => write!(f, "the identity provider's answer failed a check: {m}"),
            OidcError::NotInGroup => write!(f, "that account is not in a group allowed here"),
            OidcError::Busy => write!(f, "too many sign-ins are in progress; try again shortly"),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct Discovery {
    issuer: String,
    authorization_endpoint: String,
    token_endpoint: String,
    jwks_uri: String,
}

struct Cached {
    discovery: Discovery,
    fetched: Instant,
    keys: Vec<Jwk>,
    keys_fetched: Instant,
}

pub struct Provider {
    pub config: OidcConfig,
    http: reqwest::Client,
    cache: tokio::sync::Mutex<Option<Cached>>,
    flows: Mutex<HashMap<String, Flow>>,
}

impl std::fmt::Debug for Provider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Provider")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

fn random_token() -> String {
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// `application/x-www-form-urlencoded` for one value (RFC 6749 §2.3.1 wants the client id and
/// secret encoded this way before they go into Basic authentication).
fn form_encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'*' => {
                (b as char).to_string()
            }
            b' ' => "+".to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// Issuers compare exactly, except for one trailing slash written in the configuration.
fn same_issuer(configured: &str, published: &str) -> bool {
    configured == published || configured.trim_end_matches('/') == published.trim_end_matches('/')
}

impl Provider {
    pub fn new(config: OidcConfig) -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::limited(3))
            .user_agent(concat!("svrctlrs/", env!("CARGO_PKG_VERSION")))
            .build()
            .unwrap_or_default();
        Self {
            config,
            http,
            cache: tokio::sync::Mutex::new(None),
            flows: Mutex::new(HashMap::new()),
        }
    }

    async fn get_json<T: serde::de::DeserializeOwned>(&self, url: &str) -> Result<T, OidcError> {
        let response = self
            .http
            .get(url)
            .send()
            .await
            .map_err(|e| OidcError::Provider(format!("{url}: {e}")))?;
        if !response.status().is_success() {
            return Err(OidcError::Provider(format!(
                "{url} answered {}",
                response.status()
            )));
        }
        response
            .json::<T>()
            .await
            .map_err(|e| OidcError::Provider(format!("{url}: {e}")))
    }

    /// The discovery document and keys, fetched when missing or older than an hour.
    async fn metadata(&self) -> Result<(Discovery, Vec<Jwk>), OidcError> {
        let mut cache = self.cache.lock().await;
        if let Some(c) = cache
            .as_ref()
            .filter(|c| c.fetched.elapsed() < DISCOVERY_TTL)
        {
            return Ok((c.discovery.clone(), c.keys.clone()));
        }
        let url = format!(
            "{}/.well-known/openid-configuration",
            self.config.issuer.trim_end_matches('/')
        );
        let discovery: Discovery = self.get_json(&url).await?;
        if !same_issuer(&self.config.issuer, &discovery.issuer) {
            return Err(OidcError::Provider(format!(
                "the discovery document names the issuer {:?}, not {:?}",
                discovery.issuer, self.config.issuer
            )));
        }
        let keys = self.get_json::<JwkSet>(&discovery.jwks_uri).await?.keys;
        let now = Instant::now();
        *cache = Some(Cached {
            discovery: discovery.clone(),
            fetched: now,
            keys: keys.clone(),
            keys_fetched: now,
        });
        Ok((discovery, keys))
    }

    /// Fetch the keys again (a token named one we do not have), at most once a minute.
    async fn refresh_keys(&self) -> Result<Option<Vec<Jwk>>, OidcError> {
        let mut cache = self.cache.lock().await;
        let Some(c) = cache.as_mut() else {
            return Ok(None);
        };
        if c.keys_fetched.elapsed() < KEYS_REFRESH_MIN {
            return Ok(None);
        }
        let keys = self.get_json::<JwkSet>(&c.discovery.jwks_uri).await?.keys;
        c.keys = keys.clone();
        c.keys_fetched = Instant::now();
        Ok(Some(keys))
    }

    /// Start a sign-in: the provider URL to send the browser to, and the binding for its
    /// cookie.
    pub async fn begin(&self, purpose: Purpose) -> Result<(String, String), OidcError> {
        let (discovery, _) = self.metadata().await?;
        let state = random_token();
        let flow = Flow {
            binding: random_token(),
            nonce: random_token(),
            verifier: random_token(),
            purpose,
            created: Instant::now(),
        };
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(flow.verifier.as_bytes()));
        let url = reqwest::Url::parse_with_params(
            &discovery.authorization_endpoint,
            &[
                ("response_type", "code"),
                ("client_id", self.config.client_id.as_str()),
                ("redirect_uri", self.config.redirect_uri.as_str()),
                ("scope", self.config.scopes.as_str()),
                ("state", state.as_str()),
                ("nonce", flow.nonce.as_str()),
                ("code_challenge", challenge.as_str()),
                ("code_challenge_method", "S256"),
            ],
        )
        .map_err(|e| OidcError::Provider(format!("authorization_endpoint: {e}")))?;
        let binding = flow.binding.clone();
        {
            let mut flows = self.flows.lock().unwrap_or_else(|p| p.into_inner());
            flows.retain(|_, f| f.created.elapsed() < FLOW_TTL);
            if flows.len() >= MAX_FLOWS {
                return Err(OidcError::Busy);
            }
            flows.insert(state, flow);
        }
        Ok((url.to_string(), binding))
    }

    /// Take the flow for `state` (once), if this browser started it and it is still fresh.
    pub fn take_flow(&self, state: &str, binding: Option<&str>) -> Result<Flow, OidcError> {
        let flow = self
            .flows
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(state)
            .ok_or(OidcError::InvalidFlow)?;
        let same_browser =
            binding.is_some_and(|b| constant_time_eq(b.as_bytes(), flow.binding.as_bytes()));
        if !same_browser || flow.created.elapsed() >= FLOW_TTL {
            return Err(OidcError::InvalidFlow);
        }
        Ok(flow)
    }

    /// Exchange the code for tokens and check the ID token.
    pub async fn finish(
        &self,
        flow: &Flow,
        code: &str,
        iss_param: Option<&str>,
    ) -> Result<Identity, OidcError> {
        let (discovery, keys) = self.metadata().await?;
        // RFC 9207: a provider that names itself on the callback must name the right issuer
        if let Some(iss) = iss_param {
            if iss != discovery.issuer {
                return Err(OidcError::Token(format!(
                    "the callback came from issuer {iss:?}"
                )));
            }
        }

        let mut form = vec![
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", self.config.redirect_uri.as_str()),
            ("code_verifier", flow.verifier.as_str()),
        ];
        let mut request = self.http.post(&discovery.token_endpoint);
        match &self.config.client_secret {
            Some(secret) => {
                let basic = base64::engine::general_purpose::STANDARD.encode(format!(
                    "{}:{}",
                    form_encode(&self.config.client_id),
                    form_encode(secret)
                ));
                request = request.header(reqwest::header::AUTHORIZATION, format!("Basic {basic}"));
            }
            None => form.push(("client_id", self.config.client_id.as_str())),
        }
        let response = request
            .form(&form)
            .send()
            .await
            .map_err(|e| OidcError::Provider(format!("token endpoint: {e}")))?;
        let status = response.status();
        let body: serde_json::Value = response
            .json()
            .await
            .map_err(|e| OidcError::Provider(format!("token endpoint: {e}")))?;
        if !status.is_success() {
            let error = body["error"].as_str().unwrap_or("error");
            let description = body["error_description"].as_str().unwrap_or("");
            return Err(OidcError::Provider(format!(
                "token endpoint answered {status}: {error} {description}"
            )));
        }
        let id_token = body["id_token"]
            .as_str()
            .ok_or_else(|| OidcError::Token("no ID token in the answer".into()))?;

        let payload = match jwt::verify(id_token, &keys) {
            Err(JwtError::NoKey(_)) => match self.refresh_keys().await? {
                Some(fresh) => jwt::verify(id_token, &fresh),
                None => jwt::verify(id_token, &keys),
            },
            other => other,
        }
        .map_err(|e| OidcError::Token(e.to_string()))?;

        let identity = check_claims(
            &payload,
            &discovery.issuer,
            &self.config.client_id,
            &flow.nonce,
            chrono::Utc::now().timestamp(),
        )
        .map_err(OidcError::Token)?;
        if !self.config.allowed_groups.is_empty()
            && !identity
                .groups
                .iter()
                .any(|g| self.config.allowed_groups.contains(g))
        {
            return Err(OidcError::NotInGroup);
        }
        Ok(identity)
    }
}

/// The ID token's claims, checked (OpenID Connect Core §3.1.3.7): issuer, audience and
/// authorized party, expiry and issue time with a minute's leeway, the nonce of this flow,
/// and a subject.
pub fn check_claims(
    payload: &[u8],
    issuer: &str,
    client_id: &str,
    nonce: &str,
    now: i64,
) -> Result<Identity, String> {
    let claims: serde_json::Value =
        serde_json::from_slice(payload).map_err(|_| "the claims are not JSON".to_string())?;
    if claims["iss"].as_str() != Some(issuer) {
        return Err(format!("issued by {}, not {issuer:?}", claims["iss"]));
    }
    let audiences: Vec<&str> = match &claims["aud"] {
        serde_json::Value::String(a) => vec![a.as_str()],
        serde_json::Value::Array(a) => a.iter().filter_map(|v| v.as_str()).collect(),
        _ => vec![],
    };
    if !audiences.contains(&client_id) {
        return Err("issued for another client".into());
    }
    match claims["azp"].as_str() {
        Some(azp) if azp != client_id => return Err("authorized for another client".into()),
        None if audiences.len() > 1 => {
            return Err("several audiences and no authorized party".into())
        }
        _ => {}
    }
    let exp = claims["exp"]
        .as_i64()
        .ok_or_else(|| "no expiry".to_string())?;
    if exp + LEEWAY_SECS <= now {
        return Err("expired".into());
    }
    if claims["iat"]
        .as_i64()
        .is_some_and(|iat| iat > now + LEEWAY_SECS)
    {
        return Err("issued in the future".into());
    }
    let token_nonce = claims["nonce"].as_str().unwrap_or("");
    if !constant_time_eq(token_nonce.as_bytes(), nonce.as_bytes()) {
        return Err("the nonce does not match this sign-in".into());
    }
    let subject = claims["sub"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "no subject".to_string())?;
    let text = |key: &str| {
        claims[key]
            .as_str()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from)
    };
    Ok(Identity {
        issuer: issuer.to_string(),
        subject: subject.to_string(),
        email: text("email"),
        name: text("name")
            .or_else(|| text("preferred_username"))
            .or_else(|| text("display_name")),
        groups: claims["groups"]
            .as_array()
            .map(|g| {
                g.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const NOW: i64 = 1_800_000_000;

    fn claims(overrides: serde_json::Value) -> Vec<u8> {
        let mut c = json!({
            "iss": "https://id.example",
            "aud": "client-1",
            "sub": "user-123",
            "exp": NOW + 300,
            "iat": NOW,
            "nonce": "n-1",
            "email": "ann@example.com",
            "name": "Ann",
            "groups": ["owner"],
        });
        for (k, v) in overrides.as_object().unwrap() {
            if v.is_null() {
                c.as_object_mut().unwrap().remove(k);
            } else {
                c[k] = v.clone();
            }
        }
        serde_json::to_vec(&c).unwrap()
    }

    fn check(overrides: serde_json::Value) -> Result<Identity, String> {
        check_claims(
            &claims(overrides),
            "https://id.example",
            "client-1",
            "n-1",
            NOW,
        )
    }

    #[test]
    fn a_good_token_gives_the_identity() {
        let id = check(json!({})).unwrap();
        assert_eq!(id.subject, "user-123");
        assert_eq!(id.issuer, "https://id.example");
        assert_eq!(id.email.as_deref(), Some("ann@example.com"));
        assert_eq!(id.name.as_deref(), Some("Ann"));
        assert_eq!(id.groups, ["owner"]);
        // An audience list with us as the authorized party
        check(json!({ "aud": ["client-1", "other"], "azp": "client-1" })).unwrap();
        // Within the leeway
        check(json!({ "exp": NOW - 30 })).unwrap();
    }

    #[test]
    fn every_check_refuses_on_its_own() {
        for (what, overrides) in [
            ("issuer", json!({ "iss": "https://evil.example" })),
            ("audience", json!({ "aud": "client-2" })),
            ("no audience", json!({ "aud": null })),
            ("azp", json!({ "azp": "client-2" })),
            (
                "several aud, no azp",
                json!({ "aud": ["client-1", "other"] }),
            ),
            ("expired", json!({ "exp": NOW - 61 })),
            ("no exp", json!({ "exp": null })),
            ("future iat", json!({ "iat": NOW + 120 })),
            ("nonce", json!({ "nonce": "n-2" })),
            ("no nonce", json!({ "nonce": null })),
            ("no sub", json!({ "sub": null })),
            ("empty sub", json!({ "sub": "" })),
        ] {
            assert!(check(overrides).is_err(), "{what} should be refused");
        }
        assert!(check_claims(b"not json", "https://id.example", "client-1", "n-1", NOW).is_err());
    }

    #[test]
    fn issuers_tolerate_one_configured_trailing_slash_only() {
        assert!(same_issuer("https://id.example/", "https://id.example"));
        assert!(same_issuer("https://id.example", "https://id.example"));
        assert!(!same_issuer(
            "https://id.example.evil",
            "https://id.example"
        ));
    }

    #[test]
    fn form_encoding_follows_rfc_6749() {
        assert_eq!(form_encode("a b:c/d"), "a+b%3Ac%2Fd");
        assert_eq!(form_encode("Abc-1._*"), "Abc-1._*");
    }

    #[tokio::test]
    async fn a_flow_completes_once_in_the_browser_that_started_it() {
        let provider = Provider::new(OidcConfig {
            name: "Test".into(),
            issuer: "https://id.example".into(),
            client_id: "client-1".into(),
            client_secret: None,
            redirect_uri: "https://app.example/auth/oidc/callback".into(),
            scopes: "openid".into(),
            allowed_groups: vec![],
        });
        let flow = Flow {
            binding: "b-1".into(),
            nonce: "n".into(),
            verifier: "v".into(),
            purpose: Purpose::Login { next: "/".into() },
            created: Instant::now(),
        };
        provider
            .flows
            .lock()
            .unwrap()
            .insert("s-1".into(), flow.clone());
        provider.flows.lock().unwrap().insert("s-2".into(), flow);
        assert!(matches!(
            provider.take_flow("s-1", Some("b-2")),
            Err(OidcError::InvalidFlow)
        ));
        // A wrong binding used it up, too
        assert!(provider.take_flow("s-1", Some("b-1")).is_err());
        assert!(matches!(
            provider.take_flow("s-2", None),
            Err(OidcError::InvalidFlow)
        ));
        assert!(provider.take_flow("nope", Some("b-1")).is_err());
    }
}
