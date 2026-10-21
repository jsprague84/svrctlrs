//! Signing in beyond the password: TOTP and recovery codes (`totp`), an OpenID Connect
//! provider (`oidc`, with `jwt` for its signatures), and how this server lets people sign in
//! at all ([`SignInConfig`], from the environment).

pub mod app_codes;
pub mod jwt;
pub mod oidc;
pub mod totp;

use std::sync::Arc;

/// How people sign in to this server. Read from the environment once at start-up, so neither
/// a session nor an API token can change it. API tokens are not part of this: they work the
/// same whatever is set here.
#[derive(Debug, Clone)]
pub struct SignInConfig {
    /// `PASSWORD_LOGIN`: whether a username and password are accepted at all
    pub password_login: bool,
    provider: Option<Arc<oidc::Provider>>,
}

impl Default for SignInConfig {
    fn default() -> Self {
        Self {
            password_login: true,
            provider: None,
        }
    }
}

impl SignInConfig {
    pub fn provider(&self) -> Option<&oidc::Provider> {
        self.provider.as_deref()
    }

    #[cfg(test)]
    pub fn with_provider(config: oidc::OidcConfig, password_login: bool) -> Self {
        Self {
            password_login,
            provider: Some(Arc::new(oidc::Provider::new(config))),
        }
    }

    /// From the process environment; an error is a configuration the server refuses to start
    /// with.
    pub fn from_env() -> Result<Self, String> {
        Self::from_lookup(|key| std::env::var(key).ok())
    }

