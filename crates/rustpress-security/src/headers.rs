//! Security headers with builder pattern for HTTP responses.

use serde::{Deserialize, Serialize};

/// Security headers configuration with builder pattern.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityHeaders {
    csp: Option<String>,
    x_frame_options: Option<String>,
    x_content_type_options: Option<String>,
    strict_transport_security: Option<String>,
    referrer_policy: Option<String>,
    permissions_policy: Option<String>,
    x_xss_protection: Option<String>,
    cross_origin_opener_policy: Option<String>,
    cross_origin_resource_policy: Option<String>,
}

impl SecurityHeaders {
    /// Create a new builder with no headers set.
    pub fn new() -> Self {
        Self {
            csp: None,
            x_frame_options: None,
            x_content_type_options: None,
            strict_transport_security: None,
            referrer_policy: None,
            permissions_policy: None,
            x_xss_protection: None,
            cross_origin_opener_policy: None,
            cross_origin_resource_policy: None,
        }
    }

    /// Create a builder pre-loaded with secure defaults.
    pub fn secure_defaults() -> Self {
        Self {
            csp: Some(
                "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; \
                 img-src 'self' data: https:; font-src 'self'; object-src 'none'; \
                 frame-ancestors 'self'; base-uri 'self'; form-action 'self'"
                    .into(),
            ),
            x_frame_options: Some("SAMEORIGIN".into()),
            x_content_type_options: Some("nosniff".into()),
            strict_transport_security: Some("max-age=31536000; includeSubDomains".into()),
            referrer_policy: Some("strict-origin-when-cross-origin".into()),
            permissions_policy: Some(
                "camera=(), microphone=(), geolocation=(), payment=(), usb=(), \
                 magnetometer=(), accelerometer=(), gyroscope=()"
                    .into(),
            ),
            x_xss_protection: Some("1; mode=block".into()),
            cross_origin_opener_policy: Some("same-origin".into()),
            cross_origin_resource_policy: Some("same-origin".into()),
        }
    }

    /// Set the Content-Security-Policy header value.
    pub fn content_security_policy(mut self, policy: &str) -> Self {
        self.csp = Some(policy.to_string());
        self
    }

    /// Set the X-Frame-Options header value.
    /// Common values: "DENY", "SAMEORIGIN".
    pub fn x_frame_options(mut self, value: &str) -> Self {
        self.x_frame_options = Some(value.to_string());
        self
    }

    /// Set the X-Content-Type-Options header value.
    /// Typically "nosniff".
    pub fn x_content_type_options(mut self, value: &str) -> Self {
        self.x_content_type_options = Some(value.to_string());
        self
    }

    /// Set the Strict-Transport-Security header value.
    /// Example: "max-age=31536000; includeSubDomains; preload"
    pub fn strict_transport_security(mut self, value: &str) -> Self {
        self.strict_transport_security = Some(value.to_string());
        self
    }

    /// Set the Referrer-Policy header value.
    /// Common values: "no-referrer", "strict-origin-when-cross-origin", "same-origin".
    pub fn referrer_policy(mut self, value: &str) -> Self {
        self.referrer_policy = Some(value.to_string());
        self
    }

    /// Set the Permissions-Policy header value.
    /// Example: "camera=(), microphone=(), geolocation=()"
    pub fn permissions_policy(mut self, value: &str) -> Self {
        self.permissions_policy = Some(value.to_string());
        self
    }

    /// Set the X-XSS-Protection header value.
    pub fn x_xss_protection(mut self, value: &str) -> Self {
        self.x_xss_protection = Some(value.to_string());
        self
    }

    /// Set the Cross-Origin-Opener-Policy header value.
    pub fn cross_origin_opener_policy(mut self, value: &str) -> Self {
        self.cross_origin_opener_policy = Some(value.to_string());
        self
    }

    /// Set the Cross-Origin-Resource-Policy header value.
    pub fn cross_origin_resource_policy(mut self, value: &str) -> Self {
        self.cross_origin_resource_policy = Some(value.to_string());
        self
    }

    /// Remove a specific header by clearing its value.
    pub fn remove_csp(mut self) -> Self {
        self.csp = None;
        self
    }

