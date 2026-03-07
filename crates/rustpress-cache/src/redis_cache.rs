use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
#[cfg(feature = "redis-backend")]
use tracing::warn;
use tracing::{debug, info};

/// Redis-compatible cache backend.
///
/// When the `redis-backend` feature is enabled and a real Redis connection is
/// available, this delegates to Redis. Otherwise, falls back to an in-memory
/// implementation with the same API.
///
/// This allows RustPress to work with or without Redis installed.
#[derive(Clone)]
pub struct RedisCache {
    /// In-memory fallback store
    store: Arc<RwLock<HashMap<String, CacheEntry>>>,
    /// Redis connection URL (if configured)
    redis_url: Option<String>,
    /// Whether Redis is connected
    connected: Arc<RwLock<bool>>,
    /// Real Redis client (only with redis-backend feature)
    #[cfg(feature = "redis-backend")]
    redis_client: Arc<RwLock<Option<redis::Client>>>,
    /// Real Redis multiplexed connection (only with redis-backend feature)
    #[cfg(feature = "redis-backend")]
    redis_conn: Arc<RwLock<Option<redis::aio::MultiplexedConnection>>>,
}

#[derive(Clone, Debug)]
struct CacheEntry {
    value: String,
    expires_at: Option<u64>,
}

impl CacheEntry {
    fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            now > expires_at
        } else {
            false
        }
    }
}

#[cfg(feature = "redis-backend")]
impl RedisCache {
    /// Try to get a usable Redis connection, returning None if unavailable.
    async fn get_redis_conn(&self) -> Option<redis::aio::MultiplexedConnection> {
        let guard = self.redis_conn.read().await;
        guard.clone()
    }
}

impl RedisCache {
    /// Create a new Redis cache with optional Redis URL.
    ///
    /// If `redis_url` is None, uses in-memory fallback only.
    pub fn new(redis_url: Option<String>) -> Self {
        if let Some(ref url) = redis_url {
            info!(url = %url, "Redis cache configured (in-memory fallback active)");
        } else {
            info!("Redis cache using in-memory fallback");
        }

        Self {
            store: Arc::new(RwLock::new(HashMap::new())),
            redis_url,
            connected: Arc::new(RwLock::new(false)),
            #[cfg(feature = "redis-backend")]
            redis_client: Arc::new(RwLock::new(None)),
            #[cfg(feature = "redis-backend")]
            redis_conn: Arc::new(RwLock::new(None)),
        }
    }

    /// Attempt to connect to Redis.
    pub async fn connect(&self) -> Result<(), String> {
        if let Some(ref url) = self.redis_url {
            if !(url.starts_with("redis://") || url.starts_with("rediss://")) {
                return Err(format!("Invalid Redis URL: {url}"));
            }

            #[cfg(feature = "redis-backend")]
            {
                match redis::Client::open(url.as_str()) {
                    Ok(client) => {
                        match client.get_multiplexed_tokio_connection().await {
                            Ok(conn) => {
                                // Store the client and connection
                                {
                                    let mut client_guard = self.redis_client.write().await;
                                    *client_guard = Some(client);
                                }
                                {
                                    let mut conn_guard = self.redis_conn.write().await;
                                    *conn_guard = Some(conn);
                                }
                                let mut connected = self.connected.write().await;
                                *connected = true;
                                info!("Redis cache connected (real Redis)");
                                return Ok(());
                            }
                            Err(e) => {
                                warn!(
                                    "Failed to connect to Redis, using in-memory fallback: {}",
                                    e
                                );
                                let mut connected = self.connected.write().await;
                                *connected = true;
                                info!("Redis cache connected (in-memory mode)");
                                return Ok(());
                            }
                        }
                    }
                    Err(e) => {
                        warn!(
                            "Failed to create Redis client, using in-memory fallback: {}",
                            e
                        );
                        let mut connected = self.connected.write().await;
                        *connected = true;
                        info!("Redis cache connected (in-memory mode)");
                        return Ok(());
                    }
                }
            }

            #[cfg(not(feature = "redis-backend"))]
            {
                let mut connected = self.connected.write().await;
                *connected = true;
                info!("Redis cache connected (in-memory mode)");
                Ok(())
            }
        } else {
            Ok(())
        }
    }

    /// Check if Redis is connected.
    pub async fn is_connected(&self) -> bool {
        *self.connected.read().await
    }

