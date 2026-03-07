//! WordPress-compatible navigation menu system for RustPress.
//!
//! Reads menu locations, menu items, and metadata from the WordPress database
//! and renders them as HTML matching WordPress's `wp_nav_menu()` output.

use rustpress_core::php_serialize::php_unserialize;
use rustpress_db::entities::{
    wp_postmeta, wp_posts, wp_term_relationships, wp_term_taxonomy, wp_terms,
};
use rustpress_db::options::OptionsManager;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};
use serde::Serialize;
use std::collections::HashMap;

/// A single navigation menu item with optional children.
#[derive(Debug, Clone, Serialize)]
pub struct MenuItem {
    pub id: u64,
    pub title: String,
    pub url: String,
    pub target: String,
    pub classes: Vec<String>,
    pub menu_order: i32,
    pub parent: u64,
    pub item_type: String,
    pub object: String,
    pub object_id: u64,
    pub children: Vec<MenuItem>,
}

/// Resolved navigation menu ready for rendering.
#[derive(Debug, Clone, Serialize)]
pub struct NavMenu {
    pub name: String,
    pub slug: String,
    pub items: Vec<MenuItem>,
}

/// Get the menu location → term_id mapping from wp_options.
pub async fn get_menu_locations(options: &OptionsManager) -> HashMap<String, u64> {
    let raw = options
        .get_option("nav_menu_locations")
        .await
        .ok()
        .flatten()
        .unwrap_or_default();

    if raw.is_empty() {
        return HashMap::new();
    }

    match php_unserialize(&raw) {
        Ok(val) => {
            if let Some(map) = val.as_map() {
                map.into_iter()
                    .filter_map(|(k, v)| v.as_int().map(|id| (k, id as u64)))
                    .collect()
            } else {
                HashMap::new()
            }
        }
        Err(_) => {
            // Try JSON fallback
            serde_json::from_str::<HashMap<String, u64>>(&raw).unwrap_or_default()
        }
    }
}

/// Load a menu by its term_id, including all items with metadata.
pub async fn load_menu(db: &DatabaseConnection, menu_term_id: u64) -> Option<NavMenu> {
    // Get the menu term
    let term = wp_terms::Entity::find_by_id(menu_term_id)
        .one(db)
        .await
        .ok()??;

    // Verify it's a nav_menu taxonomy
    let taxonomy = wp_term_taxonomy::Entity::find()
        .filter(wp_term_taxonomy::Column::TermId.eq(menu_term_id))
        .filter(wp_term_taxonomy::Column::Taxonomy.eq("nav_menu"))
        .one(db)
        .await
        .ok()??;

    // Get all post IDs linked to this menu via term_relationships
    let relationships = wp_term_relationships::Entity::find()
        .filter(wp_term_relationships::Column::TermTaxonomyId.eq(taxonomy.term_taxonomy_id))
        .all(db)
        .await
        .ok()?;

    let post_ids: Vec<u64> = relationships.iter().map(|r| r.object_id).collect();
    if post_ids.is_empty() {
        return Some(NavMenu {
            name: term.name,
            slug: term.slug,
            items: vec![],
        });
    }

    // Load all nav_menu_item posts
    let posts = wp_posts::Entity::find()
        .filter(wp_posts::Column::Id.is_in(post_ids.clone()))
        .filter(wp_posts::Column::PostType.eq("nav_menu_item"))
        .filter(wp_posts::Column::PostStatus.eq("publish"))
        .order_by_asc(wp_posts::Column::MenuOrder)
        .all(db)
        .await
        .ok()?;

    // Load all postmeta for these items in one query
    let all_meta = wp_postmeta::Entity::find()
        .filter(wp_postmeta::Column::PostId.is_in(post_ids))
        .all(db)
        .await
        .ok()
        .unwrap_or_default();

    // Group meta by post_id
    let mut meta_map: HashMap<u64, HashMap<String, String>> = HashMap::new();
    for m in &all_meta {
        meta_map.entry(m.post_id).or_default().insert(
            m.meta_key.clone().unwrap_or_default(),
            m.meta_value.clone().unwrap_or_default(),
        );
    }

    // Build flat list of menu items
    let mut flat_items: Vec<MenuItem> = Vec::new();
    for post in &posts {
        let meta = meta_map.get(&post.id).cloned().unwrap_or_default();
        let item_type = meta.get("_menu_item_type").cloned().unwrap_or_default();
        let object = meta.get("_menu_item_object").cloned().unwrap_or_default();
        let object_id: u64 = meta
            .get("_menu_item_object_id")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        let parent: u64 = meta
            .get("_menu_item_menu_item_parent")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        let url = meta.get("_menu_item_url").cloned().unwrap_or_default();
        let target = meta.get("_menu_item_target").cloned().unwrap_or_default();
        let classes_str = meta.get("_menu_item_classes").cloned().unwrap_or_default();

        // Parse classes (PHP serialized array or space-separated)
        let classes = parse_menu_classes(&classes_str);

        // Resolve URL for non-custom items
        let resolved_url = if item_type == "custom" {
            url
        } else {
            // For post_type/taxonomy items, resolve from the linked object
            resolve_menu_item_url(db, &item_type, &object, object_id)
                .await
                .unwrap_or(url)
        };

        // Use post_title or resolve from the linked object
        let title = if post.post_title.is_empty() {
            resolve_menu_item_title(db, &item_type, &object, object_id)
                .await
                .unwrap_or_else(|| "Untitled".to_string())
        } else {
            post.post_title.clone()
        };

        flat_items.push(MenuItem {
            id: post.id,
            title,
            url: resolved_url,
            target,
            classes,
            menu_order: post.menu_order,
            parent,
            item_type,
            object,
            object_id,
            children: vec![],
        });
    }

    // Build tree structure
    let items = build_menu_tree(flat_items);

    Some(NavMenu {
        name: term.name,
        slug: term.slug,
        items,
    })
}

