//! API tokens: their format, how a presented token is checked, and what each access preset
//! may do.
//!
//! A token is `svr_` followed by 40 random alphanumerics (about 238 bits), so it can be told
//! apart from a session id by its prefix and length, selected with a double-click, and found
//! by a secret scanner. Only its SHA-256 is stored (the secret is random, so a slow password
//! hash would add nothing but latency on every request). The owner sees the secret once.
//!
//! **Access is decided here and only here**, by method and path, before any handler runs
//! ([`Preset::allows`], called from `routes::ui::auth::require_auth`). Anything a preset does
//! not name is refused with 403. No preset names token or account management, so a token can
//! never create, change or revoke tokens or accounts: revoking it ends everything it can do.

use axum::http::Method;
use rand::{distributions::Alphanumeric, Rng};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use svrctlrs_database::{models::ApiToken, queries::api_tokens, SqlxPool, SqlxSqlite};

/// What every token starts with
pub const TOKEN_PREFIX: &str = "svr_";
/// Random characters after the prefix
const SECRET_LEN: usize = 40;
/// Characters after the prefix kept in the clear, so a list can show which token is which
const SHOWN_LEN: usize = 6;
/// `last_used_at` is written at most this often per token
const TOUCH_INTERVAL_SECS: i64 = 60;

/// A freshly generated token: the secret for the owner (once) and what the database keeps.
pub struct NewSecret {
    pub secret: String,
    pub prefix: String,
    pub hash: String,
}

pub fn generate() -> NewSecret {
    let body: String = rand::rngs::OsRng
        .sample_iter(&Alphanumeric)
        .take(SECRET_LEN)
        .map(char::from)
        .collect();
    let secret = format!("{TOKEN_PREFIX}{body}");
    NewSecret {
        prefix: secret[..TOKEN_PREFIX.len() + SHOWN_LEN].to_string(),
        hash: hash(&secret),
        secret,
    }
}

