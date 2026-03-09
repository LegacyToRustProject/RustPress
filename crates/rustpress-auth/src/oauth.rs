//! OAuth 2.0 + OpenID Connect (PKCE flow) implementation.
//!
//! Supports Google, GitHub, Microsoft (Azure AD), Apple, and generic OIDC providers.
//! Uses the Authorization Code flow with PKCE (Proof Key for Code Exchange, RFC 7636).

use std::collections::HashMap;

use data_encoding::BASE64URL_NOPAD;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use tracing::{debug, instrument};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Provider-specific endpoint constants
// ---------------------------------------------------------------------------

mod endpoints {
    pub mod google {
        pub const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
        pub const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
        pub const USERINFO_URL: &str = "https://openidconnect.googleapis.com/v1/userinfo";
        pub const DEFAULT_SCOPES: &[&str] = &["openid", "email", "profile"];
    }

    pub mod github {
        pub const AUTH_URL: &str = "https://github.com/login/oauth/authorize";
        pub const TOKEN_URL: &str = "https://github.com/login/oauth/access_token";
        pub const USERINFO_URL: &str = "https://api.github.com/user";
        pub const USER_EMAIL_URL: &str = "https://api.github.com/user/emails";
        pub const DEFAULT_SCOPES: &[&str] = &["read:user", "user:email"];
    }

    pub mod microsoft {
        /// Replace `{tenant}` with your Azure AD tenant ID or use "common".
        pub const AUTH_URL_TMPL: &str =
            "https://login.microsoftonline.com/{tenant}/oauth2/v2.0/authorize";
        pub const TOKEN_URL_TMPL: &str =
            "https://login.microsoftonline.com/{tenant}/oauth2/v2.0/token";
        pub const USERINFO_URL: &str = "https://graph.microsoft.com/oidc/userinfo";
        pub const DEFAULT_SCOPES: &[&str] = &["openid", "email", "profile", "User.Read"];
    }

    pub mod apple {
        pub const AUTH_URL: &str = "https://appleid.apple.com/auth/authorize";
        pub const TOKEN_URL: &str = "https://appleid.apple.com/auth/token";
        /// Apple does not expose a standard userinfo endpoint; user data is in the ID token.
        pub const DEFAULT_SCOPES: &[&str] = &["openid", "name", "email"];
    }
}

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Supported OAuth 2.0 / OIDC providers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OAuthProvider {
    Google,
    GitHub,
    /// Microsoft Azure Active Directory.
    /// `tenant` defaults to `"common"` when not specified.
    Microsoft { tenant: String },
    Apple,
    /// Any OIDC-compliant provider with custom endpoints.
    Custom {
        auth_url: String,
        token_url: String,
        userinfo_url: String,
    },
}

impl OAuthProvider {
    /// Human-readable provider name used in logs and error messages.
    pub fn name(&self) -> &str {
        match self {
            OAuthProvider::Google => "google",
            OAuthProvider::GitHub => "github",
            OAuthProvider::Microsoft { .. } => "microsoft",
            OAuthProvider::Apple => "apple",
            OAuthProvider::Custom { .. } => "custom",
        }
    }

    fn auth_url(&self) -> String {
        match self {
            OAuthProvider::Google => endpoints::google::AUTH_URL.to_owned(),
            OAuthProvider::GitHub => endpoints::github::AUTH_URL.to_owned(),
            OAuthProvider::Microsoft { tenant } => {
                endpoints::microsoft::AUTH_URL_TMPL.replace("{tenant}", tenant)
            }
            OAuthProvider::Apple => endpoints::apple::AUTH_URL.to_owned(),
            OAuthProvider::Custom { auth_url, .. } => auth_url.clone(),
        }
    }

    fn token_url(&self) -> String {
        match self {
            OAuthProvider::Google => endpoints::google::TOKEN_URL.to_owned(),
            OAuthProvider::GitHub => endpoints::github::TOKEN_URL.to_owned(),
            OAuthProvider::Microsoft { tenant } => {
                endpoints::microsoft::TOKEN_URL_TMPL.replace("{tenant}", tenant)
            }
            OAuthProvider::Apple => endpoints::apple::TOKEN_URL.to_owned(),
            OAuthProvider::Custom { token_url, .. } => token_url.clone(),
        }
    }