/// Load a menu by location name.
pub async fn load_menu_by_location(
    db: &DatabaseConnection,
    options: &OptionsManager,
    location: &str,
) -> Option<NavMenu> {
    let locations = get_menu_locations(options).await;
    let menu_id = locations.get(location)?;
    load_menu(db, *menu_id).await
}

/// Render a navigation menu as HTML, matching WordPress's wp_nav_menu() output.
pub fn render_nav_menu(
    menu: &NavMenu,
    container: &str,
    container_class: &str,
    menu_class: &str,
    menu_id: &str,
    current_url: &str,
) -> String {
    let mut html = String::new();

    if !container.is_empty() {
        html.push_str(&format!("<{} class=\"{}\">", container, container_class));
    }

    html.push_str(&format!("<ul id=\"{}\" class=\"{}\">", menu_id, menu_class));

    for item in &menu.items {
        render_menu_item(&mut html, item, current_url, 0);
    }

    html.push_str("</ul>");

    if !container.is_empty() {
        html.push_str(&format!("</{}>", container));
    }

    html
}

/// Render a single menu item and its children recursively.
#[allow(clippy::only_used_in_recursion)]
fn render_menu_item(html: &mut String, item: &MenuItem, current_url: &str, depth: usize) {
    let mut li_classes = vec![
        format!("menu-item"),
        format!("menu-item-{}", item.id),
        format!("menu-item-type-{}", item.item_type),
        format!("menu-item-object-{}", item.object),
    ];

    // Add custom classes
    for cls in &item.classes {
        if !cls.is_empty() {
            li_classes.push(cls.clone());
        }
    }

    // Current page detection
    if !current_url.is_empty() && item.url == current_url {
        li_classes.push("current-menu-item".to_string());
        li_classes.push("current_page_item".to_string());
    }

    if !item.children.is_empty() {
        li_classes.push("menu-item-has-children".to_string());
    }

    html.push_str(&format!("<li class=\"{}\">", li_classes.join(" ")));

    let target_attr = if item.target.is_empty() {
        String::new()
    } else {
        format!(" target=\"{}\"", item.target)
    };

    html.push_str(&format!(
        "<a href=\"{}\"{}>{}</a>",
        item.url, target_attr, item.title
    ));

    // Render children as sub-menu
    if !item.children.is_empty() {
        html.push_str("<ul class=\"sub-menu\">");
        for child in &item.children {
            render_menu_item(html, child, current_url, depth + 1);
        }
        html.push_str("</ul>");
    }

    html.push_str("</li>");
}

