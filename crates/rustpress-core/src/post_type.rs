use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};
use tracing::debug;

/// Describes the registration arguments for a custom post type.
///
/// Equivalent to the `$args` parameter of WordPress `register_post_type()`.
#[derive(Debug, Clone)]
pub struct PostTypeArgs {
    pub label: String,
    pub labels: PostTypeLabels,
    pub public: bool,
    pub hierarchical: bool,
    pub show_ui: bool,
    pub show_in_menu: bool,
    pub show_in_rest: bool,
    pub rest_base: Option<String>,
    pub menu_position: Option<i32>,
    pub menu_icon: Option<String>,
    pub supports: Vec<PostTypeSupport>,
    pub has_archive: bool,
    pub rewrite: Option<PostTypeRewrite>,
    pub capability_type: String,
}

impl Default for PostTypeArgs {
    fn default() -> Self {
        Self {
            label: String::new(),
            labels: PostTypeLabels::default(),
            public: true,
            hierarchical: false,
            show_ui: true,
            show_in_menu: true,
            show_in_rest: true,
            rest_base: None,
            menu_position: None,
            menu_icon: None,
            supports: vec![PostTypeSupport::Title, PostTypeSupport::Editor],
            has_archive: false,
            rewrite: None,
            capability_type: "post".to_string(),
        }
    }
}

/// Labels for a post type (used in admin UI).
#[derive(Debug, Clone, Default)]
pub struct PostTypeLabels {
    pub name: String,
    pub singular_name: String,
    pub add_new: String,
    pub add_new_item: String,
    pub edit_item: String,
    pub new_item: String,
    pub view_item: String,
    pub search_items: String,
    pub not_found: String,
    pub not_found_in_trash: String,
    pub all_items: String,
}

/// Features a post type supports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PostTypeSupport {
    Title,
    Editor,
    Author,
    Thumbnail,
    Excerpt,
    Trackbacks,
    CustomFields,
    Comments,
    Revisions,
    PageAttributes,
    PostFormats,
}

/// Rewrite rules for a post type.
#[derive(Debug, Clone)]
pub struct PostTypeRewrite {
    pub slug: String,
    pub with_front: bool,
}

/// A registered post type definition.
#[derive(Debug, Clone)]
pub struct PostTypeDefinition {
    pub name: String,
    pub args: PostTypeArgs,
    pub builtin: bool,
}

/// Registry for post types (built-in and custom).
///
/// Equivalent to the global `$wp_post_types` in WordPress.
#[derive(Clone, Default)]
pub struct PostTypeRegistry {
    types: Arc<RwLock<BTreeMap<String, PostTypeDefinition>>>,
}

impl PostTypeRegistry {
    pub fn new() -> Self {
        let registry = Self::default();
        registry.register_builtin_types();
        registry
    }

    fn register_builtin_types(&self) {
        // post
        self.register(
            "post",
            PostTypeArgs {
                label: "Posts".to_string(),
                labels: PostTypeLabels {
                    name: "Posts".to_string(),
                    singular_name: "Post".to_string(),
                    add_new: "Add New".to_string(),
                    add_new_item: "Add New Post".to_string(),
                    edit_item: "Edit Post".to_string(),
                    ..Default::default()
                },
                supports: vec![
                    PostTypeSupport::Title,
                    PostTypeSupport::Editor,
                    PostTypeSupport::Author,
                    PostTypeSupport::Thumbnail,
                    PostTypeSupport::Excerpt,
                    PostTypeSupport::Comments,
                    PostTypeSupport::Revisions,
                ],
                has_archive: true,
                ..Default::default()
            },
            true,
        );

        // page
        self.register(
            "page",
            PostTypeArgs {
                label: "Pages".to_string(),
                labels: PostTypeLabels {
                    name: "Pages".to_string(),
                    singular_name: "Page".to_string(),
                    add_new: "Add New".to_string(),
                    add_new_item: "Add New Page".to_string(),
                    edit_item: "Edit Page".to_string(),
                    ..Default::default()
                },
                hierarchical: true,
                supports: vec![
                    PostTypeSupport::Title,
                    PostTypeSupport::Editor,
                    PostTypeSupport::Author,
                    PostTypeSupport::Thumbnail,
                    PostTypeSupport::PageAttributes,
                    PostTypeSupport::Revisions,
                ],
                ..Default::default()
            },
            true,
        );

        // attachment
        self.register(
            "attachment",
            PostTypeArgs {
                label: "Media".to_string(),
                labels: PostTypeLabels {
                    name: "Media".to_string(),
                    singular_name: "Media".to_string(),
                    ..Default::default()
                },
                public: true,
                show_ui: true,
                supports: vec![
                    PostTypeSupport::Title,
                    PostTypeSupport::Author,
                    PostTypeSupport::Comments,
                ],
                ..Default::default()
            },
            true,
        );

        // revision
        self.register(
            "revision",
            PostTypeArgs {
                label: "Revisions".to_string(),
                public: false,
                show_ui: false,
                show_in_menu: false,
                show_in_rest: false,
                ..Default::default()
            },
            true,
        );

        // nav_menu_item
        self.register(
            "nav_menu_item",
            PostTypeArgs {
                label: "Navigation Menu Items".to_string(),
                public: false,
                show_ui: false,
                show_in_menu: false,
                show_in_rest: true,
                rest_base: Some("menu-items".to_string()),
                ..Default::default()
            },
            true,
        );
    }

