//! The svrctlrs REST API from svrctl: requests with the token, the error envelope turned into
//! a [`Failure`], responses read into the `svrctlrs-api` types the server itself sends.

use reqwest::Method;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::time::Duration;
use svrctlrs_api::credentials::{CredentialList, CredentialSummary};
use svrctlrs_api::servers::{Server, ServerList};

use crate::out::Failure;

pub struct Api {
    base: String,
    token: Option<String>,
    http: reqwest::Client,
}

/// What came back: the status and the body (Null when empty).
pub struct Reply {
    pub status: u16,
    pub body: Value,
}

impl Api {
    pub fn new(url: &str, token: Option<String>) -> Self {
        Self {
            base: url.trim_end_matches('/').to_string(),
            token: token.filter(|t| !t.trim().is_empty()),
            http: reqwest::Client::new(),
        }
    }

    /// One call to `/api/v1{path}`. `wait` is how long the call itself may take (an exec
    /// gets its timeout plus room to connect).
    pub async fn send(
        &self,
        method: Method,
        path: &str,
        body: Option<&Value>,
        auth: bool,
        wait: Duration,
    ) -> Result<Reply, Failure> {
        let url = format!("{}/api/v1{path}", self.base);
        let mut req = self.http.request(method, &url).timeout(wait);
        if auth {
            let Some(token) = &self.token else {
                return Err(Failure::new(
                    "NO_TOKEN",
                    "No API token: set SVRCTLRS_TOKEN or pass --token",
                ));
            };
            req = req.bearer_auth(token);
        }
        if let Some(b) = body {
            req = req.json(b);
        }
        let response = req.send().await.map_err(|e| {
            Failure::new(
                "UNREACHABLE",
                format!("Could not reach svrctlrs at {}: {e}", self.base),
            )
        })?;
        let status = response.status().as_u16();
        let text = response.text().await.unwrap_or_default();
        let body: Value = if text.trim().is_empty() {
            Value::Null
        } else {
            serde_json::from_str(&text).unwrap_or(Value::String(text))
        };
        if status >= 400 {
            return Err(failure_from(status, &body));
        }
        Ok(Reply { status, body })
    }

    pub async fn call<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        body: Option<&Value>,
    ) -> Result<T, Failure> {
        let reply = self
            .send(method, path, body, true, Duration::from_secs(90))
            .await?;
        decode(reply.body)
    }

    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, Failure> {
        self.call(Method::GET, path, None).await
    }

    /// A server by id or by name (ignoring case).
    pub async fn server(&self, reference: &str) -> Result<Server, Failure> {
        let list: ServerList = self.get("/servers").await?;
        pick(
            list.servers,
            reference,
            "server",
            |s: &Server| s.id,
            |s: &Server| s.name.clone(),
        )
    }

    /// A credential by id or by name (ignoring case).
    pub async fn credential(&self, reference: &str) -> Result<CredentialSummary, Failure> {
        let list: CredentialList = self.get("/credentials").await?;
        pick(
            list.credentials,
            reference,
            "credential",
            |c: &CredentialSummary| c.id,
            |c: &CredentialSummary| c.name.clone(),
        )
    }
}

/// Read a body into the type the server sends.
pub fn decode<T: DeserializeOwned>(body: Value) -> Result<T, Failure> {
    serde_json::from_value(body.clone()).map_err(|e| Failure {
        details: Some(Box::new(body)),
        ..Failure::new(
            "BAD_RESPONSE",
            format!("svrctlrs answered something svrctl does not understand ({e}); are the two the same version?"),
        )
    })
}

/// The API's error envelope (`{success: false, error: {code, message, details}}`), or the
/// older `{error: "…"}` of the login route, as a failure.
fn failure_from(status: u16, body: &Value) -> Failure {
    let (code, message, details) = match body.get("error") {
        Some(Value::Object(e)) => (
            e.get("code")
                .and_then(Value::as_str)
                .unwrap_or("ERROR")
                .to_string(),
            e.get("message")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            e.get("details").cloned().map(Box::new),
        ),
        Some(Value::String(m)) => (default_code(status).to_string(), m.clone(), None),
        _ => (
            default_code(status).to_string(),
            format!("svrctlrs answered {status}"),
            None,
        ),
    };
    Failure {
        code,
        message,
        status: Some(status),
        details,
    }
}

fn default_code(status: u16) -> &'static str {
    match status {
        400 => "BAD_REQUEST",
        401 => "UNAUTHORIZED",
        403 => "FORBIDDEN",
        404 => "NOT_FOUND",
        409 => "CONFLICT",
        408 => "TIMEOUT",
        s if s >= 500 => "INTERNAL_ERROR",
        _ => "ERROR",
    }
}

fn pick<T>(
    items: Vec<T>,
    reference: &str,
    kind: &str,
    id: impl Fn(&T) -> i64,
    name: impl Fn(&T) -> String,
) -> Result<T, Failure> {
    let reference = reference.trim();
    let by_id = reference.parse::<i64>().ok();
    let mut found: Vec<T> = Vec::new();
    let mut names = Vec::new();
    for item in items {
        names.push(name(&item));
        if Some(id(&item)) == by_id {
            return Ok(item);
        }
        if name(&item).eq_ignore_ascii_case(reference) {
            found.push(item);
        }
    }
    match found.len() {
        1 => Ok(found.remove(0)),
        0 => Err(Failure {
            status: Some(404),
            details: Some(Box::new(serde_json::json!({ "known": names }))),
            ..Failure::new(
                "NOT_FOUND",
                format!("No {kind} named or numbered {reference:?}"),
            )
        }),
        _ => Err(Failure::new(
            "AMBIGUOUS",
            format!("More than one {kind} is called {reference:?}"),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_error_envelope_and_the_login_error_become_failures() {
        let f = failure_from(
            428,
            &json!({ "success": false, "error": { "code": "HOST_KEY_UNCONFIRMED", "message": "m", "details": { "fingerprint": "SHA256:x" } } }),
        );
        assert_eq!(
            (f.code.as_str(), f.status),
            ("HOST_KEY_UNCONFIRMED", Some(428))
        );
        assert_eq!(f.details.unwrap()["fingerprint"], "SHA256:x");
        let f = failure_from(401, &json!({ "error": "Authentication required" }));
        assert_eq!(
            (f.code.as_str(), f.message.as_str()),
            ("UNAUTHORIZED", "Authentication required")
        );
    }

    #[test]
    fn references_are_ids_or_names() {
        let items = vec![(1, "web"), (2, "DB"), (3, "db")];
        let p = |r: &str| pick(items.clone(), r, "server", |i| i.0, |i| i.1.to_string());
        assert_eq!(p("1").unwrap().0, 1);
        assert_eq!(p("WEB").unwrap().0, 1);
        assert_eq!(p("db").unwrap_err().code, "AMBIGUOUS");
        assert_eq!(p("3").unwrap().0, 3);
        assert_eq!(p("nope").unwrap_err().code, "NOT_FOUND");
    }
}
