//! Network and site management for WordPress multisite.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use thiserror::Error;

/// Errors that can occur during network/site operations.
#[derive(Debug, Error)]
pub enum MultisiteError {
    #[error("Network not found: {0}")]
    NetworkNotFound(u64),
    #[error("Site not found: blog_id {0}")]
    SiteNotFound(u64),
    #[error("Duplicate domain/path combination: {domain}{path}")]
    DuplicateSite { domain: String, path: String },
    #[error("Network already exists: {0}")]
    NetworkAlreadyExists(u64),
}

/// Represents a WordPress multisite network (wp_site table).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Network {
    /// Network ID (site_id in wp_site).
    pub id: u64,
    /// Primary domain for the network.
    pub domain: String,
    /// Base path for the network (e.g., "/").
    pub path: String,
    /// Human-readable network name.
    pub site_name: String,
    /// Administrator email address.
    pub admin_email: String,
    /// When the network was created.
    pub created_at: DateTime<Utc>,
}

/// Status of a site within the network.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SiteStatus {
    Active,
    Archived,
    Deleted,
    Spam,
}

impl SiteStatus {
    /// Determine the status from the individual flag fields.
    pub fn from_flags(archived: bool, deleted: bool, spam: bool) -> Self {
        if spam {
            SiteStatus::Spam
        } else if deleted {
            SiteStatus::Deleted
        } else if archived {
            SiteStatus::Archived
        } else {
            SiteStatus::Active
        }
    }
}

/// Represents a site (blog) within a multisite network (wp_blogs table).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Site {
    /// Unique blog identifier.
    pub blog_id: u64,
    /// Domain for this site.
    pub domain: String,
    /// Path for this site (e.g., "/" or "/site2/").
    pub path: String,
    /// Network ID this site belongs to (site_id in wp_blogs).
    pub site_id: u64,
    /// When the site was registered.
    pub registered: DateTime<Utc>,
    /// When the site was last updated.
    pub last_updated: DateTime<Utc>,
    /// Whether the site is publicly visible.
    pub public: bool,
    /// Whether the site is archived.
    pub archived: bool,
    /// Whether the site is flagged as mature content.
    pub mature: bool,
    /// Whether the site is flagged as spam.
    pub spam: bool,
    /// Whether the site is soft-deleted.
    pub deleted: bool,
    /// Language ID for the site.
    pub lang_id: u64,
}

impl Site {
    /// Get the computed status of this site based on its flags.
    pub fn status(&self) -> SiteStatus {
        SiteStatus::from_flags(self.archived, self.deleted, self.spam)
    }
}

/// In-memory manager for networks and sites.
///
/// Provides CRUD operations for multisite networks and their associated sites.
/// Thread-safe via interior mutability with `RwLock`.
#[derive(Debug, Clone)]
pub struct NetworkManager {
    networks: Arc<RwLock<HashMap<u64, Network>>>,
    sites: Arc<RwLock<HashMap<u64, Site>>>,
    next_network_id: Arc<RwLock<u64>>,
    next_blog_id: Arc<RwLock<u64>>,
}

impl NetworkManager {
    /// Create a new empty NetworkManager.
    pub fn new() -> Self {
        Self {
            networks: Arc::new(RwLock::new(HashMap::new())),
            sites: Arc::new(RwLock::new(HashMap::new())),
            next_network_id: Arc::new(RwLock::new(1)),
            next_blog_id: Arc::new(RwLock::new(1)),
        }
    }

    /// Create a new network and return it.
    ///
    /// The network is assigned an auto-incrementing ID.
    pub fn create_network(
        &self,
        domain: String,
        path: String,
        site_name: String,
        admin_email: String,
    ) -> Network {
        let mut next_id = self.next_network_id.write().unwrap();
        let id = *next_id;
        *next_id += 1;

        let now = Utc::now();
        let network = Network {
            id,
            domain,
            path,
            site_name,
            admin_email,
            created_at: now,
        };

        self.networks.write().unwrap().insert(id, network.clone());

        tracing::info!(network_id = id, "Created multisite network");
        network
    }

    /// Get a network by its ID.
    pub fn get_network(&self, id: u64) -> Result<Network, MultisiteError> {
        self.networks
            .read()
            .unwrap()
            .get(&id)
            .cloned()
            .ok_or(MultisiteError::NetworkNotFound(id))
    }

    /// Create a new site within a network.
    ///
    /// Returns an error if the network doesn't exist or the domain/path
    /// combination is already taken.
    pub fn create_site(
        &self,
        network_id: u64,
        domain: String,
        path: String,
    ) -> Result<Site, MultisiteError> {
        // Verify network exists
        if !self.networks.read().unwrap().contains_key(&network_id) {
            return Err(MultisiteError::NetworkNotFound(network_id));
        }

        // Check for duplicate domain/path
        {
            let sites = self.sites.read().unwrap();
            for site in sites.values() {
                if site.domain == domain && site.path == path {
                    return Err(MultisiteError::DuplicateSite { domain, path });
                }
            }
        }

        let mut next_id = self.next_blog_id.write().unwrap();
        let blog_id = *next_id;
        *next_id += 1;

        let now = Utc::now();
        let site = Site {
            blog_id,
            domain,
            path,
            site_id: network_id,
            registered: now,
            last_updated: now,
            public: true,
            archived: false,
            mature: false,
            spam: false,
            deleted: false,
            lang_id: 0,
        };

        self.sites.write().unwrap().insert(blog_id, site.clone());

        tracing::info!(blog_id, network_id, "Created site in network");
        Ok(site)
    }