/// Hex SHA-256 of the whole token, prefix included.
pub fn hash(secret: &str) -> String {
    Sha256::digest(secret.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Whether a credential is meant as an API token (as opposed to a session id). Session ids
/// are 22 characters of base64 and never take this path unless they start with `svr_`, in
/// which case [`well_formed`] fails them on length.
pub fn is_api_token(value: &str) -> bool {
    value.starts_with(TOKEN_PREFIX)
}

fn well_formed(value: &str) -> bool {
    value.strip_prefix(TOKEN_PREFIX).is_some_and(|body| {
        body.len() == SECRET_LEN && body.bytes().all(|b| b.is_ascii_alphanumeric())
    })
}

/// How much of the owner's access a token carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Preset {
    /// Everything the owner can do, terminals and files included, except managing tokens and
    /// accounts.
    FullAdmin,
    /// Read-only, plus managing servers, host keys and credentials.
    ServersKeys,
    /// Reading the inventory and configuration: no changes, no terminals, no files, no
    /// recordings' content.
    ReadOnly,
}

/// Paths a read-only token may GET (and HEAD). `*` is one path segment.
const READ: &[&str] = &[
    "/api/v1/status",
    "/api/v1/openapi.json",
    "/api/v1/servers",
    "/api/v1/servers/*",
    "/api/v1/servers/*/host-keys",
    "/api/v1/credentials",
    "/api/v1/credentials/*",
    "/api/v1/settings",
    "/api/v1/settings/*",
    "/api/v1/profiles",
    "/api/v1/profiles/*",
    "/api/v1/quick-commands",
    "/api/v1/quick-commands/*",
    "/api/v1/users",
    "/api/v1/users/me/preferences/*",
    "/api/v1/audit",
    "/api/v1/sessions",
    "/api/v1/recordings",
    "/api/v1/recordings/*",
    // A token waits on its own approval request here
    "/api/v1/approvals/*",
];

/// What "servers & keys" adds to [`READ`]: managing servers, host keys and credentials.
const SERVERS_KEYS: &[(&str, &str)] = &[
    ("POST", "/api/v1/servers"),
    ("POST", "/api/v1/servers/import"),
    ("PUT", "/api/v1/servers/*"),
    ("DELETE", "/api/v1/servers/*"),
    ("POST", "/api/v1/servers/*/test"),
    ("DELETE", "/api/v1/servers/*/host-keys"),
    ("POST", "/api/v1/servers/*/host-keys/trust"),
    ("POST", "/api/v1/servers/*/authorize-key"),
    ("POST", "/api/v1/credentials"),
    ("POST", "/api/v1/credentials/generate-key"),
    ("PUT", "/api/v1/credentials/*"),
    ("DELETE", "/api/v1/credentials/*"),
    ("POST", "/api/v1/servers/*/diagnose"),
    ("POST", "/api/v1/servers/onboard"),
    ("POST", "/api/v1/servers/*/rotate-key"),
];

/// What "full admin" adds to both: terminals (through a WebSocket ticket), files,
/// recordings, sessions, and the app's own settings, profiles and quick commands.
const FULL_ADMIN: &[(&str, &str)] = &[
    ("POST", "/api/v1/auth/ws-ticket"),
    ("GET", "/ws/terminal/pty"),
    ("POST", "/api/v1/servers/*/exec"),
    ("GET", "/api/v1/servers/*/files"),
    ("DELETE", "/api/v1/servers/*/files"),
    ("GET", "/api/v1/servers/*/files/content"),
    ("PUT", "/api/v1/servers/*/files/content"),
    ("POST", "/api/v1/servers/*/files/mkdir"),
    ("POST", "/api/v1/servers/*/files/rename"),
    ("GET", "/api/v1/recordings/*/cast"),
    ("DELETE", "/api/v1/recordings/*"),
    ("DELETE", "/api/v1/sessions/*"),
    ("POST", "/api/v1/settings"),
    ("PUT", "/api/v1/settings"),
    ("PUT", "/api/v1/settings/*"),
    ("POST", "/api/v1/profiles"),
    ("PUT", "/api/v1/profiles/*"),
    ("DELETE", "/api/v1/profiles/*"),
    ("POST", "/api/v1/quick-commands"),
    ("PUT", "/api/v1/quick-commands/*"),
    ("DELETE", "/api/v1/quick-commands/*"),
    ("PUT", "/api/v1/users/me/preferences/*"),
    ("POST", "/api/v1/notifications/test"),
    ("POST", "/api/v1/notifications/test-approval"),
];

impl Preset {
    pub const ALL: [Preset; 3] = [Preset::FullAdmin, Preset::ServersKeys, Preset::ReadOnly];

    pub fn as_str(self) -> &'static str {
        match self {
            Preset::FullAdmin => "full_admin",
            Preset::ServersKeys => "servers_keys",
            Preset::ReadOnly => "read_only",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.as_str() == s)
    }

    pub fn label(self) -> &'static str {
        match self {
            Preset::FullAdmin => "full admin",
            Preset::ServersKeys => "servers & keys",
            Preset::ReadOnly => "read-only",
        }
    }

    /// Whether a token with this preset may make this request. Deny by default: the method
    /// and every path segment must match a rule exactly (`*` is one non-empty segment).
    pub fn allows(self, method: &Method, path: &str) -> bool {
        let method = if method == Method::HEAD {
            "GET"
        } else {
            method.as_str()
        };
        let read = method == "GET" && READ.iter().any(|p| path_matches(p, path));
        let named = |rules: &[(&str, &str)]| {
            rules
                .iter()
                .any(|(m, p)| *m == method && path_matches(p, path))
        };
        match self {
            Preset::ReadOnly => read,
            Preset::ServersKeys => read || named(SERVERS_KEYS),
            Preset::FullAdmin => read || named(SERVERS_KEYS) || named(FULL_ADMIN),
        }
    }
}

