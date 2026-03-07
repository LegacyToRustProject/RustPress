//! Comprehensive OWASP Top 10 security tests for RustPress.

#[cfg(test)]
mod owasp_tests {
    use crate::audit_log::{AuditEventType, AuditLog};
    use crate::rate_limiter::RateLimiter;
    use crate::ssrf;
    use crate::waf::WafEngine;

    // === A01: Broken Access Control ===

    #[test]
    fn test_waf_blocks_directory_traversal() {
        let waf = WafEngine::with_default_rules();
        let result = waf.check_request(
            "GET",
            "/wp-content/uploads/../../../etc/passwd",
            "",
            "",
            &std::collections::HashMap::new(),
        );
        assert!(matches!(result, crate::WafResult::Block { .. }));
    }

    #[test]
    fn test_waf_blocks_path_traversal_in_path() {
        let waf = WafEngine::with_default_rules();
        // Non-encoded path traversal in the request path itself
        let result = waf.check_request(
            "GET",
            "/wp-content/uploads/../../wp-config.php",
            "",
            "",
            &std::collections::HashMap::new(),
        );
        assert!(matches!(result, crate::WafResult::Block { .. }));
    }

    // === A03: Injection ===

    #[test]
    fn test_waf_blocks_sql_injection_union() {
        let waf = WafEngine::with_default_rules();
        let result = waf.check_request(
            "GET",
            "/wp-json/wp/v2/posts",
            "search=' UNION SELECT * FROM wp_users--",
            "",
            &std::collections::HashMap::new(),
        );
        assert!(matches!(result, crate::WafResult::Block { .. }));
    }

    #[test]
    fn test_waf_blocks_sql_injection_or_1eq1() {
        let waf = WafEngine::with_default_rules();
        let result = waf.check_request(
            "GET",
            "/api/posts",
            "id=1 OR 1=1",
            "",
            &std::collections::HashMap::new(),
        );
        assert!(matches!(result, crate::WafResult::Block { .. }));
    }

    #[test]
    fn test_waf_blocks_xss_script_tag() {
        let waf = WafEngine::with_default_rules();
        let result = waf.check_request(
            "GET",
            "/search",
            "s=<script>alert('xss')</script>",
            "",
            &std::collections::HashMap::new(),
        );
        assert!(matches!(result, crate::WafResult::Block { .. }));
    }

    #[test]
    fn test_waf_blocks_xss_event_handler() {
        let waf = WafEngine::with_default_rules();
        let result = waf.check_request(
            "GET",
            "/page",
            "q=<img onerror=alert(1) src=x>",
            "",
            &std::collections::HashMap::new(),
        );
        assert!(matches!(result, crate::WafResult::Block { .. }));
    }

    #[test]
    fn test_waf_blocks_command_injection() {
        let waf = WafEngine::with_default_rules();
        let result = waf.check_request(
            "GET",
            "/api/system",
            "cmd=;cat /etc/passwd",
            "",
            &std::collections::HashMap::new(),
        );
        assert!(matches!(result, crate::WafResult::Block { .. }));
    }

    #[test]
    fn test_waf_allows_clean_request() {
        let waf = WafEngine::with_default_rules();
        let result = waf.check_request(
            "GET",
            "/wp-json/wp/v2/posts",
            "page=1&per_page=10",
            "",
            &std::collections::HashMap::new(),
        );
        assert!(matches!(result, crate::WafResult::Allow));
    }

    #[test]
    fn test_waf_blocks_sql_injection_drop_table() {
        let waf = WafEngine::with_default_rules();
        let result = waf.check_request(
            "POST",
            "/api/posts",
            "",
            "'; DROP TABLE wp_posts; --",
            &std::collections::HashMap::new(),
        );
        assert!(matches!(result, crate::WafResult::Block { .. }));
    }

    // === A07: Authentication Failures ===

    #[test]
    fn test_rate_limiter_login_endpoint() {
        let mut limiter = RateLimiter::new();
        let ip = "192.168.1.100";
        for _ in 0..60 {
            let result = limiter.check(ip, "/wp-login.php");
            assert!(matches!(result, crate::RateLimitResult::Allowed { .. }));
        }
        let result = limiter.check(ip, "/wp-login.php");
        assert!(matches!(result, crate::RateLimitResult::Limited { .. }));
    }

