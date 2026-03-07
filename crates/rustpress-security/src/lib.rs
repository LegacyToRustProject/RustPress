//! RustPress Security - WAF, rate limiting, login protection, and security scanning.
//!
//! A Wordfence-equivalent security plugin for RustPress, providing:
//! - Web Application Firewall (WAF) with configurable rules
//! - Rate limiting with sliding window counters
//! - Login brute-force protection with auto-lockout
//! - Security scanner for common misconfigurations
//! - Security headers with builder pattern
//! - Audit logging for security events (OWASP A09)
//! - SSRF protection for outbound requests (OWASP A10)

pub mod audit_log;
pub mod headers;
pub mod login_protection;
pub mod rate_limiter;
pub mod scanner;
pub mod ssrf;
pub mod waf;
pub mod wordfence_compat;

#[cfg(test)]
mod tests;

pub use audit_log::AuditLog;
pub use headers::SecurityHeaders;
pub use login_protection::LoginProtection;
pub use rate_limiter::{RateLimitResult, RateLimiter};
pub use scanner::{CheckStatus, SecurityCheck, SecurityScanner};
pub use ssrf::{validate_resolved_ip, validate_url};
pub use waf::{WafAction, WafEngine, WafResult, WafRule};
pub use wordfence_compat::WordfenceSettings;
