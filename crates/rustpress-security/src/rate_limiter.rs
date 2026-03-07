//! Rate limiting with sliding window counters.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::warn;

/// Result of a rate limit check.
#[derive(Debug, Clone, PartialEq)]
pub enum RateLimitResult {
    /// Request is allowed; `remaining` shows how many requests are left in the window.
    Allowed { remaining: u32 },
    /// Request is rate-limited; `retry_after` is the number of seconds to wait.
    Limited { retry_after: u64 },
}

/// Configuration for a rate limit category.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    /// Maximum number of requests allowed in the window.
    pub max_requests: u32,
    /// Window duration in seconds.
    pub window_secs: u64,
}

/// Endpoint category for rate limiting.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EndpointCategory {
    Login,
    Api,
    General,
    Custom(String),
}

/// Internal record of timestamps for a particular key.
struct WindowEntry {
    timestamps: Vec<DateTime<Utc>>,
}

impl WindowEntry {
    fn new() -> Self {
        Self {
            timestamps: Vec::new(),
        }
    }

    /// Remove timestamps older than the window and return count of remaining.
    fn prune_and_count(&mut self, window_secs: u64, now: DateTime<Utc>) -> u32 {
        let cutoff = now - chrono::Duration::seconds(window_secs as i64);
        self.timestamps.retain(|t| *t > cutoff);
        self.timestamps.len() as u32
    }

    /// Add a new timestamp.
    fn record(&mut self, now: DateTime<Utc>) {
        self.timestamps.push(now);
    }

    /// Return the earliest timestamp still in the window (for retry_after calculation).
    fn earliest(&self) -> Option<DateTime<Utc>> {
        self.timestamps.first().copied()
    }
}

/// Rate limiter using sliding window counters keyed by (IP, endpoint category).
pub struct RateLimiter {
    configs: HashMap<EndpointCategory, RateLimitConfig>,
    /// Key is "ip:category" string.
    windows: HashMap<String, WindowEntry>,
    /// Tracks the last cleanup time.
    last_cleanup: DateTime<Utc>,
    /// How often (in seconds) to run cleanup of expired entries.
    cleanup_interval_secs: u64,
}

impl RateLimiter {
    /// Create a new rate limiter with default configurations.
    pub fn new() -> Self {
        let mut configs = HashMap::new();
        configs.insert(
            EndpointCategory::Login,
            RateLimitConfig {
                max_requests: 30,
                window_secs: 60,
            },
        );
        configs.insert(
            EndpointCategory::Api,
            RateLimitConfig {
                max_requests: 60,
                window_secs: 60,
            },
        );
        configs.insert(
            EndpointCategory::General,
            RateLimitConfig {
                max_requests: 120,
                window_secs: 60,
            },
        );

        Self {
            configs,
            windows: HashMap::new(),
            last_cleanup: Utc::now(),
            cleanup_interval_secs: 300,
        }
    }

    /// Set the rate limit configuration for a given category.
    pub fn set_config(&mut self, category: EndpointCategory, config: RateLimitConfig) {
        self.configs.insert(category, config);
    }

    /// Classify an endpoint path into a category.
    pub fn classify_endpoint(endpoint: &str) -> EndpointCategory {
        if endpoint.contains("wp-login")
            || endpoint.contains("/login")
            || endpoint.contains("/auth")
        {
            EndpointCategory::Login
        } else if endpoint.starts_with("/wp-json/")
            || endpoint.starts_with("/api/")
            || endpoint.contains("xmlrpc.php")
        {
            EndpointCategory::Api
        } else {
            EndpointCategory::General
        }
    }

    /// Check if a request from the given IP to the given endpoint is allowed.
    /// This also records the request if it is allowed.
    pub fn check(&mut self, ip: &str, endpoint: &str) -> RateLimitResult {
        let now = Utc::now();
        self.maybe_cleanup(now);

        let category = Self::classify_endpoint(endpoint);
        let config = self
            .configs
            .get(&category)
            .cloned()
            .unwrap_or(RateLimitConfig {
                max_requests: 120,
                window_secs: 60,
            });

        let key = format!("{}:{:?}", ip, category);
        let entry = self.windows.entry(key).or_insert_with(WindowEntry::new);
        let current_count = entry.prune_and_count(config.window_secs, now);

        if current_count >= config.max_requests {
            let retry_after = if let Some(earliest) = entry.earliest() {
                let expires_at = earliest + chrono::Duration::seconds(config.window_secs as i64);
                let diff = expires_at.signed_duration_since(now);
                diff.num_seconds().max(1) as u64
            } else {
                config.window_secs
            };

            warn!(
                ip = %ip,
                endpoint = %endpoint,
                category = ?category,
                "Rate limit exceeded"
            );

            RateLimitResult::Limited { retry_after }
        } else {
            entry.record(now);
            let remaining = config.max_requests - current_count - 1;
            RateLimitResult::Allowed { remaining }
        }
    }

