//! Multisite routing: resolve incoming requests to specific sites.
//!
//! Supports both subdirectory mode (example.com/site2/) and
//! subdomain mode (site2.example.com), as well as custom domain mapping.

use crate::network::Site;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// The multisite URL scheme in use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MultisiteMode {
    /// Sites are accessed via path prefixes: example.com/site2/
    SubDirectory,
    /// Sites are accessed via subdomains: site2.example.com
    SubDomain,
}

/// Maps a custom domain to a specific blog.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainMapping {
    /// The blog ID this domain maps to.
    pub blog_id: u64,
    /// The custom domain (e.g., "custom.example.org").
    pub domain: String,
    /// Whether this is the primary domain for the blog.
    pub primary: bool,
}

/// Resolves incoming HTTP requests to the correct site in a multisite network.
#[derive(Debug, Clone)]
pub struct SiteResolver {
    /// The mode of multisite URL handling.
    mode: MultisiteMode,
    /// The primary network domain (e.g., "example.com").
    network_domain: String,
    /// All registered sites, keyed by blog_id.
    sites: Arc<RwLock<HashMap<u64, Site>>>,
    /// Custom domain mappings: domain string -> DomainMapping.
    domain_mappings: Arc<RwLock<HashMap<String, DomainMapping>>>,
}

