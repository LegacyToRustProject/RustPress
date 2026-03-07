//! Login brute-force protection with auto-lockout.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{info, warn};

/// Configuration for login protection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginProtectionConfig {
    /// Maximum failed attempts before lockout.
    pub max_attempts: u32,
    /// Lockout duration in seconds.
    pub lockout_duration_secs: u64,
    /// Time window in seconds within which failed attempts are counted.
    pub attempt_window_secs: u64,
}

impl Default for LoginProtectionConfig {
    fn default() -> Self {
        Self {
            max_attempts: 5,
            lockout_duration_secs: 900, // 15 minutes
            attempt_window_secs: 600,   // 10 minutes
        }
    }
}

/// Record of failed login attempts for a specific IP.
struct AttemptRecord {
    /// Timestamps of failed attempts within the window.
    failed_attempts: Vec<DateTime<Utc>>,
    /// If locked out, the time the lockout expires.
    locked_until: Option<DateTime<Utc>>,
    /// Usernames that were attempted from this IP.
    attempted_usernames: Vec<String>,
}

impl AttemptRecord {
    fn new() -> Self {
        Self {
            failed_attempts: Vec::new(),
            locked_until: None,
            attempted_usernames: Vec::new(),
        }
    }

    /// Prune attempts older than the window.
    fn prune(&mut self, window_secs: u64, now: DateTime<Utc>) {
        let cutoff = now - chrono::Duration::seconds(window_secs as i64);
        self.failed_attempts.retain(|t| *t > cutoff);
    }
}

/// Login protection engine that tracks failed attempts and enforces lockouts.
pub struct LoginProtection {
    config: LoginProtectionConfig,
    records: HashMap<String, AttemptRecord>,
}

impl LoginProtection {
    /// Create a new login protection instance with default configuration.
    pub fn new() -> Self {
        Self {
            config: LoginProtectionConfig::default(),
            records: HashMap::new(),
        }
    }

    /// Create a new login protection instance with custom configuration.
    pub fn with_config(config: LoginProtectionConfig) -> Self {
        Self {
            config,
            records: HashMap::new(),
        }
    }

    /// Record a failed login attempt from the given IP for the given username.
    /// If the number of failed attempts reaches the threshold, the IP is locked out.
    pub fn record_failed_attempt(&mut self, ip: &str, username: &str) {
        let now = Utc::now();
        let record = self
            .records
            .entry(ip.to_string())
            .or_insert_with(AttemptRecord::new);

        record.prune(self.config.attempt_window_secs, now);
        record.failed_attempts.push(now);

        if !record.attempted_usernames.contains(&username.to_string()) {
            record.attempted_usernames.push(username.to_string());
        }

        let count = record.failed_attempts.len() as u32;

        if count >= self.config.max_attempts {
            let locked_until =
                now + chrono::Duration::seconds(self.config.lockout_duration_secs as i64);
            record.locked_until = Some(locked_until);
            warn!(
                ip = %ip,
                username = %username,
                attempts = count,
                locked_until = %locked_until,
                "IP locked out due to excessive failed login attempts"
            );
        } else {
            info!(
                ip = %ip,
                username = %username,
                attempts = count,
                max = self.config.max_attempts,
                "Failed login attempt recorded"
            );
        }
    }

    /// Check if the given IP is currently locked out.
    pub fn is_locked_out(&self, ip: &str) -> bool {
        if let Some(record) = self.records.get(ip) {
            if let Some(locked_until) = record.locked_until {
                return Utc::now() < locked_until;
            }
        }
        false
    }

    /// Record a successful login, clearing the failed attempt history for the IP.
    pub fn record_successful_login(&mut self, ip: &str, username: &str) {
        info!(
            ip = %ip,
            username = %username,
            "Successful login, clearing failed attempt history"
        );
        self.records.remove(ip);
    }

    /// Get the number of failed attempts for the given IP within the current window.
    pub fn get_failed_attempts(&mut self, ip: &str) -> u32 {
        let now = Utc::now();
        if let Some(record) = self.records.get_mut(ip) {
            record.prune(self.config.attempt_window_secs, now);
            record.failed_attempts.len() as u32
        } else {
            0
        }
    }

    /// Get the usernames that were attempted from the given IP.
    pub fn get_attempted_usernames(&self, ip: &str) -> Vec<String> {
        self.records
            .get(ip)
            .map(|r| r.attempted_usernames.clone())
            .unwrap_or_default()
    }