    /// Check without recording (peek at rate limit status).
    pub fn peek(&mut self, ip: &str, endpoint: &str) -> RateLimitResult {
        let now = Utc::now();
        let category = Self::classify_endpoint(endpoint);
        let config = self
            .configs
            .get(&category)
            .cloned()
            .unwrap_or(RateLimitConfig {
                max_requests: 120,
                window_secs: 60,
            });

        let key = format!("{}:{:?}", ip, category);
        if let Some(entry) = self.windows.get_mut(&key) {
            let current_count = entry.prune_and_count(config.window_secs, now);
            if current_count >= config.max_requests {
                let retry_after = if let Some(earliest) = entry.earliest() {
                    let expires_at =
                        earliest + chrono::Duration::seconds(config.window_secs as i64);
                    let diff = expires_at.signed_duration_since(now);
                    diff.num_seconds().max(1) as u64
                } else {
                    config.window_secs
                };
                RateLimitResult::Limited { retry_after }
            } else {
                RateLimitResult::Allowed {
                    remaining: config.max_requests - current_count,
                }
            }
        } else {
            RateLimitResult::Allowed {
                remaining: config.max_requests,
            }
        }
    }

    /// Reset rate limit counters for a specific IP.
    pub fn reset(&mut self, ip: &str) {
        self.windows.retain(|key, _| !key.starts_with(&format!("{}:", ip)));
    }

    /// Periodically remove expired entries to prevent memory growth.
    fn maybe_cleanup(&mut self, now: DateTime<Utc>) {
        let elapsed = now
            .signed_duration_since(self.last_cleanup)
            .num_seconds()
            .unsigned_abs();
        if elapsed < self.cleanup_interval_secs {
            return;
        }
        self.last_cleanup = now;

        // Find the maximum window size across all configs for cleanup threshold.
        let max_window = self.configs.values().map(|c| c.window_secs).max().unwrap_or(60);

        self.windows.retain(|_, entry| {
            entry.prune_and_count(max_window, now);
            !entry.timestamps.is_empty()
        });
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allow_under_limit() {
        let mut limiter = RateLimiter::new();
        let result = limiter.check("192.168.1.1", "/hello");
        match result {
            RateLimitResult::Allowed { remaining } => {
                // General limit is 120/min, first request => 119 remaining
                assert_eq!(remaining, 119);
            }
            _ => panic!("Expected Allowed"),
        }
    }

    #[test]
    fn test_login_rate_limit() {
        let mut limiter = RateLimiter::new();
        // Login limit is 30/min
        for i in 0..30 {
            let result = limiter.check("10.0.0.1", "/wp-login.php");
            match result {
                RateLimitResult::Allowed { remaining } => {
                    assert_eq!(remaining, 29 - i);
                }
                _ => panic!("Expected Allowed on attempt {}", i),
            }
        }
        // 31st attempt should be limited
        let result = limiter.check("10.0.0.1", "/wp-login.php");
        match result {
            RateLimitResult::Limited { retry_after } => {
                assert!(retry_after > 0);
            }
            _ => panic!("Expected Limited"),
        }
    }

    #[test]
    fn test_different_ips_independent() {
        let mut limiter = RateLimiter::new();
        // Exhaust IP1's login limit
        for _ in 0..5 {
            limiter.check("10.0.0.1", "/wp-login.php");
        }
        // IP2 should still be allowed
        let result = limiter.check("10.0.0.2", "/wp-login.php");
        match result {
            RateLimitResult::Allowed { .. } => {}
            _ => panic!("Expected Allowed for different IP"),
        }
    }

    #[test]
    fn test_endpoint_classification() {
        assert_eq!(
            RateLimiter::classify_endpoint("/wp-login.php"),
            EndpointCategory::Login
        );
        assert_eq!(
            RateLimiter::classify_endpoint("/wp-json/wp/v2/posts"),
            EndpointCategory::Api
        );
        assert_eq!(
            RateLimiter::classify_endpoint("/hello-world"),
            EndpointCategory::General
        );
        assert_eq!(
            RateLimiter::classify_endpoint("/xmlrpc.php"),
            EndpointCategory::Api
        );
    }

    #[test]
    fn test_reset_clears_counters() {
        let mut limiter = RateLimiter::new();
        // Exhaust login limit (30/min)
        for _ in 0..30 {
            limiter.check("10.0.0.1", "/wp-login.php");
        }
        // Should be limited
        assert!(matches!(
            limiter.check("10.0.0.1", "/wp-login.php"),
            RateLimitResult::Limited { .. }
        ));
        // Reset
        limiter.reset("10.0.0.1");
        // Should be allowed again
        assert!(matches!(
            limiter.check("10.0.0.1", "/wp-login.php"),
            RateLimitResult::Allowed { .. }
        ));
    }

    #[test]
    fn test_custom_config() {
        let mut limiter = RateLimiter::new();
        limiter.set_config(
            EndpointCategory::Login,
            RateLimitConfig {
                max_requests: 2,
                window_secs: 60,
            },
        );
        limiter.check("10.0.0.1", "/wp-login.php");
        limiter.check("10.0.0.1", "/wp-login.php");
        let result = limiter.check("10.0.0.1", "/wp-login.php");
        assert!(matches!(result, RateLimitResult::Limited { .. }));
    }
}
