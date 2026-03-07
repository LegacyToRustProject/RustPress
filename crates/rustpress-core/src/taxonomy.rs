use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};
use tracing::debug;

/// Describes the registration arguments for a taxonomy.
///
/// Equivalent to the `$args` parameter of WordPress `register_taxonomy()`.
#[derive(Debug, Clone)]
pub struct TaxonomyArgs {
    pub label: String,
    pub labels: TaxonomyLabels,
    pub public: bool,
    pub hierarchical: bool,
    pub show_ui: bool,
    pub show_in_menu: bool,
    pub show_in_rest: bool,
    pub rest_base: Option<String>,
    pub show_tagcloud: bool,
    pub show_admin_column: bool,
    pub rewrite: Option<TaxonomyRewrite>,
}

impl Default for TaxonomyArgs {
    fn default() -> Self {
        Self {
            label: String::new(),
            labels: TaxonomyLabels::default(),
            public: true,
            hierarchical: false,
            show_ui: true,
            show_in_menu: true,
            show_in_rest: true,
            rest_base: None,
            show_tagcloud: true,
            show_admin_column: false,
            rewrite: None,
        }
    }
}

/// Labels for a taxonomy (used in admin UI).
#[derive(Debug, Clone, Default)]
pub struct TaxonomyLabels {
    pub name: String,
    pub singular_name: String,
    pub search_items: String,
    pub all_items: String,
    pub parent_item: String,
    pub edit_item: String,
    pub update_item: String,
    pub add_new_item: String,
    pub new_item_name: String,
    pub menu_name: String,
}

/// Rewrite rules for a taxonomy.
#[derive(Debug, Clone)]
pub struct TaxonomyRewrite {
    pub slug: String,
    pub with_front: bool,
    pub hierarchical: bool,
}

/// A registered taxonomy definition.
#[derive(Debug, Clone)]
pub struct TaxonomyDefinition {
    pub name: String,
    pub object_types: Vec<String>,
    pub args: TaxonomyArgs,
    pub builtin: bool,
}

/// Registry for taxonomies (built-in and custom).
///
/// Equivalent to the global `$wp_taxonomies` in WordPress.
#[derive(Clone, Default)]
pub struct TaxonomyRegistry {
    taxonomies: Arc<RwLock<BTreeMap<String, TaxonomyDefinition>>>,
}

impl TaxonomyRegistry {
    pub fn new() -> Self {
        let registry = Self::default();
        registry.register_builtin_taxonomies();
        registry
    }

    fn register_builtin_taxonomies(&self) {
        // category
        self.register(
            "category",
            vec!["post".to_string()],
            TaxonomyArgs {
                label: "Categories".to_string(),
                labels: TaxonomyLabels {
                    name: "Categories".to_string(),
                    singular_name: "Category".to_string(),
                    search_items: "Search Categories".to_string(),
                    all_items: "All Categories".to_string(),
                    parent_item: "Parent Category".to_string(),
                    edit_item: "Edit Category".to_string(),
                    update_item: "Update Category".to_string(),
                    add_new_item: "Add New Category".to_string(),
                    new_item_name: "New Category Name".to_string(),
                    menu_name: "Categories".to_string(),
                },
                hierarchical: true,
                show_admin_column: true,
                rest_base: Some("categories".to_string()),
                ..Default::default()
            },
            true,
        );

        // post_tag
        self.register(
            "post_tag",
            vec!["post".to_string()],
            TaxonomyArgs {
                label: "Tags".to_string(),
                labels: TaxonomyLabels {
                    name: "Tags".to_string(),
                    singular_name: "Tag".to_string(),
                    search_items: "Search Tags".to_string(),
                    all_items: "All Tags".to_string(),
                    edit_item: "Edit Tag".to_string(),
                    update_item: "Update Tag".to_string(),
                    add_new_item: "Add New Tag".to_string(),
                    new_item_name: "New Tag Name".to_string(),
                    menu_name: "Tags".to_string(),
                    ..Default::default()
                },
                hierarchical: false,
                show_admin_column: true,
                show_tagcloud: true,
                rest_base: Some("tags".to_string()),
                ..Default::default()
            },
            true,
        );

        // nav_menu
        self.register(
            "nav_menu",
            vec!["nav_menu_item".to_string()],
            TaxonomyArgs {
                label: "Navigation Menus".to_string(),
                public: false,
                show_ui: false,
                show_in_menu: false,
                show_in_rest: true,
                rest_base: Some("menus".to_string()),
                ..Default::default()
            },
            true,
        );

        // post_format
        self.register(
            "post_format",
            vec!["post".to_string()],
            TaxonomyArgs {
                label: "Post Formats".to_string(),
                public: false,
                show_ui: false,
                show_in_rest: false,
                ..Default::default()
            },
            true,
        );
    }