    /// `get` returns a variable's value; an empty value counts as unset (compose files pass
    /// unset variables through as empty strings).
    pub fn from_lookup(get: impl Fn(&str) -> Option<String>) -> Result<Self, String> {
        let var = |key: &str| {
            get(key)
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
        };

        let provider = match var("OIDC_ISSUER") {
            None => {
                for key in [
                    "OIDC_CLIENT_ID",
                    "OIDC_CLIENT_SECRET",
                    "OIDC_ALLOWED_GROUPS",
                ] {
                    if var(key).is_some() {
                        return Err(format!("{key} is set but OIDC_ISSUER is not"));
                    }
                }
                None
            }
            Some(issuer) => {
                let issuer_url = reqwest::Url::parse(&issuer)
                    .map_err(|e| format!("OIDC_ISSUER is not a URL ({e})"))?;
                let client_id =
                    var("OIDC_CLIENT_ID").ok_or("OIDC_ISSUER is set but OIDC_CLIENT_ID is not")?;
                let public_url = var("PUBLIC_URL").ok_or(
                    "OIDC_ISSUER is set but PUBLIC_URL is not; set it to the address people \
                     open, e.g. https://svrctlrs.example.com",
                )?;
                let public = reqwest::Url::parse(&public_url)
                    .map_err(|e| format!("PUBLIC_URL is not a URL ({e})"))?;
                for (key, url) in [("OIDC_ISSUER", &issuer_url), ("PUBLIC_URL", &public)] {
                    let local = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
                    if url.scheme() != "https" && !local {
                        return Err(format!("{key} must be https:// (http only for localhost)"));
                    }
                }
                let allowed_groups: Vec<String> = var("OIDC_ALLOWED_GROUPS")
                    .map(|g| {
                        g.split(',')
                            .map(str::trim)
                            .filter(|s| !s.is_empty())
                            .map(String::from)
                            .collect()
                    })
                    .unwrap_or_default();
                let scopes = var("OIDC_SCOPES").unwrap_or_else(|| {
                    if allowed_groups.is_empty() {
                        "openid profile email".into()
                    } else {
                        "openid profile email groups".into()
                    }
                });
                if !scopes.split_whitespace().any(|s| s == "openid") {
                    return Err("OIDC_SCOPES must include openid".into());
                }
                Some(Arc::new(oidc::Provider::new(oidc::OidcConfig {
                    name: var("OIDC_NAME").unwrap_or_else(|| "single sign-on".into()),
                    issuer,
                    client_id,
                    client_secret: var("OIDC_CLIENT_SECRET"),
                    redirect_uri: format!(
                        "{}/auth/oidc/callback",
                        public_url.trim_end_matches('/')
                    ),
                    scopes,
                    allowed_groups,
                })))
            }
        };

        let password_login = match var("PASSWORD_LOGIN").map(|v| v.to_ascii_lowercase()) {
            None => true,
            Some(v) if ["on", "true", "1", "yes"].contains(&v.as_str()) => true,
            Some(v) if ["off", "false", "0", "no"].contains(&v.as_str()) => false,
            Some(v) => return Err(format!("PASSWORD_LOGIN must be on or off, not {v:?}")),
        };
        if !password_login && provider.is_none() {
            return Err(
                "PASSWORD_LOGIN=off needs an OpenID Connect provider (OIDC_ISSUER), or nobody \
                 could sign in"
                    .into(),
            );
        }
        Ok(Self {
            password_login,
            provider,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn config(vars: &[(&str, &str)]) -> Result<SignInConfig, String> {
        let map: HashMap<String, String> = vars
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        SignInConfig::from_lookup(|k| map.get(k).cloned())
    }

    const PROVIDER: &[(&str, &str)] = &[
        ("OIDC_ISSUER", "https://id.example"),
        ("OIDC_CLIENT_ID", "client-1"),
        ("OIDC_CLIENT_SECRET", "s3cret"),
        ("PUBLIC_URL", "https://svrctlrs.example/"),
    ];

    #[test]
    fn nothing_set_is_password_only() {
        let c = config(&[]).unwrap();
        assert!(c.password_login);
        assert!(c.provider().is_none());
        // Empty strings are unset
        let c = config(&[("OIDC_ISSUER", ""), ("PASSWORD_LOGIN", " ")]).unwrap();
        assert!(c.password_login && c.provider().is_none());
    }

    #[test]
    fn a_provider_builds_its_callback_from_public_url() {
        let c = config(PROVIDER).unwrap();
        let p = c.provider().unwrap();
        assert_eq!(
            p.config.redirect_uri,
            "https://svrctlrs.example/auth/oidc/callback"
        );
        assert_eq!(p.config.scopes, "openid profile email");
        assert_eq!(p.config.name, "single sign-on");
        assert!(!format!("{:?}", p.config).contains("s3cret"));

        let mut vars = PROVIDER.to_vec();
        vars.push(("OIDC_ALLOWED_GROUPS", "owner, admins"));
        vars.push(("OIDC_NAME", "Pocket ID"));
        vars.push(("PASSWORD_LOGIN", "off"));
        let c = config(&vars).unwrap();
        let p = c.provider().unwrap();
        assert_eq!(p.config.allowed_groups, ["owner", "admins"]);
        assert_eq!(p.config.scopes, "openid profile email groups");
        assert!(!c.password_login);
    }

    #[test]
    fn half_configurations_refuse_to_start() {
        assert!(config(&[("PASSWORD_LOGIN", "off")]).is_err());
        assert!(config(&[("PASSWORD_LOGIN", "maybe")]).is_err());
        assert!(config(&[("OIDC_CLIENT_ID", "x")]).is_err());
        assert!(config(&[
            ("OIDC_ISSUER", "https://id.example"),
            ("PUBLIC_URL", "https://a.example")
        ])
        .is_err());
        assert!(config(&[
            ("OIDC_ISSUER", "https://id.example"),
            ("OIDC_CLIENT_ID", "x")
        ])
        .is_err());
        let mut plain = PROVIDER.to_vec();
        plain[0] = ("OIDC_ISSUER", "http://id.example");
        assert!(config(&plain).is_err());
        let mut local = PROVIDER.to_vec();
        local[0] = ("OIDC_ISSUER", "http://127.0.0.1:9000");
        assert!(config(&local).is_ok());
        let mut no_openid = PROVIDER.to_vec();
        no_openid.push(("OIDC_SCOPES", "profile email"));
        assert!(config(&no_openid).is_err());
    }
}