/// Build a tree from flat menu items using parent references.
fn build_menu_tree(flat: Vec<MenuItem>) -> Vec<MenuItem> {
    let mut items_by_id: HashMap<u64, MenuItem> = HashMap::new();
    let mut children_map: HashMap<u64, Vec<u64>> = HashMap::new();
    let mut root_ids: Vec<u64> = Vec::new();

    // First pass: collect all items and identify parents
    for item in flat {
        let id = item.id;
        let parent = item.parent;
        items_by_id.insert(id, item);
        if parent == 0 || !items_by_id.contains_key(&parent) {
            // Will determine root vs child in second pass
        }
        children_map.entry(parent).or_default().push(id);
    }

    // Identify roots (parent == 0 or parent not in the menu)
    for (id, item) in &items_by_id {
        if item.parent == 0 || !items_by_id.contains_key(&item.parent) {
            root_ids.push(*id);
        }
    }

    // Sort roots by menu_order
    root_ids.sort_by_key(|id| items_by_id.get(id).map(|i| i.menu_order).unwrap_or(0));

    // Recursively build tree
    fn build_children(
        parent_id: u64,
        children_map: &HashMap<u64, Vec<u64>>,
        items_by_id: &mut HashMap<u64, MenuItem>,
    ) -> Vec<MenuItem> {
        let child_ids = match children_map.get(&parent_id) {
            Some(ids) => ids.clone(),
            None => return vec![],
        };

        let mut children: Vec<MenuItem> = Vec::new();
        for id in child_ids {
            if let Some(mut item) = items_by_id.remove(&id) {
                item.children = build_children(id, children_map, items_by_id);
                children.push(item);
            }
        }
        children.sort_by_key(|i| i.menu_order);
        children
    }

    let mut result = Vec::new();
    for id in root_ids {
        if let Some(mut item) = items_by_id.remove(&id) {
            item.children = build_children(id, &children_map, &mut items_by_id);
            result.push(item);
        }
    }
    result
}

/// Parse menu item CSS classes from PHP serialized or plain format.
fn parse_menu_classes(raw: &str) -> Vec<String> {
    if raw.is_empty() {
        return vec![];
    }

    // Try PHP unserialization first
    if let Ok(val) = php_unserialize(raw) {
        return val.as_string_list();
    }

    // Fallback: space-separated
    raw.split_whitespace()
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Resolve the URL for a menu item linked to a post or taxonomy.
async fn resolve_menu_item_url(
    db: &DatabaseConnection,
    item_type: &str,
    object: &str,
    object_id: u64,
) -> Option<String> {
    match item_type {
        "post_type" => {
            let post = wp_posts::Entity::find_by_id(object_id)
                .one(db)
                .await
                .ok()??;
            Some(format!("/{}/", post.post_name))
        }
        "taxonomy" => {
            let term = wp_terms::Entity::find_by_id(object_id)
                .one(db)
                .await
                .ok()??;
            match object {
                "category" => Some(format!("/category/{}/", term.slug)),
                "post_tag" => Some(format!("/tag/{}/", term.slug)),
                _ => Some(format!("/{}//", term.slug)),
            }
        }
        _ => None,
    }
}

/// Resolve the title for a menu item from its linked object.
async fn resolve_menu_item_title(
    db: &DatabaseConnection,
    item_type: &str,
    _object: &str,
    object_id: u64,
) -> Option<String> {
    match item_type {
        "post_type" => {
            let post = wp_posts::Entity::find_by_id(object_id)
                .one(db)
                .await
                .ok()??;
            Some(post.post_title)
        }
        "taxonomy" => {
            let term = wp_terms::Entity::find_by_id(object_id)
                .one(db)
                .await
                .ok()??;
            Some(term.name)
        }
        _ => None,
    }
}
