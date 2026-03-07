//! Wordfence compatibility layer.
//!
//! While RustPress implements its own WAF, rate limiter, and login protection,
//! this module provides compatibility with Wordfence's wp_options keys and
//! data formats for seamless migration from WordPress + Wordfence.
//!
//! ## Wordfence wp_options Keys
//! - `wordfence_version` — Installed version
//! - `wf_firewall_enabled` — WAF on/off (1/0)
//! - `wf_login_sec_enabled` — Login security on/off
//! - `wf_login_sec_maxFailures` — Max login failures before lockout
//! - `wf_login_sec_lockoutMins` — Lockout duration in minutes
//! - `wf_rate_limit_enabled` — Rate limiting on/off
//! - `wf_rate_limit_maxRequestsPerMin` — Max requests per minute
//! - `wf_blocked_ips` — PHP serialized array of blocked IPs
//! - `wf_whitelisted_ips` — PHP serialized array of allowed IPs
//! - `wf_scan_schedule` — Scan schedule setting
//!
//! ## Wordfence Custom Tables (not used by RustPress)
//! Wordfence uses custom tables like `wp_wfBlocks`, `wp_wfHits`, `wp_wfLogins`.
//! RustPress does NOT create or read these tables — it stores equivalent data
//! in-memory. This compat layer only handles the wp_options settings.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::login_protection::LoginProtection;

/// Wordfence wp_options key constants.
pub mod option_keys {
    pub const VERSION: &str = "wordfence_version";
    pub const FIREWALL_ENABLED: &str = "wf_firewall_enabled";
    pub const LOGIN_SEC_ENABLED: &str = "wf_login_sec_enabled";
    pub const LOGIN_SEC_MAX_FAILURES: &str = "wf_login_sec_maxFailures";
    pub const LOGIN_SEC_LOCKOUT_MINS: &str = "wf_login_sec_lockoutMins";
    pub const RATE_LIMIT_ENABLED: &str = "wf_rate_limit_enabled";
    pub const RATE_LIMIT_MAX_RPM: &str = "wf_rate_limit_maxRequestsPerMin";
    pub const BLOCKED_IPS: &str = "wf_blocked_ips";
    pub const WHITELISTED_IPS: &str = "wf_whitelisted_ips";
    pub const SCAN_SCHEDULE: &str = "wf_scan_schedule";
    pub const TWO_FACTOR_ENABLED: &str = "wf_two_factor_enabled";
    pub const COUNTRY_BLOCKING: &str = "wf_country_blocking";
}

/// Wordfence-compatible security settings, read from wp_options.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WordfenceSettings {
    pub firewall_enabled: bool,
    pub login_security_enabled: bool,
    pub max_login_failures: u32,
    pub lockout_minutes: u32,
    pub rate_limit_enabled: bool,
    pub max_requests_per_minute: u32,
    pub blocked_ips: Vec<String>,
    pub whitelisted_ips: Vec<String>,
    pub scan_schedule: String,
    pub two_factor_enabled: bool,
    pub country_blocking_enabled: bool,
}

impl Default for WordfenceSettings {
    fn default() -> Self {
        Self {
            firewall_enabled: true,
            login_security_enabled: true,
            max_login_failures: 5,
            lockout_minutes: 30,
            rate_limit_enabled: true,
            max_requests_per_minute: 120,
            blocked_ips: vec![],
            whitelisted_ips: vec![],
            scan_schedule: "daily".into(),
            two_factor_enabled: false,
            country_blocking_enabled: false,
        }
    }
}

impl WordfenceSettings {
    /// Parse Wordfence settings from wp_options key-value pairs.
    pub fn from_options(options: &HashMap<String, String>) -> Self {
        let blocked_ips = options
            .get(option_keys::BLOCKED_IPS)
            .map(|v| parse_ip_list(v))
            .unwrap_or_default();

        let whitelisted_ips = options
            .get(option_keys::WHITELISTED_IPS)
            .map(|v| parse_ip_list(v))
            .unwrap_or_default();

        Self {
            firewall_enabled: is_enabled(options.get(option_keys::FIREWALL_ENABLED), true),
            login_security_enabled: is_enabled(
                options.get(option_keys::LOGIN_SEC_ENABLED),
                true,
            ),
            max_login_failures: parse_u32(
                options.get(option_keys::LOGIN_SEC_MAX_FAILURES),
                5,
            ),
            lockout_minutes: parse_u32(
                options.get(option_keys::LOGIN_SEC_LOCKOUT_MINS),
                30,
            ),
            rate_limit_enabled: is_enabled(
                options.get(option_keys::RATE_LIMIT_ENABLED),
                true,
            ),
            max_requests_per_minute: parse_u32(
                options.get(option_keys::RATE_LIMIT_MAX_RPM),
                120,
            ),
            blocked_ips,
            whitelisted_ips,
            scan_schedule: options
                .get(option_keys::SCAN_SCHEDULE)
                .cloned()
                .unwrap_or_else(|| "daily".into()),
            two_factor_enabled: is_enabled(
                options.get(option_keys::TWO_FACTOR_ENABLED),
                false,
            ),
            country_blocking_enabled: is_enabled(
                options.get(option_keys::COUNTRY_BLOCKING),
                false,
            ),
        }
    }