    fn userinfo_url(&self) -> Option<String> {
        match self {
            OAuthProvider::Google => Some(endpoints::google::USERINFO_URL.to_owned()),
            OAuthProvider::GitHub => Some(endpoints::github::USERINFO_URL.to_owned()),
            OAuthProvider::Microsoft { .. } => {
                Some(endpoints::microsoft::USERINFO_URL.to_owned())
            }
            // Apple encodes user info in the JWT id_token; no separate endpoint.
            OAuthProvider::Apple => None,
            OAuthProvider::Custom { userinfo_url, .. } => Some(userinfo_url.clone()),
        }
    }

    fn default_scopes(&self) -> Vec<String> {
        match self {
            OAuthProvider::Google => {
                endpoints::google::DEFAULT_SCOPES.iter().map(|s| s.to_string()).collect()
            }
            OAuthProvider::GitHub => {
                endpoints::github::DEFAULT_SCOPES.iter().map(|s| s.to_string()).collect()
            }
            OAuthProvider::Microsoft { .. } => {
                endpoints::microsoft::DEFAULT_SCOPES.iter().map(|s| s.to_string()).collect()
            }
            OAuthProvider::Apple => {
                endpoints::apple::DEFAULT_SCOPES.iter().map(|s| s.to_string()).collect()
            }
            OAuthProvider::Custom { .. } => vec!["openid".to_owned(), "email".to_owned()],
        }
    }
}

// ---------------------------------------------------------------------------

/// Configuration for a specific OAuth client registration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthConfig {
    /// OAuth application client ID.
    pub client_id: String,
    /// OAuth application client secret.
    pub client_secret: String,
    /// Registered redirect URI (must match the provider's allow-list).
    pub redirect_uri: String,
    /// Requested scopes. If empty, provider defaults are used.
    pub scopes: Vec<String>,
    /// Additional static query parameters appended to the authorization URL.
    /// Useful for provider-specific hints (e.g., `login_hint`, `hd` domain).
    #[serde(default)]
    pub extra_params: HashMap<String, String>,
}

// ---------------------------------------------------------------------------

/// PKCE state kept server-side between the authorization redirect and callback.
///
/// Should be stored in the user's session (keyed by `state`) and retrieved
/// during the callback to validate the `state` parameter and supply the
/// `code_verifier` for token exchange.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthState {
    /// Opaque random value sent as the `state` query parameter.
    /// Must be compared to the value returned by the provider to prevent CSRF.
    pub state: String,
    /// High-entropy random string used to derive `code_challenge` (RFC 7636 §4.1).
    pub code_verifier: String,
    /// Provider that issued this state, so the callback handler knows which
    /// token endpoint to use.
    pub provider: OAuthProvider,
}

// ---------------------------------------------------------------------------

/// Tokens returned by the authorization server after a successful code exchange.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthTokens {
    /// Bearer access token for API calls.
    pub access_token: String,
    /// OIDC ID token (JWT). Present for OpenID Connect providers.
    pub id_token: Option<String>,
    /// Refresh token. Present when `offline_access` scope was requested.
    pub refresh_token: Option<String>,
    /// Lifetime of `access_token` in seconds, as reported by the server.
    pub expires_in: Option<u64>,
    /// Token type, typically `"Bearer"`.
    pub token_type: String,
}

// ---------------------------------------------------------------------------

