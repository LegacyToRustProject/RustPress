use moka::future::Cache;
use std::time::Duration;
use tracing::debug;

/// WordPress-compatible object cache.
///
/// Caches arbitrary key-value pairs with optional groups and TTL.
/// Uses moka for high-performance concurrent caching.
#[derive(Clone)]
pub struct ObjectCache {
    cache: Cache<String, String>,
}

impl ObjectCache {
    pub fn new(max_capacity: u64, ttl_seconds: u64) -> Self {
        let cache = Cache::builder()
            .max_capacity(max_capacity)
            .time_to_live(Duration::from_secs(ttl_seconds))
            .build();

        Self { cache }
    }

    /// Build a cache key from group and key.
    fn cache_key(group: &str, key: &str) -> String {
        format!("{}:{}", group, key)
    }

    /// Get a value from the cache.
    pub async fn get(&self, key: &str, group: &str) -> Option<String> {
        let cache_key = Self::cache_key(group, key);
        let result = self.cache.get(&cache_key).await;
        if result.is_some() {
            debug!(key, group, "cache hit");
        }
        result
    }

    /// Set a value in the cache.
    pub async fn set(&self, key: &str, value: &str, group: &str) {
        let cache_key = Self::cache_key(group, key);
        self.cache.insert(cache_key, value.to_string()).await;
        debug!(key, group, "cache set");
    }

    /// Delete a value from the cache.
    pub async fn delete(&self, key: &str, group: &str) {
        let cache_key = Self::cache_key(group, key);
        self.cache.invalidate(&cache_key).await;
    }

    /// Flush the entire cache.
    pub async fn flush(&self) {
        self.cache.invalidate_all();
        debug!("cache flushed");
    }

    /// Get the number of entries in the cache.
    pub fn entry_count(&self) -> u64 {
        self.cache.entry_count()
    }

    /// WordPress-compatible wp_cache_get.
    pub async fn wp_cache_get(&self, key: &str, group: &str) -> Option<String> {
        self.get(key, if group.is_empty() { "default" } else { group })
            .await
    }

    /// WordPress-compatible wp_cache_set.
    pub async fn wp_cache_set(&self, key: &str, value: &str, group: &str) {
        self.set(key, value, if group.is_empty() { "default" } else { group })
            .await;
    }

    /// WordPress-compatible wp_cache_delete.
    pub async fn wp_cache_delete(&self, key: &str, group: &str) {
        self.delete(key, if group.is_empty() { "default" } else { group })
            .await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_set_and_get() {
        let cache = ObjectCache::new(100, 3600);
        cache.set("key1", "value1", "default").await;

        let result = cache.get("key1", "default").await;
        assert_eq!(result, Some("value1".to_string()));
    }

    #[tokio::test]
    async fn test_get_nonexistent() {
        let cache = ObjectCache::new(100, 3600);
        assert!(cache.get("missing", "default").await.is_none());
    }

    #[tokio::test]
    async fn test_delete() {
        let cache = ObjectCache::new(100, 3600);
        cache.set("key1", "value1", "default").await;
        cache.delete("key1", "default").await;

        assert!(cache.get("key1", "default").await.is_none());
    }

    #[tokio::test]
    async fn test_groups_are_isolated() {
        let cache = ObjectCache::new(100, 3600);
        cache.set("key1", "value_a", "group_a").await;
        cache.set("key1", "value_b", "group_b").await;

        assert_eq!(
            cache.get("key1", "group_a").await,
            Some("value_a".to_string())
        );
        assert_eq!(
            cache.get("key1", "group_b").await,
            Some("value_b".to_string())
        );
    }

    #[tokio::test]
    async fn test_flush() {
        let cache = ObjectCache::new(100, 3600);
        cache.set("k1", "v1", "default").await;
        cache.set("k2", "v2", "default").await;
        cache.flush().await;

        assert!(cache.get("k1", "default").await.is_none());
        assert!(cache.get("k2", "default").await.is_none());
    }

    #[tokio::test]
    async fn test_wp_cache_api() {
        let cache = ObjectCache::new(100, 3600);
        cache.wp_cache_set("option", "hello", "").await;

        let result = cache.wp_cache_get("option", "").await;
        assert_eq!(result, Some("hello".to_string()));

        cache.wp_cache_delete("option", "").await;
        assert!(cache.wp_cache_get("option", "").await.is_none());
    }
}