    /// Convert to wp_options key-value pairs for writing.
    pub fn to_options(&self) -> Vec<(String, String)> {
        vec![
            (
                option_keys::FIREWALL_ENABLED.into(),
                bool_to_str(self.firewall_enabled),
            ),
            (
                option_keys::LOGIN_SEC_ENABLED.into(),
                bool_to_str(self.login_security_enabled),
            ),
            (
                option_keys::LOGIN_SEC_MAX_FAILURES.into(),
                self.max_login_failures.to_string(),
            ),
            (
                option_keys::LOGIN_SEC_LOCKOUT_MINS.into(),
                self.lockout_minutes.to_string(),
            ),
            (
                option_keys::RATE_LIMIT_ENABLED.into(),
                bool_to_str(self.rate_limit_enabled),
            ),
            (
                option_keys::RATE_LIMIT_MAX_RPM.into(),
                self.max_requests_per_minute.to_string(),
            ),
            (
                option_keys::BLOCKED_IPS.into(),
                self.blocked_ips.join(","),
            ),
            (
                option_keys::WHITELISTED_IPS.into(),
                self.whitelisted_ips.join(","),
            ),
            (
                option_keys::SCAN_SCHEDULE.into(),
                self.scan_schedule.clone(),
            ),
            (
                option_keys::TWO_FACTOR_ENABLED.into(),
                bool_to_str(self.two_factor_enabled),
            ),
            (
                option_keys::COUNTRY_BLOCKING.into(),
                bool_to_str(self.country_blocking_enabled),
            ),
        ]
    }

    /// Apply these settings to the RustPress login protection.
    ///
    /// Creates a new LoginProtection instance with the configured settings.
    pub fn create_login_protection(&self) -> LoginProtection {
        use crate::login_protection::LoginProtectionConfig;

        let config = LoginProtectionConfig {
            max_attempts: self.max_login_failures,
            lockout_duration_secs: (self.lockout_minutes as u64) * 60,
            attempt_window_secs: (self.lockout_minutes as u64) * 60,
        };
        LoginProtection::with_config(config)
    }

    /// Check if a request should be allowed by the IP-based rules.
    ///
    /// Returns `true` if the request is allowed, `false` if it should be blocked.
    pub fn check_ip(&self, ip: &str) -> bool {
        // Whitelisted IPs always pass
        if self.is_ip_whitelisted(ip) {
            return true;
        }
        // Blocked IPs are denied
        if self.is_ip_blocked(ip) {
            return false;
        }
        true
    }

    /// Check if an IP is blocked.
    pub fn is_ip_blocked(&self, ip: &str) -> bool {
        self.blocked_ips.iter().any(|blocked| blocked == ip)
    }

    /// Check if an IP is whitelisted.
    pub fn is_ip_whitelisted(&self, ip: &str) -> bool {
        self.whitelisted_ips.iter().any(|allowed| allowed == ip)
    }

    /// Add an IP to the blocked list.
    pub fn block_ip(&mut self, ip: &str) {
        if !self.is_ip_blocked(ip) {
            self.blocked_ips.push(ip.to_string());
        }
    }

    /// Remove an IP from the blocked list.
    pub fn unblock_ip(&mut self, ip: &str) -> bool {
        let before = self.blocked_ips.len();
        self.blocked_ips.retain(|b| b != ip);
        self.blocked_ips.len() < before
    }
}

fn is_enabled(val: Option<&String>, default: bool) -> bool {
    match val {
        Some(v) => v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("yes"),
        None => default,
    }
}

fn parse_u32(val: Option<&String>, default: u32) -> u32 {
    val.and_then(|v| v.parse().ok()).unwrap_or(default)
}

fn bool_to_str(b: bool) -> String {
    if b { "1" } else { "0" }.to_string()
}