/// Normalized user information fetched from the provider's userinfo endpoint
/// (or extracted from the OIDC ID token for Apple).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthUserInfo {
    /// Provider-specific user ID (`sub` claim in OIDC).
    pub sub: String,
    /// Primary email address. May be `None` if the user has no verified email
    /// or the email scope was not granted.
    pub email: Option<String>,
    /// Display name.
    pub name: Option<String>,
    /// URL of the user's profile picture.
    pub picture: Option<String>,
    /// The provider this info was sourced from.
    pub provider: OAuthProvider,
    /// Raw attributes returned by the provider (unparsed extras).
    #[serde(default)]
    pub raw: HashMap<String, serde_json::Value>,
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that can arise during the OAuth 2.0 / OIDC flow.
#[derive(Debug, Error)]
pub enum OAuthError {
    /// The `state` parameter returned by the provider did not match the stored value.
    #[error("invalid OAuth state: CSRF check failed")]
    InvalidState,

    /// The authorization code → token exchange failed.
    #[error("token exchange failed: {0}")]
    TokenExchange(String),

    /// Fetching user information from the provider's userinfo endpoint failed.
    #[error("userinfo fetch failed: {0}")]
    UserInfoFetch(String),

    /// The provider is configured incorrectly or does not support the requested operation.
    #[error("invalid provider configuration: {0}")]
    InvalidProvider(String),

    /// An HTTP transport error occurred.
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// JSON (de)serialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

// ---------------------------------------------------------------------------
// PKCE helpers
// ---------------------------------------------------------------------------

/// Generate a cryptographically random PKCE `code_verifier` (43-128 ASCII chars,
/// RFC 7636 §4.1).
///
/// Uses `rand::thread_rng` with the `Alphanumeric` sampler so every byte maps
/// to an unreserved URI character.
fn generate_code_verifier() -> String {
    use rand::distributions::Alphanumeric;
    use rand::Rng;
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(96) // well within [43, 128]
        .map(char::from)
        .collect()
}

/// Derive the PKCE `code_challenge` from a verifier using the S256 method:
/// `BASE64URL(SHA256(ASCII(code_verifier)))` (RFC 7636 §4.2).
fn pkce_code_challenge(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    BASE64URL_NOPAD.encode(&digest)
}

// ---------------------------------------------------------------------------
// Main client
// ---------------------------------------------------------------------------

/// OAuth 2.0 / OIDC client for a single provider registration.
///
/// # Example
///
/// ```rust,ignore
/// let config = OAuthConfig {
///     client_id: "my-client-id".into(),
///     client_secret: "my-secret".into(),
///     redirect_uri: "https://example.com/auth/callback".into(),
///     scopes: vec![],
///     extra_params: Default::default(),
/// };
/// let client = OAuthClient::new(OAuthProvider::Google, config);
/// let (url, state) = client.authorization_url();
/// // Redirect user to `url`, save `state` in their session.
/// ```
#[derive(Debug, Clone)]
pub struct OAuthClient {
    provider: OAuthProvider,
    config: OAuthConfig,
    http: reqwest::Client,
}

impl OAuthClient {
    /// Create a new `OAuthClient` for the given provider and configuration.
    pub fn new(provider: OAuthProvider, config: OAuthConfig) -> Self {
        let http = reqwest::Client::builder()
            .user_agent(concat!(
                "RustPress/",
                env!("CARGO_PKG_VERSION"),
                " (+https://github.com/rustpress/rustpress)"
            ))
            .build()
            .expect("failed to build reqwest client");

        Self { provider, config, http }
    }

