//! TOTP (RFC 6238: HMAC-SHA1, 6 digits, 30-second steps) and one-time recovery codes for
//! password sign-in. Pure functions; the enrolment and the last accepted step live in the
//! database (`queries::user_totp`).

use data_encoding::BASE32_NOPAD;
use hmac::{Hmac, KeyInit, Mac};
use rand::RngCore;
use sha1::Sha1;
use sha2::{Digest, Sha256};

pub const PERIOD_SECS: i64 = 30;
const DIGITS: u32 = 6;
/// Steps either side of now that still count, for a phone clock that is a little off.
const SKEW_STEPS: i64 = 1;
const RECOVERY_CODES: usize = 10;
const ISSUER: &str = "SvrCtlRS";

/// A new random secret: 20 bytes (the RFC's HMAC-SHA1 key size), base32 without padding.
pub fn new_secret() -> String {
    let mut bytes = [0u8; 20];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    BASE32_NOPAD.encode(&bytes)
}

fn secret_bytes(secret: &str) -> Option<Vec<u8>> {
    let cleaned: String = secret
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '=')
        .map(|c| c.to_ascii_uppercase())
        .collect();
    BASE32_NOPAD.decode(cleaned.as_bytes()).ok()
}

/// The time step a Unix time falls in.
pub fn step_at(unix_secs: i64) -> i64 {
    unix_secs.div_euclid(PERIOD_SECS)
}

/// The code for `step` (RFC 4226 HOTP with the step as the counter).
fn code_for(key: &[u8], step: i64) -> u32 {
    // Any key length is valid for HMAC, so this cannot fail.
    let mut mac = <Hmac<Sha1> as KeyInit>::new_from_slice(key).expect("HMAC takes any key");
    mac.update(&step.to_be_bytes());
    let digest = mac.finalize().into_bytes();
    let offset = (digest[digest.len() - 1] & 0x0f) as usize;
    let binary = u32::from_be_bytes([
        digest[offset] & 0x7f,
        digest[offset + 1],
        digest[offset + 2],
        digest[offset + 3],
    ]);
    binary % 10u32.pow(DIGITS)
}

/// The code an authenticator shows for `secret` at `unix_secs`, zero-padded.
#[cfg(test)]
pub fn code_at(secret: &str, unix_secs: i64) -> String {
    let key = secret_bytes(secret).expect("valid secret");
    format!("{:06}", code_for(&key, step_at(unix_secs)))
}

/// Six digits, spaces allowed (authenticators show `123 456`).
pub fn normalise_code(input: &str) -> Option<String> {
    let digits: String = input.chars().filter(|c| !c.is_whitespace()).collect();
    (digits.len() == DIGITS as usize && digits.bytes().all(|b| b.is_ascii_digit()))
        .then_some(digits)
}