    /// Generate the list of header name-value pairs.
    pub fn generate_headers(&self) -> Vec<(String, String)> {
        let mut headers = Vec::new();

        if let Some(ref v) = self.csp {
            headers.push(("Content-Security-Policy".into(), v.clone()));
        }
        if let Some(ref v) = self.x_frame_options {
            headers.push(("X-Frame-Options".into(), v.clone()));
        }
        if let Some(ref v) = self.x_content_type_options {
            headers.push(("X-Content-Type-Options".into(), v.clone()));
        }
        if let Some(ref v) = self.strict_transport_security {
            headers.push(("Strict-Transport-Security".into(), v.clone()));
        }
        if let Some(ref v) = self.referrer_policy {
            headers.push(("Referrer-Policy".into(), v.clone()));
        }
        if let Some(ref v) = self.permissions_policy {
            headers.push(("Permissions-Policy".into(), v.clone()));
        }
        if let Some(ref v) = self.x_xss_protection {
            headers.push(("X-XSS-Protection".into(), v.clone()));
        }
        if let Some(ref v) = self.cross_origin_opener_policy {
            headers.push(("Cross-Origin-Opener-Policy".into(), v.clone()));
        }
        if let Some(ref v) = self.cross_origin_resource_policy {
            headers.push(("Cross-Origin-Resource-Policy".into(), v.clone()));
        }

        headers
    }

    /// Get the number of headers that will be generated.
    pub fn header_count(&self) -> usize {
        self.generate_headers().len()
    }
}

impl Default for SecurityHeaders {
    fn default() -> Self {
        Self::secure_defaults()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_secure_defaults_has_all_headers() {
        let headers = SecurityHeaders::secure_defaults();
        let generated = headers.generate_headers();

        let names: Vec<&str> = generated.iter().map(|(k, _)| k.as_str()).collect();
        assert!(names.contains(&"Content-Security-Policy"));
        assert!(names.contains(&"X-Frame-Options"));
        assert!(names.contains(&"X-Content-Type-Options"));
        assert!(names.contains(&"Strict-Transport-Security"));
        assert!(names.contains(&"Referrer-Policy"));
        assert!(names.contains(&"Permissions-Policy"));
        assert!(names.contains(&"X-XSS-Protection"));
    }

    #[test]
    fn test_empty_builder_no_headers() {
        let headers = SecurityHeaders::new();
        let generated = headers.generate_headers();
        assert!(generated.is_empty());
    }

    #[test]
    fn test_builder_pattern() {
        let headers = SecurityHeaders::new()
            .x_frame_options("DENY")
            .x_content_type_options("nosniff")
            .referrer_policy("no-referrer");

        let generated = headers.generate_headers();
        assert_eq!(generated.len(), 3);

        let map: std::collections::HashMap<String, String> =
            generated.into_iter().collect();
        assert_eq!(map.get("X-Frame-Options").unwrap(), "DENY");
        assert_eq!(map.get("X-Content-Type-Options").unwrap(), "nosniff");
        assert_eq!(map.get("Referrer-Policy").unwrap(), "no-referrer");
    }

    #[test]
    fn test_custom_csp() {
        let headers = SecurityHeaders::new()
            .content_security_policy("default-src 'none'");
        let generated = headers.generate_headers();
        assert_eq!(generated.len(), 1);
        assert_eq!(generated[0].0, "Content-Security-Policy");
        assert_eq!(generated[0].1, "default-src 'none'");
    }

    #[test]
    fn test_override_default() {
        let headers = SecurityHeaders::secure_defaults()
            .x_frame_options("DENY");
        let generated = headers.generate_headers();

        let frame_opt: Vec<_> = generated
            .iter()
            .filter(|(k, _)| k == "X-Frame-Options")
            .collect();
        assert_eq!(frame_opt.len(), 1);
        assert_eq!(frame_opt[0].1, "DENY");
    }

    #[test]
    fn test_remove_csp() {
        let headers = SecurityHeaders::secure_defaults().remove_csp();
        let generated = headers.generate_headers();

        let has_csp = generated
            .iter()
            .any(|(k, _)| k == "Content-Security-Policy");
        assert!(!has_csp);
    }

    #[test]
    fn test_hsts_with_preload() {
        let headers = SecurityHeaders::new()
            .strict_transport_security("max-age=63072000; includeSubDomains; preload");
        let generated = headers.generate_headers();
        assert_eq!(generated.len(), 1);
        assert!(generated[0].1.contains("preload"));
    }

    #[test]
    fn test_header_count() {
        let headers = SecurityHeaders::secure_defaults();
        assert_eq!(headers.header_count(), 9);
    }
}