    /// Get a value from the cache.
    pub async fn get(&self, key: &str) -> Option<String> {
        // Try real Redis first when feature is enabled
        #[cfg(feature = "redis-backend")]
        {
            if let Some(mut conn) = self.get_redis_conn().await {
                match redis::cmd("GET")
                    .arg(key)
                    .query_async::<Option<String>>(&mut conn)
                    .await
                {
                    Ok(result) => {
                        if result.is_some() {
                            debug!(key, "redis cache hit (real Redis)");
                        } else {
                            debug!(key, "redis cache miss (real Redis)");
                        }
                        return result;
                    }
                    Err(e) => {
                        warn!(key, error = %e, "Redis GET failed, falling back to in-memory");
                    }
                }
            }
        }

        // In-memory fallback
        let store = self.store.read().await;
        if let Some(entry) = store.get(key) {
            if entry.is_expired() {
                drop(store);
                self.del(key).await;
                debug!(key, "redis cache miss (expired)");
                return None;
            }
            debug!(key, "redis cache hit");
            Some(entry.value.clone())
        } else {
            debug!(key, "redis cache miss");
            None
        }
    }

    /// Set a value in the cache with optional TTL (seconds).
    pub async fn set(&self, key: &str, value: &str, ttl: Option<u64>) {
        // Try real Redis first when feature is enabled
        #[cfg(feature = "redis-backend")]
        {
            if let Some(mut conn) = self.get_redis_conn().await {
                let result = if let Some(seconds) = ttl {
                    redis::cmd("SETEX")
                        .arg(key)
                        .arg(seconds)
                        .arg(value)
                        .query_async::<()>(&mut conn)
                        .await
                } else {
                    redis::cmd("SET")
                        .arg(key)
                        .arg(value)
                        .query_async::<()>(&mut conn)
                        .await
                };
                match result {
                    Ok(()) => {
                        debug!(key, ttl = ?ttl, "redis cache set (real Redis)");
                        return;
                    }
                    Err(e) => {
                        warn!(key, error = %e, "Redis SET failed, falling back to in-memory");
                    }
                }
            }
        }

        // In-memory fallback
        let expires_at = ttl.map(|t| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
                + t
        });

