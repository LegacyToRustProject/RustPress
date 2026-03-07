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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jwt_generate_and_validate() {
        let manager = JwtManager::new("test-secret-key-that-is-at-least-32-bytes-long", 24);
        let token = manager
            .generate_token(1, "admin", "admin@example.com", "administrator")
            .unwrap();
        let claims = manager.validate_token(&token).unwrap();
        assert_eq!(claims.sub, 1);
        assert_eq!(claims.login, "admin");
        assert_eq!(claims.role, "administrator");
    }
}
