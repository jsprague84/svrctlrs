//! Signed-in devices: every browser and desktop-app sign-in is a row in `devices`, and the
//! auth layer accepts a session only while its row exists. Signing a device out deletes the
//! row and the session, and closes the terminals it has open (its shells stay, detached, for
//! the account's other devices). API tokens are not devices; they have their own list.
//!
//! The row names its session by the session id's SHA-256, never the id itself. The session
//! keeps the row's id under [`DEVICE_KEY`], which tells a signed-out session written back by
//! a request that was still running (refused) from a sign-in made before devices existed
//! (recorded on its next use).

use axum::http::{header, HeaderMap};
use std::collections::HashSet;
use svrctlrs_database::models::{Device, NewDevice};
use svrctlrs_database::queries::devices;
use svrctlrs_database::{SqlxPool, SqlxSqlite};

/// Session key holding the device row's id
pub(crate) const DEVICE_KEY: &str = "device_id";
/// Session key rewritten when last use is recorded, so a browser session is saved again and
/// its 24 hours of inactivity start over (the session store saves only changed sessions)
pub(crate) const SEEN_KEY: &str = "seen_at";
/// Last use is written at most this often per device
const TOUCH_EVERY_SECS: i64 = 60;
/// Longest user agent kept
const MAX_USER_AGENT: usize = 256;

/// The device a request came from, next to its `CurrentUser` (never on a token request)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceAuth(pub i64);

/// The SHA-256 (hex) of a session id
pub fn session_hash(session_id: &str) -> String {
    crate::api_tokens::hash(session_id)
}

/// The request's `User-Agent`, cut to a sane length
pub fn user_agent(headers: &HeaderMap) -> Option<String> {
    headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|ua| !ua.is_empty())
        .map(|ua| ua.chars().take(MAX_USER_AGENT).collect())
}

/// Record a sign-in for the session `session_id`
pub async fn record(
    pool: &SqlxPool<SqlxSqlite>,
    user_id: i64,
    session_id: &str,
    method: &str,
    headers: &HeaderMap,
) -> svrctlrs_core::Result<Device> {
    devices::create(
        pool,
        &NewDevice {
            user_id,
            session_hash: session_hash(session_id),
            method: method.to_string(),
            user_agent: user_agent(headers),
            ip: Some(crate::routes::ui::auth::client_ip(headers)),
        },
    )
    .await
}

/// What the auth layer learns about a signed-in session
#[derive(Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Still signed in. `adopted`: recorded just now (the session should keep [`DEVICE_KEY`]);
    /// `touched`: last use was due and is being written.
    Valid {
        device_id: i64,
        adopted: bool,
        touched: bool,
    },
    /// Its device was signed out: refuse, and drop the session
    SignedOut,
    /// The lookup failed; refuse
    Error,
}

/// Judge the session `session_id` of `user_id`. `marker` is the session's [`DEVICE_KEY`] and
/// `method` its sign-in method, for recording a sign-in older than the device list.
pub async fn judge(
    pool: &SqlxPool<SqlxSqlite>,
    session_id: &str,
    user_id: i64,
    marker: Option<i64>,
    method: Option<&str>,
    headers: &HeaderMap,
) -> Verdict {
    match devices::find_by_hash(pool, &session_hash(session_id)).await {
        Ok(Some(device)) if device.user_id == user_id => {
            let due = (chrono::Utc::now() - device.last_seen_at).num_seconds() >= TOUCH_EVERY_SECS;
            if due {
                let pool = pool.clone();
                let ip = crate::routes::ui::auth::client_ip(headers);
                let ua = user_agent(headers);
                tokio::spawn(async move {
                    if let Err(e) = devices::touch(&pool, device.id, &ip, ua.as_deref()).await {
                        tracing::warn!(error = %e, device_id = device.id, "Failed to record device use");
                    }
                });
            }
            Verdict::Valid {
                device_id: device.id,
                adopted: false,
                touched: due,
            }
        }
        Ok(Some(_)) => Verdict::SignedOut,
        Ok(None) if marker.is_some() => Verdict::SignedOut,
        Ok(None) => {
            match record(
                pool,
                user_id,
                session_id,
                method.unwrap_or("unknown"),
                headers,
            )
            .await
            {
                Ok(device) => {
                    tracing::info!(
                        user_id,
                        device_id = device.id,
                        "Recorded an earlier sign-in as a device"
                    );
                    Verdict::Valid {
                        device_id: device.id,
                        adopted: true,
                        touched: false,
                    }
                }
                Err(e) => {
                    tracing::error!(error = %e, "Could not record the sign-in");
                    Verdict::Error
                }
            }
        }
        Err(e) => {
            tracing::error!(error = %e, "Could not look the device up");
            Verdict::Error
        }
    }
}

/// Ids of the live sessions whose hash is in `hashes`
async fn sessions_with_hashes(
    pool: &SqlxPool<SqlxSqlite>,
    hashes: &HashSet<&str>,
) -> svrctlrs_core::Result<Vec<String>> {
    Ok(live_session_ids(pool)
        .await?
        .into_iter()
        .filter(|id| hashes.contains(session_hash(id).as_str()))
        .collect())
}

async fn live_session_ids(pool: &SqlxPool<SqlxSqlite>) -> svrctlrs_core::Result<Vec<String>> {
    use anyhow::Context;
    svrctlrs_database::sqlx::query_scalar(
        // Stored as text; the store itself compares it this way
        "SELECT id FROM tower_sessions WHERE datetime(expiry_date) > datetime('now')",
    )
    .fetch_all(pool)
    .await
    .context("Failed to list sessions")
    .map_err(svrctlrs_core::Error::Database)
}