/// `pattern` and `path` have the same segments, `*` standing for any one non-empty segment.
/// One trailing slash on the path is ignored. Nothing is decoded or normalised, exactly as
/// the router does not, so a path that matches here is the path the router sees.
pub(crate) fn path_matches(pattern: &str, path: &str) -> bool {
    let path = match path.strip_suffix('/') {
        Some(p) if !p.is_empty() => p,
        _ => path,
    };
    let mut want = pattern.split('/');
    let mut have = path.split('/');
    loop {
        match (want.next(), have.next()) {
            (None, None) => return true,
            (Some("*"), Some(seg)) if !seg.is_empty() => {}
            (Some(a), Some(b)) if a == b => {}
            _ => return false,
        }
    }
}

/// The token a request authenticated with. Set as a request extension next to
/// `CurrentUser`, carried by WebSocket tickets, and named in every audit row the request
/// writes (see `audit.rs`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenAuth {
    pub id: i64,
    pub user_id: i64,
    pub name: String,
    pub preset: Preset,
    /// Gated deletes wait for the owner's approval (`approvals.rs`)
    pub require_approval: bool,
}

/// What became of a presented token.
#[derive(Debug)]
pub enum TokenCheck {
    /// Good; `touch` says whether its last-used time is due for an update.
    Valid {
        token: TokenAuth,
        touch: bool,
    },
    Malformed,
    Unknown,
    Expired(String),
    Revoked(String),
    /// The lookup itself failed; refuse, and the log says why.
    Error,
}

/// Look a presented token up and judge it at `now`.
pub async fn check(
    pool: &SqlxPool<SqlxSqlite>,
    value: &str,
    now: chrono::DateTime<chrono::Utc>,
) -> TokenCheck {
    if !well_formed(value) {
        return TokenCheck::Malformed;
    }
    match api_tokens::find_by_hash(pool, &hash(value)).await {
        Ok(Some(row)) => judge(row, now),
        Ok(None) => TokenCheck::Unknown,
        Err(e) => {
            tracing::error!(error = %e, "API token lookup failed");
            TokenCheck::Error
        }
    }
}

fn judge(row: ApiToken, now: chrono::DateTime<chrono::Utc>) -> TokenCheck {
    if row.revoked_at.is_some() {
        return TokenCheck::Revoked(row.name);
    }
    if row.is_expired_at(now) {
        return TokenCheck::Expired(row.name);
    }
    let Some(preset) = Preset::parse(&row.preset) else {
        tracing::error!(token_id = row.id, preset = %row.preset, "API token has an unknown preset");
        return TokenCheck::Error;
    };
    let touch = row
        .last_used_at
        .is_none_or(|at| (now - at).num_seconds() >= TOUCH_INTERVAL_SECS);
    TokenCheck::Valid {
        token: TokenAuth {
            id: row.id,
            user_id: row.user_id,
            name: row.name,
            preset,
            require_approval: row.require_approval,
        },
        touch,
    }
}

