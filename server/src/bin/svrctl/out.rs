//! How svrctl answers: one JSON document on stdout when stdout is not a terminal (or with
//! `--json`), short text otherwise; errors as `{"error": {code, message, hint, …}}`; and exit
//! codes an agent can branch on.

use serde::Serialize;
use serde_json::{json, Value};
use std::io::IsTerminal;

/// Exit codes. `exec` exits with the remote command's own status instead (124 on a timeout).
pub mod code {
    /// Done
    pub const OK: u8 = 0;
    /// The call worked and its answer is a failure: the server is unreachable, onboarding or
    /// rotation stopped, the approval was denied
    pub const FAILED: u8 = 1;
    /// Bad arguments (clap's own code)
    pub const USAGE: u8 = 2;
    /// Waiting on a human: a host key to confirm, an approval to give
    pub const NEEDS_HUMAN: u8 = 3;
    /// No such server, credential or request
    pub const NOT_FOUND: u8 = 4;
    /// Not authenticated, or the token's access does not cover it
    pub const REFUSED: u8 = 5;
    /// The request was invalid or conflicts with the current state
    pub const INVALID: u8 = 6;
    /// svrctlrs could not be reached, or failed itself
    pub const UNAVAILABLE: u8 = 7;
    /// `exec`: the command was still running at its deadline
    pub const TIMED_OUT: u8 = 124;
}

/// Why a command did not get an answer it could print.
#[derive(Debug)]
pub struct Failure {
    /// The API's error code (`NOT_FOUND`, `TOKEN_SCOPE`, …), or svrctl's own
    /// (`UNREACHABLE`, `NO_TOKEN`, `AMBIGUOUS`, …)
    pub code: String,
    pub message: String,
    /// HTTP status, when there was one
    pub status: Option<u16>,
    /// The API's `error.details`
    pub details: Option<Box<Value>>,
}

impl Failure {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_string(),
            message: message.into(),
            status: None,
            details: None,
        }
    }

    pub fn exit_code(&self) -> u8 {
        match self.code.as_str() {
            "USAGE" => code::USAGE,
            "HOST_KEY_UNCONFIRMED" | "APPROVAL_PENDING" => code::NEEDS_HUMAN,
            "NOT_FOUND" => code::NOT_FOUND,
            "UNAUTHORIZED" | "FORBIDDEN" | "TOKEN_INVALID" | "TOKEN_EXPIRED" | "TOKEN_REVOKED"
            | "TOKEN_SCOPE" | "TOKEN_IN_URL" | "NO_TOKEN" => code::REFUSED,
            "UNREACHABLE" | "INTERNAL_ERROR" | "BAD_RESPONSE" => code::UNAVAILABLE,
            _ => match self.status {
                Some(s) if s >= 500 => code::UNAVAILABLE,
                Some(401 | 403) => code::REFUSED,
                Some(404) => code::NOT_FOUND,
                _ => code::INVALID,
            },
        }
    }

    /// What to do about it, for a reader that has no other context
    pub fn hint(&self) -> String {
        let fingerprint = self
            .details
            .as_ref()
            .and_then(|d| d.get("fingerprint"))
            .and_then(Value::as_str);
        match self.code.as_str() {
            "NO_TOKEN" | "UNAUTHORIZED" => "Set SVRCTLRS_TOKEN to an API token (svrctlrs: Settings → Access → Agent tokens) and SVRCTLRS_URL to the server".into(),
            "TOKEN_INVALID" => "The token is not one svrctlrs knows; check SVRCTLRS_TOKEN (svr_ and 40 letters and digits)".into(),
            "TOKEN_EXPIRED" | "TOKEN_REVOKED" => "Ask the owner for a new token (Settings → Access → Agent tokens)".into(),
            "TOKEN_SCOPE" | "FORBIDDEN" => "This token's access does not cover that; the owner can make a token with more access, or do it in the app".into(),
            "NOT_FOUND" => "List what exists with `svrctl servers` or `svrctl credentials`".into(),
            "HOST_KEY_UNCONFIRMED" => match fingerprint {
                Some(fp) => format!("Compare {fp} with `ssh-keygen -lf /etc/ssh/ssh_host_*_key.pub` on the host over a path you trust, then `svrctl trust <server> --fingerprint {fp}`"),
                None => "Confirm the server's host key: `svrctl host-keys <server>`, compare, then `svrctl trust`".into(),
            },
            "HOST_KEY_MISMATCH" => "The server presents a different key than the trusted one. Run `svrctl diagnose <server>`; forget the stored keys only once you know why it changed".into(),
            "CONFLICT" => "Something with that name exists, or the change conflicts with the current state; look with `svrctl servers` / `svrctl credentials`".into(),
            "APPROVAL_PENDING" => "The owner approves or denies it signed in, at the url in `approval`; run the same command with --wait to wait for the answer".into(),
            "UNREACHABLE" => "svrctlrs did not answer; check SVRCTLRS_URL and that the server runs (`svrctl health`)".into(),
            "AMBIGUOUS" => "Use the id instead of the name (`svrctl servers` lists both)".into(),
            "BAD_REQUEST" | "USAGE" => "Check the arguments; `svrctl <command> --help` has examples".into(),
            _ => "See `svrctl <command> --help`; `svrctl diagnose <server>` explains connection problems".into(),
        }
    }
}