    /// Get a site by its blog ID.
    pub fn get_site(&self, blog_id: u64) -> Result<Site, MultisiteError> {
        self.sites
            .read()
            .unwrap()
            .get(&blog_id)
            .cloned()
            .ok_or(MultisiteError::SiteNotFound(blog_id))
    }

    /// List all sites in a given network.
    pub fn list_sites(&self, network_id: u64) -> Result<Vec<Site>, MultisiteError> {
        if !self.networks.read().unwrap().contains_key(&network_id) {
            return Err(MultisiteError::NetworkNotFound(network_id));
        }

        let sites = self.sites.read().unwrap();
        let result: Vec<Site> = sites
            .values()
            .filter(|s| s.site_id == network_id)
            .cloned()
            .collect();

        Ok(result)
    }

    /// Update a site's mutable fields.
    ///
    /// Accepts a closure that receives a mutable reference to the site.
    /// Returns the updated site or an error if not found.
    pub fn update_site<F>(&self, blog_id: u64, updater: F) -> Result<Site, MultisiteError>
    where
        F: FnOnce(&mut Site),
    {
        let mut sites = self.sites.write().unwrap();
        let site = sites
            .get_mut(&blog_id)
            .ok_or(MultisiteError::SiteNotFound(blog_id))?;

        updater(site);
        site.last_updated = Utc::now();

        tracing::info!(blog_id, "Updated site");
        Ok(site.clone())
    }

    /// Delete a site by marking it as deleted (soft delete).
    ///
    /// This mirrors WordPress behavior where sites are flagged rather than
    /// physically removed from the database.
    pub fn delete_site(&self, blog_id: u64) -> Result<Site, MultisiteError> {
        self.update_site(blog_id, |site| {
            site.deleted = true;
        })
    }
}

impl Default for NetworkManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_network_and_get() {
        let mgr = NetworkManager::new();
        let net = mgr.create_network(
            "example.com".into(),
            "/".into(),
            "My Network".into(),
            "admin@example.com".into(),
        );
        assert_eq!(net.id, 1);
        assert_eq!(net.domain, "example.com");

        let fetched = mgr.get_network(1).unwrap();
        assert_eq!(fetched.site_name, "My Network");
    }

    #[test]
    fn test_get_nonexistent_network() {
        let mgr = NetworkManager::new();
        let result = mgr.get_network(999);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), MultisiteError::NetworkNotFound(999)));
    }

    #[test]
    fn test_create_site_and_list() {
        let mgr = NetworkManager::new();
        let net = mgr.create_network(
            "example.com".into(),
            "/".into(),
            "Net".into(),
            "a@b.com".into(),
        );

        let site1 = mgr
            .create_site(net.id, "example.com".into(), "/".into())
            .unwrap();
        let site2 = mgr
            .create_site(net.id, "example.com".into(), "/blog/".into())
            .unwrap();

        assert_eq!(site1.blog_id, 1);
        assert_eq!(site2.blog_id, 2);

        let sites = mgr.list_sites(net.id).unwrap();
        assert_eq!(sites.len(), 2);
    }

    #[test]
    fn test_duplicate_site_rejected() {
        let mgr = NetworkManager::new();
        mgr.create_network("example.com".into(), "/".into(), "Net".into(), "a@b.com".into());
        mgr.create_site(1, "example.com".into(), "/".into()).unwrap();

        let result = mgr.create_site(1, "example.com".into(), "/".into());
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), MultisiteError::DuplicateSite { .. }));
    }

    #[test]
    fn test_create_site_invalid_network() {
        let mgr = NetworkManager::new();
        let result = mgr.create_site(999, "x.com".into(), "/".into());
        assert!(matches!(result.unwrap_err(), MultisiteError::NetworkNotFound(999)));
    }

    #[test]
    fn test_update_and_delete_site() {
        let mgr = NetworkManager::new();
        mgr.create_network("example.com".into(), "/".into(), "Net".into(), "a@b.com".into());
        mgr.create_site(1, "example.com".into(), "/".into()).unwrap();

        // Update: archive the site
        let updated = mgr
            .update_site(1, |site| {
                site.archived = true;
            })
            .unwrap();
        assert!(updated.archived);
        assert_eq!(updated.status(), SiteStatus::Archived);

        // Delete (soft)
        let deleted = mgr.delete_site(1).unwrap();
        assert!(deleted.deleted);
        assert_eq!(deleted.status(), SiteStatus::Deleted);
    }

    #[test]
    fn test_site_status_from_flags() {
        assert_eq!(SiteStatus::from_flags(false, false, false), SiteStatus::Active);
        assert_eq!(SiteStatus::from_flags(true, false, false), SiteStatus::Archived);
        assert_eq!(SiteStatus::from_flags(false, true, false), SiteStatus::Deleted);
        assert_eq!(SiteStatus::from_flags(false, false, true), SiteStatus::Spam);
        // Spam takes priority
        assert_eq!(SiteStatus::from_flags(true, true, true), SiteStatus::Spam);
    }
}