    /// Get the lockout expiry time for an IP, if locked out.
    pub fn lockout_expires_at(&self, ip: &str) -> Option<DateTime<Utc>> {
        self.records
            .get(ip)
            .and_then(|r| r.locked_until)
            .filter(|&t| Utc::now() < t)
    }

    /// Manually unlock an IP.
    pub fn unlock(&mut self, ip: &str) {
        if let Some(record) = self.records.get_mut(ip) {
            record.locked_until = None;
            record.failed_attempts.clear();
            info!(ip = %ip, "IP manually unlocked");
        }
    }

    /// Get the number of currently tracked IPs.
    pub fn tracked_ip_count(&self) -> usize {
        self.records.len()
    }

    /// Clean up expired records to free memory.
    pub fn cleanup_expired(&mut self) {
        let now = Utc::now();
        let window = self.config.attempt_window_secs;

        self.records.retain(|_, record| {
            // Keep if locked out and lockout hasn't expired
            if let Some(locked_until) = record.locked_until {
                if now < locked_until {
                    return true;
                }
            }
            // Keep if there are recent attempts
            record.prune(window, now);
            !record.failed_attempts.is_empty()
        });
    }
}

impl Default for LoginProtection {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_lockout_initially() {
        let protection = LoginProtection::new();
        assert!(!protection.is_locked_out("192.168.1.1"));
    }

    #[test]
    fn test_lockout_after_max_attempts() {
        let mut protection = LoginProtection::with_config(LoginProtectionConfig {
            max_attempts: 3,
            lockout_duration_secs: 900,
            attempt_window_secs: 600,
        });

        let ip = "10.0.0.1";
        protection.record_failed_attempt(ip, "admin");
        assert!(!protection.is_locked_out(ip));

        protection.record_failed_attempt(ip, "admin");
        assert!(!protection.is_locked_out(ip));

        protection.record_failed_attempt(ip, "admin");
        assert!(protection.is_locked_out(ip));
    }

    #[test]
    fn test_successful_login_clears_history() {
        let mut protection = LoginProtection::new();
        let ip = "10.0.0.1";

        protection.record_failed_attempt(ip, "admin");
        protection.record_failed_attempt(ip, "admin");
        assert_eq!(protection.get_failed_attempts(ip), 2);

        protection.record_successful_login(ip, "admin");
        assert_eq!(protection.get_failed_attempts(ip), 0);
        assert!(!protection.is_locked_out(ip));
    }

    #[test]
    fn test_different_ips_independent() {
        let mut protection = LoginProtection::with_config(LoginProtectionConfig {
            max_attempts: 2,
            lockout_duration_secs: 900,
            attempt_window_secs: 600,
        });

        // Lock out IP1
        protection.record_failed_attempt("10.0.0.1", "admin");
        protection.record_failed_attempt("10.0.0.1", "admin");
        assert!(protection.is_locked_out("10.0.0.1"));

        // IP2 should not be affected
        assert!(!protection.is_locked_out("10.0.0.2"));
    }

    #[test]
    fn test_get_failed_attempts() {
        let mut protection = LoginProtection::new();
        let ip = "10.0.0.1";

        assert_eq!(protection.get_failed_attempts(ip), 0);
        protection.record_failed_attempt(ip, "admin");
        assert_eq!(protection.get_failed_attempts(ip), 1);
        protection.record_failed_attempt(ip, "editor");
        assert_eq!(protection.get_failed_attempts(ip), 2);
    }

    #[test]
    fn test_attempted_usernames_tracked() {
        let mut protection = LoginProtection::new();
        let ip = "10.0.0.1";

        protection.record_failed_attempt(ip, "admin");
        protection.record_failed_attempt(ip, "root");
        protection.record_failed_attempt(ip, "admin");

        let usernames = protection.get_attempted_usernames(ip);
        assert!(usernames.contains(&"admin".to_string()));
        assert!(usernames.contains(&"root".to_string()));
        assert_eq!(usernames.len(), 2); // "admin" not duplicated
    }

    #[test]
    fn test_manual_unlock() {
        let mut protection = LoginProtection::with_config(LoginProtectionConfig {
            max_attempts: 1,
            lockout_duration_secs: 900,
            attempt_window_secs: 600,
        });

        let ip = "10.0.0.1";
        protection.record_failed_attempt(ip, "admin");
        assert!(protection.is_locked_out(ip));

        protection.unlock(ip);
        assert!(!protection.is_locked_out(ip));
    }
}
