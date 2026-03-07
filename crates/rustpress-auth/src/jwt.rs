use chrono::{Duration, Utc};
use jsonwebtoken::{
    decode, encode, Algorithm, DecodingKey, EncodingKey, Header, TokenData, Validation,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::debug;
use uuid::Uuid;

#[derive(Error, Debug)]
pub enum JwtError {
    #[error("JWT encoding error: {0}")]
    Encode(String),
    #[error("JWT decoding error: {0}")]
    Decode(String),
    #[error("Token expired")]
    Expired,
    #[error("Invalid token")]
    Invalid,
}

/// JWT claims structure.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    /// JWT ID (unique per token, used for blacklisting on logout)
    pub jti: String,
    /// Subject (user ID)
    pub sub: u64,
    /// User login name
    pub login: String,
    /// User email
    pub email: String,
    /// User role
    pub role: String,
    /// Issued at (Unix timestamp)
    pub iat: i64,
    /// Expiration (Unix timestamp)
    pub exp: i64,
}

/// JWT token manager for authentication.
#[derive(Clone)]
pub struct JwtManager {
    secret: String,
    expiration_hours: i64,
}

impl JwtManager {
    /// Create a new JWT manager.
    ///
    /// # Panics
    /// Panics if the secret is shorter than 32 bytes (256 bits), which is the
    /// minimum recommended by OWASP for HMAC-SHA256 signing keys.
    pub fn new(secret: &str, expiration_hours: i64) -> Self {
        assert!(
            secret.len() >= 32,
            "JWT secret must be at least 32 bytes (256 bits). Got {} bytes.",
            secret.len()
        );
        Self {
            secret: secret.to_string(),
            expiration_hours,
        }
    }

