//! Security audit logging system.
//!
//! Records security-relevant events: login attempts, permission changes,
//! content modifications, WAF blocks, and rate limiting events.

use std::collections::VecDeque;
use std::sync::Mutex;

/// Categories of security events.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuditEventType {
    LoginSuccess,
    LoginFailure,
    Logout,
    PasswordChange,
    RoleChange,
    ContentCreate,
    ContentUpdate,
    ContentDelete,
    SettingsChange,
    WafBlock,
    RateLimited,
    FileUpload,
    PluginActivation,
    BruteForceDetected,
    IpBlocked,
    IpUnblocked,
    SessionCreated,
    SessionDestroyed,
}

impl std::fmt::Display for AuditEventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LoginSuccess => write!(f, "login_success"),
            Self::LoginFailure => write!(f, "login_failure"),
            Self::Logout => write!(f, "logout"),
            Self::PasswordChange => write!(f, "password_change"),
            Self::RoleChange => write!(f, "role_change"),
            Self::ContentCreate => write!(f, "content_create"),
            Self::ContentUpdate => write!(f, "content_update"),
            Self::ContentDelete => write!(f, "content_delete"),
            Self::SettingsChange => write!(f, "settings_change"),
            Self::WafBlock => write!(f, "waf_block"),
            Self::RateLimited => write!(f, "rate_limited"),
            Self::FileUpload => write!(f, "file_upload"),
            Self::PluginActivation => write!(f, "plugin_activation"),
            Self::BruteForceDetected => write!(f, "brute_force_detected"),
            Self::IpBlocked => write!(f, "ip_blocked"),
            Self::IpUnblocked => write!(f, "ip_unblocked"),
            Self::SessionCreated => write!(f, "session_created"),
            Self::SessionDestroyed => write!(f, "session_destroyed"),
        }
    }
}

