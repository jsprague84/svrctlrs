//! The web app manifest, with home-screen shortcuts for the signed-in user.
//!
//! The SPA ships `manifest.webmanifest`; this handler serves that file with a `shortcuts`
//! list added: "New terminal", then the servers the user opened shells on most recently. An
//! installed app (Android, desktop Chrome/Edge) shows them on a long-press of its icon, and
//! `/?connect=<id>` opens a tab on that server. The page links the manifest with
//! `crossorigin="use-credentials"`, so the session cookie comes along; without a session the
//! list has only the generic entry. The path is exempt from the auth middleware (it must load
//! on the login page too), so the user is read from the session here.

use axum::{
    extract::State,
    http::{header, HeaderValue},
    response::{IntoResponse, Response},
};
use serde_json::{json, Value};
use tower_sessions::Session;
use tracing::{debug, instrument};

use crate::routes::ui::auth::USER_ID_KEY;
use crate::state::AppState;

/// Android shows at most four shortcuts; the first is "New terminal".
const RECENT_SERVERS: i64 = 3;

#[instrument(skip(state, session))]
pub async fn manifest(State(state): State<AppState>, session: Session) -> Response {
    let spa_dir = std::env::var("SPA_DIR").unwrap_or_else(|_| "ui/build".to_string());
    let shipped = tokio::fs::read_to_string(format!("{spa_dir}/manifest.webmanifest"))
        .await
        .ok()
        .and_then(|s| serde_json::from_str::<Value>(&s).ok());

    let recent = match session.get::<i64>(USER_ID_KEY).await.unwrap_or(None) {
        Some(user_id) => {
            svrctlrs_database::queries::audit::recent_servers(&state.pool, user_id, RECENT_SERVERS)
                .await
                .unwrap_or_else(|e| {
                    debug!(error = %e, "No recent servers for the manifest");
                    Vec::new()
                })
        }
        None => Vec::new(),
    };

    let body = with_shortcuts(shipped.unwrap_or_else(fallback), &recent);
    (
        [
            (
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/manifest+json"),
            ),
            // Per user: never shared by a cache, revalidated on every fetch
            (
                header::CACHE_CONTROL,
                HeaderValue::from_static("private, no-cache"),
            ),
            (header::VARY, HeaderValue::from_static("Cookie")),
        ],
        body.to_string(),
    )
        .into_response()
}

/// The manifest with its `shortcuts` replaced: "New terminal", then one per recent server.
pub fn with_shortcuts(mut manifest: Value, recent: &[(i64, String)]) -> Value {
    let icons = json!([{ "src": "/icons/icon-192.png", "sizes": "192x192", "type": "image/png" }]);
    let mut shortcuts = vec![json!({
        "name": "New terminal",
        "short_name": "New",
        "description": "Pick a server and open a shell",
        "url": "/?open=servers",
        "icons": icons,
    })];
    for (id, name) in recent {
        shortcuts.push(json!({
            "name": name,
            "short_name": short_name(name),
            "description": format!("Open a shell on {name}"),
            "url": format!("/?connect={id}"),
            "icons": icons,
        }));
    }
    if let Value::Object(map) = &mut manifest {
        map.insert("shortcuts".to_string(), Value::Array(shortcuts));
    }
    manifest
}

/// Launchers truncate long labels; keep the start of the name.
fn short_name(name: &str) -> String {
    const MAX: usize = 12;
    if name.chars().count() <= MAX {
        name.to_string()
    } else {
        format!("{}…", name.chars().take(MAX - 1).collect::<String>())
    }
}

/// Used only when the SPA build is missing its manifest (a broken install): enough to load.
fn fallback() -> Value {
    json!({
        "id": "/",
        "name": "SvrCtlRS",
        "short_name": "SvrCtlRS",
        "start_url": "/",
        "scope": "/",
        "display": "standalone",
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcuts_list_new_terminal_then_recent_servers() {
        let shipped = json!({ "name": "SvrCtlRS", "shortcuts": [{ "name": "stale" }] });
        let m = with_shortcuts(
            shipped,
            &[
                (7, "web-01".to_string()),
                (3, "a-very-long-server-name".to_string()),
            ],
        );
        assert_eq!(m["name"], "SvrCtlRS", "the shipped fields are kept");
        let s = m["shortcuts"].as_array().unwrap();
        assert_eq!(s.len(), 3, "the shipped list is replaced, not appended to");
        assert_eq!(s[0]["url"], "/?open=servers");
        assert_eq!(s[1]["name"], "web-01");
        assert_eq!(s[1]["url"], "/?connect=7");
        assert_eq!(s[2]["short_name"], "a-very-long…");
        assert_eq!(s[2]["name"], "a-very-long-server-name");
    }

    #[test]
    fn signed_out_gets_only_the_generic_shortcut() {
        let m = with_shortcuts(fallback(), &[]);
        let s = m["shortcuts"].as_array().unwrap();
        assert_eq!(s.len(), 1);
        assert_eq!(s[0]["name"], "New terminal");
    }
}
