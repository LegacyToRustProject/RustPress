//! SSRF (Server-Side Request Forgery) protection.
//!
//! Prevents RustPress from making HTTP requests to internal/private IP addresses
//! when processing oEmbed, pingback, trackback, or other URL-fetching operations.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// Check if an IP address is a private/internal address that should be blocked.
pub fn is_private_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_private_ipv4(v4),
        IpAddr::V6(v6) => is_private_ipv6(v6),
    }
}

fn is_private_ipv4(ip: Ipv4Addr) -> bool {
    // RFC 1918 private ranges
    ip.is_private()
        // Loopback (127.0.0.0/8)
        || ip.is_loopback()
        // Link-local (169.254.0.0/16) - includes AWS metadata endpoint 169.254.169.254
        || ip.is_link_local()
        // Shared address space (100.64.0.0/10) - RFC 6598
        || (ip.octets()[0] == 100 && (ip.octets()[1] & 0xC0) == 64)
        // Documentation (192.0.2.0/24, 198.51.100.0/24, 203.0.113.0/24)
        || ip.is_documentation()
        // Broadcast
        || ip.is_broadcast()
        // Unspecified (0.0.0.0)
        || ip.is_unspecified()
}

fn is_private_ipv6(ip: Ipv6Addr) -> bool {
    ip.is_loopback()
        || ip.is_unspecified()
        // Unique local address (fc00::/7)
        || (ip.segments()[0] & 0xFE00) == 0xFC00
        // Link-local (fe80::/10)
        || (ip.segments()[0] & 0xFFC0) == 0xFE80
        // IPv4-mapped addresses: check inner IPv4
        || ip.to_ipv4_mapped().is_some_and(is_private_ipv4)
}

/// Validate a URL is safe to fetch (not pointing to private/internal addresses).
///
/// Returns `Ok(())` if the URL is safe, or `Err(reason)` if it should be blocked.
pub fn validate_url(url: &str) -> Result<(), String> {
    let parsed = url::Url::parse(url).map_err(|e| format!("Invalid URL: {e}"))?;

    // Only allow http and https schemes
    match parsed.scheme() {
        "http" | "https" => {}
        scheme => return Err(format!("Blocked scheme: {scheme}")),
    }

    let host = parsed
        .host_str()
        .ok_or_else(|| "No host in URL".to_string())?;

    // Block obvious internal hostnames
    let lower_host = host.to_lowercase();
    if lower_host == "localhost"
        || lower_host == "internal"
        || lower_host.ends_with(".local")
        || lower_host.ends_with(".internal")
    {
        return Err(format!("Blocked internal hostname: {host}"));
    }

    // If the host is a raw IP address, check it directly
    if let Ok(ip) = host.parse::<IpAddr>() {
        if is_private_ip(ip) {
            return Err(format!("Blocked private IP: {ip}"));
        }
    }

    // For domain names, DNS resolution happens at request time.
    // The caller should also validate the resolved IP after DNS lookup
    // to prevent DNS rebinding attacks.
    Ok(())
}

