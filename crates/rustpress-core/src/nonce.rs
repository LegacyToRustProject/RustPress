use std::time::{SystemTime, UNIX_EPOCH};

/// WordPress-compatible nonce (number used once) system.
///
/// Corresponds to `wp_create_nonce()` / `wp_verify_nonce()` in
/// `wp-includes/pluggable.php`.
///
/// WordPress nonces are time-based tokens valid for 24 hours (two 12-hour ticks).
/// They are NOT truly single-use — they can be reused within their validity window.
pub struct NonceManager {
    secret: String,
}

impl NonceManager {
    pub fn new(secret: &str) -> Self {
        Self {
            secret: secret.to_string(),
        }
    }

    /// Create a nonce for the given action and user.
    ///
    /// Equivalent to WordPress `wp_create_nonce($action)`.
    ///
    /// The nonce is valid for 24 hours (two 12-hour ticks),
    /// matching WordPress behavior.
    pub fn create_nonce(&self, action: &str, user_id: u64) -> String {
        let tick = self.nonce_tick();
        self.hash_nonce(tick, action, user_id)
    }

    /// Verify a nonce for the given action and user.
    ///
    /// Equivalent to WordPress `wp_verify_nonce($nonce, $action)`.
    ///
    /// Returns:
    /// - `Some(1)` if valid in the current 12-hour tick
    /// - `Some(2)` if valid from the previous 12-hour tick
    /// - `None` if invalid
    pub fn verify_nonce(&self, nonce: &str, action: &str, user_id: u64) -> Option<u8> {
        let tick = self.nonce_tick();

        // Check current tick
        let expected = self.hash_nonce(tick, action, user_id);
        if constant_time_eq(nonce, &expected) {
            return Some(1);
        }

        // Check previous tick
        let expected_prev = self.hash_nonce(tick - 1.0, action, user_id);
        if constant_time_eq(nonce, &expected_prev) {
            return Some(2);
        }

        None
    }

    /// Get the current nonce tick (changes every 12 hours, matching WordPress).
    ///
    /// Equivalent to WordPress `wp_nonce_tick()`.
    fn nonce_tick(&self) -> f64 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as f64;
        // WordPress uses DAY_IN_SECONDS / 2 = 43200 seconds = 12 hours
        (now / 43200.0).ceil()
    }

    /// Hash the nonce components.
    fn hash_nonce(&self, tick: f64, action: &str, user_id: u64) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let input = format!("{}|{}|{}|{}", tick as u64, action, user_id, self.secret);
        let mut hasher = DefaultHasher::new();
        input.hash(&mut hasher);
        let hash = hasher.finish();
        format!("{:016x}", hash)[..10].to_string()
    }
}

/// Constant-time string comparison to prevent timing attacks.
fn constant_time_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut result = 0u8;
    for (x, y) in a.bytes().zip(b.bytes()) {
        result |= x ^ y;
    }
    result == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_nonce() {
        let manager = NonceManager::new("test-secret-key");
        let nonce = manager.create_nonce("save_post", 1);
        assert!(!nonce.is_empty());
        assert_eq!(nonce.len(), 10);
    }

    #[test]
    fn test_verify_nonce_valid() {
        let manager = NonceManager::new("test-secret-key");
        let nonce = manager.create_nonce("delete_post", 42);
        let result = manager.verify_nonce(&nonce, "delete_post", 42);
        assert_eq!(result, Some(1));
    }

    #[test]
    fn test_verify_nonce_wrong_action() {
        let manager = NonceManager::new("test-secret-key");
        let nonce = manager.create_nonce("save_post", 1);
        let result = manager.verify_nonce(&nonce, "delete_post", 1);
        assert_eq!(result, None);
    }

    #[test]
    fn test_verify_nonce_wrong_user() {
        let manager = NonceManager::new("test-secret-key");
        let nonce = manager.create_nonce("save_post", 1);
        let result = manager.verify_nonce(&nonce, "save_post", 2);
        assert_eq!(result, None);
    }

    #[test]
    fn test_verify_nonce_wrong_nonce() {
        let manager = NonceManager::new("test-secret-key");
        let result = manager.verify_nonce("invalidnonce", "save_post", 1);
        assert_eq!(result, None);
    }

    #[test]
    fn test_different_secrets_different_nonces() {
        let m1 = NonceManager::new("secret-1");
        let m2 = NonceManager::new("secret-2");
        let n1 = m1.create_nonce("action", 1);
        let n2 = m2.create_nonce("action", 1);
        assert_ne!(n1, n2);
    }

    #[test]
    fn test_constant_time_eq() {
        assert!(constant_time_eq("abc", "abc"));
        assert!(!constant_time_eq("abc", "abd"));
        assert!(!constant_time_eq("abc", "abcd"));
    }
}