    /// Register a new taxonomy.
    ///
    /// Equivalent to WordPress `register_taxonomy($taxonomy, $object_type, $args)`.
    pub fn register(
        &self,
        name: &str,
        object_types: Vec<String>,
        args: TaxonomyArgs,
        builtin: bool,
    ) {
        let mut taxonomies = self.taxonomies.write().expect("taxonomy lock poisoned");
        debug!(name, ?object_types, "taxonomy registered");
        taxonomies.insert(
            name.to_string(),
            TaxonomyDefinition {
                name: name.to_string(),
                object_types,
                args,
                builtin,
            },
        );
    }

    /// Get a registered taxonomy by name.
    pub fn get(&self, name: &str) -> Option<TaxonomyDefinition> {
        let taxonomies = self.taxonomies.read().expect("taxonomy lock poisoned");
        taxonomies.get(name).cloned()
    }

    /// Get all registered taxonomies.
    pub fn get_all(&self) -> Vec<TaxonomyDefinition> {
        let taxonomies = self.taxonomies.read().expect("taxonomy lock poisoned");
        taxonomies.values().cloned().collect()
    }

    /// Get taxonomies for a specific object type.
    pub fn get_for_object_type(&self, object_type: &str) -> Vec<TaxonomyDefinition> {
        let taxonomies = self.taxonomies.read().expect("taxonomy lock poisoned");
        taxonomies
            .values()
            .filter(|t| t.object_types.iter().any(|ot| ot == object_type))
            .cloned()
            .collect()
    }

    /// Check if a taxonomy is registered.
    pub fn exists(&self, name: &str) -> bool {
        let taxonomies = self.taxonomies.read().expect("taxonomy lock poisoned");
        taxonomies.contains_key(name)
    }

    /// Check if a taxonomy is hierarchical.
    pub fn is_hierarchical(&self, name: &str) -> bool {
        let taxonomies = self.taxonomies.read().expect("taxonomy lock poisoned");
        taxonomies.get(name).is_some_and(|t| t.args.hierarchical)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_taxonomies() {
        let registry = TaxonomyRegistry::new();
        assert!(registry.exists("category"));
        assert!(registry.exists("post_tag"));
        assert!(registry.exists("nav_menu"));
        assert!(registry.exists("post_format"));
    }

    #[test]
    fn test_hierarchical() {
        let registry = TaxonomyRegistry::new();
        assert!(registry.is_hierarchical("category"));
        assert!(!registry.is_hierarchical("post_tag"));
    }

    #[test]
    fn test_register_custom_taxonomy() {
        let registry = TaxonomyRegistry::new();
        registry.register(
            "product_cat",
            vec!["product".to_string()],
            TaxonomyArgs {
                label: "Product Categories".to_string(),
                hierarchical: true,
                ..Default::default()
            },
            false,
        );
        assert!(registry.exists("product_cat"));
        let tax = registry.get("product_cat").unwrap();
        assert!(!tax.builtin);
        assert!(tax.args.hierarchical);
    }

    #[test]
    fn test_get_for_object_type() {
        let registry = TaxonomyRegistry::new();
        let post_taxonomies = registry.get_for_object_type("post");
        let names: Vec<&str> = post_taxonomies.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"category"));
        assert!(names.contains(&"post_tag"));
    }
}