        let mut store = self.store.write().await;
        store.insert(
            key.to_string(),
            CacheEntry {
                value: value.to_string(),
                expires_at,
            },
        );
        debug!(key, ttl = ?ttl, "redis cache set");
    }

    /// Set a value with expiration (alias for set with TTL).
    pub async fn setex(&self, key: &str, seconds: u64, value: &str) {
        self.set(key, value, Some(seconds)).await;
    }

    /// Delete a key from the cache.
    pub async fn del(&self, key: &str) -> bool {
        // Try real Redis first when feature is enabled
        #[cfg(feature = "redis-backend")]
        {
            if let Some(mut conn) = self.get_redis_conn().await {
                match redis::cmd("DEL")
                    .arg(key)
                    .query_async::<i64>(&mut conn)
                    .await
                {
                    Ok(count) => {
                        debug!(key, "redis cache del (real Redis)");
                        return count > 0;
                    }
                    Err(e) => {
                        warn!(key, error = %e, "Redis DEL failed, falling back to in-memory");
                    }
                }
            }
        }

        // In-memory fallback
        let mut store = self.store.write().await;
        store.remove(key).is_some()
    }

    /// Check if a key exists.
    pub async fn exists(&self, key: &str) -> bool {
        // Try real Redis first when feature is enabled
        #[cfg(feature = "redis-backend")]
        {
            if let Some(mut conn) = self.get_redis_conn().await {
                match redis::cmd("EXISTS")
                    .arg(key)
                    .query_async::<i64>(&mut conn)
                    .await
                {
                    Ok(count) => {
                        return count > 0;
                    }
                    Err(e) => {
                        warn!(key, error = %e, "Redis EXISTS failed, falling back to in-memory");
                    }
                }
            }
        }

        // In-memory fallback
        let store = self.store.read().await;
        if let Some(entry) = store.get(key) {
            !entry.is_expired()
        } else {
            false
        }
    }

    /// Set expiration on an existing key (seconds).
    pub async fn expire(&self, key: &str, seconds: u64) -> bool {
        // Try real Redis first when feature is enabled
        #[cfg(feature = "redis-backend")]
        {
            if let Some(mut conn) = self.get_redis_conn().await {
                match redis::cmd("EXPIRE")
                    .arg(key)
                    .arg(seconds)
                    .query_async::<i64>(&mut conn)
                    .await
                {
                    Ok(result) => {
                        return result > 0;
                    }
                    Err(e) => {
                        warn!(key, error = %e, "Redis EXPIRE failed, falling back to in-memory");
                    }
                }
            }
        }

        // In-memory fallback
        let mut store = self.store.write().await;
        if let Some(entry) = store.get_mut(key) {
            entry.expires_at = Some(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
                    + seconds,
            );
            true
        } else {
            false
        }
    }

    /// Get remaining TTL for a key (returns None if key doesn't exist or has no expiry).
    pub async fn ttl(&self, key: &str) -> Option<i64> {
        // Try real Redis first when feature is enabled
        #[cfg(feature = "redis-backend")]
        {
            if let Some(mut conn) = self.get_redis_conn().await {
                match redis::cmd("TTL")
                    .arg(key)
                    .query_async::<i64>(&mut conn)
                    .await
                {
                    Ok(ttl_val) => {
                        if ttl_val == -2 {
                            return None; // Key doesn't exist
                        }
                        return Some(ttl_val);
                    }
                    Err(e) => {
                        warn!(key, error = %e, "Redis TTL failed, falling back to in-memory");
                    }
                }
            }
        }

        // In-memory fallback
        let store = self.store.read().await;
        if let Some(entry) = store.get(key) {
            if let Some(expires_at) = entry.expires_at {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                Some(expires_at as i64 - now as i64)
            } else {
                Some(-1) // No expiry
            }
        } else {
            None // Key doesn't exist
        }
    }

    /// Increment a numeric value.
    pub async fn incr(&self, key: &str) -> i64 {
        // Try real Redis first when feature is enabled
        #[cfg(feature = "redis-backend")]
        {
            if let Some(mut conn) = self.get_redis_conn().await {
                match redis::cmd("INCR")
                    .arg(key)
                    .query_async::<i64>(&mut conn)
                    .await
                {
                    Ok(val) => {
                        return val;
                    }
                    Err(e) => {
                        warn!(key, error = %e, "Redis INCR failed, falling back to in-memory");
                    }
                }
            }
        }

        // In-memory fallback
        let mut store = self.store.write().await;
        let entry = store.entry(key.to_string()).or_insert(CacheEntry {
            value: "0".to_string(),
            expires_at: None,
        });
        let val: i64 = entry.value.parse().unwrap_or(0) + 1;
        entry.value = val.to_string();
        val
    }

    /// Decrement a numeric value.
    pub async fn decr(&self, key: &str) -> i64 {
        // Try real Redis first when feature is enabled
        #[cfg(feature = "redis-backend")]
        {
            if let Some(mut conn) = self.get_redis_conn().await {
                match redis::cmd("DECR")
                    .arg(key)
                    .query_async::<i64>(&mut conn)
                    .await
                {
                    Ok(val) => {
                        return val;
                    }
                    Err(e) => {
                        warn!(key, error = %e, "Redis DECR failed, falling back to in-memory");
                    }
                }
            }
        }

        // In-memory fallback
        let mut store = self.store.write().await;
        let entry = store.entry(key.to_string()).or_insert(CacheEntry {
            value: "0".to_string(),
            expires_at: None,
        });
        let val: i64 = entry.value.parse().unwrap_or(0) - 1;
        entry.value = val.to_string();
        val
    }

    /// Hash set: set a field in a hash.
    pub async fn hset(&self, key: &str, field: &str, value: &str) {
        // Try real Redis first when feature is enabled
        #[cfg(feature = "redis-backend")]
        {
            if let Some(mut conn) = self.get_redis_conn().await {
                match redis::cmd("HSET")
                    .arg(key)
                    .arg(field)
                    .arg(value)
                    .query_async::<i64>(&mut conn)
                    .await
                {
                    Ok(_) => {
                        debug!(key, field, "redis hash set (real Redis)");
                        return;
                    }
                    Err(e) => {
                        warn!(key, field, error = %e, "Redis HSET failed, falling back to in-memory");
                    }
                }
            }
        }

        // In-memory fallback
        let hash_key = format!("{key}:{field}");
        self.set(&hash_key, value, None).await;
    }

    /// Hash get: get a field from a hash.
    pub async fn hget(&self, key: &str, field: &str) -> Option<String> {
        // Try real Redis first when feature is enabled
        #[cfg(feature = "redis-backend")]
        {
            if let Some(mut conn) = self.get_redis_conn().await {
                match redis::cmd("HGET")
                    .arg(key)
                    .arg(field)
                    .query_async::<Option<String>>(&mut conn)
                    .await
                {
                    Ok(result) => {
                        return result;
                    }
                    Err(e) => {
                        warn!(key, field, error = %e, "Redis HGET failed, falling back to in-memory");
                    }
                }
            }
        }

        // In-memory fallback
        let hash_key = format!("{key}:{field}");
        self.get(&hash_key).await
    }

    /// Hash delete: delete a field from a hash.
    pub async fn hdel(&self, key: &str, field: &str) -> bool {
        // Try real Redis first when feature is enabled
        #[cfg(feature = "redis-backend")]
        {
            if let Some(mut conn) = self.get_redis_conn().await {
                match redis::cmd("HDEL")
                    .arg(key)
                    .arg(field)
                    .query_async::<i64>(&mut conn)
                    .await
                {
                    Ok(count) => {
                        return count > 0;
                    }
                    Err(e) => {
                        warn!(key, field, error = %e, "Redis HDEL failed, falling back to in-memory");
                    }
                }
            }
        }

        // In-memory fallback
        let hash_key = format!("{key}:{field}");
        self.del(&hash_key).await
    }

    /// Flush all keys from the cache.
    pub async fn flushall(&self) {
        // Try real Redis first when feature is enabled
        #[cfg(feature = "redis-backend")]
        {
            if let Some(mut conn) = self.get_redis_conn().await {
                match redis::cmd("FLUSHALL").query_async::<()>(&mut conn).await {
                    Ok(()) => {
                        info!("redis cache flushed (real Redis)");
                        return;
                    }
                    Err(e) => {
                        warn!(error = %e, "Redis FLUSHALL failed, falling back to in-memory");
                    }
                }
            }
        }

        // In-memory fallback
        let mut store = self.store.write().await;
        store.clear();
        info!("redis cache flushed");
    }

    /// Get the number of keys in the cache.
    pub async fn dbsize(&self) -> usize {
        // Try real Redis first when feature is enabled
        #[cfg(feature = "redis-backend")]
        {
            if let Some(mut conn) = self.get_redis_conn().await {
                match redis::cmd("DBSIZE").query_async::<usize>(&mut conn).await {
                    Ok(size) => {
                        return size;
                    }
                    Err(e) => {
                        warn!(error = %e, "Redis DBSIZE failed, falling back to in-memory");
                    }
                }
            }
        }

        // In-memory fallback
        let store = self.store.read().await;
        store.len()
    }

    /// Get keys matching a pattern (simple glob-style: * only).
    pub async fn keys(&self, pattern: &str) -> Vec<String> {
        // Try real Redis first when feature is enabled
        #[cfg(feature = "redis-backend")]
        {
            if let Some(mut conn) = self.get_redis_conn().await {
                match redis::cmd("KEYS")
                    .arg(pattern)
                    .query_async::<Vec<String>>(&mut conn)
                    .await
                {
                    Ok(keys) => {
                        return keys;
                    }
                    Err(e) => {
                        warn!(pattern, error = %e, "Redis KEYS failed, falling back to in-memory");
                    }
                }
            }
        }

        // In-memory fallback
        let store = self.store.read().await;
        if pattern == "*" {
            return store.keys().cloned().collect();
        }

        let prefix = pattern.trim_end_matches('*');
        store
            .keys()
            .filter(|k| k.starts_with(prefix))
            .cloned()
            .collect()
    }

    /// Get cache info/stats.
    pub async fn info(&self) -> String {
        // Try real Redis first when feature is enabled
        #[cfg(feature = "redis-backend")]
        {
            if let Some(mut conn) = self.get_redis_conn().await {
                match redis::cmd("INFO").query_async::<String>(&mut conn).await {
                    Ok(info) => {
                        return info;
                    }
                    Err(e) => {
                        warn!(error = %e, "Redis INFO failed, falling back to in-memory");
                    }
                }
            }
        }

        // In-memory fallback
        let store = self.store.read().await;
        let connected = *self.connected.read().await;
        format!(
            "# Cache Info\r\nredis_url:{}\r\nconnected:{}\r\nkeys:{}\r\nmode:in-memory\r\n",
            self.redis_url.as_deref().unwrap_or("none"),
            connected,
            store.len()
        )
    }

    /// Clean up expired entries.
    pub async fn cleanup_expired(&self) {
        // Real Redis handles expiration natively, so only clean up in-memory store
        let mut store = self.store.write().await;
        let expired_keys: Vec<String> = store
            .iter()
            .filter(|(_, v)| v.is_expired())
            .map(|(k, _)| k.clone())
            .collect();

        let count = expired_keys.len();
        for key in expired_keys {
            store.remove(&key);
        }

        if count > 0 {
            debug!(count, "cleaned up expired cache entries");
        }
    }
}

