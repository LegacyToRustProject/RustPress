//! Login audit helpers for the auth crate.
//!
//! Provides utility functions for extracting audit-relevant data from HTTP
//! headers and re-exports the core audit types from `rustpress-security`.

use axum::http::HeaderMap;

/// Extract the client IP from request headers.
///
/// Checks `X-Forwarded-For` (first IP in comma-separated list), then
/// `X-Real-IP`, falling back to `"unknown"`.
pub fn extract_client_ip(headers: &HeaderMap) -> String {
    headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.split(',').next().unwrap_or("unknown").trim().to_string())
        .or_else(|| {
            headers
                .get("x-real-ip")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| "unknown".to_string())
}

/// Extract the User-Agent string from request headers.
pub fn extract_user_agent(headers: &HeaderMap) -> Option<String> {
    headers
        .get(axum::http::header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderMap;
    use rustpress_security::audit_log::{AuditEventType, AuditLog};

    #[test]
    fn test_login_success_is_recorded() {
        let log = AuditLog::new(100);
        log.log_login_success_with_ua("192.168.1.1", 1, "admin", Some("Mozilla/5.0"));

        let entries = log.login_events(10);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].event_type, AuditEventType::LoginSuccess);
        assert_eq!(entries[0].username, Some("admin".to_string()));
        assert_eq!(
            entries[0].user_agent,
            Some("Mozilla/5.0".to_string())
        );
    }

    #[test]
    fn test_login_failure_is_recorded() {
        let log = AuditLog::new(100);
        log.log_login_failure_with_ua("10.0.0.5", "hacker", Some("curl/7.68.0"));

        let entries = log.login_events(10);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].event_type, AuditEventType::LoginFailure);
        assert_eq!(entries[0].username, Some("hacker".to_string()));
        assert_eq!(
            entries[0].user_agent,
            Some("curl/7.68.0".to_string())
        );
    }

    #[test]
    fn test_ip_recorded_correctly() {
        let log = AuditLog::new(100);
        log.log_login_failure_with_ua("203.0.113.42", "user1", None);

        let entries = log.login_events(10);
        assert_eq!(entries[0].ip_address, "203.0.113.42");
    }

    #[test]
    fn test_extract_client_ip_x_forwarded_for() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "203.0.113.50, 70.41.3.18".parse().unwrap());

        assert_eq!(extract_client_ip(&headers), "203.0.113.50");
    }

    #[test]
    fn test_extract_client_ip_x_real_ip() {
        let mut headers = HeaderMap::new();
        headers.insert("x-real-ip", "198.51.100.1".parse().unwrap());

        assert_eq!(extract_client_ip(&headers), "198.51.100.1");
    }

    #[test]
    fn test_extract_client_ip_no_header() {
        let headers = HeaderMap::new();
        assert_eq!(extract_client_ip(&headers), "unknown");
    }

    #[test]
    fn test_extract_client_ip_forwarded_for_takes_priority() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "1.2.3.4".parse().unwrap());
        headers.insert("x-real-ip", "5.6.7.8".parse().unwrap());

        assert_eq!(extract_client_ip(&headers), "1.2.3.4");
    }

    #[test]
    fn test_extract_user_agent() {
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::USER_AGENT,
            "Mozilla/5.0 (X11; Linux x86_64)".parse().unwrap(),
        );

        assert_eq!(
            extract_user_agent(&headers),
            Some("Mozilla/5.0 (X11; Linux x86_64)".to_string())
        );
    }

    #[test]
    fn test_extract_user_agent_missing() {
        let headers = HeaderMap::new();
        assert_eq!(extract_user_agent(&headers), None);
    }

    #[test]
    fn test_login_events_filters_only_login_types() {
        let log = AuditLog::new(100);
        log.log_login_success("1.1.1.1", 1, "admin");
        log.log_waf_block("2.2.2.2", "rule-1", "/path");
        log.log_login_failure("3.3.3.3", "bad");
        log.log_rate_limited("4.4.4.4", "/api");

        let login_entries = log.login_events(10);
        assert_eq!(login_entries.len(), 2);
        // newest first
        assert_eq!(login_entries[0].event_type, AuditEventType::LoginFailure);
        assert_eq!(login_entries[1].event_type, AuditEventType::LoginSuccess);
    }

    #[test]
    fn test_login_without_user_agent() {
        let log = AuditLog::new(100);
        log.log_login_success("1.1.1.1", 1, "admin");

        let entries = log.login_events(10);
        assert_eq!(entries[0].user_agent, None);
    }
}
