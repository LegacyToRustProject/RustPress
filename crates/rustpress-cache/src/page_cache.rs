use moka::future::Cache;
use std::time::Duration;
use tracing::{debug, info};

/// Full-page cache for rendered HTML output.
///
/// Caches the entire HTML response for a given URL path,
/// dramatically reducing database queries and template rendering.
#[derive(Clone)]
pub struct PageCache {
    cache: Cache<String, CachedPage>,
    enabled: bool,
}

/// A cached page with its rendered HTML and metadata.
#[derive(Clone, Debug)]
pub struct CachedPage {
    pub html: String,
    pub content_type: String,
    pub status_code: u16,
}

impl PageCache {
    pub fn new(max_capacity: u64, ttl_seconds: u64) -> Self {
        let cache = Cache::builder()
            .max_capacity(max_capacity)
            .time_to_live(Duration::from_secs(ttl_seconds))
            .build();

        Self {
            cache,
            enabled: true,
        }
    }

    /// Get a cached page for the given URL path.
    pub async fn get(&self, path: &str) -> Option<CachedPage> {
        if !self.enabled {
            return None;
        }
        let result = self.cache.get(&path.to_string()).await;
        if result.is_some() {
            debug!(path, "page cache hit");
        }
        result
    }

    /// Cache a rendered page.
    pub async fn set(&self, path: &str, page: CachedPage) {
        if !self.enabled {
            return;
        }
        self.cache.insert(path.to_string(), page).await;
        debug!(path, "page cached");
    }

    /// Invalidate a specific page.
    pub async fn invalidate(&self, path: &str) {
        self.cache.invalidate(&path.to_string()).await;
    }

    /// Flush the entire page cache.
    pub async fn flush(&self) {
        self.cache.invalidate_all();
        info!("page cache flushed");
    }

    /// Enable or disable the page cache.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        info!(enabled, "page cache status changed");
    }

    /// Check if page cache is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Get cache statistics.
    pub fn entry_count(&self) -> u64 {
        self.cache.entry_count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_set_and_get_page() {
        let cache = PageCache::new(100, 3600);
        let page = CachedPage {
            html: "<h1>Hello</h1>".to_string(),
            content_type: "text/html".to_string(),
            status_code: 200,
        };

        cache.set("/hello", page).await;
        let result = cache.get("/hello").await;
        assert!(result.is_some());
        assert_eq!(result.unwrap().html, "<h1>Hello</h1>");
    }

    #[tokio::test]
    async fn test_disabled_cache() {
        let mut cache = PageCache::new(100, 3600);
        cache.set_enabled(false);

        let page = CachedPage {
            html: "<h1>Test</h1>".to_string(),
            content_type: "text/html".to_string(),
            status_code: 200,
        };

        cache.set("/test", page).await;
        assert!(cache.get("/test").await.is_none());
    }

    #[tokio::test]
    async fn test_invalidate() {
        let cache = PageCache::new(100, 3600);
        let page = CachedPage {
            html: "<h1>Delete me</h1>".to_string(),
            content_type: "text/html".to_string(),
            status_code: 200,
        };

        cache.set("/page", page).await;
        cache.invalidate("/page").await;
        assert!(cache.get("/page").await.is_none());
    }

    #[tokio::test]
    async fn test_flush_all() {
        let cache = PageCache::new(100, 3600);
        for i in 0..5 {
            let page = CachedPage {
                html: format!("<p>{}</p>", i),
                content_type: "text/html".to_string(),
                status_code: 200,
            };
            cache.set(&format!("/page/{}", i), page).await;
        }

        cache.flush().await;
        assert!(cache.get("/page/0").await.is_none());
    }

    #[test]
    fn test_enable_disable() {
        let mut cache = PageCache::new(100, 3600);
        assert!(cache.is_enabled());

        cache.set_enabled(false);
        assert!(!cache.is_enabled());

        cache.set_enabled(true);
        assert!(cache.is_enabled());
    }
}