    /// Register a new post type.
    ///
    /// Equivalent to WordPress `register_post_type($post_type, $args)`.
    pub fn register(&self, name: &str, args: PostTypeArgs, builtin: bool) {
        let mut types = self.types.write().expect("post type lock poisoned");
        debug!(name, "post type registered");
        types.insert(
            name.to_string(),
            PostTypeDefinition {
                name: name.to_string(),
                args,
                builtin,
            },
        );
    }

    /// Get a registered post type by name.
    pub fn get(&self, name: &str) -> Option<PostTypeDefinition> {
        let types = self.types.read().expect("post type lock poisoned");
        types.get(name).cloned()
    }

    /// Get all registered post types.
    pub fn get_all(&self) -> Vec<PostTypeDefinition> {
        let types = self.types.read().expect("post type lock poisoned");
        types.values().cloned().collect()
    }

    /// Get all public post types (for REST API).
    pub fn get_public(&self) -> Vec<PostTypeDefinition> {
        let types = self.types.read().expect("post type lock poisoned");
        types
            .values()
            .filter(|pt| pt.args.public)
            .cloned()
            .collect()
    }

    /// Get all post types visible in REST API.
    pub fn get_rest_visible(&self) -> Vec<PostTypeDefinition> {
        let types = self.types.read().expect("post type lock poisoned");
        types
            .values()
            .filter(|pt| pt.args.show_in_rest)
            .cloned()
            .collect()
    }

    /// Check if a post type is registered.
    pub fn exists(&self, name: &str) -> bool {
        let types = self.types.read().expect("post type lock poisoned");
        types.contains_key(name)
    }

    /// Check if a post type supports a feature.
    pub fn supports(&self, name: &str, feature: &PostTypeSupport) -> bool {
        let types = self.types.read().expect("post type lock poisoned");
        types
            .get(name)
            .is_some_and(|pt| pt.args.supports.contains(feature))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_types_registered() {
        let registry = PostTypeRegistry::new();
        assert!(registry.exists("post"));
        assert!(registry.exists("page"));
        assert!(registry.exists("attachment"));
        assert!(registry.exists("revision"));
        assert!(registry.exists("nav_menu_item"));
    }

    #[test]
    fn test_register_custom_post_type() {
        let registry = PostTypeRegistry::new();
        registry.register(
            "product",
            PostTypeArgs {
                label: "Products".to_string(),
                has_archive: true,
                supports: vec![
                    PostTypeSupport::Title,
                    PostTypeSupport::Editor,
                    PostTypeSupport::Thumbnail,
                ],
                ..Default::default()
            },
            false,
        );
        assert!(registry.exists("product"));
        let pt = registry.get("product").unwrap();
        assert!(!pt.builtin);
        assert_eq!(pt.args.label, "Products");
    }

    #[test]
    fn test_supports() {
        let registry = PostTypeRegistry::new();
        assert!(registry.supports("post", &PostTypeSupport::Title));
        assert!(registry.supports("post", &PostTypeSupport::Comments));
        assert!(!registry.supports("post", &PostTypeSupport::PageAttributes));
        assert!(registry.supports("page", &PostTypeSupport::PageAttributes));
    }

    #[test]
    fn test_get_public() {
        let registry = PostTypeRegistry::new();
        let public = registry.get_public();
        let names: Vec<&str> = public.iter().map(|pt| pt.name.as_str()).collect();
        assert!(names.contains(&"post"));
        assert!(names.contains(&"page"));
        assert!(!names.contains(&"revision"));
    }
}