/// The step `code` is valid for at `unix_secs` (now ± one step), if any is later than
/// `last_step`. The caller still has to claim that step in the database
/// (`user_totp::accept_step`) so the code works once.
pub fn verify(secret: &str, code: &str, unix_secs: i64, last_step: i64) -> Option<i64> {
    let code = normalise_code(code)?;
    let wanted: u32 = code.parse().ok()?;
    let key = secret_bytes(secret)?;
    let now = step_at(unix_secs);
    let mut found = None;
    // Look at every candidate step so the time taken does not say which one matched.
    for step in (now - SKEW_STEPS)..=(now + SKEW_STEPS) {
        let matches = constant_time_eq(&code_for(&key, step).to_be_bytes(), &wanted.to_be_bytes());
        if matches && step > last_step && found.is_none() {
            found = Some(step);
        }
    }
    found
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// The `otpauth://` URI authenticator apps read from the QR code.
pub fn otpauth_uri(account: &str, secret: &str) -> String {
    let label = format!("{ISSUER}:{account}");
    format!(
        "otpauth://totp/{}?secret={secret}&issuer={ISSUER}&algorithm=SHA1&digits={DIGITS}&period={PERIOD_SECS}",
        percent_encode(&label)
    )
}

fn percent_encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b':' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// The URI as an SVG QR code (dark modules on white, with the quiet zone), for an `<img>`.
pub fn qr_svg(uri: &str) -> Option<String> {
    let code = qrcode::QrCode::with_error_correction_level(uri, qrcode::EcLevel::M).ok()?;
    Some(
        code.render::<qrcode::render::svg::Color<'_>>()
            .min_dimensions(200, 200)
            .dark_color(qrcode::render::svg::Color("#000000"))
            .light_color(qrcode::render::svg::Color("#ffffff"))
            .build(),
    )
}

/// Ten new recovery codes, `xxxx-xxxx-xxxx` from the base32 alphabet in lower case (60 bits
/// each), and their hashes for the database.
pub fn new_recovery_codes() -> (Vec<String>, Vec<String>) {
    let codes: Vec<String> = (0..RECOVERY_CODES)
        .map(|_| {
            let mut bytes = [0u8; 8];
            rand::rngs::OsRng.fill_bytes(&mut bytes);
            let text = BASE32_NOPAD.encode(&bytes).to_ascii_lowercase();
            format!("{}-{}-{}", &text[0..4], &text[4..8], &text[8..12])
        })
        .collect();
    let hashes = codes.iter().filter_map(|c| recovery_hash(c)).collect();
    (codes, hashes)
}

/// The stored hash of a recovery code as typed (any case, dashes and spaces ignored), or None
/// when the input cannot be one.
pub fn recovery_hash(input: &str) -> Option<String> {
    let normalised: String = input
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .map(|c| c.to_ascii_lowercase())
        .collect();
    if normalised.len() != 12 || !normalised.bytes().all(|b| b.is_ascii_alphanumeric()) {
        return None;
    }
    let digest = Sha256::digest(normalised.as_bytes());
    Some(digest.iter().map(|b| format!("{b:02x}")).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// RFC 6238 appendix B, SHA-1 column, truncated to six digits. The key is the ASCII
    /// string "12345678901234567890".
    #[test]
    fn matches_the_rfc_6238_test_vectors() {
        let secret = BASE32_NOPAD.encode(b"12345678901234567890");
        for (time, code) in [
            (59, "287082"),
            (1_111_111_109, "081804"),
            (1_111_111_111, "050471"),
            (1_234_567_890, "005924"),
            (2_000_000_000, "279037"),
            (20_000_000_000, "353130"),
        ] {
            assert_eq!(code_at(&secret, time), code, "t = {time}");
        }
    }

    #[test]
    fn accepts_one_step_either_side_and_never_a_used_step() {
        let secret = new_secret();
        let now = 1_700_000_000;
        let step = step_at(now);
        let current = code_at(&secret, now);
        assert_eq!(verify(&secret, &current, now, 0), Some(step));
        // Spaces as an authenticator shows them
        let spaced = format!("{} {}", &current[..3], &current[3..]);
        assert_eq!(verify(&secret, &spaced, now, 0), Some(step));
        // The previous and next codes
        assert_eq!(
            verify(&secret, &code_at(&secret, now - 30), now, 0),
            Some(step - 1)
        );
        assert_eq!(
            verify(&secret, &code_at(&secret, now + 30), now, 0),
            Some(step + 1)
        );
        // Two steps away: no
        assert_eq!(verify(&secret, &code_at(&secret, now - 60), now, 0), None);
        // Already used this step (or a later one)
        assert_eq!(verify(&secret, &current, now, step), None);
        // Not six digits
        assert_eq!(verify(&secret, "12345", now, 0), None);
        assert_eq!(verify(&secret, "12345a", now, 0), None);
    }

    #[test]
    fn the_uri_names_the_issuer_and_escapes_the_account() {
        let uri = otpauth_uri("ann@svrctlrs.example", "JBSWY3DPEHPK3PXP");
        assert_eq!(
            uri,
            "otpauth://totp/SvrCtlRS:ann%40svrctlrs.example?secret=JBSWY3DPEHPK3PXP&issuer=SvrCtlRS&algorithm=SHA1&digits=6&period=30"
        );
        let svg = qr_svg(&uri).unwrap();
        assert!(svg.contains("<svg"), "{svg}");
    }

    #[test]
    fn recovery_codes_are_distinct_and_their_hashes_ignore_case_and_dashes() {
        let (codes, hashes) = new_recovery_codes();
        assert_eq!(codes.len(), 10);
        assert_eq!(hashes.len(), 10);
        let unique: std::collections::BTreeSet<_> = codes.iter().collect();
        assert_eq!(unique.len(), 10);
        let code = &codes[0];
        assert_eq!(code.len(), 14);
        let typed = code.replace('-', " ").to_ascii_uppercase();
        assert_eq!(recovery_hash(&typed), Some(hashes[0].clone()));
        assert_eq!(recovery_hash("123456"), None);
    }

    #[test]
    fn new_secrets_decode_to_twenty_bytes() {
        assert_eq!(secret_bytes(&new_secret()).unwrap().len(), 20);
    }
}