/// Validate a resolved IP address is safe (for use after DNS resolution).
///
/// This should be called after DNS resolution to prevent DNS rebinding attacks
/// where a domain initially resolves to a public IP but later to a private one.
pub fn validate_resolved_ip(ip: IpAddr) -> Result<(), String> {
    if is_private_ip(ip) {
        Err(format!("DNS resolved to private IP: {ip}"))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_private_ipv4() {
        assert!(is_private_ip("127.0.0.1".parse().unwrap()));
        assert!(is_private_ip("10.0.0.1".parse().unwrap()));
        assert!(is_private_ip("172.16.0.1".parse().unwrap()));
        assert!(is_private_ip("192.168.1.1".parse().unwrap()));
        assert!(is_private_ip("169.254.169.254".parse().unwrap())); // AWS metadata
        assert!(is_private_ip("0.0.0.0".parse().unwrap()));

        assert!(!is_private_ip("8.8.8.8".parse().unwrap()));
        assert!(!is_private_ip("1.1.1.1".parse().unwrap()));
    }

    #[test]
    fn test_private_ipv6() {
        assert!(is_private_ip("::1".parse().unwrap()));
        assert!(is_private_ip("::".parse().unwrap()));
        assert!(is_private_ip("fc00::1".parse().unwrap()));
        assert!(is_private_ip("fe80::1".parse().unwrap()));

        assert!(!is_private_ip("2001:4860:4860::8888".parse().unwrap()));
    }

    #[test]
    fn test_validate_url_safe() {
        assert!(validate_url("https://example.com/page").is_ok());
        assert!(validate_url("http://example.com:8080/api").is_ok());
    }

    #[test]
    fn test_validate_url_blocked() {
        assert!(validate_url("http://127.0.0.1/admin").is_err());
        assert!(validate_url("http://169.254.169.254/latest/meta-data/").is_err());
        assert!(validate_url("http://localhost/").is_err());
        assert!(validate_url("http://10.0.0.1/").is_err());
        assert!(validate_url("http://192.168.1.1/").is_err());
        assert!(validate_url("ftp://example.com/file").is_err());
        assert!(validate_url("file:///etc/passwd").is_err());
        assert!(validate_url("gopher://evil.com/").is_err());
    }

    #[test]
    fn test_validate_url_internal_hostnames() {
        assert!(validate_url("http://server.local/api").is_err());
        assert!(validate_url("http://db.internal/query").is_err());
    }

    // --- is_private_ip extended ---

    #[test]
    fn test_private_ip_rfc6598_shared_space() {
        // 100.64.0.0/10 (RFC 6598 shared address space)
        assert!(is_private_ip("100.64.0.1".parse().unwrap()));
        assert!(is_private_ip("100.127.255.255".parse().unwrap()));
    }

    #[test]
    fn test_private_ip_broadcast() {
        assert!(is_private_ip("255.255.255.255".parse().unwrap()));
    }

    #[test]
    fn test_private_ip_documentation_ranges() {
        // 192.0.2.0/24 (TEST-NET-1)
        assert!(is_private_ip("192.0.2.1".parse().unwrap()));
        // 198.51.100.0/24 (TEST-NET-2)
        assert!(is_private_ip("198.51.100.1".parse().unwrap()));
        // 203.0.113.0/24 (TEST-NET-3)
        assert!(is_private_ip("203.0.113.1".parse().unwrap()));
    }

    #[test]
    fn test_public_ip_not_private() {
        assert!(!is_private_ip("8.8.8.8".parse().unwrap()));
        assert!(!is_private_ip("1.1.1.1".parse().unwrap()));
        assert!(!is_private_ip("204.0.0.1".parse().unwrap()));
        assert!(!is_private_ip("93.184.216.34".parse().unwrap())); // example.com
    }

    #[test]
    fn test_private_ipv6_mapped_v4() {
        // IPv4-mapped IPv6: ::ffff:10.0.0.1 should be private
        let ip: IpAddr = "::ffff:10.0.0.1".parse().unwrap();
        assert!(is_private_ip(ip));
    }

    #[test]
    fn test_public_ipv6_not_private() {
        // Google's public IPv6 DNS
        assert!(!is_private_ip("2001:4860:4860::8888".parse().unwrap()));
    }

    // --- validate_url extended ---

    #[test]
    fn test_validate_url_https_allowed() {
        assert!(validate_url("https://example.com/").is_ok());
    }

    #[test]
    fn test_validate_url_http_allowed() {
        assert!(validate_url("http://example.com/").is_ok());
    }

    #[test]
    fn test_validate_url_ftp_blocked() {
        assert!(validate_url("ftp://files.example.com/").is_err());
    }

    #[test]
    fn test_validate_url_file_scheme_blocked() {
        assert!(validate_url("file:///etc/passwd").is_err());
    }

    #[test]
    fn test_validate_url_data_scheme_blocked() {
        assert!(validate_url("data:text/html,<h1>test</h1>").is_err());
    }

    #[test]
    fn test_validate_url_localhost_blocked() {
        assert!(validate_url("http://localhost/").is_err());
    }

    #[test]
    fn test_validate_url_172_16_blocked() {
        assert!(validate_url("http://172.16.0.1/").is_err());
    }

    #[test]
    fn test_validate_url_dot_local_blocked() {
        assert!(validate_url("http://myserver.local/").is_err());
    }

    #[test]
    fn test_validate_url_dot_internal_blocked() {
        assert!(validate_url("http://api.internal/v1/").is_err());
    }

    #[test]
    fn test_validate_url_invalid_string_rejected() {
        assert!(validate_url("not a url").is_err());
        assert!(validate_url("").is_err());
    }

    // --- validate_resolved_ip ---

    #[test]
    fn test_validate_resolved_ip_public_ok() {
        assert!(validate_resolved_ip("8.8.8.8".parse().unwrap()).is_ok());
    }

    #[test]
    fn test_validate_resolved_ip_private_blocked() {
        assert!(validate_resolved_ip("192.168.1.1".parse().unwrap()).is_err());
        assert!(validate_resolved_ip("10.0.0.1".parse().unwrap()).is_err());
        assert!(validate_resolved_ip("127.0.0.1".parse().unwrap()).is_err());
    }

    #[test]
    fn test_validate_resolved_ip_ipv6_private_blocked() {
        assert!(validate_resolved_ip("::1".parse().unwrap()).is_err());
        assert!(validate_resolved_ip("fc00::1".parse().unwrap()).is_err());
    }

    #[test]
    fn test_validate_resolved_ip_ipv6_public_ok() {
        assert!(validate_resolved_ip("2001:4860:4860::8888".parse().unwrap()).is_ok());
    }
}