/// Sign these devices out (their rows are already deleted): end their sessions and close the
/// terminals they have open.
pub async fn signed_out(state: &crate::state::AppState, gone: &[Device]) {
    let hashes: HashSet<&str> = gone.iter().map(|d| d.session_hash.as_str()).collect();
    match sessions_with_hashes(&state.pool, &hashes).await {
        Ok(ids) => {
            for id in ids {
                if let Err(e) =
                    svrctlrs_database::sqlx::query("DELETE FROM tower_sessions WHERE id = ?")
                        .bind(&id)
                        .execute(&state.pool)
                        .await
                {
                    // The row is gone, so the session is refused anyway; it expires on its own
                    tracing::warn!(error = %e, "Could not delete a signed-out session");
                }
            }
        }
        Err(e) => tracing::warn!(error = %e, "Could not list sessions to end"),
    }
    for device in gone {
        let _ = state.device_sign_outs.send(device.id);
    }
}

/// The session `session_id` is signing out (the caller ends the session): forget its device
/// and close the terminals it has open. Returns the device's id, if it had one.
pub async fn session_ending(state: &crate::state::AppState, session_id: &str) -> Option<i64> {
    let device = match devices::find_by_hash(&state.pool, &session_hash(session_id)).await {
        Ok(found) => found?,
        Err(e) => {
            tracing::warn!(error = %e, "Could not look the device up");
            return None;
        }
    };
    if let Err(e) = devices::delete(&state.pool, device.user_id, device.id).await {
        tracing::warn!(error = %e, "Could not forget the device");
    }
    let _ = state.device_sign_outs.send(device.id);
    Some(device.id)
}

/// Forget devices whose session is gone (expired, ended by `svrctl reset-password`, or not in
/// a restored backup, which carries no sessions). Returns how many.
pub async fn prune(pool: &SqlxPool<SqlxSqlite>) -> svrctlrs_core::Result<u64> {
    let live: HashSet<String> = live_session_ids(pool)
        .await?
        .iter()
        .map(|id| session_hash(id))
        .collect();
    let stale: Vec<i64> = devices::all_hashes(pool)
        .await?
        .into_iter()
        .filter(|(_, hash)| !live.contains(hash))
        .map(|(id, _)| id)
        .collect();
    if stale.is_empty() {
        return Ok(0);
    }
    devices::delete_ids(pool, &stale).await
}

/// A short name for a device from its user agent: "Firefox on Android", "Desktop app on
/// Linux". Good enough to tell one's own devices apart; not a fingerprint.
pub fn label(method: &str, user_agent: Option<&str>) -> String {
    let ua = user_agent.unwrap_or("");
    let app = method == "app" || ua.starts_with("svrctlrs-desktop");
    let os = [
        ("Android", "Android"),
        ("iPhone", "iOS"),
        ("iPad", "iPadOS"),
        ("CrOS", "ChromeOS"),
        ("Windows", "Windows"),
        ("Mac OS X", "macOS"),
        ("Macintosh", "macOS"),
        ("Linux", "Linux"),
    ]
    .iter()
    .find(|(needle, _)| ua.contains(needle))
    .map(|(_, name)| *name);
    let client = if app {
        Some("Desktop app")
    } else {
        [
            ("Edg/", "Edge"),
            ("OPR/", "Opera"),
            ("Firefox/", "Firefox"),
            ("FxiOS/", "Firefox"),
            ("CriOS/", "Chrome"),
            ("Chrome/", "Chrome"),
            ("Safari/", "Safari"),
            ("curl/", "curl"),
        ]
        .iter()
        .find(|(needle, _)| ua.contains(needle))
        .map(|(_, name)| *name)
    };
    match (client, os) {
        (Some(c), Some(o)) => format!("{c} on {o}"),
        (Some(c), None) => c.to_string(),
        (None, Some(o)) => format!("Browser on {o}"),
        (None, None) if ua.is_empty() => "Unknown device".to_string(),
        (None, None) => ua.split_whitespace().next().unwrap_or(ua).to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::label;

    #[test]
    fn labels_name_the_client_and_the_system() {
        let cases = [
            ("oidc", Some("Mozilla/5.0 (X11; Linux x86_64; rv:131.0) Gecko/20100101 Firefox/131.0"), "Firefox on Linux"),
            ("oidc", Some("Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/129.0.0.0 Mobile Safari/537.36"), "Chrome on Android"),
            ("oidc", Some("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/129.0.0.0 Safari/537.36 Edg/129.0.0.0"), "Edge on Windows"),
            ("password", Some("Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1"), "Safari on iOS"),
            ("password", Some("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Safari/605.1.15"), "Safari on macOS"),
            ("app", Some("svrctlrs-desktop/3.5.0 (Linux)"), "Desktop app on Linux"),
            ("app", Some("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/605.1.15 (KHTML, like Gecko)"), "Desktop app on Linux"),
            ("app", None, "Desktop app"),
            ("password", Some("curl/8.9.1"), "curl"),
            ("password", Some("python-requests/2.32"), "python-requests/2.32"),
            ("oidc", None, "Unknown device"),
        ];
        for (method, ua, want) in cases {
            assert_eq!(label(method, ua), want, "{ua:?}");
        }
    }
}
