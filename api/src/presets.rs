//! What each API-token preset may do, decided by method and path alone.
//!
//! **Access is decided here and only here**: the server's auth layer
//! (`server/src/routes/ui/auth.rs`, through `server/src/api_tokens.rs`) asks [`Preset::allows`]
//! before any handler runs. Anything a preset does not name is refused with 403. No preset
//! names token or account management or signed-in devices, so a token can never create, change
//! or revoke tokens or accounts, nor see or end a sign-in: revoking it ends everything it can
//! do. The table lives in this crate so that `svrctl` reads the same one: its parity test
//! (`server/src/bin/svrctl/surface.rs`) checks that the preset `docs/cli.md` names for a
//! command is the least that reaches the call.

use serde::{Deserialize, Serialize};

/// How much of the owner's access a token carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Preset {
    /// Everything the owner can do, terminals and files included, except managing tokens,
    /// accounts and signed-in devices.
    FullAdmin,
    /// Read-only, plus managing servers, host keys and credentials.
    ServersKeys,
    /// Reading the inventory and configuration: no changes, no terminals, no files, no
    /// recordings' content.
    ReadOnly,
}

/// Paths a read-only token may GET (and HEAD). `*` is one path segment.
pub const READ: &[&str] = &[
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
    "/api/v1/backups",
    // A token waits on its own approval request here
    "/api/v1/approvals/*",
];

/// What "servers & keys" adds to [`READ`]: managing servers, host keys and credentials.
pub const SERVERS_KEYS: &[(&str, &str)] = &[
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
pub const FULL_ADMIN: &[(&str, &str)] = &[
    ("POST", "/api/v1/auth/ws-ticket"),
    ("POST", "/api/v1/backups"),
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
    pub fn allows(self, method: &str, path: &str) -> bool {
        let method = if method == "HEAD" { "GET" } else { method };
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
pub fn path_matches(pattern: &str, path: &str) -> bool {
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

#[cfg(test)]
mod tests {
    use super::*;

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
            // nor anyone's signed-in devices
            ("GET", "/api/v1/users/me/devices", none),
            ("DELETE", "/api/v1/users/me/devices/1", none),
            ("POST", "/api/v1/users/me/devices/sign-out-others", none),
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
            for preset in [FullAdmin, ServersKeys, ReadOnly] {
                assert_eq!(
                    preset.allows(method, path),
                    allowed.contains(&preset),
                    "{preset:?} {method} {path}"
                );
            }
        }
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