/// Severity levels for audit events.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Info,
    Warning,
    Critical,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Info => write!(f, "info"),
            Self::Warning => write!(f, "warning"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// A single audit log entry.
#[derive(Debug, Clone)]
pub struct AuditEntry {
    pub timestamp: u64,
    pub event_type: AuditEventType,
    pub severity: Severity,
    pub ip_address: String,
    pub user_id: Option<u64>,
    pub username: Option<String>,
    pub description: String,
    pub metadata: Option<String>,
}

/// In-memory audit log with bounded capacity.
///
/// Logs are also emitted via `tracing` for persistence via log aggregation.
pub struct AuditLog {
    entries: Mutex<VecDeque<AuditEntry>>,
    max_entries: usize,
}

impl AuditLog {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Mutex::new(VecDeque::with_capacity(max_entries)),
            max_entries,
        }
    }

    /// Record a security audit event.
    ///
    /// The event is both stored in memory and emitted via `tracing` so external
    /// log collectors (stdout, file, ELK, etc.) can capture it.
    pub fn log(&self, entry: AuditEntry) {
        // Emit via tracing (never includes passwords or tokens)
        match entry.severity {
            Severity::Critical => {
                tracing::error!(
                    event = %entry.event_type,
                    severity = "critical",
                    ip = %entry.ip_address,
                    user_id = ?entry.user_id,
                    username = ?entry.username,
                    "[AUDIT] {}",
                    entry.description
                );
            }
            Severity::Warning => {
                tracing::warn!(
                    event = %entry.event_type,
                    severity = "warning",
                    ip = %entry.ip_address,
                    user_id = ?entry.user_id,
                    "[AUDIT] {}",
                    entry.description
                );
            }
            Severity::Info => {
                tracing::info!(
                    event = %entry.event_type,
                    severity = "info",
                    ip = %entry.ip_address,
                    user_id = ?entry.user_id,
                    "[AUDIT] {}",
                    entry.description
                );
            }
        }

        // Store in memory ring buffer
        if let Ok(mut entries) = self.entries.lock() {
            if entries.len() >= self.max_entries {
                entries.pop_front();
            }
            entries.push_back(entry);
        }
    }

    /// Convenience: log a login success event.
    pub fn log_login_success(&self, ip: &str, user_id: u64, username: &str) {
        self.log(AuditEntry {
            timestamp: now_unix(),
            event_type: AuditEventType::LoginSuccess,
            severity: Severity::Info,
            ip_address: ip.to_string(),
            user_id: Some(user_id),
            username: Some(username.to_string()),
            description: format!("Successful login for user '{}'", username),
            metadata: None,
        });
    }

    /// Convenience: log a login failure event.
    pub fn log_login_failure(&self, ip: &str, username: &str) {
        self.log(AuditEntry {
            timestamp: now_unix(),
            event_type: AuditEventType::LoginFailure,
            severity: Severity::Warning,
            ip_address: ip.to_string(),
            user_id: None,
            username: Some(username.to_string()),
            description: format!("Failed login attempt for user '{}'", username),
            metadata: None,
        });
    }

    /// Convenience: log a WAF block event.
    pub fn log_waf_block(&self, ip: &str, rule_id: &str, path: &str) {
        self.log(AuditEntry {
            timestamp: now_unix(),
            event_type: AuditEventType::WafBlock,
            severity: Severity::Warning,
            ip_address: ip.to_string(),
            user_id: None,
            username: None,
            description: format!("WAF blocked request to '{}' (rule: {})", path, rule_id),
            metadata: Some(format!("rule_id={}", rule_id)),
        });
    }

    /// Convenience: log a rate limit event.
    pub fn log_rate_limited(&self, ip: &str, path: &str) {
        self.log(AuditEntry {
            timestamp: now_unix(),
            event_type: AuditEventType::RateLimited,
            severity: Severity::Warning,
            ip_address: ip.to_string(),
            user_id: None,
            username: None,
            description: format!("Rate limited request to '{}'", path),
            metadata: None,
        });
    }

    /// Convenience: log a brute force detection.
    pub fn log_brute_force(&self, ip: &str) {
        self.log(AuditEntry {
            timestamp: now_unix(),
            event_type: AuditEventType::BruteForceDetected,
            severity: Severity::Critical,
            ip_address: ip.to_string(),
            user_id: None,
            username: None,
            description: format!("Brute force attack detected from {}", ip),
            metadata: None,
        });
    }

    /// Convenience: log content modification.
    pub fn log_content_change(
        &self,
        event_type: AuditEventType,
        ip: &str,
        user_id: u64,
        description: &str,
    ) {
        self.log(AuditEntry {
            timestamp: now_unix(),
            event_type,
            severity: Severity::Info,
            ip_address: ip.to_string(),
            user_id: Some(user_id),
            username: None,
            description: description.to_string(),
            metadata: None,
        });
    }

    /// Convenience: log a settings change.
    pub fn log_settings_change(&self, ip: &str, user_id: u64, setting: &str) {
        self.log(AuditEntry {
            timestamp: now_unix(),
            event_type: AuditEventType::SettingsChange,
            severity: Severity::Info,
            ip_address: ip.to_string(),
            user_id: Some(user_id),
            username: None,
            description: format!("Settings changed: {}", setting),
            metadata: None,
        });
    }

    /// Get recent audit entries (newest first).
    pub fn recent(&self, limit: usize) -> Vec<AuditEntry> {
        if let Ok(entries) = self.entries.lock() {
            entries.iter().rev().take(limit).cloned().collect()
        } else {
            Vec::new()
        }
    }

    /// Get all entries matching a specific event type.
    pub fn by_type(&self, event_type: &AuditEventType, limit: usize) -> Vec<AuditEntry> {
        if let Ok(entries) = self.entries.lock() {
            entries
                .iter()
                .rev()
                .filter(|e| &e.event_type == event_type)
                .take(limit)
                .cloned()
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Count events of a specific type within the last N seconds.
    pub fn count_since(&self, event_type: &AuditEventType, seconds: u64) -> usize {
        let cutoff = now_unix().saturating_sub(seconds);
        if let Ok(entries) = self.entries.lock() {
            entries
                .iter()
                .rev()
                .take_while(|e| e.timestamp >= cutoff)
                .filter(|e| &e.event_type == event_type)
                .count()
        } else {
            0
        }
    }
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_log_basic() {
        let log = AuditLog::new(100);
        log.log_login_success("192.168.1.1", 1, "admin");
        log.log_login_failure("10.0.0.5", "hacker");
        log.log_waf_block("10.0.0.5", "sqli-001", "/wp-admin/");

        let recent = log.recent(10);
        assert_eq!(recent.len(), 3);
        assert_eq!(recent[0].event_type, AuditEventType::WafBlock);
        assert_eq!(recent[1].event_type, AuditEventType::LoginFailure);
        assert_eq!(recent[2].event_type, AuditEventType::LoginSuccess);
    }

    #[test]
    fn test_audit_log_capacity() {
        let log = AuditLog::new(3);
        for i in 0..5 {
            log.log_login_failure(&format!("10.0.0.{}", i), "test");
        }
        let recent = log.recent(10);
        assert_eq!(recent.len(), 3);
    }

    #[test]
    fn test_audit_log_by_type() {
        let log = AuditLog::new(100);
        log.log_login_success("1.1.1.1", 1, "admin");
        log.log_login_failure("2.2.2.2", "bad");
        log.log_login_failure("3.3.3.3", "worse");

        let failures = log.by_type(&AuditEventType::LoginFailure, 10);
        assert_eq!(failures.len(), 2);
    }

    #[test]
    fn test_count_since() {
        let log = AuditLog::new(100);
        log.log_login_failure("1.1.1.1", "test1");
        log.log_login_failure("1.1.1.1", "test2");

        let count = log.count_since(&AuditEventType::LoginFailure, 60);
        assert_eq!(count, 2);
    }

    #[test]
    fn test_event_type_display() {
        assert_eq!(AuditEventType::LoginSuccess.to_string(), "login_success");
        assert_eq!(AuditEventType::WafBlock.to_string(), "waf_block");
    }

    #[test]
    fn test_severity_ordering() {
        assert!(Severity::Info < Severity::Warning);
        assert!(Severity::Warning < Severity::Critical);
    }
}