    /// Build the authorization URL and generate PKCE state.
    ///
    /// Returns `(authorization_url, oauth_state)`. The caller must:
    /// 1. Persist `oauth_state` (keyed by `oauth_state.state`) in the user session.
    /// 2. Redirect the user to `authorization_url`.
    #[instrument(skip(self), fields(provider = %self.provider.name()))]
    pub fn authorization_url(&self) -> (String, OAuthState) {
        let state_token = Uuid::new_v4().to_string();
        let code_verifier = generate_code_verifier();
        let code_challenge = pkce_code_challenge(&code_verifier);

        let scopes = if self.config.scopes.is_empty() {
            self.provider.default_scopes()
        } else {
            self.config.scopes.clone()
        };

        let mut params: Vec<(&str, String)> = vec![
            ("response_type", "code".to_owned()),
            ("client_id", self.config.client_id.clone()),
            ("redirect_uri", self.config.redirect_uri.clone()),
            ("scope", scopes.join(" ")),
            ("state", state_token.clone()),
            ("code_challenge", code_challenge),
            ("code_challenge_method", "S256".to_owned()),
        ];

        // Apple requires `response_mode=form_post` for web flows.
        if self.provider == OAuthProvider::Apple {
            params.push(("response_mode", "form_post".to_owned()));
        }

        // Append caller-supplied extra params (e.g., `login_hint`).
        for (k, v) in &self.config.extra_params {
            params.push((k.as_str(), v.clone()));
        }

        let query = serde_urlencoded::to_string(&params)
            .unwrap_or_default();

        let url = format!("{}?{}", self.provider.auth_url(), query);

        debug!(url = %url, "generated OAuth authorization URL");

        let oauth_state = OAuthState {
            state: state_token,
            code_verifier,
            provider: self.provider.clone(),
        };

        (url, oauth_state)
    }

    /// Exchange an authorization `code` for tokens.
    ///
    /// `state` must be the [`OAuthState`] that was previously persisted for
    /// the matching `state` query parameter returned by the provider.
    #[instrument(skip(self, code, state), fields(provider = %self.provider.name()))]
    pub async fn exchange_code(
        &self,
        code: &str,
        state: &OAuthState,
    ) -> Result<OAuthTokens, OAuthError> {
        let token_url = self.provider.token_url();
        debug!(token_url = %token_url, "exchanging authorization code for tokens");

        let mut form: Vec<(&str, &str)> = vec![
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", &self.config.redirect_uri),
            ("client_id", &self.config.client_id),
            ("code_verifier", &state.code_verifier),
        ];

        // GitHub uses form-encoded client credentials; most OIDC providers
        // also accept them in the request body (vs. HTTP Basic auth).
        let secret_ref = self.config.client_secret.as_str();
        form.push(("client_secret", secret_ref));

        let response = self
            .http
            .post(&token_url)
            .header(reqwest::header::ACCEPT, "application/json")
            .form(&form)
            .send()
            .await
            .map_err(OAuthError::Http)?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(OAuthError::TokenExchange(format!(
                "HTTP {status}: {body}"
            )));
        }

        // GitHub returns `application/x-www-form-urlencoded` by default; the
        // `Accept: application/json` header above coerces it to JSON for all
        // major providers.
        let raw: serde_json::Value = response.json().await?;

        let tokens = OAuthTokens {
            access_token: raw["access_token"]
                .as_str()
                .ok_or_else(|| OAuthError::TokenExchange("missing access_token".into()))?
                .to_owned(),
            id_token: raw["id_token"].as_str().map(str::to_owned),
            refresh_token: raw["refresh_token"].as_str().map(str::to_owned),
            expires_in: raw["expires_in"].as_u64(),
            token_type: raw["token_type"]
                .as_str()
                .unwrap_or("Bearer")
                .to_owned(),
        };

        debug!("token exchange successful");
        Ok(tokens)
    }

    /// Fetch normalized user information using the `access_token`.
    ///
    /// For Apple, this will return an error because Apple does not expose a
    /// userinfo endpoint — callers should parse the `id_token` JWT instead.
    #[instrument(skip(self, access_token), fields(provider = %self.provider.name()))]
    pub async fn fetch_user_info(
        &self,
        access_token: &str,
    ) -> Result<OAuthUserInfo, OAuthError> {
        let userinfo_url = self.provider.userinfo_url().ok_or_else(|| {
            OAuthError::InvalidProvider(format!(
                "provider '{}' does not expose a userinfo endpoint; \
                 parse the id_token JWT instead",
                self.provider.name()
            ))
        })?;

        debug!(userinfo_url = %userinfo_url, "fetching user info");

        let response = self
            .http
            .get(&userinfo_url)
            .bearer_auth(access_token)
            .header(reqwest::header::ACCEPT, "application/json")
            .send()
            .await
            .map_err(OAuthError::Http)?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(OAuthError::UserInfoFetch(format!(
                "HTTP {status}: {body}"
            )));
        }

