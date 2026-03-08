use moka::sync::Cache;
use std::net::IpAddr;
use std::time::{Duration, Instant};

/// Tracks failed login attempts per IP and enforces lockout after threshold.
///
/// Uses moka in-memory cache with automatic TTL-based expiration.
/// WordPress standard: 5 failures → 15 minute lockout.
#[derive(Clone)]
pub struct LoginAttemptTracker {
    /// Maps IP → FailureRecord (auto-expires after lockout_duration)
    cache: Cache<IpAddr, FailureRecord>,
    max_attempts: u32,
    lockout_duration: Duration,
}

#[derive(Clone, Debug)]
pub struct FailureRecord {
    pub count: u32,
    pub locked_at: Option<Instant>,
}

/// Error returned when login is rate-limited.
#[derive(Debug, thiserror::Error)]
pub enum LoginError {
    #[error("Too many login attempts. Please try again in {retry_after_minutes} minutes.")]
    RateLimited { retry_after_minutes: u64 },
}

impl LoginAttemptTracker {
    /// Create a new tracker with WordPress-standard defaults:
    /// 5 max attempts, 15 minute lockout.
    pub fn new() -> Self {
        Self::with_config(5, Duration::from_secs(15 * 60))
    }

    /// Create a tracker with custom configuration.
    pub fn with_config(max_attempts: u32, lockout_duration: Duration) -> Self {
        Self {
            cache: Cache::builder()
                .time_to_live(lockout_duration)
                .max_capacity(100_000)
                .build(),
            max_attempts,
            lockout_duration,
        }
    }

    /// Check if the IP is allowed to attempt login, and record a failure if not locked.
    ///
    /// Returns `Ok(())` if the attempt is allowed.
    /// Returns `Err(LoginError::RateLimited)` if the IP is locked out.
    pub fn check_and_record(&self, ip: IpAddr) -> Result<(), LoginError> {
        let record = self.cache.get(&ip);

        match record {
            Some(rec) if rec.locked_at.is_some() => {
                // Still locked out
                let elapsed = rec.locked_at.unwrap().elapsed();
                let remaining = self
                    .lockout_duration
                    .checked_sub(elapsed)
                    .unwrap_or(Duration::ZERO);
                let minutes = remaining.as_secs().div_ceil(60);
                Err(LoginError::RateLimited {
                    retry_after_minutes: minutes.max(1),
                })
            }
            Some(rec) => {
                let new_count = rec.count + 1;
                if new_count >= self.max_attempts {
                    // Lock the IP
                    self.cache.insert(
                        ip,
                        FailureRecord {
                            count: new_count,
                            locked_at: Some(Instant::now()),
                        },
                    );
                    let minutes = self.lockout_duration.as_secs() / 60;
                    Err(LoginError::RateLimited {
                        retry_after_minutes: minutes,
                    })
                } else {
                    self.cache.insert(
                        ip,
                        FailureRecord {
                            count: new_count,
                            locked_at: None,
                        },
                    );
                    Ok(())
                }
            }
            None => {
                // First failure
                self.cache.insert(
                    ip,
                    FailureRecord {
                        count: 1,
                        locked_at: None,
                    },
                );
                Ok(())
            }
        }
    }

    /// Clear failure records for an IP (call on successful login).
    pub fn clear(&self, ip: &IpAddr) {
        self.cache.invalidate(ip);
    }

    /// Check if an IP is currently locked out without recording.
    pub fn is_locked(&self, ip: &IpAddr) -> bool {
        self.cache
            .get(ip)
            .map(|rec| rec.locked_at.is_some())
            .unwrap_or(false)
    }
}

impl Default for LoginAttemptTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    fn test_ip() -> IpAddr {
        IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100))
    }

    fn test_ip2() -> IpAddr {
        IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1))
    }

    #[test]
    fn test_first_failure_allowed() {
        let tracker = LoginAttemptTracker::new();
        assert!(tracker.check_and_record(test_ip()).is_ok());
    }

    #[test]
    fn test_under_threshold_allowed() {
        let tracker = LoginAttemptTracker::new();
        let ip = test_ip();
        // 4 failures should be allowed (threshold is 5)
        for _ in 0..4 {
            assert!(tracker.check_and_record(ip).is_ok());
        }
    }

    #[test]
    fn test_fifth_attempt_locks() {
        let tracker = LoginAttemptTracker::new();
        let ip = test_ip();
        // First 4 attempts OK
        for _ in 0..4 {
            assert!(tracker.check_and_record(ip).is_ok());
        }
        // 5th attempt triggers lockout
        let result = tracker.check_and_record(ip);
        assert!(result.is_err());
        match result.unwrap_err() {
            LoginError::RateLimited {
                retry_after_minutes,
            } => {
                assert_eq!(retry_after_minutes, 15);
            }
        }
    }

    #[test]
    fn test_locked_ip_stays_locked() {
        let tracker = LoginAttemptTracker::new();
        let ip = test_ip();
        // Trigger lockout
        for _ in 0..5 {
            let _ = tracker.check_and_record(ip);
        }
        // 6th attempt should also be locked
        assert!(tracker.check_and_record(ip).is_err());
        assert!(tracker.is_locked(&ip));
    }

    #[test]
    fn test_clear_resets_failures() {
        let tracker = LoginAttemptTracker::new();
        let ip = test_ip();
        // Record some failures
        for _ in 0..3 {
            let _ = tracker.check_and_record(ip);
        }
        // Clear
        tracker.clear(&ip);
        // Should be allowed again
        assert!(tracker.check_and_record(ip).is_ok());
        assert!(!tracker.is_locked(&ip));
    }

    #[test]
    fn test_different_ips_independent() {
        let tracker = LoginAttemptTracker::new();
        let ip1 = test_ip();
        let ip2 = test_ip2();
        // Lock ip1
        for _ in 0..5 {
            let _ = tracker.check_and_record(ip1);
        }
        assert!(tracker.is_locked(&ip1));
        // ip2 should still be allowed
        assert!(tracker.check_and_record(ip2).is_ok());
        assert!(!tracker.is_locked(&ip2));
    }

    #[test]
    fn test_auto_expire_after_lockout() {
        // Use very short lockout for testing
        let tracker = LoginAttemptTracker::with_config(2, Duration::from_millis(50));
        let ip = test_ip();
        // Trigger lockout
        let _ = tracker.check_and_record(ip);
        let _ = tracker.check_and_record(ip);
        assert!(tracker.is_locked(&ip));

        // Wait for TTL expiration
        std::thread::sleep(Duration::from_millis(100));
        // moka should have expired the entry
        assert!(!tracker.is_locked(&ip));
        assert!(tracker.check_and_record(ip).is_ok());
    }
}