/// Whether the token is still usable now (for a WebSocket ticket minted with it).
pub async fn still_valid(pool: &SqlxPool<SqlxSqlite>, id: i64) -> bool {
    match api_tokens::get(pool, id).await {
        Ok(Some(row)) => matches!(judge(row, chrono::Utc::now()), TokenCheck::Valid { .. }),
        Ok(None) => false,
        Err(e) => {
            tracing::error!(error = %e, "API token lookup failed");
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_tokens_have_the_documented_shape() {
        let t = generate();
        assert!(t.secret.starts_with("svr_"));
        assert_eq!(t.secret.len(), 44);
        assert!(well_formed(&t.secret));
        assert_eq!(t.prefix, &t.secret[..10]);
        assert_eq!(t.hash, hash(&t.secret));
        assert_eq!(t.hash.len(), 64);
        assert_ne!(generate().secret, t.secret);
    }

    #[test]
    fn malformed_tokens_are_recognised() {
        assert!(!well_formed("svr_short"));
        assert!(!well_formed(&format!("svr_{}", "a".repeat(41))));
        assert!(!well_formed(&format!("svr_{}-", "a".repeat(39))));
        assert!(!well_formed(&format!("xyz_{}", "a".repeat(40))));
        assert!(well_formed(&format!("svr_{}", "aZ9".repeat(13) + "q")));
        // A session id is not an API token
        assert!(!is_api_token("dGhpcyBpcyBhIHNlc3Npb24"));
    }

    #[test]
    fn paths_match_segment_by_segment() {
        assert!(path_matches("/api/v1/servers", "/api/v1/servers"));
        assert!(path_matches("/api/v1/servers", "/api/v1/servers/"));
        assert!(path_matches("/api/v1/servers/*", "/api/v1/servers/12"));
        assert!(!path_matches("/api/v1/servers/*", "/api/v1/servers"));
        assert!(!path_matches("/api/v1/servers/*", "/api/v1/servers//"));
        assert!(!path_matches("/api/v1/servers/*", "/api/v1/servers/1/test"));
        assert!(!path_matches(
            "/api/v1/servers/*/test",
            "/api/v1/servers//test"
        ));
        assert!(!path_matches("/api/v1/servers", "/API/v1/servers"));
        assert!(!path_matches("/api/v1/servers", "/api/v1/serversx"));
    }

    /// Every route the app serves, and which presets may use it. A route added to the app
    /// without a line here is refused to every token, which is the safe failure.
    #[test]
    fn presets_allow_exactly_what_they_name() {
        use Preset::*;
        let all: &[Preset] = &[FullAdmin, ServersKeys, ReadOnly];
        let write: &[Preset] = &[FullAdmin, ServersKeys];
        let full: &[Preset] = &[FullAdmin];
        let none: &[Preset] = &[];
        let cases: &[(&str, &str, &[Preset])] = &[
            ("GET", "/api/v1/status", all),
            ("GET", "/api/v1/openapi.json", all),
            ("GET", "/api/v1/servers", all),
            ("HEAD", "/api/v1/servers", all),
            ("POST", "/api/v1/servers", write),
            ("POST", "/api/v1/servers/import", write),
            ("GET", "/api/v1/servers/3", all),
            ("PUT", "/api/v1/servers/3", write),
            ("DELETE", "/api/v1/servers/3", write),
            ("POST", "/api/v1/servers/3/test", write),
            ("GET", "/api/v1/servers/3/host-keys", all),
            ("DELETE", "/api/v1/servers/3/host-keys", write),
            ("POST", "/api/v1/servers/3/host-keys/trust", write),
            ("POST", "/api/v1/servers/3/authorize-key", write),
            ("POST", "/api/v1/servers/3/diagnose", write),
            ("POST", "/api/v1/servers/onboard", write),
            ("POST", "/api/v1/servers/3/rotate-key", write),
            ("POST", "/api/v1/servers/3/exec", full),
            ("GET", "/api/v1/servers/3/files", full),
            ("DELETE", "/api/v1/servers/3/files", full),
            ("GET", "/api/v1/servers/3/files/content", full),
            ("PUT", "/api/v1/servers/3/files/content", full),
            ("POST", "/api/v1/servers/3/files/mkdir", full),
            ("POST", "/api/v1/servers/3/files/rename", full),
            ("GET", "/api/v1/credentials", all),
            ("POST", "/api/v1/credentials", write),
            ("POST", "/api/v1/credentials/generate-key", write),
            ("GET", "/api/v1/credentials/4", all),
            ("PUT", "/api/v1/credentials/4", write),
            ("DELETE", "/api/v1/credentials/4", write),
            ("GET", "/api/v1/settings", all),
            ("POST", "/api/v1/settings", full),
            ("PUT", "/api/v1/settings", full),
            ("GET", "/api/v1/settings/notification.url", all),
            ("PUT", "/api/v1/settings/notification.url", full),
            ("GET", "/api/v1/profiles", all),
            ("POST", "/api/v1/profiles", full),
            ("PUT", "/api/v1/profiles/1", full),
            ("DELETE", "/api/v1/profiles/1", full),
            ("GET", "/api/v1/quick-commands", all),
            ("POST", "/api/v1/quick-commands", full),
            ("PUT", "/api/v1/quick-commands/1", full),
            ("DELETE", "/api/v1/quick-commands/1", full),
            ("POST", "/api/v1/notifications/test", full),
            ("GET", "/api/v1/users", all),
            ("GET", "/api/v1/users/me/preferences/terminal", all),
            ("PUT", "/api/v1/users/me/preferences/terminal", full),
            ("GET", "/api/v1/audit", all),
            ("GET", "/api/v1/sessions", all),
            ("DELETE", "/api/v1/sessions/abc", full),
            ("GET", "/api/v1/recordings", all),
            ("GET", "/api/v1/recordings/5", all),
            ("GET", "/api/v1/recordings/5/cast", full),
            ("DELETE", "/api/v1/recordings/5", full),
            ("POST", "/api/v1/auth/ws-ticket", full),
            ("GET", "/ws/terminal/pty", full),
            ("POST", "/api/v1/notifications/test-approval", full),
            ("GET", "/api/v1/approvals/Zx9_abc", all),
            ("PUT", "/api/v1/tokens/1", none),
            // Approving is the owner's alone
            ("GET", "/api/v1/approvals", none),
            ("POST", "/api/v1/approvals/Zx9_abc/approve", none),
            ("POST", "/api/v1/approvals/Zx9_abc/deny", none),
            // Never for a token: tokens, accounts, the owner's password, the HTML app
            ("GET", "/api/v1/tokens", none),
            ("POST", "/api/v1/tokens", none),
            ("POST", "/api/v1/tokens/1/revoke", none),
            ("DELETE", "/api/v1/tokens/1", none),
            ("POST", "/api/v1/users", none),
            ("DELETE", "/api/v1/users/2", none),
            ("DELETE", "/api/v1/users/me", none),
            ("PUT", "/api/v1/users/me/password", none),
            ("GET", "/", none),
            ("GET", "/settings", none),
            ("POST", "/auth/logout", none),
            // Near misses
            ("GET", "/api/v1/servers/3/host-keys/extra", none),
            ("GET", "/api/v1/servers/../tokens", none),
            ("GET", "/api/v1/%74okens", none),
            ("PATCH", "/api/v1/servers/3", none),
        ];
        for (method, path, allowed) in cases {
            let m: Method = method.parse().unwrap();
            for preset in [FullAdmin, ServersKeys, ReadOnly] {
                assert_eq!(
                    preset.allows(&m, path),
                    allowed.contains(&preset),
                    "{preset:?} {method} {path}"
                );
            }
        }
    }

    /// Every route a preset allows is a documented route: the preset table and the OpenAPI
    /// document (which comes from the routers themselves) cannot drift apart.
    #[test]
    fn every_route_a_preset_allows_is_documented() {
        let doc = serde_json::to_value(crate::routes::openapi()).unwrap();
        let paths = doc["paths"].as_object().unwrap();
        let documented = |method: &str, path: &str| {
            paths.iter().any(|(template, item)| {
                let t: Vec<&str> = template.split('/').collect();
                let p: Vec<&str> = path.split('/').collect();
                t.len() == p.len()
                    && t.iter()
                        .zip(&p)
                        .all(|(a, b)| a == b || (a.starts_with('{') && a.ends_with('}')))
                    && item.get(method.to_ascii_lowercase()).is_some()
            })
        };
        let mut missing = Vec::new();
        for list in [
            READ.iter().map(|p| ("GET", *p)).collect::<Vec<_>>(),
            SERVERS_KEYS.to_vec(),
            FULL_ADMIN.to_vec(),
        ] {
            for (method, pattern) in list {
                if pattern.starts_with("/ws/") {
                    continue; // the WebSocket is described in docs/API.md
                }
                let concrete = pattern.replace('*', "1");
                if !documented(method, &concrete) {
                    missing.push(format!("{method} {pattern}"));
                }
            }
        }
        assert!(missing.is_empty(), "allowed but undocumented: {missing:?}");
    }

    #[test]
    fn presets_round_trip_their_names() {
        for p in Preset::ALL {
            assert_eq!(Preset::parse(p.as_str()), Some(p));
            assert_eq!(
                serde_json::to_value(p).unwrap(),
                serde_json::json!(p.as_str())
            );
        }
        assert_eq!(Preset::parse("admin"), None);
    }
}
