//! Blog switching support (switch_to_blog / restore_current_blog).
//!
//! WordPress allows temporarily switching the "current blog" context so that
//! functions like `get_option()` read from a different site's tables.
//! This module provides a stack-based switching mechanism.

use crate::tables;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};

/// Represents the context of the currently active blog.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlogContext {
    /// The blog ID of the current context.
    pub current_blog_id: u64,
    /// The table prefix for this blog (e.g., "wp_" or "wp_2_").
    pub table_prefix: String,
    /// The site URL for this blog.
    pub site_url: String,
    /// Whether we've switched away from the original blog.
    pub is_switched: bool,
}

impl BlogContext {
    /// Create a new BlogContext for a given blog.
    pub fn new(blog_id: u64, site_url: String) -> Self {
        let table_prefix = if tables::is_main_site(blog_id) {
            "wp_".to_string()
        } else {
            format!("wp_{}_", blog_id)
        };

        Self {
            current_blog_id: blog_id,
            table_prefix,
            site_url,
            is_switched: false,
        }
    }

    /// Get the full table name for a base table in this blog's context.
    pub fn table(&self, base_table: &str) -> String {
        tables::table_name(self.current_blog_id, base_table)
    }
}

/// URL resolver callback type for looking up site URLs by blog_id.
type UrlResolver = Arc<dyn Fn(u64) -> String + Send + Sync>;

/// Manages blog switching with a stack-based approach.
///
/// Supports nested switching: you can switch to blog A, then to blog B,
/// then restore back to A, then restore back to the original.
#[derive(Clone)]
pub struct SwitchManager {
    /// Stack of previous blog contexts. The current context is at the top.
    stack: Arc<RwLock<Vec<BlogContext>>>,
    /// The current active context.
    current: Arc<RwLock<BlogContext>>,
    /// Function to resolve a blog_id to its site URL.
    url_resolver: UrlResolver,
}

impl std::fmt::Debug for SwitchManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SwitchManager")
            .field("stack", &self.stack)
            .field("current", &self.current)
            .finish()
    }
}

impl SwitchManager {
    /// Create a new SwitchManager starting at the given blog.
    ///
    /// # Arguments
    /// * `initial_blog_id` - The blog_id to start with (usually 1).
    /// * `url_resolver` - A function that returns the site URL for a given blog_id.
    pub fn new<F>(initial_blog_id: u64, url_resolver: F) -> Self
    where
        F: Fn(u64) -> String + Send + Sync + 'static,
    {
        let resolver = Arc::new(url_resolver);
        let initial_url = resolver(initial_blog_id);
        let context = BlogContext::new(initial_blog_id, initial_url);

        Self {
            stack: Arc::new(RwLock::new(Vec::new())),
            current: Arc::new(RwLock::new(context)),
            url_resolver: resolver,
        }
    }

    /// Switch to a different blog.
    ///
    /// The current blog context is pushed onto the stack, and a new context
    /// is created for the target blog. Returns the new context.
    ///
    /// This mirrors WordPress's `switch_to_blog()` function.
    pub fn switch_to_blog(&self, blog_id: u64) -> BlogContext {
        let mut stack = self.stack.write().unwrap();
        let mut current = self.current.write().unwrap();

        // Push current context onto the stack
        stack.push(current.clone());

        // Create new context for the target blog
        let site_url = (self.url_resolver)(blog_id);
        let mut new_context = BlogContext::new(blog_id, site_url);
        new_context.is_switched = true;

        *current = new_context.clone();

        tracing::debug!(blog_id, "Switched to blog");
        new_context
    }

    /// Restore the previous blog context from the stack.
    ///
    /// Returns the restored context, or the current context if the stack is empty
    /// (i.e., we're already at the original blog).
    ///
    /// This mirrors WordPress's `restore_current_blog()` function.
    pub fn restore_current_blog(&self) -> BlogContext {
        let mut stack = self.stack.write().unwrap();
        let mut current = self.current.write().unwrap();

        if let Some(previous) = stack.pop() {
            let mut restored = previous;
            // If the stack is now empty, we're back to the original context
            restored.is_switched = !stack.is_empty();
            *current = restored.clone();

            tracing::debug!(blog_id = current.current_blog_id, "Restored blog context");
            restored
        } else {
            // Already at the original blog
            current.clone()
        }
    }

