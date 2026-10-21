//! Signing the desktop app in through the system browser, and keeping its session token in the
//! OS keyring (ROADMAP 22; the server half is `server/src/routes/ui/app_sign_in.rs`).
//!
//! RFC 8252 shape with svrctlrs as the authority: the app listens on a random `127.0.0.1` port,
//! opens `<server>/auth/app` with a PKCE challenge and a `state`, and the browser — signed in
//! however the server allows — sends a one-time code back to that port once the user approves.
//! The app checks `state` and trades code and verifier for a session token. No password, and no
//! identity provider, ever passes through the app.

use std::time::Duration;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use rand::RngCore;
use sha2::{Digest, Sha256};
use tauri_plugin_http::reqwest;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// How long the browser has to come back
const SIGN_IN_TIMEOUT: Duration = Duration::from_secs(5 * 60);
/// The keyring service; the account is the server's address, so each server has its own token
const KEYRING_SERVICE: &str = "cc.jsnode.svrctlrs";

fn random() -> String {
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

fn entry(server_url: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(KEYRING_SERVICE, server_url)
        .map_err(|e| format!("the system keyring cannot be used: {e}"))
}

/// The token stored for `server_url`, if any. A keyring that cannot be reached reads as none.
#[tauri::command]
pub fn stored_token(server_url: String) -> Option<String> {
    entry(&server_url).ok()?.get_password().ok()
}

/// Keep `token` for `server_url` (also used once to move an old token out of localStorage).
#[tauri::command]
pub fn store_token(server_url: String, token: String) -> Result<(), String> {
    entry(&server_url)?
        .set_password(&token)
        .map_err(|e| format!("the system keyring refused the token: {e}"))
}

/// Forget the token for `server_url` (sign-out, or a server change).
#[tauri::command]
pub fn forget_token(server_url: String) {
    if let Ok(e) = entry(&server_url) {
        let _ = e.delete_credential();
    }
}

/// The query of the first `GET /callback?…` request on `listener`; anything else (a favicon
/// request) gets a 404 and the wait goes on.
async fn wait_for_callback(listener: TcpListener) -> Result<(String, String), String> {
    loop {
        let (mut stream, _) = listener
            .accept()
            .await
            .map_err(|e| format!("the sign-in listener failed: {e}"))?;
        let mut buf = vec![0u8; 8192];
        let n = stream.read(&mut buf).await.unwrap_or(0);
        let request = String::from_utf8_lossy(&buf[..n]).to_string();
        let target = request
            .lines()
            .next()
            .and_then(|line| line.strip_prefix("GET "))
            .and_then(|rest| rest.split(' ').next())
            .unwrap_or("");
        let Some(query) = target.strip_prefix("/callback?") else {
            let _ = stream
                .write_all(
                    b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .await;
            continue;
        };
        let mut code = None;
        let mut state = None;
        for (k, v) in url::form_urlencoded::parse(query.as_bytes()) {
            match k.as_ref() {
                "code" => code = Some(v.into_owned()),
                "state" => state = Some(v.into_owned()),
                _ => {}
            }
        }
        let page = "<!DOCTYPE html><html><head><meta charset=\"utf-8\"><title>SvrCtlRS</title>\
            <style>body{font-family:system-ui,sans-serif;background:#1a1b26;color:#c0caf5;\
            display:flex;align-items:center;justify-content:center;min-height:100vh;margin:0}\
            </style></head><body><p>Signed in. You can close this tab and go back to the \
            SvrCtlRS app.</p></body></html>";
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\n\
             Cache-Control: no-store\r\nConnection: close\r\n\r\n{page}",
            page.len()
        );
        let _ = stream.write_all(response.as_bytes()).await;
        let _ = stream.shutdown().await;
        return match (code, state) {
            (Some(c), Some(s)) => Ok((c, s)),
            _ => Err("the browser came back without a sign-in code".into()),
        };
    }
}

/// Sign in to `server_url` through the system browser; the session token, which is also kept
/// in the keyring.
#[tauri::command]
pub async fn sign_in_with_browser(
    app: tauri::AppHandle,
    server_url: String,
) -> Result<String, String> {
    use tauri_plugin_opener::OpenerExt;

    let server_url = server_url.trim_end_matches('/').to_string();
    let verifier = random();
    let state = random();
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|e| format!("cannot listen for the browser: {e}"))?;
    let port = listener
        .local_addr()
        .map_err(|e| format!("cannot listen for the browser: {e}"))?
        .port();

    let url = format!(
        "{server_url}/auth/app?port={port}&state={state}&code_challenge={challenge}&code_challenge_method=S256"
    );
    app.opener()
        .open_url(&url, None::<&str>)
        .map_err(|e| format!("cannot open the browser: {e}"))?;

    let (code, returned_state) = tokio::time::timeout(SIGN_IN_TIMEOUT, wait_for_callback(listener))
        .await
        .map_err(|_| "the browser did not come back within five minutes".to_string())??;
    if returned_state != state {
        return Err("the browser came back from a different sign-in; try again".into());
    }

    let response = reqwest::Client::new()
        .post(format!("{server_url}/api/v1/auth/app-token"))
        .header("content-type", "application/json")
        .body(serde_json::json!({ "code": code, "code_verifier": verifier }).to_string())
        .timeout(Duration::from_secs(30))
        .send()
        .await
        .map_err(|e| format!("cannot reach {server_url}: {e}"))?;
    let status = response.status();
    let body: serde_json::Value = response
        .text()
        .await
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    if !status.is_success() {
        return Err(body["error"]
            .as_str()
            .unwrap_or("the server refused the sign-in")
            .to_string());
    }
    let token = body["session_id"]
        .as_str()
        .filter(|t| !t.is_empty())
        .ok_or("the server answered without a session")?
        .to_string();
    // A keyring that cannot be used leaves the app signed in until it quits
    if let Err(e) = store_token(server_url, token.clone()) {
        log::warn!("{e}");
    }
    Ok(token)
}