    /// Generate a JWT token for a user.
    pub fn generate_token(
        &self,
        user_id: u64,
        login: &str,
        email: &str,
        role: &str,
    ) -> Result<String, JwtError> {
        let now = Utc::now();
        let exp = now + Duration::hours(self.expiration_hours);

        let claims = Claims {
            jti: Uuid::new_v4().to_string(),
            sub: user_id,
            login: login.to_string(),
            email: email.to_string(),
            role: role.to_string(),
            iat: now.timestamp(),
            exp: exp.timestamp(),
        };

        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.secret.as_bytes()),
        )
        .map_err(|e| JwtError::Encode(e.to_string()))?;

        debug!(user_id, login, "JWT token generated");
        Ok(token)
    }

    /// Validate and decode a JWT token.
    ///
    /// Returns `JwtError::Invalid` if the token's `jti` is on the blacklist
    /// (i.e. the user has already logged out).
    pub fn validate_token(&self, token: &str) -> Result<Claims, JwtError> {
        // Explicitly enforce HS256 to prevent algorithm confusion attacks
        let validation = Validation::new(Algorithm::HS256);
        let token_data: TokenData<Claims> = decode(
            token,
            &DecodingKey::from_secret(self.secret.as_bytes()),
            &validation,
        )
        .map_err(|e| match e.kind() {
            jsonwebtoken::errors::ErrorKind::ExpiredSignature => JwtError::Expired,
            _ => JwtError::Decode(e.to_string()),
        })?;

        // Reject tokens that were explicitly invalidated at logout
        if crate::jwt_blacklist::is_blacklisted(&token_data.claims.jti) {
            return Err(JwtError::Invalid);
        }

        Ok(token_data.claims)
    }

    /// Refresh a token (generate new token with extended expiration).
    pub fn refresh_token(&self, claims: &Claims) -> Result<String, JwtError> {
        self.generate_token(claims.sub, &claims.login, &claims.email, &claims.role)
    }

    /// Generate a short-lived (5-minute) "2FA pending" token.
    ///
    /// This token proves that password verification succeeded but the full
    /// session is withheld until the TOTP code is verified.
    /// The role is set to the sentinel value `"2fa_pending"`.
    pub fn generate_pending_token(&self, user_id: u64, login: &str) -> Result<String, JwtError> {
        let now = Utc::now();
        let exp = now + Duration::minutes(5);
        let claims = Claims {
            jti: Uuid::new_v4().to_string(),
            sub: user_id,
            login: login.to_string(),
            email: String::new(),
            role: "2fa_pending".to_string(),
            iat: now.timestamp(),
            exp: exp.timestamp(),
        };
        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.secret.as_bytes()),
        )
        .map_err(|e| JwtError::Encode(e.to_string()))
    }

    /// Validate a "2FA pending" token and return `(user_id, login)`.
    ///
    /// Only accepts tokens whose role is exactly `"2fa_pending"`.
    pub fn validate_pending_token(&self, token: &str) -> Result<(u64, String), JwtError> {
        let validation = Validation::new(Algorithm::HS256);
        let token_data: TokenData<Claims> = decode(
            token,
            &DecodingKey::from_secret(self.secret.as_bytes()),
            &validation,
        )
        .map_err(|e| match e.kind() {
            jsonwebtoken::errors::ErrorKind::ExpiredSignature => JwtError::Expired,
            _ => JwtError::Decode(e.to_string()),
        })?;

        if token_data.claims.role != "2fa_pending" {
            return Err(JwtError::Invalid);
        }
        Ok((token_data.claims.sub, token_data.claims.login))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &str = "test-secret-key-that-is-at-least-32-bytes-long";

    fn manager() -> JwtManager {
        JwtManager::new(SECRET, 24)
    }

    // --- generate_token / validate_token ---

    #[test]
    fn test_jwt_generate_and_validate() {
        let mgr = manager();
        let token = mgr
            .generate_token(1, "admin", "admin@example.com", "administrator")
            .unwrap();
        let claims = mgr.validate_token(&token).unwrap();
        assert_eq!(claims.sub, 1);
        assert_eq!(claims.login, "admin");
        assert_eq!(claims.role, "administrator");
    }

    #[test]
    fn test_jwt_email_preserved() {
        let mgr = manager();
        let token = mgr
            .generate_token(42, "user", "user@example.com", "editor")
            .unwrap();
        let claims = mgr.validate_token(&token).unwrap();
        assert_eq!(claims.email, "user@example.com");
    }

    #[test]
    fn test_jwt_user_id_preserved() {
        let mgr = manager();
        let token = mgr
            .generate_token(9999, "u", "u@u.com", "subscriber")
            .unwrap();
        let claims = mgr.validate_token(&token).unwrap();
        assert_eq!(claims.sub, 9999);
    }

    #[test]
    fn test_jwt_jti_is_uuid() {
        let mgr = manager();
        let token = mgr
            .generate_token(1, "a", "a@a.com", "administrator")
            .unwrap();
        let claims = mgr.validate_token(&token).unwrap();
        // UUIDs are 36 chars: 8-4-4-4-12
        assert_eq!(claims.jti.len(), 36);
        assert!(claims.jti.contains('-'));
    }

    #[test]
    fn test_jwt_two_tokens_different_jti() {
        let mgr = manager();
        let t1 = mgr
            .generate_token(1, "a", "a@a.com", "administrator")
            .unwrap();
        let t2 = mgr
            .generate_token(1, "a", "a@a.com", "administrator")
            .unwrap();
        let c1 = mgr.validate_token(&t1).unwrap();
        let c2 = mgr.validate_token(&t2).unwrap();
        assert_ne!(c1.jti, c2.jti);
    }

    #[test]
    fn test_jwt_wrong_secret_rejected() {
        let mgr = manager();
        let token = mgr
            .generate_token(1, "admin", "a@a.com", "administrator")
            .unwrap();
        let other = JwtManager::new("totally-different-secret-that-is-also-long", 24);
        assert!(other.validate_token(&token).is_err());
    }

    #[test]
    fn test_jwt_tampered_token_rejected() {
        let mgr = manager();
        let token = mgr
            .generate_token(1, "admin", "a@a.com", "administrator")
            .unwrap();
        let tampered = token[..token.len() - 3].to_string() + "xxx";
        assert!(mgr.validate_token(&tampered).is_err());
    }

    #[test]
    fn test_jwt_garbage_input_rejected() {
        let mgr = manager();
        assert!(mgr.validate_token("not.a.token").is_err());
        assert!(mgr.validate_token("").is_err());
    }

    #[test]
    fn test_jwt_iat_set() {
        let before = Utc::now().timestamp();
        let mgr = manager();
        let token = mgr
            .generate_token(1, "a", "a@a.com", "administrator")
            .unwrap();
        let after = Utc::now().timestamp();
        let claims = mgr.validate_token(&token).unwrap();
        assert!(claims.iat >= before);
        assert!(claims.iat <= after);
    }

    #[test]
    fn test_jwt_exp_is_24h_after_iat() {
        let mgr = manager();
        let token = mgr
            .generate_token(1, "a", "a@a.com", "administrator")
            .unwrap();
        let claims = mgr.validate_token(&token).unwrap();
        let diff = claims.exp - claims.iat;
        // Allow ±2 seconds clock wiggle
        assert!(diff >= 86398 && diff <= 86402, "diff={diff}");
    }

    // --- refresh_token ---

    #[test]
    fn test_jwt_refresh_preserves_user_id() {
        let mgr = manager();
        let token = mgr.generate_token(7, "bob", "bob@b.com", "author").unwrap();
        let claims = mgr.validate_token(&token).unwrap();
        let refreshed = mgr.refresh_token(&claims).unwrap();
        let new_claims = mgr.validate_token(&refreshed).unwrap();
        assert_eq!(new_claims.sub, 7);
        assert_eq!(new_claims.login, "bob");
        assert_eq!(new_claims.role, "author");
    }

    #[test]
    fn test_jwt_refresh_issues_new_jti() {
        let mgr = manager();
        let token = mgr.generate_token(7, "bob", "bob@b.com", "author").unwrap();
        let claims = mgr.validate_token(&token).unwrap();
        let orig_jti = claims.jti.clone();
        let refreshed = mgr.refresh_token(&claims).unwrap();
        let new_claims = mgr.validate_token(&refreshed).unwrap();
        assert_ne!(orig_jti, new_claims.jti);
    }

    // --- generate_pending_token / validate_pending_token ---

    #[test]
    fn test_pending_token_round_trip() {
        let mgr = manager();
        let token = mgr.generate_pending_token(5, "alice").unwrap();
        let (uid, login) = mgr.validate_pending_token(&token).unwrap();
        assert_eq!(uid, 5);
        assert_eq!(login, "alice");
    }

    #[test]
    fn test_pending_token_role_is_2fa_pending() {
        let mgr = manager();
        let token = mgr.generate_pending_token(5, "alice").unwrap();
        // Decode without role check to inspect claims
        let validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::HS256);
        let data: jsonwebtoken::TokenData<Claims> = jsonwebtoken::decode(
            &token,
            &jsonwebtoken::DecodingKey::from_secret(SECRET.as_bytes()),
            &validation,
        )
        .unwrap();
        assert_eq!(data.claims.role, "2fa_pending");
    }

    #[test]
    fn test_pending_token_rejects_full_token() {
        let mgr = manager();
        // A full session token (role="administrator") must NOT pass validate_pending_token
        let full_token = mgr
            .generate_token(1, "admin", "a@a.com", "administrator")
            .unwrap();
        assert!(mgr.validate_pending_token(&full_token).is_err());
    }

    #[test]
    fn test_pending_token_wrong_secret_rejected() {
        let mgr = manager();
        let token = mgr.generate_pending_token(5, "alice").unwrap();
        let other = JwtManager::new("another-secret-at-least-32-bytes-long-here", 24);
        assert!(other.validate_pending_token(&token).is_err());
    }

    #[test]
    fn test_pending_token_garbage_rejected() {
        let mgr = manager();
        assert!(mgr.validate_pending_token("bad.token.here").is_err());
    }

    // --- JwtManager::new panics on short secret ---

    #[test]
    #[should_panic(expected = "JWT secret must be at least 32 bytes")]
    fn test_jwt_manager_panics_on_short_secret() {
        JwtManager::new("tooshort", 24);
    }

    // --- Blacklist integration ---

    #[test]
    fn test_blacklisted_token_rejected() {
        let mgr = manager();
        let token = mgr
            .generate_token(1, "admin", "a@a.com", "administrator")
            .unwrap();
        let claims = mgr.validate_token(&token).unwrap();
        // Blacklist the jti
        crate::jwt_blacklist::blacklist_token(&claims.jti);
        // Should now be rejected
        assert!(mgr.validate_token(&token).is_err());
    }
}