    #[test]
    fn test_rate_limiter_api_endpoint() {
        let mut limiter = RateLimiter::new();
        let ip = "10.0.0.1";
        for _ in 0..300 {
            let result = limiter.check(ip, "/wp-json/wp/v2/posts");
            assert!(matches!(result, crate::RateLimitResult::Allowed { .. }));
        }
        let result = limiter.check(ip, "/wp-json/wp/v2/posts");
        assert!(matches!(result, crate::RateLimitResult::Limited { .. }));
    }

    #[test]
    fn test_rate_limiter_different_ips_independent() {
        let mut limiter = RateLimiter::new();
        for _ in 0..60 {
            limiter.check("1.1.1.1", "/wp-login.php");
        }
        let result = limiter.check("2.2.2.2", "/wp-login.php");
        assert!(matches!(result, crate::RateLimitResult::Allowed { .. }));
    }

    // === A09: Security Logging ===

    #[test]
    fn test_audit_log_records_events() {
        let log = AuditLog::new(1000);
        log.log_login_success("192.168.1.1", 1, "admin");
        log.log_login_failure("10.0.0.5", "hacker");
        log.log_waf_block("10.0.0.5", "sqli-001", "/wp-admin/");
        log.log_rate_limited("10.0.0.5", "/wp-login.php");
        log.log_brute_force("10.0.0.5");

        let recent = log.recent(10);
        assert_eq!(recent.len(), 5);
        assert_eq!(recent[0].event_type, AuditEventType::BruteForceDetected);
        assert_eq!(recent[4].event_type, AuditEventType::LoginSuccess);
    }

    #[test]
    fn test_audit_log_no_password_in_entries() {
        let log = AuditLog::new(1000);
        log.log_login_failure("1.1.1.1", "admin");
        let entries = log.recent(1);
        assert_eq!(entries.len(), 1);
        assert!(!entries[0].description.contains("password"));
        assert!(entries[0].metadata.is_none());
    }

    #[test]
    fn test_audit_log_count_since() {
        let log = AuditLog::new(1000);
        log.log_login_failure("1.1.1.1", "user1");
        log.log_login_failure("1.1.1.1", "user2");
        log.log_login_success("2.2.2.2", 1, "admin");

        assert_eq!(log.count_since(&AuditEventType::LoginFailure, 60), 2);
        assert_eq!(log.count_since(&AuditEventType::LoginSuccess, 60), 1);
    }

    // === A10: SSRF ===

    #[test]
    fn test_ssrf_blocks_localhost() {
        assert!(ssrf::validate_url("http://127.0.0.1/admin").is_err());
        assert!(ssrf::validate_url("http://localhost/secret").is_err());
    }

    #[test]
    fn test_ssrf_blocks_aws_metadata() {
        assert!(ssrf::validate_url("http://169.254.169.254/latest/meta-data/").is_err());
    }

    #[test]
    fn test_ssrf_blocks_private_networks() {
        assert!(ssrf::validate_url("http://10.0.0.1/internal").is_err());
        assert!(ssrf::validate_url("http://172.16.0.1/admin").is_err());
        assert!(ssrf::validate_url("http://192.168.1.1/config").is_err());
    }

    #[test]
    fn test_ssrf_blocks_non_http_schemes() {
        assert!(ssrf::validate_url("ftp://evil.com/malware").is_err());
        assert!(ssrf::validate_url("file:///etc/passwd").is_err());
        assert!(ssrf::validate_url("gopher://evil.com/").is_err());
    }

    #[test]
    fn test_ssrf_allows_public_urls() {
        assert!(ssrf::validate_url("https://example.com/page").is_ok());
        assert!(ssrf::validate_url("https://wordpress.org/plugins/").is_ok());
    }