fn parse_ip_list(raw: &str) -> Vec<String> {
    if raw.is_empty() {
        return vec![];
    }

    // Try PHP serialized format first
    if raw.starts_with("a:") {
        if let Ok(val) = rustpress_core::php_unserialize(raw) {
            return val.as_string_list();
        }
    }

    // Fall back to comma-separated
    raw.split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_settings() {
        let settings = WordfenceSettings::default();
        assert!(settings.firewall_enabled);
        assert!(settings.login_security_enabled);
        assert_eq!(settings.max_login_failures, 5);
        assert_eq!(settings.lockout_minutes, 30);
        assert!(settings.rate_limit_enabled);
        assert_eq!(settings.max_requests_per_minute, 120);
        assert!(settings.blocked_ips.is_empty());
    }

    #[test]
    fn test_from_options() {
        let mut opts = HashMap::new();
        opts.insert("wf_firewall_enabled".into(), "1".into());
        opts.insert("wf_login_sec_enabled".into(), "0".into());
        opts.insert("wf_login_sec_maxFailures".into(), "10".into());
        opts.insert("wf_login_sec_lockoutMins".into(), "60".into());
        opts.insert("wf_rate_limit_enabled".into(), "1".into());
        opts.insert("wf_rate_limit_maxRequestsPerMin".into(), "200".into());
        opts.insert("wf_blocked_ips".into(), "192.168.1.1,10.0.0.1".into());
        opts.insert("wf_whitelisted_ips".into(), "127.0.0.1".into());
        opts.insert("wf_scan_schedule".into(), "weekly".into());

        let settings = WordfenceSettings::from_options(&opts);
        assert!(settings.firewall_enabled);
        assert!(!settings.login_security_enabled);
        assert_eq!(settings.max_login_failures, 10);
        assert_eq!(settings.lockout_minutes, 60);
        assert_eq!(settings.max_requests_per_minute, 200);
        assert_eq!(settings.blocked_ips, vec!["192.168.1.1", "10.0.0.1"]);
        assert_eq!(settings.whitelisted_ips, vec!["127.0.0.1"]);
        assert_eq!(settings.scan_schedule, "weekly");
    }

    #[test]
    fn test_to_options_roundtrip() {
        let original = WordfenceSettings {
            firewall_enabled: true,
            login_security_enabled: false,
            max_login_failures: 3,
            lockout_minutes: 15,
            rate_limit_enabled: true,
            max_requests_per_minute: 60,
            blocked_ips: vec!["1.2.3.4".into()],
            whitelisted_ips: vec!["5.6.7.8".into()],
            scan_schedule: "hourly".into(),
            two_factor_enabled: true,
            country_blocking_enabled: false,
        };

        let pairs = original.to_options();
        let opts: HashMap<String, String> = pairs.into_iter().collect();
        let restored = WordfenceSettings::from_options(&opts);

        assert_eq!(restored.firewall_enabled, original.firewall_enabled);
        assert_eq!(
            restored.login_security_enabled,
            original.login_security_enabled
        );
        assert_eq!(restored.max_login_failures, original.max_login_failures);
        assert_eq!(restored.lockout_minutes, original.lockout_minutes);
        assert_eq!(
            restored.max_requests_per_minute,
            original.max_requests_per_minute
        );
        assert_eq!(restored.blocked_ips, original.blocked_ips);
        assert_eq!(restored.whitelisted_ips, original.whitelisted_ips);
        assert_eq!(restored.scan_schedule, original.scan_schedule);
    }

    #[test]
    fn test_ip_management() {
        let mut settings = WordfenceSettings::default();

        settings.block_ip("192.168.1.1");
        assert!(settings.is_ip_blocked("192.168.1.1"));
        assert!(!settings.is_ip_blocked("10.0.0.1"));

        // No duplicates
        settings.block_ip("192.168.1.1");
        assert_eq!(settings.blocked_ips.len(), 1);

        assert!(settings.unblock_ip("192.168.1.1"));
        assert!(!settings.is_ip_blocked("192.168.1.1"));
        assert!(!settings.unblock_ip("nonexistent"));
    }

    #[test]
    fn test_is_enabled_parsing() {
        assert!(is_enabled(Some(&"1".into()), false));
        assert!(is_enabled(Some(&"true".into()), false));
        assert!(is_enabled(Some(&"yes".into()), false));
        assert!(!is_enabled(Some(&"0".into()), true));
        assert!(!is_enabled(Some(&"false".into()), true));
        assert!(is_enabled(None, true));
        assert!(!is_enabled(None, false));
    }

    #[test]
    fn test_parse_ip_list_comma_separated() {
        let ips = parse_ip_list("1.2.3.4, 5.6.7.8, 9.10.11.12");
        assert_eq!(ips, vec!["1.2.3.4", "5.6.7.8", "9.10.11.12"]);
    }

    #[test]
    fn test_parse_ip_list_empty() {
        let ips = parse_ip_list("");
        assert!(ips.is_empty());
    }

    #[test]
    fn test_option_keys_constants() {
        assert_eq!(option_keys::VERSION, "wordfence_version");
        assert_eq!(option_keys::FIREWALL_ENABLED, "wf_firewall_enabled");
        assert_eq!(option_keys::BLOCKED_IPS, "wf_blocked_ips");
    }
}
