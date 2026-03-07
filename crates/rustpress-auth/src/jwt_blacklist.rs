//! JWT token blacklist using Moka in-memory cache.
//!
//! When a user logs out, their JWT's `jti` claim is added to this blacklist.
//! The `validate_token` method checks this before accepting a token.
//! Entries expire automatically after 24 hours (max JWT lifetime).

use moka::sync::Cache;
use std::sync::OnceLock;
use std::time::Duration;

static BLACKLIST: OnceLock<Cache<String, ()>> = OnceLock::new();

fn get_blacklist() -> &'static Cache<String, ()> {
    BLACKLIST.get_or_init(|| {
        Cache::builder()
            .max_capacity(100_000)
            .time_to_live(Duration::from_secs(86400)) // 24h = max JWT lifetime
            .build()
    })
}

/// Add a JWT ID to the blacklist (called on logout).
pub fn blacklist_token(jti: &str) {
    get_blacklist().insert(jti.to_string(), ());
}

/// Check whether a JWT ID is blacklisted.
pub fn is_blacklisted(jti: &str) -> bool {
    get_blacklist().contains_key(jti)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blacklist_and_check() {
        let jti = "test-jti-unique-12345";
        assert!(!is_blacklisted(jti));
        blacklist_token(jti);
        assert!(is_blacklisted(jti));
    }

    #[test]
    fn test_non_blacklisted_token() {
        assert!(!is_blacklisted("some-other-jti-not-added"));
    }

    #[test]
    fn test_empty_jti_not_blacklisted_by_default() {
        assert!(!is_blacklisted(""));
    }

    #[test]
    fn test_blacklist_multiple_jtis() {
        let jti_a = "multi-test-jti-aaa";
        let jti_b = "multi-test-jti-bbb";
        let jti_c = "multi-test-jti-ccc-not-added";
        blacklist_token(jti_a);
        blacklist_token(jti_b);
        assert!(is_blacklisted(jti_a));
        assert!(is_blacklisted(jti_b));
        assert!(!is_blacklisted(jti_c));
    }

    #[test]
    fn test_blacklist_is_idempotent() {
        let jti = "idempotent-jti-test-xyz";
        blacklist_token(jti);
        blacklist_token(jti); // second call should not panic
        assert!(is_blacklisted(jti));
    }

    #[test]
    fn test_blacklist_uuid_style_jti() {
        let jti = "550e8400-e29b-41d4-a716-446655440000";
        assert!(!is_blacklisted(jti));
        blacklist_token(jti);
        assert!(is_blacklisted(jti));
    }

    #[test]
    fn test_is_blacklisted_with_special_chars() {
        let jti = "jti:with-special.chars/here@test";
        assert!(!is_blacklisted(jti));
        blacklist_token(jti);
        assert!(is_blacklisted(jti));
    }
}