    #[test]
    fn test_ssrf_blocks_internal_hostnames() {
        assert!(ssrf::validate_url("http://db.internal/query").is_err());
        assert!(ssrf::validate_url("http://redis.local/").is_err());
    }

    // === A05: Security Misconfiguration ===

    #[test]
    fn test_scanner_detects_debug_mode() {
        use crate::scanner::{ScannerContext, SecurityScanner};
        let ctx = ScannerContext {
            debug_mode: true,
            ..Default::default()
        };
        let scanner = SecurityScanner::new(ctx);
        let checks = scanner.run_all_checks();
        let debug_check = checks.iter().find(|c| c.name == "Debug Mode").unwrap();
        assert_eq!(debug_check.status, crate::CheckStatus::Fail);
    }

    #[test]
    fn test_scanner_detects_default_admin() {
        use crate::scanner::{ScannerContext, SecurityScanner};
        let ctx = ScannerContext {
            admin_usernames: vec!["admin".to_string()],
            ..Default::default()
        };
        let scanner = SecurityScanner::new(ctx);
        let checks = scanner.run_all_checks();
        let admin_check = checks
            .iter()
            .find(|c| c.name == "Default Admin Username")
            .unwrap();
        assert_eq!(admin_check.status, crate::CheckStatus::Warning);
    }

    #[test]
    fn test_scanner_detects_default_prefix() {
        use crate::scanner::{ScannerContext, SecurityScanner};
        let ctx = ScannerContext {
            db_prefix: "wp_".to_string(),
            ..Default::default()
        };
        let scanner = SecurityScanner::new(ctx);
        let checks = scanner.run_all_checks();
        let prefix_check = checks.iter().find(|c| c.name == "Database Prefix").unwrap();
        assert_eq!(prefix_check.status, crate::CheckStatus::Warning);
    }

    // === WAF bypass attempts ===

    #[test]
    fn test_waf_blocks_sql_in_user_agent() {
        let waf = WafEngine::with_default_rules();
        let mut headers = std::collections::HashMap::new();
        headers.insert(
            "user-agent".to_string(),
            "Mozilla/5.0' OR 1=1--".to_string(),
        );
        let result = waf.check_request("GET", "/", "", "", &headers);
        assert!(matches!(result, crate::WafResult::Block { .. }));
    }

    #[test]
    fn test_waf_blocks_sql_comment_bypass() {
        let waf = WafEngine::with_default_rules();
        let result = waf.check_request(
            "GET",
            "/api/posts",
            "id=1/**/UNION/**/SELECT/**/password/**/FROM/**/users",
            "",
            &std::collections::HashMap::new(),
        );
        assert!(matches!(result, crate::WafResult::Block { .. }));
    }

    // === Nonce system (CSRF) ===

    #[test]
    fn test_nonce_prevents_csrf() {
        use rustpress_core::nonce::NonceManager;
        let manager = NonceManager::new("secret-key-for-testing");

        let nonce = manager.create_nonce("save_post", 1);
        assert!(!nonce.is_empty());
        assert!(manager.verify_nonce(&nonce, "save_post", 1).is_some());
        assert!(manager.verify_nonce(&nonce, "delete_post", 1).is_none());
        assert!(manager.verify_nonce(&nonce, "save_post", 2).is_none());
        assert!(manager
            .verify_nonce("fake_nonce!", "save_post", 1)
            .is_none());
    }

    // === Security headers ===

    #[test]
    fn test_security_headers_defaults() {
        use crate::headers::SecurityHeaders;
        let headers = SecurityHeaders::secure_defaults();
        let header_list = headers.generate_headers();
        let header_names: Vec<&str> = header_list.iter().map(|(k, _)| k.as_str()).collect();
        assert!(header_names.contains(&"X-Content-Type-Options"));
        assert!(header_names.contains(&"X-Frame-Options"));
        assert!(header_names.contains(&"Referrer-Policy"));
        assert!(header_names.contains(&"Content-Security-Policy"));
    }

    #[test]
    fn test_security_headers_count() {
        use crate::headers::SecurityHeaders;
        let headers = SecurityHeaders::secure_defaults();
        assert!(headers.header_count() >= 5);
    }
}