    /// Get the current blog ID.
    pub fn get_current_blog_id(&self) -> u64 {
        self.current.read().unwrap().current_blog_id
    }

    /// Get a clone of the current blog context.
    pub fn current_context(&self) -> BlogContext {
        self.current.read().unwrap().clone()
    }

    /// Check if we're currently switched away from the original blog.
    pub fn is_switched(&self) -> bool {
        self.current.read().unwrap().is_switched
    }

    /// Get the depth of the switch stack (number of pending restores).
    pub fn switch_depth(&self) -> usize {
        self.stack.read().unwrap().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_manager() -> SwitchManager {
        SwitchManager::new(1, |id| format!("http://example.com/site/{}", id))
    }

    #[test]
    fn test_initial_state() {
        let mgr = make_manager();
        assert_eq!(mgr.get_current_blog_id(), 1);
        assert!(!mgr.is_switched());
        assert_eq!(mgr.switch_depth(), 0);
    }

    #[test]
    fn test_switch_to_blog() {
        let mgr = make_manager();

        let ctx = mgr.switch_to_blog(2);
        assert_eq!(ctx.current_blog_id, 2);
        assert!(ctx.is_switched);
        assert_eq!(ctx.table_prefix, "wp_2_");
        assert_eq!(mgr.get_current_blog_id(), 2);
        assert_eq!(mgr.switch_depth(), 1);
    }

    #[test]
    fn test_switch_and_restore() {
        let mgr = make_manager();

        mgr.switch_to_blog(2);
        assert_eq!(mgr.get_current_blog_id(), 2);

        let restored = mgr.restore_current_blog();
        assert_eq!(restored.current_blog_id, 1);
        assert!(!restored.is_switched);
        assert_eq!(mgr.switch_depth(), 0);
    }

    #[test]
    fn test_nested_switching() {
        let mgr = make_manager();

        // Switch 1 -> 2 -> 3
        mgr.switch_to_blog(2);
        mgr.switch_to_blog(3);
        assert_eq!(mgr.get_current_blog_id(), 3);
        assert_eq!(mgr.switch_depth(), 2);

        // Restore 3 -> 2
        let ctx = mgr.restore_current_blog();
        assert_eq!(ctx.current_blog_id, 2);
        assert!(ctx.is_switched); // still switched (stack not empty)
        assert_eq!(mgr.switch_depth(), 1);

        // Restore 2 -> 1
        let ctx = mgr.restore_current_blog();
        assert_eq!(ctx.current_blog_id, 1);
        assert!(!ctx.is_switched); // back to original
        assert_eq!(mgr.switch_depth(), 0);
    }

    #[test]
    fn test_restore_at_bottom_is_noop() {
        let mgr = make_manager();

        // Restoring when we haven't switched should return current context
        let ctx = mgr.restore_current_blog();
        assert_eq!(ctx.current_blog_id, 1);
        assert!(!ctx.is_switched);
    }

    #[test]
    fn test_blog_context_table_name() {
        let ctx = BlogContext::new(1, "http://example.com".into());
        assert_eq!(ctx.table("posts"), "wp_posts");
        assert_eq!(ctx.table_prefix, "wp_");

        let ctx2 = BlogContext::new(5, "http://example.com/site/5".into());
        assert_eq!(ctx2.table("posts"), "wp_5_posts");
        assert_eq!(ctx2.table_prefix, "wp_5_");
    }

    #[test]
    fn test_current_context_clone() {
        let mgr = make_manager();
        let ctx = mgr.current_context();
        assert_eq!(ctx.current_blog_id, 1);
        assert_eq!(ctx.site_url, "http://example.com/site/1");
    }
}
