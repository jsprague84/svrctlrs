//! API tokens: their format and how a presented token is checked. What each access preset
//! may do is `svrctlrs_api::presets` (re-exported here), shared with `svrctl`'s parity test.
//!
//! A token is `svr_` followed by 40 random alphanumerics (about 238 bits), so it can be told
//! apart from a session id by its prefix and length, selected with a double-click, and found
//! by a secret scanner. Only its SHA-256 is stored (the secret is random, so a slow password
//! hash would add nothing but latency on every request). The owner sees the secret once.
//!
//! **Access is decided by the preset table only**, by method and path, before any handler
//! runs ([`Preset::allows`], called from `routes::ui::auth::require_auth`). Anything a preset
//! does not name is refused with 403. No preset names token or account management or
//! signed-in devices, so a token can never create, change or revoke tokens or accounts, nor
//! see or end a sign-in: revoking it ends everything it can do.

use rand::{distributions::Alphanumeric, Rng};
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

pub use svrctlrs_api::presets::{path_matches, Preset};

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
        use svrctlrs_api::presets::{FULL_ADMIN, READ, SERVERS_KEYS};
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
}
