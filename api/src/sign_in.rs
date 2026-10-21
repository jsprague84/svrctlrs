//! How the caller signs in: two-factor (TOTP) and identities linked at an OpenID Connect
//! provider. Only a signed-in person reaches these routes; no API token preset names them.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// `GET /users/me/sign-in`
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SignInMethods {
    /// Whether this server accepts a password at all (`PASSWORD_LOGIN`)
    pub password_login: bool,
    pub totp: TotpStatus,
    /// The OpenID Connect provider this server signs in with, if one is configured
    pub provider: Option<SignInProvider>,
    /// The caller's identities at that provider (or at one configured before)
    pub identities: Vec<LinkedIdentity>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TotpStatus {
    /// A password sign-in also asks for a code
    pub enabled: bool,
    /// Unused recovery codes
    pub recovery_codes_left: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SignInProvider {
    /// What the sign-in button says (`OIDC_NAME`)
    pub name: String,
    pub issuer: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct LinkedIdentity {
    pub id: i64,
    pub issuer: String,
    /// What the provider said at the last sign-in
    pub email: Option<String>,
    pub display_name: Option<String>,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
}

/// The caller's current password, for changes that could lock them out
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PasswordConfirmation {
    pub password: String,
}

/// `POST /users/me/totp`: what to load into the authenticator app. Shown once; the secret is
/// not readable again.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TotpEnrolment {
    /// Base32, for typing in by hand
    pub secret: String,
    pub otpauth_uri: String,
    /// The URI as an SVG QR code
    pub qr_svg: String,
}

/// `POST /users/me/totp/confirm`
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TotpCode {
    pub code: String,
}

/// Ten one-time recovery codes. Shown once; only their hashes are kept.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RecoveryCodes {
    pub codes: Vec<String>,
}
