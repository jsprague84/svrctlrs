//! Signature checks for a provider's ID token (a JWS in compact serialization) against its
//! published keys (JWKS).
//!
//! No JWT library: the RSA, ECDSA and Ed25519 code russh already builds (`ssh-key`) does the
//! cryptography. An SSH `rsa-sha2-256` signature is RSASSA-PKCS1-v1_5 with SHA-256, which is
//! exactly RS256; ES256 and EdDSA differ from their SSH forms only in how the signature bytes
//! are framed. Only those three algorithms are accepted, each only with a key of its own type,
//! so `none`, HMAC and key-type confusion are refused before any key is looked at.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use russh::keys::signature::Verifier;
use russh::keys::ssh_key::{
    self,
    encoding::Encode,
    public::{EcdsaPublicKey, Ed25519PublicKey, KeyData, RsaPublicKey},
    Algorithm, EcdsaCurve, HashAlg, Mpint,
};
use serde::Deserialize;

/// One key from a provider's `jwks_uri`
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Jwk {
    pub kty: String,
    #[serde(default)]
    pub kid: Option<String>,
    #[serde(default)]
    pub alg: Option<String>,
    #[serde(default, rename = "use")]
    pub key_use: Option<String>,
    #[serde(default)]
    pub n: Option<String>,
    #[serde(default)]
    pub e: Option<String>,
    #[serde(default)]
    pub crv: Option<String>,
    #[serde(default)]
    pub x: Option<String>,
    #[serde(default)]
    pub y: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct JwkSet {
    pub keys: Vec<Jwk>,
}

#[derive(Debug, Deserialize)]
struct Header {
    alg: String,
    #[serde(default)]
    kid: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum JwtError {
    /// Not three base64url parts with a JSON header
    Malformed,
    /// An algorithm other than RS256, ES256 or EdDSA (`none` and HMAC included)
    UnsupportedAlgorithm(String),
    /// No published key fits the token's `kid` and algorithm; the key set may be stale
    NoKey(Option<String>),
    BadSignature,
}

impl std::fmt::Display for JwtError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JwtError::Malformed => write!(f, "the ID token is not a signed JWT"),
            JwtError::UnsupportedAlgorithm(a) => {
                write!(
                    f,
                    "the ID token is signed with {a}; RS256, ES256 or EdDSA is needed"
                )
            }
            JwtError::NoKey(Some(kid)) => write!(f, "the provider publishes no key {kid:?}"),
            JwtError::NoKey(None) => write!(f, "the provider publishes no key for this token"),
            JwtError::BadSignature => write!(f, "the ID token's signature does not verify"),
        }
    }
}

fn b64(part: &str) -> Option<Vec<u8>> {
    URL_SAFE_NO_PAD.decode(part.trim_end_matches('=')).ok()
}

/// Check `token`'s signature with one of `keys` and return its payload bytes. Claims are the
/// caller's to check.
pub fn verify(token: &str, keys: &[Jwk]) -> Result<Vec<u8>, JwtError> {
    let mut parts = token.split('.');
    let (Some(header_b64), Some(payload_b64), Some(signature_b64), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(JwtError::Malformed);
    };
    let header: Header = b64(header_b64)
        .and_then(|h| serde_json::from_slice(&h).ok())
        .ok_or(JwtError::Malformed)?;
    let payload = b64(payload_b64).ok_or(JwtError::Malformed)?;
    let signature = b64(signature_b64).ok_or(JwtError::Malformed)?;
    let kty = match header.alg.as_str() {
        "RS256" => "RSA",
        "ES256" => "EC",
        "EdDSA" => "OKP",
        other => return Err(JwtError::UnsupportedAlgorithm(other.to_string())),
    };
    let signing_input = &token[..header_b64.len() + 1 + payload_b64.len()];

    let candidates: Vec<&Jwk> = keys
        .iter()
        .filter(|k| k.kty == kty)
        .filter(|k| k.alg.as_deref().is_none_or(|a| a == header.alg))
        .filter(|k| k.key_use.as_deref().is_none_or(|u| u == "sig"))
        .filter(|k| match (&header.kid, &k.kid) {
            (Some(want), Some(have)) => want == have,
            (Some(_), None) => true,
            (None, _) => true,
        })
        .collect();
    if candidates.is_empty() {
        return Err(JwtError::NoKey(header.kid));
    }
    let good = candidates.iter().any(|k| {
        verify_with(&header.alg, k, signing_input.as_bytes(), &signature).unwrap_or(false)
    });
    if good {
        Ok(payload)
    } else {
        Err(JwtError::BadSignature)
    }
}

