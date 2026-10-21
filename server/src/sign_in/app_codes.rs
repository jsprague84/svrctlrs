//! One-time codes that hand a browser sign-in to the desktop app (RFC 8252 shape: the app
//! listens on a `127.0.0.1` port and proves with a PKCE verifier that it started the sign-in).
//!
//! The browser, signed in, approves the app on `/auth/app`; svrctlrs mints a code bound to the
//! user and the app's code challenge and sends the browser to the app's loopback port with it.
//! The app trades code and verifier for a session token at `POST /api/v1/auth/app-token`. A
//! code lives a minute and works once; without the verifier it is worthless, so another
//! program on the machine that sees the redirect gains nothing.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use rand::RngCore;
use sha2::{Digest, Sha256};

/// How long the app has to redeem a code
pub const CODE_TTL: Duration = Duration::from_secs(60);
/// Codes waiting at once; more are refused rather than grow without bound
const MAX_CODES: usize = 64;

struct Pending {
    user_id: i64,
    challenge: String,
    issued: Instant,
}

#[derive(Default)]
pub struct AppCodes {
    codes: Mutex<HashMap<String, Pending>>,
}

/// A PKCE S256 challenge as the app sends it: 43 base64url characters (SHA-256).
pub fn valid_challenge(challenge: &str) -> bool {
    challenge.len() == 43
        && challenge
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

impl AppCodes {
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, Pending>> {
        self.codes.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// A code for `user_id` and the app's `challenge`; None when too many are waiting.
    pub fn issue(&self, user_id: i64, challenge: &str) -> Option<String> {
        let mut bytes = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut bytes);
        let code = URL_SAFE_NO_PAD.encode(bytes);
        let mut codes = self.lock();
        codes.retain(|_, p| p.issued.elapsed() < CODE_TTL);
        if codes.len() >= MAX_CODES {
            return None;
        }
        codes.insert(
            code.clone(),
            Pending {
                user_id,
                challenge: challenge.to_string(),
                issued: Instant::now(),
            },
        );
        Some(code)
    }

    /// The user the code was issued for, if the verifier matches its challenge and it is still
    /// fresh. The code is used up either way.
    pub fn redeem(&self, code: &str, verifier: &str) -> Option<i64> {
        let pending = self.lock().remove(code)?;
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        (pending.issued.elapsed() < CODE_TTL
            && constant_time_eq(challenge.as_bytes(), pending.challenge.as_bytes()))
        .then_some(pending.user_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn challenge_of(verifier: &str) -> String {
        URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
    }

    #[test]
    fn a_code_redeems_once_and_only_with_its_verifier() {
        let codes = AppCodes::default();
        let verifier = "a-verifier-the-app-keeps-to-itself-0123456789";
        let challenge = challenge_of(verifier);
        assert!(valid_challenge(&challenge));

        let code = codes.issue(7, &challenge).unwrap();
        assert_eq!(codes.redeem(&code, verifier), Some(7));
        assert_eq!(codes.redeem(&code, verifier), None);

        // A wrong verifier uses the code up too
        let code = codes.issue(7, &challenge).unwrap();
        assert_eq!(codes.redeem(&code, "someone else"), None);
        assert_eq!(codes.redeem(&code, verifier), None);
        assert_eq!(codes.redeem("never-issued", verifier), None);
    }

    #[test]
    fn challenges_are_s256_shaped() {
        assert!(!valid_challenge("short"));
        assert!(!valid_challenge(&"a".repeat(44)));
        assert!(!valid_challenge(&format!("{}+", "a".repeat(42))));
    }

    #[test]
    fn waiting_codes_are_bounded() {
        let codes = AppCodes::default();
        let challenge = challenge_of("v");
        for _ in 0..MAX_CODES {
            assert!(codes.issue(1, &challenge).is_some());
        }
        assert!(codes.issue(1, &challenge).is_none());
    }
}