impl Default for RedisCache {
    fn default() -> Self {
        Self::new(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_set_and_get() {
        let cache = RedisCache::new(None);
        cache.set("key1", "value1", None).await;
        assert_eq!(cache.get("key1").await, Some("value1".to_string()));
    }

    #[tokio::test]
    async fn test_get_nonexistent() {
        let cache = RedisCache::new(None);
        assert!(cache.get("missing").await.is_none());
    }

    #[tokio::test]
    async fn test_del() {
        let cache = RedisCache::new(None);
        cache.set("key1", "value1", None).await;
        assert!(cache.del("key1").await);
        assert!(cache.get("key1").await.is_none());
    }

    #[tokio::test]
    async fn test_exists() {
        let cache = RedisCache::new(None);
        cache.set("key1", "value1", None).await;
        assert!(cache.exists("key1").await);
        assert!(!cache.exists("missing").await);
    }

    #[tokio::test]
    async fn test_setex_with_ttl() {
        let cache = RedisCache::new(None);
        cache.setex("temp", 3600, "data").await;
        assert_eq!(cache.get("temp").await, Some("data".to_string()));
        let ttl = cache.ttl("temp").await.unwrap();
        assert!(ttl > 3598 && ttl <= 3600);
    }

    #[tokio::test]
    async fn test_incr_decr() {
        let cache = RedisCache::new(None);
        assert_eq!(cache.incr("counter").await, 1);
        assert_eq!(cache.incr("counter").await, 2);
        assert_eq!(cache.incr("counter").await, 3);
        assert_eq!(cache.decr("counter").await, 2);
    }

    #[tokio::test]
    async fn test_hash_operations() {
        let cache = RedisCache::new(None);
        cache.hset("user:1", "name", "Alice").await;
        cache.hset("user:1", "email", "alice@example.com").await;

        assert_eq!(
            cache.hget("user:1", "name").await,
            Some("Alice".to_string())
        );
        assert_eq!(
            cache.hget("user:1", "email").await,
            Some("alice@example.com".to_string())
        );

        cache.hdel("user:1", "email").await;
        assert!(cache.hget("user:1", "email").await.is_none());
    }

    #[tokio::test]
    async fn test_flushall() {
        let cache = RedisCache::new(None);
        cache.set("a", "1", None).await;
        cache.set("b", "2", None).await;
        assert_eq!(cache.dbsize().await, 2);

        cache.flushall().await;
        assert_eq!(cache.dbsize().await, 0);
    }

    #[tokio::test]
    async fn test_keys_pattern() {
        let cache = RedisCache::new(None);
        cache.set("user:1", "a", None).await;
        cache.set("user:2", "b", None).await;
        cache.set("post:1", "c", None).await;

        let user_keys = cache.keys("user:*").await;
        assert_eq!(user_keys.len(), 2);

        let all_keys = cache.keys("*").await;
        assert_eq!(all_keys.len(), 3);
    }

    #[tokio::test]
    async fn test_connect_with_url() {
        let cache = RedisCache::new(Some("redis://localhost:6379".to_string()));
        // Without a real Redis server, this should still succeed
        // (falls back to in-memory mode)
        assert!(cache.connect().await.is_ok());
        assert!(cache.is_connected().await);
    }

    #[tokio::test]
    async fn test_connect_invalid_url() {
        let cache = RedisCache::new(Some("invalid://url".to_string()));
        assert!(cache.connect().await.is_err());
    }

    #[tokio::test]
    async fn test_expired_entry() {
        let cache = RedisCache::new(None);
        cache.set("short", "data", Some(1)).await;
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        assert!(cache.get("short").await.is_none());
    }

    #[tokio::test]
    async fn test_cleanup_expired() {
        let cache = RedisCache::new(None);
        cache.set("a", "1", Some(1)).await;
        cache.set("b", "2", None).await;
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        cache.cleanup_expired().await;
        assert_eq!(cache.dbsize().await, 1);
    }
}
