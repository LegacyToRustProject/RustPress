use moka::sync::Cache;
use std::net::IpAddr;
use std::time::{Duration, Instant};

/// Tracks failed login attempts per IP and enforces lockout after threshold.
///
/// Uses moka in-memory cache with automatic TTL-based expiration.
/// WordPress standard: 5 failures → 15 minute lockout.
///
/// All check-and-record operations are atomic per-IP using moka's
/// `entry().and_upsert_with()` API, preventing TOCTOU race conditions
/// where burst requests could bypass the rate limit.
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

    /// Atomically check if the IP is allowed to attempt login, and record a failure.
    ///
    /// Uses moka's `entry().and_upsert_with()` for atomic read-modify-write,
    /// preventing TOCTOU races where concurrent requests could slip through between
    /// a separate `is_locked()` check and `check_and_record()` call.
    ///
    /// Returns `Ok(())` if the attempt is allowed (count incremented).
    /// Returns `Err(LoginError::RateLimited)` if the IP is locked out.
    pub fn check_and_record(&self, ip: IpAddr) -> Result<(), LoginError> {
        let max = self.max_attempts;
        let lockout = self.lockout_duration;

        let entry = self.cache.entry(ip).and_upsert_with(|maybe_entry| {
            match maybe_entry {
                Some(existing) => {
                    let old = existing.into_value();
                    if old.locked_at.is_some() {
                        // Already locked - keep the record unchanged
                        old
                    } else {
                        let new_count = old.count + 1;
                        if new_count >= max {
                            // Threshold reached - lock
                            FailureRecord {
                                count: new_count,
                                locked_at: Some(Instant::now()),
                            }
                        } else {
                            // Increment count, still under threshold
                            FailureRecord {
                                count: new_count,
                                locked_at: None,
                            }
                        }
                    }
                }
                None => {
                    // First failure for this IP
                    if max <= 1 {
                        FailureRecord {
                            count: 1,
                            locked_at: Some(Instant::now()),
                        }
                    } else {
                        FailureRecord {
                            count: 1,
                            locked_at: None,
                        }
                    }
                }
            }
        });

        let record = entry.into_value();
        if record.locked_at.is_some() {
            let minutes = lockout.as_secs().div_ceil(60);
            Err(LoginError::RateLimited {
                retry_after_minutes: minutes.max(1),
            })
        } else {
            Ok(())
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
    use std::sync::Arc;

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
        for _ in 0..4 {
            assert!(tracker.check_and_record(ip).is_ok());
        }
    }

    #[test]
    fn test_fifth_attempt_locks() {
        let tracker = LoginAttemptTracker::new();
        let ip = test_ip();
        for _ in 0..4 {
            assert!(tracker.check_and_record(ip).is_ok());
        }
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
        for _ in 0..5 {
            let _ = tracker.check_and_record(ip);
        }
        assert!(tracker.check_and_record(ip).is_err());
        assert!(tracker.is_locked(&ip));
    }

    #[test]
    fn test_clear_resets_failures() {
        let tracker = LoginAttemptTracker::new();
        let ip = test_ip();
        for _ in 0..3 {
            let _ = tracker.check_and_record(ip);
        }
        tracker.clear(&ip);
        assert!(tracker.check_and_record(ip).is_ok());
        assert!(!tracker.is_locked(&ip));
    }

    #[test]
    fn test_different_ips_independent() {
        let tracker = LoginAttemptTracker::new();
        let ip1 = test_ip();
        let ip2 = test_ip2();
        for _ in 0..5 {
            let _ = tracker.check_and_record(ip1);
        }
        assert!(tracker.is_locked(&ip1));
        assert!(tracker.check_and_record(ip2).is_ok());
        assert!(!tracker.is_locked(&ip2));
    }

    #[test]
    fn test_auto_expire_after_lockout() {
        let tracker = LoginAttemptTracker::with_config(2, Duration::from_millis(50));
        let ip = test_ip();
        let _ = tracker.check_and_record(ip);
        let _ = tracker.check_and_record(ip);
        assert!(tracker.is_locked(&ip));

        std::thread::sleep(Duration::from_millis(100));
        assert!(!tracker.is_locked(&ip));
        assert!(tracker.check_and_record(ip).is_ok());
    }

    #[test]
    fn test_concurrent_burst_respects_max_attempts() {
        // 10 threads simultaneously hitting the same IP with max_attempts=5
        // At most 4 should be allowed (attempts 1-4), the 5th locks.
        let tracker = Arc::new(LoginAttemptTracker::with_config(5, Duration::from_secs(60)));
        let ip = test_ip();
        let barrier = Arc::new(std::sync::Barrier::new(10));

        let handles: Vec<_> = (0..10)
            .map(|_| {
                let t = tracker.clone();
                let b = barrier.clone();
                std::thread::spawn(move || {
                    b.wait();
                    t.check_and_record(ip)
                })
            })
            .collect();

        let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        let ok_count = results.iter().filter(|r| r.is_ok()).count();
        let err_count = results.iter().filter(|r| r.is_err()).count();

        assert_eq!(
            ok_count, 4,
            "expected exactly 4 allowed attempts, got {ok_count}"
        );
        assert_eq!(
            err_count, 6,
            "expected exactly 6 rejected attempts, got {err_count}"
        );
    }

    #[test]
    fn test_concurrent_burst_different_ips_no_interference() {
        // 10 threads, 5 different IPs, 2 threads per IP, max_attempts=3
        let tracker = Arc::new(LoginAttemptTracker::with_config(3, Duration::from_secs(60)));
        let barrier = Arc::new(std::sync::Barrier::new(10));

        let handles: Vec<_> = (0..10)
            .map(|i| {
                let t = tracker.clone();
                let b = barrier.clone();
                let ip_byte = (i / 2) as u8 + 1;
                std::thread::spawn(move || {
                    let ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, ip_byte));
                    b.wait();
                    t.check_and_record(ip)
                })
            })
            .collect();

        let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        let ok_count = results.iter().filter(|r| r.is_ok()).count();

        assert_eq!(
            ok_count, 10,
            "expected all 10 attempts allowed, got {ok_count}"
        );
    }
}
