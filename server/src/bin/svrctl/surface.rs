//! The agent surface (docs/agents.md, "The parity check"): every operation of the API and how
//! an agent reaches it. An `svrctl` command with the least token preset that makes the call,
//! or only `svrctl api` (rare), or no agent use, each with its reason. `svrctl openapi --find`
//! prints the way next to each operation it finds.
//!
//! The tests make the table impossible to forget: an operation without a row fails, a row
//! without an operation fails, and so does a command that clap does not know, that
//! `docs/cli.md` does not list or the skill does not mention, and a preset that cannot make
//! the call or is more than it needs. Copied from rustRoast's `bin/roast/surface.rs`.

use serde_json::{json, Value};
use svrctlrs_api::presets::Preset;

/// How an agent reaches one operation.
#[derive(Debug, Clone, Copy)]
pub enum Via {
    /// `svrctl <command> [--flag]`: the subcommand path and the flags that make this call,
    /// and the least preset a token needs for it
    Cmd(&'static str, Preset),
    /// Only with `svrctl api METHOD PATH`; why there is no command
    Api(&'static str),
    /// Not for agents; why
    None(&'static str),
}

const R: Preset = Preset::ReadOnly;
const SK: Preset = Preset::ServersKeys;
const FA: Preset = Preset::FullAdmin;

const OWNER: &str = "the owner's, signed in to the web app: sign-in, accounts, devices and tokens are in no preset (docs/agents.md, \"Tokens\")";
const TERMINAL: &str =
    "the web terminal's own look and snippets, set by the person at the terminal";
const FILES: &str = "the web app's file browser; an agent reads and changes files over its own SSH (svrctlrs is not a second door), or checks one with `exec`";
const RECORDINGS: &str = "recordings are watched in the web app's player (/recordings)";
const SHELLS: &str =
    "the web app's detached shells (the Sessions panel), which a person resumes or ends";

/// Operations that need no token at all (the auth layer lets them through)
const PUBLIC: &[(&str, &str)] = &[
    ("GET", "/api/v1/health"),
    ("POST", "/api/v1/auth/login"),
    ("POST", "/api/v1/auth/app-token"),
];

/// Every operation of the OpenAPI document, by method and path template.
pub const SURFACE: &[(&str, &str, Via)] = &[
    // system
    ("GET", "/api/v1/health", Via::Cmd("health", R)),
    ("GET", "/api/v1/status", Via::Cmd("status", R)),
    ("GET", "/api/v1/openapi.json", Via::Cmd("openapi", R)),
    ("GET", "/api/v1/audit", Via::Cmd("audit", R)),
    ("GET", "/api/v1/backups", Via::Cmd("backups", R)),
    ("POST", "/api/v1/backups", Via::Cmd("backups --now", FA)),
    // servers
    ("GET", "/api/v1/servers", Via::Cmd("servers", R)),
    ("POST", "/api/v1/servers", Via::Cmd("server add", SK)),
    ("POST", "/api/v1/servers/import", Via::Cmd("import", SK)),
    ("POST", "/api/v1/servers/onboard", Via::Cmd("onboard", SK)),
    ("GET", "/api/v1/servers/{id}", Via::Cmd("server show", R)),
    ("PUT", "/api/v1/servers/{id}", Via::Cmd("server edit", SK)),
    ("DELETE", "/api/v1/servers/{id}", Via::Cmd("server rm", SK)),
    ("POST", "/api/v1/servers/{id}/test", Via::Cmd("server test", SK)),
    ("POST", "/api/v1/servers/{id}/diagnose", Via::Cmd("diagnose", SK)),
    ("POST", "/api/v1/servers/{id}/rotate-key", Via::Cmd("rotate-key", SK)),
    ("POST", "/api/v1/servers/{id}/authorize-key", Via::Cmd("key authorize", SK)),
    ("POST", "/api/v1/servers/{id}/exec", Via::Cmd("exec", FA)),
    ("GET", "/api/v1/servers/{id}/host-keys", Via::Cmd("host-keys", R)),
    ("DELETE", "/api/v1/servers/{id}/host-keys", Via::Cmd("forget-keys", SK)),
    ("POST", "/api/v1/servers/{id}/host-keys/trust", Via::Cmd("trust", SK)),
    ("GET", "/api/v1/servers/{id}/files", Via::Api(FILES)),
    ("DELETE", "/api/v1/servers/{id}/files", Via::Api(FILES)),
    ("GET", "/api/v1/servers/{id}/files/content", Via::Api(FILES)),
    ("PUT", "/api/v1/servers/{id}/files/content", Via::Api(FILES)),
    ("POST", "/api/v1/servers/{id}/files/mkdir", Via::Api(FILES)),
    ("POST", "/api/v1/servers/{id}/files/rename", Via::Api(FILES)),
    // credentials
    ("GET", "/api/v1/credentials", Via::Cmd("credentials", R)),
    ("POST", "/api/v1/credentials", Via::None("a password or private key is entered by the owner in the web app, never handed to an agent; `key generate` makes a key on svrctlrs")),
    ("POST", "/api/v1/credentials/generate-key", Via::Cmd("key generate", SK)),
    ("GET", "/api/v1/credentials/{id}", Via::Cmd("credential show", R)),
    ("PUT", "/api/v1/credentials/{id}", Via::Cmd("credential edit", SK)),
    ("DELETE", "/api/v1/credentials/{id}", Via::Cmd("credential rm", SK)),
    // approvals
    ("GET", "/api/v1/approvals", Via::None("the owner's list of requests to decide (/approvals); an agent reads its own with `approval <id>`")),
    ("GET", "/api/v1/approvals/{id}", Via::Cmd("approval", R)),
    ("POST", "/api/v1/approvals/{id}/approve", Via::None("the owner's alone: an agent never approves its own request")),
    ("POST", "/api/v1/approvals/{id}/deny", Via::None(OWNER)),
    // settings and notifications
    ("GET", "/api/v1/settings", Via::Cmd("settings", R)),
    ("GET", "/api/v1/settings/{key}", Via::Cmd("settings", R)),
    ("PUT", "/api/v1/settings/{key}", Via::Cmd("settings set", FA)),
    ("PUT", "/api/v1/settings", Via::Api("several at once, for the settings form; `settings set` writes one")),
    ("POST", "/api/v1/settings", Via::Api("adds a key svrctlrs does not read; every key it reads exists already (`settings` lists them)")),
    ("POST", "/api/v1/notifications/test", Via::Api("a test message through the notification channel, rare: after `settings set notification.…`")),
    ("POST", "/api/v1/notifications/test-approval", Via::Api("a test link through the approval channel, rare: after `settings set approval.…`")),
    // the web terminal's own
    ("GET", "/api/v1/profiles", Via::Api(TERMINAL)),
    ("POST", "/api/v1/profiles", Via::Api(TERMINAL)),
    ("GET", "/api/v1/profiles/{id}", Via::Api(TERMINAL)),
    ("PUT", "/api/v1/profiles/{id}", Via::Api(TERMINAL)),
    ("DELETE", "/api/v1/profiles/{id}", Via::Api(TERMINAL)),
    ("GET", "/api/v1/quick-commands", Via::Api(TERMINAL)),
    ("POST", "/api/v1/quick-commands", Via::Api(TERMINAL)),
    ("GET", "/api/v1/quick-commands/{id}", Via::Api(TERMINAL)),
    ("PUT", "/api/v1/quick-commands/{id}", Via::Api(TERMINAL)),
    ("DELETE", "/api/v1/quick-commands/{id}", Via::Api(TERMINAL)),
    ("GET", "/api/v1/users/me/preferences/{key}", Via::Api(TERMINAL)),
    ("PUT", "/api/v1/users/me/preferences/{key}", Via::Api(TERMINAL)),
    ("POST", "/api/v1/auth/ws-ticket", Via::Api("a one-time ticket for a terminal's WebSocket (docs/API.md); an agent runs one command with `exec`, or uses its own SSH")),
    ("GET", "/api/v1/sessions", Via::Api(SHELLS)),
    ("DELETE", "/api/v1/sessions/{id}", Via::Api(SHELLS)),
    ("PUT", "/api/v1/sessions/keep", Via::None("the account's keep time for detached shells, set in the Sessions panel")),
    ("GET", "/api/v1/recordings", Via::Api(RECORDINGS)),
    ("GET", "/api/v1/recordings/{id}", Via::Api(RECORDINGS)),
    ("GET", "/api/v1/recordings/{id}/cast", Via::Api(RECORDINGS)),
    ("DELETE", "/api/v1/recordings/{id}", Via::Api(RECORDINGS)),
    // sign-in, accounts, devices, tokens: the owner's
    ("POST", "/api/v1/auth/login", Via::None("a person's sign-in; a login carrying a token is refused")),
    ("POST", "/api/v1/auth/app-token", Via::None("the desktop app's sign-in through the browser")),
    ("GET", "/api/v1/users", Via::Api("the accounts, listed for the owner; managing them is the owner's")),
    ("POST", "/api/v1/users", Via::None(OWNER)),
    ("DELETE", "/api/v1/users/{id}", Via::None(OWNER)),
    ("PUT", "/api/v1/users/me/password", Via::None(OWNER)),
    ("GET", "/api/v1/users/me/sign-in", Via::None(OWNER)),
    ("DELETE", "/api/v1/users/me/identities/{id}", Via::None(OWNER)),
    ("POST", "/api/v1/users/me/totp", Via::None(OWNER)),
    ("POST", "/api/v1/users/me/totp/confirm", Via::None(OWNER)),
    ("POST", "/api/v1/users/me/totp/disable", Via::None(OWNER)),
    ("POST", "/api/v1/users/me/totp/recovery-codes", Via::None(OWNER)),
    ("GET", "/api/v1/users/me/devices", Via::None(OWNER)),
    ("DELETE", "/api/v1/users/me/devices/{id}", Via::None(OWNER)),
    ("POST", "/api/v1/users/me/devices/sign-out-others", Via::None(OWNER)),
    ("GET", "/api/v1/tokens", Via::None(OWNER)),
    ("POST", "/api/v1/tokens", Via::None(OWNER)),
    ("PUT", "/api/v1/tokens/{id}", Via::None(OWNER)),
    ("DELETE", "/api/v1/tokens/{id}", Via::None(OWNER)),
    ("POST", "/api/v1/tokens/{id}/revoke", Via::None(OWNER)),
];

/// The command an agent uses for an operation, or what to know instead, for `openapi --find`.
pub fn how(method: &str, path: &str) -> Option<String> {
    SURFACE
        .iter()
        .find(|(m, p, _)| m.eq_ignore_ascii_case(method) && *p == path)
        .map(|(m, p, via)| match via {
            Via::Cmd(cmd, _) if PUBLIC.contains(&(*m, *p)) => format!("svrctl {cmd} (no token)"),
            Via::Cmd(cmd, preset) => format!("svrctl {cmd} ({} token)", preset.as_str()),
            Via::Api(why) => format!("svrctl api: {why}"),
            Via::None(why) => format!("not for agents: {why}"),
        })
}

/// The operations of an OpenAPI document whose path, operation id or summary contain `find`
/// (any case): method, path, id, summary, the request body's type, and how an agent makes it.
pub fn find_operations(doc: &Value, find: &str) -> Vec<Value> {
    let needle = find.to_lowercase();
    let mut out = Vec::new();
    for (path, item) in doc["paths"].as_object().into_iter().flatten() {
        for method in ["get", "post", "put", "patch", "delete"] {
            let Some(op) = item.get(method) else { continue };
            let id = op["operationId"].as_str().unwrap_or("");
            let summary = op["summary"].as_str().unwrap_or("");
            let hay = format!("{path} {id} {summary}").to_lowercase();
            if !hay.contains(&needle) {
                continue;
            }
            let body = op["requestBody"]["content"]["application/json"]["schema"]["$ref"]
                .as_str()
                .and_then(|r| r.rsplit('/').next());
            out.push(json!({
                "method": method.to_uppercase(),
                "path": path,
                "id": id,
                "summary": summary,
                "body": body,
                "svrctl": how(method, path),
            }));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;
    use std::collections::BTreeSet;

    // docs/ is not in the image's build context: these are read by the tests only.
    // `docs/openapi.json` equals the router's own document (the server's drift test).
    const OPENAPI: &str = include_str!("../../../../docs/openapi.json");
    const CLI_MD: &str = include_str!("../../../../docs/cli.md");
    const SKILL: &str = include_str!("../../../../skills/svrctlrs/SKILL.md");

    /// Commands that run next to the database, on the host, which an agent with a token never
    /// does: the skill leaves them out on purpose
    const HOST_ONLY: &[&str] = &["backup", "restore", "reset-password"];

    fn doc() -> Value {
        serde_json::from_str(OPENAPI).unwrap()
    }

    fn operations() -> BTreeSet<(String, String)> {
        let doc = doc();
        let mut ops = BTreeSet::new();
        for (path, item) in doc["paths"].as_object().unwrap() {
            for m in ["get", "post", "put", "patch", "delete"] {
                if item.get(m).is_some() {
                    ops.insert((m.to_uppercase(), path.clone()));
                }
            }
        }
        ops
    }

    #[test]
    fn every_operation_has_one_row_and_every_row_an_operation() {
        let ops = operations();
        assert!(ops.len() > 80, "only {} operations", ops.len());
        let mut seen = BTreeSet::new();
        let mut doubled = Vec::new();
        for (m, p, _) in SURFACE {
            if !seen.insert((m.to_string(), p.to_string())) {
                doubled.push(format!("{m} {p}"));
            }
        }
        assert!(doubled.is_empty(), "rows listed twice: {doubled:?}");
        let missing: Vec<_> = ops.difference(&seen).collect();
        assert!(
            missing.is_empty(),
            "operations with no row in server/src/bin/svrctl/surface.rs (a command, `api` or `none`, with a reason): {missing:?}"
        );
        let stale: Vec<_> = seen.difference(&ops).collect();
        assert!(
            stale.is_empty(),
            "rows for operations that are gone: {stale:?}"
        );
        for (m, p, via) in SURFACE {
            if let Via::Api(why) | Via::None(why) = via {
                assert!(why.len() > 10, "{m} {p}: say why");
            }
        }
    }

    /// `credential show` → the clap command, or None.
    fn find<'a>(root: &'a clap::Command, words: &[&str]) -> Option<&'a clap::Command> {
        words.iter().try_fold(root, |cmd, w| cmd.find_subcommand(w))
    }

    fn split(cmd: &str) -> (Vec<&str>, Vec<&str>) {
        cmd.split_whitespace().partition(|w| !w.starts_with("--"))
    }

    #[test]
    fn every_command_named_exists_with_its_flags() {
        let root = crate::Cli::command();
        let mut bad = Vec::new();
        for (m, p, via) in SURFACE {
            let Via::Cmd(cmd, _) = via else { continue };
            let (words, flags) = split(cmd);
            let Some(found) = find(&root, &words) else {
                bad.push(format!("{m} {p}: no command `svrctl {cmd}`"));
                continue;
            };
            for flag in flags {
                let long = flag.trim_start_matches("--");
                if !found.get_arguments().any(|a| a.get_long() == Some(long)) {
                    bad.push(format!(
                        "{m} {p}: `svrctl {}` has no {flag}",
                        words.join(" ")
                    ));
                }
            }
        }
        assert!(bad.is_empty(), "{bad:#?}");
    }

    /// Every command a person or agent can run: `credential show`, `settings` (which also
    /// lists), but not `server` alone, which only groups.
    fn runnable(cmd: &clap::Command, path: &str, out: &mut Vec<String>) {
        for sub in cmd.get_subcommands() {
            if sub.get_name() == "help" {
                continue;
            }
            let here = if path.is_empty() {
                sub.get_name().to_string()
            } else {
                format!("{path} {}", sub.get_name())
            };
            if !sub.is_subcommand_required_set() {
                out.push(here.clone());
            }
            runnable(sub, &here, out);
        }
    }

    /// The row of docs/cli.md's command table that documents `cmd`.
    fn cli_md_row(cmd: &str) -> Option<&'static str> {
        CLI_MD.lines().find(|l| {
            l.strip_prefix("| `")
                .and_then(|rest| rest.strip_prefix(cmd))
                .is_some_and(|after| after.starts_with(' ') || after.starts_with('`'))
        })
    }

    /// The row's last column, "Needs".
    fn needs(row: &str) -> &str {
        row.trim_end()
            .trim_end_matches('|')
            .rsplit('|')
            .next()
            .unwrap_or("")
    }

    fn in_skill(cmd: &str) -> bool {
        let needle = format!("svrctl {cmd}");
        SKILL.match_indices(&needle).any(|(i, _)| {
            SKILL[i + needle.len()..]
                .chars()
                .next()
                .is_none_or(|c| !c.is_alphanumeric() && c != '-' && c != '_')
        })
    }

    #[test]
    fn every_command_is_in_the_docs_and_the_skill() {
        let mut all = Vec::new();
        runnable(&crate::Cli::command(), "", &mut all);
        assert!(all.len() > 25, "only {} commands", all.len());
        let mut bad = Vec::new();
        for cmd in &all {
            if cli_md_row(cmd).is_none() {
                bad.push(format!("`{cmd}` has no row in docs/cli.md's command table"));
            }
            if !HOST_ONLY.contains(&cmd.as_str()) && !in_skill(cmd) {
                bad.push(format!("`svrctl {cmd}` is not in skills/svrctlrs/SKILL.md"));
            }
        }
        // and the flags the table names are in the command's row
        for (_, _, via) in SURFACE {
            let Via::Cmd(cmd, _) = via else { continue };
            let (words, flags) = split(cmd);
            let words = words.join(" ");
            if let Some(row) = cli_md_row(&words) {
                for f in flags {
                    if !row.contains(f) {
                        bad.push(format!("docs/cli.md's row for `{words}` does not show {f}"));
                    }
                }
            }
        }
        assert!(bad.is_empty(), "{bad:#?}");
    }

    fn concrete(template: &str) -> String {
        template
            .split('/')
            .map(|s| if s.starts_with('{') { "x1" } else { s })
            .collect::<Vec<_>>()
            .join("/")
    }

    /// The presets from least to most: each covers everything the ones before it do.
    const ORDER: [Preset; 3] = [Preset::ReadOnly, Preset::ServersKeys, Preset::FullAdmin];

    /// The preset named reaches the call, nothing less would, and docs/cli.md says it. A call
    /// no preset reaches is `none`: an agent cannot make it, with a command or `api`.
    #[test]
    fn every_preset_named_can_reach_its_call_and_is_the_least() {
        let mut bad = Vec::new();
        for (m, p, via) in SURFACE {
            let path = concrete(p);
            let public = PUBLIC.contains(&(*m, *p));
            let reachable = public || Preset::FullAdmin.allows(m, &path);
            match via {
                Via::Cmd(cmd, preset) => {
                    let (words, _) = split(cmd);
                    let words = words.join(" ");
                    let want = if public {
                        if *preset != Preset::ReadOnly {
                            bad.push(format!("{m} {p} needs no token: name read_only"));
                        }
                        "no token"
                    } else {
                        if !preset.allows(m, &path) {
                            bad.push(format!(
                                "{m} {p}: a {} token cannot make this call",
                                preset.as_str()
                            ));
                        }
                        let mut below = ORDER.iter().take_while(|l| *l != preset);
                        if let Some(l) = below.find(|l| l.allows(m, &path)) {
                            bad.push(format!(
                                "{m} {p}: {} is enough, not {}",
                                l.as_str(),
                                preset.as_str()
                            ));
                        }
                        preset.as_str()
                    };
                    if !cli_md_row(&words).is_some_and(|row| needs(row).contains(want)) {
                        bad.push(format!(
                            "docs/cli.md's row for `{words}` does not say {want} in Needs"
                        ));
                    }
                }
                Via::Api(_) if !reachable => {
                    bad.push(format!(
                        "{m} {p}: no preset reaches it, so `none`, not `api`"
                    ));
                }
                _ => {}
            }
        }
        assert!(bad.is_empty(), "{bad:#?}");
    }

    #[test]
    fn find_shows_the_way() {
        assert_eq!(
            how("post", "/api/v1/servers").unwrap(),
            "svrctl server add (servers_keys token)"
        );
        assert_eq!(
            how("GET", "/api/v1/health").unwrap(),
            "svrctl health (no token)"
        );
        assert!(how("GET", "/api/v1/tokens")
            .unwrap()
            .starts_with("not for agents"));
        assert!(how("GET", "/nothing").is_none());
        let found = find_operations(&doc(), "rotate");
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0]["svrctl"], "svrctl rotate-key (servers_keys token)");
        assert_eq!(found[0]["method"], "POST");
    }
}