        let raw: serde_json::Map<String, serde_json::Value> = response.json().await?;

        let user_info = self.normalize_user_info(raw)?;
        debug!(sub = %user_info.sub, "fetched user info");
        Ok(user_info)
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    /// Map provider-specific JSON fields to the common [`OAuthUserInfo`] shape.
    fn normalize_user_info(
        &self,
        raw: serde_json::Map<String, serde_json::Value>,
    ) -> Result<OAuthUserInfo, OAuthError> {
        let get_str = |key: &str| -> Option<String> {
            raw.get(key).and_then(|v| v.as_str()).map(str::to_owned)
        };

        let (sub, email, name, picture) = match &self.provider {
            OAuthProvider::GitHub => {
                // GitHub uses `login` as the primary identifier and `id` as the
                // numeric sub. `email` may be null if the user's email is private.
                let sub = raw
                    .get("id")
                    .and_then(|v| v.as_i64())
                    .map(|id| id.to_string())
                    .or_else(|| get_str("login"))
                    .ok_or_else(|| {
                        OAuthError::UserInfoFetch("missing 'id' field in GitHub response".into())
                    })?;
                let name = get_str("name").or_else(|| get_str("login"));
                let picture = get_str("avatar_url");
                let email = get_str("email");
                (sub, email, name, picture)
            }
            // Google, Microsoft, and generic OIDC all follow the standard OIDC
            // UserInfo schema (RFC 8414).
            _ => {
                let sub = get_str("sub").ok_or_else(|| {
                    OAuthError::UserInfoFetch(
                        "missing 'sub' field in OIDC userinfo response".into(),
                    )
                })?;
                let email = get_str("email");
                let name = get_str("name");
                let picture = get_str("picture");
                (sub, email, name, picture)
            }
        };

        Ok(OAuthUserInfo {
            sub,
            email,
            name,
            picture,
            provider: self.provider.clone(),
            raw: raw.into_iter().collect(),
        })
    }
}

// ---------------------------------------------------------------------------
// serde_urlencoded shim — re-export for convenience
// ---------------------------------------------------------------------------

// (serde_urlencoded is already a transitive dependency via reqwest; we rely on
// it for building the authorization URL query string above.)

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_challenge_is_base64url_of_sha256() {
        // RFC 7636 Appendix B test vector.
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let challenge = pkce_code_challenge(verifier);
        // Expected: BASE64URL(SHA256(ASCII(verifier)))
        assert!(!challenge.contains('+'));
        assert!(!challenge.contains('/'));
        assert!(!challenge.contains('='));
        assert!(!challenge.is_empty());
    }

    #[test]
    fn code_verifier_length_is_within_rfc_bounds() {
        let v = generate_code_verifier();
        assert!(v.len() >= 43, "verifier too short: {}", v.len());
        assert!(v.len() <= 128, "verifier too long: {}", v.len());
    }

    #[test]
    fn authorization_url_contains_pkce_params() {
        let config = OAuthConfig {
            client_id: "test-client".into(),
            client_secret: "test-secret".into(),
            redirect_uri: "https://example.com/callback".into(),
            scopes: vec![],
            extra_params: Default::default(),
        };
        let client = OAuthClient::new(OAuthProvider::Google, config);
        let (url, state) = client.authorization_url();

        assert!(url.contains("code_challenge="), "missing code_challenge in URL");
        assert!(url.contains("code_challenge_method=S256"), "missing S256 method");
        assert!(url.contains(&state.state), "state token not present in URL");
    }

    #[test]
    fn microsoft_tenant_substituted_in_urls() {
        let provider = OAuthProvider::Microsoft {
            tenant: "contoso.onmicrosoft.com".into(),
        };
        assert!(provider.auth_url().contains("contoso.onmicrosoft.com"));
        assert!(provider.token_url().contains("contoso.onmicrosoft.com"));
    }

    #[test]
    fn apple_has_no_userinfo_url() {
        assert!(OAuthProvider::Apple.userinfo_url().is_none());
    }
}
