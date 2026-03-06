use moka::future::Cache;
use tracing::debug;

/// WordPress-compatible transients cache.
///
/// Transients are short-lived cached values with explicit expiration.
/// Used for caching API responses, computed values, etc.
#[derive(Clone)]
pub struct TransientCache {
    cache: Cache<String, String>,
}

impl TransientCache {
    /// Create a new transient cache with maximum capacity.
    pub fn new(max_capacity: u64) -> Self {
        let cache = Cache::builder()
            .max_capacity(max_capacity)
            .build();

        Self { cache }
    }

    /// Set a transient with expiration (in seconds).
    /// Equivalent to WordPress set_transient().
    pub async fn set_transient(&self, key: &str, value: &str, expiration: u64) {
        // For per-key TTL, we store the expiration with the value
        // moka doesn't support per-entry TTL directly in this version,
        // so we use a wrapper approach
        let cache_entry = format!("{}:{}", expiration_timestamp(expiration), value);
        self.cache.insert(key.to_string(), cache_entry).await;
        debug!(key, expiration, "transient set");
    }

    /// Get a transient value.
    /// Returns None if the transient doesn't exist or has expired.
    /// Equivalent to WordPress get_transient().
    pub async fn get_transient(&self, key: &str) -> Option<String> {
        let entry = self.cache.get(&key.to_string()).await?;

        // Parse the stored format "timestamp:value"
        let (ts_str, value) = entry.split_once(':')?;
        let expires_at: u64 = ts_str.parse().ok()?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        if expires_at > 0 && now > expires_at {
            // Expired - remove it
            self.cache.invalidate(&key.to_string()).await;
            debug!(key, "transient expired");
            return None;
        }

        debug!(key, "transient hit");
        Some(value.to_string())
    }

    /// Delete a transient.
    /// Equivalent to WordPress delete_transient().
    pub async fn delete_transient(&self, key: &str) {
        self.cache.invalidate(&key.to_string()).await;
    }
}

/// Calculate expiration timestamp.
fn expiration_timestamp(seconds: u64) -> u64 {
    if seconds == 0 {
        return 0; // Never expires
    }
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + seconds
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_set_and_get_transient() {
        let cache = TransientCache::new(100);
        cache.set_transient("api_cache", "response_data", 3600).await;

        let result = cache.get_transient("api_cache").await;
        assert_eq!(result, Some("response_data".to_string()));
    }

    #[tokio::test]
    async fn test_nonexistent_transient() {
        let cache = TransientCache::new(100);
        assert!(cache.get_transient("missing").await.is_none());
    }

    #[tokio::test]
    async fn test_delete_transient() {
        let cache = TransientCache::new(100);
        cache.set_transient("temp", "data", 3600).await;
        cache.delete_transient("temp").await;

        assert!(cache.get_transient("temp").await.is_none());
    }

    #[tokio::test]
    async fn test_expired_transient() {
        let cache = TransientCache::new(100);
        // Set with 1-second expiration
        cache.set_transient("short_lived", "data", 1).await;

        // Wait for expiration
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        assert!(cache.get_transient("short_lived").await.is_none());
    }

    #[tokio::test]
    async fn test_never_expires_transient() {
        let cache = TransientCache::new(100);
        cache.set_transient("permanent", "data", 0).await;

        let result = cache.get_transient("permanent").await;
        assert_eq!(result, Some("data".to_string()));
    }

    #[test]
    fn test_expiration_timestamp() {
        let ts = expiration_timestamp(3600);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        assert!(ts > now);
        assert!(ts <= now + 3600);

        assert_eq!(expiration_timestamp(0), 0);
    }
}
