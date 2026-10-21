//! Signing in beyond the password: TOTP and recovery codes (`totp`), and how this server lets
//! people sign in at all ([`SignInConfig`], from the environment).

pub mod totp;

/// How people sign in to this server. Read from the environment once at start-up, so neither
/// a session nor an API token can change it.
#[derive(Debug, Clone)]
pub struct SignInConfig {
    /// `PASSWORD_LOGIN`: whether a username and password are accepted at all
    pub password_login: bool,
}

/// The OpenID Connect provider, when one is configured
#[derive(Debug, Clone)]
pub struct ProviderInfo {
    pub name: String,
    pub issuer: String,
}

impl Default for SignInConfig {
    fn default() -> Self {
        Self {
            password_login: true,
        }
    }
}

impl SignInConfig {
    pub fn provider(&self) -> Option<&ProviderInfo> {
        None
    }
}