/// Where the answer goes and in which form.
pub struct Out {
    pub json: bool,
    /// Indented JSON: only for a person who asked for `--json` on a terminal. A pipe (an
    /// agent, a script) gets one compact line.
    pretty: bool,
}

/// Leave out what costs an agent tokens and says nothing: every `null`, at any depth. A key
/// that is absent means the same as `null` in every answer.
pub fn slim(value: &mut Value) {
    match value {
        Value::Object(map) => {
            map.retain(|_, v| !v.is_null());
            map.values_mut().for_each(slim);
        }
        Value::Array(items) => items.iter_mut().for_each(slim),
        _ => {}
    }
}

impl Out {
    pub fn new(force_json: bool, force_text: bool) -> Self {
        let terminal = std::io::stdout().is_terminal();
        Self {
            json: force_json || (!force_text && !terminal),
            pretty: terminal,
        }
    }

    fn print_json(&self, mut value: Value) {
        slim(&mut value);
        let text = if self.pretty {
            serde_json::to_string_pretty(&value)
        } else {
            serde_json::to_string(&value)
        };
        println!(
            "{}",
            text.unwrap_or_else(|e| json!({ "error": e.to_string() }).to_string())
        );
    }

    /// Print a result: the JSON of `value` (without nulls), or the text `human` makes of it.
    pub fn emit<T: Serialize>(&self, value: &T, human: impl FnOnce(&T) -> String) {
        if self.json {
            match serde_json::to_value(value) {
                Ok(v) => self.print_json(v),
                Err(e) => println!("{}", json!({ "error": e.to_string() })),
            }
        } else {
            let text = human(value);
            if !text.is_empty() {
                println!("{text}");
            }
        }
    }

    /// Print a failure and give its exit code.
    pub fn fail(&self, f: &Failure) -> u8 {
        if self.json {
            let mut error = json!({ "code": f.code, "message": f.message, "hint": f.hint() });
            if let Some(s) = f.status {
                error["status"] = json!(s);
            }
            if let Some(d) = &f.details {
                error["details"] = (**d).clone();
            }
            self.print_json(json!({ "error": error }));
        } else {
            eprintln!("error: {}", f.message);
            eprintln!("hint: {}", f.hint());
        }
        f.exit_code()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slim_drops_nulls_at_any_depth_and_keeps_false_and_empty() {
        let mut v = json!({
            "a": null, "b": false, "c": [], "d": [{ "e": null, "f": 0 }], "g": { "h": null }
        });
        slim(&mut v);
        assert_eq!(
            v,
            json!({ "b": false, "c": [], "d": [{ "f": 0 }], "g": {} })
        );
    }

    #[test]
    fn exit_codes_follow_the_error_kind() {
        let f = |c: &str, s: Option<u16>| Failure {
            status: s,
            ..Failure::new(c, "x")
        };
        assert_eq!(
            f("HOST_KEY_UNCONFIRMED", Some(428)).exit_code(),
            code::NEEDS_HUMAN
        );
        assert_eq!(f("TOKEN_SCOPE", Some(403)).exit_code(), code::REFUSED);
        assert_eq!(f("NOT_FOUND", Some(404)).exit_code(), code::NOT_FOUND);
        assert_eq!(f("CONFLICT", Some(409)).exit_code(), code::INVALID);
        assert_eq!(f("HOST_KEY_MISMATCH", Some(409)).exit_code(), code::INVALID);
        assert_eq!(f("UNREACHABLE", None).exit_code(), code::UNAVAILABLE);
        assert_eq!(f("SOMETHING", Some(502)).exit_code(), code::UNAVAILABLE);
    }

    #[test]
    fn hints_name_the_next_command() {
        let f = Failure {
            details: Some(Box::new(json!({ "fingerprint": "SHA256:abc" }))),
            ..Failure::new("HOST_KEY_UNCONFIRMED", "x")
        };
        assert!(f
            .hint()
            .contains("svrctl trust <server> --fingerprint SHA256:abc"));
    }
}