impl SiteResolver {
    /// Create a new SiteResolver.
    ///
    /// # Arguments
    /// * `mode` - Whether to use subdirectory or subdomain routing.
    /// * `network_domain` - The primary domain of the network.
    pub fn new(mode: MultisiteMode, network_domain: String) -> Self {
        Self {
            mode,
            network_domain,
            sites: Arc::new(RwLock::new(HashMap::new())),
            domain_mappings: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a site with the resolver.
    pub fn register_site(&self, site: Site) {
        self.sites.write().unwrap().insert(site.blog_id, site);
    }

    /// Add a custom domain mapping.
    pub fn add_domain_mapping(&self, mapping: DomainMapping) {
        self.domain_mappings
            .write()
            .unwrap()
            .insert(mapping.domain.clone(), mapping);
    }

    /// Remove a custom domain mapping.
    pub fn remove_domain_mapping(&self, domain: &str) {
        self.domain_mappings.write().unwrap().remove(domain);
    }

    /// Resolve an incoming request to a site.
    ///
    /// # Arguments
    /// * `host` - The Host header value (e.g., "site2.example.com" or "example.com").
    /// * `path` - The request path (e.g., "/site2/hello-world").
    ///
    /// # Returns
    /// The matching `Site`, or `None` if no site matches.
    pub fn resolve_site(&self, host: &str, path: &str) -> Option<Site> {
        // Strip port from host if present
        let host_no_port = host.split(':').next().unwrap_or(host);

        // 1. Check custom domain mappings first
        {
            let mappings = self.domain_mappings.read().unwrap();
            if let Some(mapping) = mappings.get(host_no_port) {
                let sites = self.sites.read().unwrap();
                return sites.get(&mapping.blog_id).cloned();
            }
        }

        // 2. Mode-specific resolution
        match self.mode {
            MultisiteMode::SubDomain => self.resolve_subdomain(host_no_port),
            MultisiteMode::SubDirectory => self.resolve_subdirectory(path),
        }
    }

    /// Resolve by subdomain: extract the subdomain prefix from the host.
    ///
    /// For "site2.example.com" with network domain "example.com",
    /// the subdomain is "site2".
    fn resolve_subdomain(&self, host: &str) -> Option<Site> {
        let sites = self.sites.read().unwrap();

        // If host matches network domain exactly, return main site (blog_id 1)
        if host == self.network_domain {
            return sites.values().find(|s| s.blog_id == 1).cloned();
        }

        // Check if host is a subdomain of the network domain
        let suffix = format!(".{}", self.network_domain);
        if host.ends_with(&suffix) {
            let subdomain = &host[..host.len() - suffix.len()];
            // Find site whose domain matches the full host
            return sites
                .values()
                .find(|s| s.domain == host || s.domain == subdomain)
                .cloned();
        }

        None
    }

    /// Resolve by subdirectory: extract the first path segment.
    ///
    /// For path "/site2/hello-world", the site path is "/site2/".
    /// For path "/hello-world" (no prefix), return the main site.
    fn resolve_subdirectory(&self, path: &str) -> Option<Site> {
        let sites = self.sites.read().unwrap();

        // Normalize path
        let path = if path.is_empty() { "/" } else { path };

        // Extract the first path segment as a potential site slug
        let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

        if let Some(first_segment) = segments.first() {
            let site_path = format!("/{}/", first_segment);

            // Try to find a site with this path
            if let Some(site) = sites.values().find(|s| s.path == site_path) {
                return Some(site.clone());
            }
        }

        // Fall back to main site (path = "/")
        sites.values().find(|s| s.path == "/").cloned()
    }

    /// Get the current multisite mode.
    pub fn mode(&self) -> MultisiteMode {
        self.mode
    }

    /// Get the network domain.
    pub fn network_domain(&self) -> &str {
        &self.network_domain
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn make_site(blog_id: u64, domain: &str, path: &str) -> Site {
        let now = Utc::now();
        Site {
            blog_id,
            domain: domain.to_string(),
            path: path.to_string(),
            site_id: 1,
            registered: now,
            last_updated: now,
            public: true,
            archived: false,
            mature: false,
            spam: false,
            deleted: false,
            lang_id: 0,
        }
    }

    #[test]
    fn test_subdirectory_resolve_main_site() {
        let resolver = SiteResolver::new(MultisiteMode::SubDirectory, "example.com".into());
        resolver.register_site(make_site(1, "example.com", "/"));
        resolver.register_site(make_site(2, "example.com", "/blog/"));

        let site = resolver.resolve_site("example.com", "/hello-world").unwrap();
        assert_eq!(site.blog_id, 1);
    }

    #[test]
    fn test_subdirectory_resolve_sub_site() {
        let resolver = SiteResolver::new(MultisiteMode::SubDirectory, "example.com".into());
        resolver.register_site(make_site(1, "example.com", "/"));
        resolver.register_site(make_site(2, "example.com", "/blog/"));

        let site = resolver.resolve_site("example.com", "/blog/my-post").unwrap();
        assert_eq!(site.blog_id, 2);
    }

    #[test]
    fn test_subdomain_resolve_main_site() {
        let resolver = SiteResolver::new(MultisiteMode::SubDomain, "example.com".into());
        resolver.register_site(make_site(1, "example.com", "/"));
        resolver.register_site(make_site(2, "site2.example.com", "/"));

        let site = resolver.resolve_site("example.com", "/").unwrap();
        assert_eq!(site.blog_id, 1);
    }

    #[test]
    fn test_subdomain_resolve_sub_site() {
        let resolver = SiteResolver::new(MultisiteMode::SubDomain, "example.com".into());
        resolver.register_site(make_site(1, "example.com", "/"));
        resolver.register_site(make_site(2, "site2.example.com", "/"));

        let site = resolver.resolve_site("site2.example.com", "/").unwrap();
        assert_eq!(site.blog_id, 2);
    }

    #[test]
    fn test_custom_domain_mapping() {
        let resolver = SiteResolver::new(MultisiteMode::SubDirectory, "example.com".into());
        resolver.register_site(make_site(1, "example.com", "/"));
        resolver.register_site(make_site(2, "example.com", "/blog/"));

        resolver.add_domain_mapping(DomainMapping {
            blog_id: 2,
            domain: "custom.org".into(),
            primary: true,
        });

        // Custom domain should resolve to blog_id 2
        let site = resolver.resolve_site("custom.org", "/any-path").unwrap();
        assert_eq!(site.blog_id, 2);
    }

    #[test]
    fn test_host_with_port_stripped() {
        let resolver = SiteResolver::new(MultisiteMode::SubDomain, "example.com".into());
        resolver.register_site(make_site(1, "example.com", "/"));

        let site = resolver.resolve_site("example.com:8080", "/").unwrap();
        assert_eq!(site.blog_id, 1);
    }

    #[test]
    fn test_no_match_returns_none() {
        let resolver = SiteResolver::new(MultisiteMode::SubDomain, "example.com".into());
        // No sites registered
        let result = resolver.resolve_site("unknown.com", "/");
        assert!(result.is_none());
    }

    #[test]
    fn test_remove_domain_mapping() {
        let resolver = SiteResolver::new(MultisiteMode::SubDomain, "example.com".into());
        resolver.register_site(make_site(1, "example.com", "/"));

        resolver.add_domain_mapping(DomainMapping {
            blog_id: 1,
            domain: "mapped.com".into(),
            primary: true,
        });

        assert!(resolver.resolve_site("mapped.com", "/").is_some());

        resolver.remove_domain_mapping("mapped.com");
        // After removal, mapped.com should not resolve (no subdomain match either)
        assert!(resolver.resolve_site("mapped.com", "/").is_none());
    }
}