/// None when the key itself cannot be read for that algorithm.
fn verify_with(alg: &str, jwk: &Jwk, message: &[u8], signature: &[u8]) -> Option<bool> {
    let (key, signature) = match alg {
        "RS256" => {
            let n = b64(jwk.n.as_deref()?)?;
            let e = b64(jwk.e.as_deref()?)?;
            let key = RsaPublicKey::new(
                Mpint::from_positive_bytes(&e),
                Mpint::from_positive_bytes(&n),
            )
            .ok()?;
            // A short modulus is not worth trusting, whatever the provider publishes
            if key.key_size() < 2048 {
                return Some(false);
            }
            let sig = ssh_key::Signature::new(
                Algorithm::Rsa {
                    hash: Some(HashAlg::Sha256),
                },
                signature.to_vec(),
            )
            .ok()?;
            (KeyData::Rsa(key), sig)
        }
        "ES256" => {
            if jwk.crv.as_deref() != Some("P-256") {
                return None;
            }
            let x = b64(jwk.x.as_deref()?)?;
            let y = b64(jwk.y.as_deref()?)?;
            if x.len() != 32 || y.len() != 32 || signature.len() != 64 {
                return Some(false);
            }
            let mut point = Vec::with_capacity(65);
            point.push(0x04);
            point.extend_from_slice(&x);
            point.extend_from_slice(&y);
            let key = EcdsaPublicKey::from_sec1_bytes(&point).ok()?;
            // JWS carries r || s; SSH carries them as two mpints
            let mut framed = Vec::new();
            Mpint::from_positive_bytes(&signature[..32])
                .encode(&mut framed)
                .ok()?;
            Mpint::from_positive_bytes(&signature[32..])
                .encode(&mut framed)
                .ok()?;
            let sig = ssh_key::Signature::new(
                Algorithm::Ecdsa {
                    curve: EcdsaCurve::NistP256,
                },
                framed,
            )
            .ok()?;
            (KeyData::Ecdsa(key), sig)
        }
        "EdDSA" => {
            if jwk.crv.as_deref() != Some("Ed25519") {
                return None;
            }
            let x: [u8; 32] = b64(jwk.x.as_deref()?)?.try_into().ok()?;
            let sig = ssh_key::Signature::new(Algorithm::Ed25519, signature.to_vec()).ok()?;
            (KeyData::Ed25519(Ed25519PublicKey(x)), sig)
        }
        _ => return None,
    };
    Some(key.verify(message, &signature).is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    // RFC 7515 appendix A.2 (RS256) and A.3 (ES256), RFC 8037 appendix A.4/A.5 (Ed25519)
    const RS256_N: &str = "ofgWCuLjybRlzo0tZWJjNiuSfb4p4fAkd_wWJcyQoTbji9k0l8W26mPddxHmfHQp-Vaw-4qPCJrcS2mJPMEzP1Pt0Bm4d4QlL-yRT-SFd2lZS-pCgNMsD1W_YpRPEwOWvG6b32690r2jZ47soMZo9wGzjb_7OMg0LOL-bSf63kpaSHSXndS5z5rexMdbBYUsLA9e-KXBdQOS-UTo7WTBEMa2R2CapHg665xsmtdVMTBQY4uDZlxvb3qCo5ZwKh9kG4LT6_I5IhlJH7aGhyxXFvUK-DWNmoudF8NAco9_h9iaGNj8q2ethFkMLs91kzk2PAcDTW9gb54h4FRWyuXpoQ";
    const RS256_JWS: &str = "eyJhbGciOiJSUzI1NiJ9.eyJpc3MiOiJqb2UiLA0KICJleHAiOjEzMDA4MTkzODAsDQogImh0dHA6Ly9leGFtcGxlLmNvbS9pc19yb290Ijp0cnVlfQ.cC4hiUPoj9Eetdgtv3hF80EGrhuB__dzERat0XF9g2VtQgr9PJbu3XOiZj5RZmh7AAuHIm4Bh-0Qc_lF5YKt_O8W2Fp5jujGbds9uJdbF9CUAr7t1dnZcAcQjbKBYNX4BAynRFdiuB--f_nZLgrnbyTyWzO75vRK5h6xBArLIARNPvkSjtQBMHlb1L07Qe7K0GarZRmB_eSN9383LcOLn6_dO--xi12jzDwusC-eOkHWEsqtFZESc6BfI7noOPqvhJ1phCnvWh6IeYI2w9QOYEUipUTI8np6LbgGY9Fs98rqVt5AXLIhWkWywlVmtVrBp0igcN_IoypGlUPQGe77Rw";
    const ES256_X: &str = "f83OJ3D2xF1Bg8vub9tLe1gHMzV76e8Tus9uPHvRVEU";
    const ES256_Y: &str = "x_FEzRu9m36HLN_tue659LNpXW6pCyStikYjKIWI5a0";
    const ES256_JWS: &str = "eyJhbGciOiJFUzI1NiJ9.eyJpc3MiOiJqb2UiLA0KICJleHAiOjEzMDA4MTkzODAsDQogImh0dHA6Ly9leGFtcGxlLmNvbS9pc19yb290Ijp0cnVlfQ.DtEhU3ljbEg8L38VWAfUAqOyKAM6-Xx-F4GawxaepmXFCgfTjDxw5djxLa8ISlSApmWQxfKTUJqPP3-Kg6NU1Q";
    const ED25519_X: &str = "11qYAYKxCrfVS_7TyWQHOg7hcvPapiMlrwIaaPcHURo";
    const EDDSA_JWS: &str = "eyJhbGciOiJFZERTQSJ9.RXhhbXBsZSBvZiBFZDI1NTE5IHNpZ25pbmc.hgyY0il_MGCjP0JzlnLWG1PPOt7-09PGcvMg3AIbQR6dWbhijcNR4ki4iylGjg5BhVsPt9g7sVvpAr_MuM0KAg";

    fn rsa_key(kid: Option<&str>) -> Jwk {
        Jwk {
            kty: "RSA".into(),
            kid: kid.map(String::from),
            alg: None,
            key_use: Some("sig".into()),
            n: Some(RS256_N.into()),
            e: Some("AQAB".into()),
            crv: None,
            x: None,
            y: None,
        }
    }

    fn ec_key() -> Jwk {
        Jwk {
            kty: "EC".into(),
            crv: Some("P-256".into()),
            x: Some(ES256_X.into()),
            y: Some(ES256_Y.into()),
            ..rsa_key(None)
        }
    }

    fn ed_key() -> Jwk {
        Jwk {
            kty: "OKP".into(),
            crv: Some("Ed25519".into()),
            x: Some(ED25519_X.into()),
            ..rsa_key(None)
        }
    }

    /// Change one character of the signature part
    fn tamper(token: &str) -> String {
        let mut t = token.to_string();
        let last = t.pop().unwrap();
        t.push(if last == 'A' { 'B' } else { 'A' });
        t
    }

    #[test]
    fn the_rfc_examples_verify() {
        let claims = verify(RS256_JWS, &[rsa_key(None)]).unwrap();
        assert!(String::from_utf8(claims)
            .unwrap()
            .contains("\"iss\":\"joe\""));
        verify(ES256_JWS, &[ec_key()]).unwrap();
        let payload = verify(EDDSA_JWS, &[ed_key()]).unwrap();
        assert_eq!(payload, b"Example of Ed25519 signing");
    }

    #[test]
    fn a_changed_signature_or_payload_does_not() {
        assert_eq!(
            verify(&tamper(RS256_JWS), &[rsa_key(None)]),
            Err(JwtError::BadSignature)
        );
        assert_eq!(
            verify(&tamper(ES256_JWS), &[ec_key()]),
            Err(JwtError::BadSignature)
        );
        assert_eq!(
            verify(&tamper(EDDSA_JWS), &[ed_key()]),
            Err(JwtError::BadSignature)
        );
        // Same signature over another payload
        let parts: Vec<&str> = RS256_JWS.split('.').collect();
        let forged = format!("{}.{}.{}", parts[0], "eyJpc3MiOiJldmUifQ", parts[2]);
        assert_eq!(
            verify(&forged, &[rsa_key(None)]),
            Err(JwtError::BadSignature)
        );
    }

    #[test]
    fn none_hmac_and_mismatched_key_types_are_refused() {
        // {"alg":"none"} with an empty signature
        let none = "eyJhbGciOiJub25lIn0.eyJpc3MiOiJqb2UifQ.";
        assert_eq!(
            verify(none, &[rsa_key(None)]),
            Err(JwtError::UnsupportedAlgorithm("none".into()))
        );
        // {"alg":"HS256"}: an HMAC "keyed" with the public key must never be tried
        let hs = "eyJhbGciOiJIUzI1NiJ9.eyJpc3MiOiJqb2UifQ.c2ln";
        assert_eq!(
            verify(hs, &[rsa_key(None)]),
            Err(JwtError::UnsupportedAlgorithm("HS256".into()))
        );
        // An RS256 token with only an EC key on offer
        assert_eq!(verify(RS256_JWS, &[ec_key()]), Err(JwtError::NoKey(None)));
        // A key marked for another algorithm
        let mut wrong_alg = rsa_key(None);
        wrong_alg.alg = Some("RS512".into());
        assert_eq!(verify(RS256_JWS, &[wrong_alg]), Err(JwtError::NoKey(None)));
    }

    #[test]
    fn keys_are_picked_by_kid() {
        // {"alg":"RS256","kid":"k2"} over the RFC payload is not signed; only the kid matters
        let header = URL_SAFE_NO_PAD.encode(br#"{"alg":"RS256","kid":"k2"}"#);
        let parts: Vec<&str> = RS256_JWS.split('.').collect();
        let token = format!("{header}.{}.{}", parts[1], parts[2]);
        assert_eq!(
            verify(&token, &[rsa_key(Some("k1"))]),
            Err(JwtError::NoKey(Some("k2".into())))
        );
        assert_eq!(
            verify(&token, &[rsa_key(Some("k2"))]),
            Err(JwtError::BadSignature)
        );
    }

    #[test]
    fn malformed_tokens_are_refused() {
        for bad in ["", "a.b", "a.b.c.d", "!!.e30.c2ln", "e30.e30.c2ln"] {
            assert!(verify(bad, &[rsa_key(None)]).is_err(), "{bad}");
        }
    }
}
