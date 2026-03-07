//! RustPress Security - WAF, rate limiting, login protection, and security scanning.
//!
//! A Wordfence-equivalent security plugin for RustPress, providing:
//! - Web Application Firewall (WAF) with configurable rules
//! - Rate limiting with sliding window counters
//! - Login brute-force protection with auto-lockout
//! - Security scanner for common misconfigurations
//! - Security headers with builder pattern

pub mod headers;
pub mod login_protection;
pub mod rate_limiter;
pub mod scanner;
pub mod waf;

pub use headers::SecurityHeaders;
pub use login_protection::LoginProtection;
pub use rate_limiter::{RateLimitResult, RateLimiter};
pub use scanner::{CheckStatus, SecurityCheck, SecurityScanner};
pub use waf::{WafAction, WafEngine, WafResult, WafRule};
