use axum::{extract::Path, http::StatusCode, routing::get, Json, Router};
use serde_json::{json, Value};
use std::collections::HashMap;

use crate::ApiState;

pub fn routes() -> Router<ApiState> {
    Router::new()
        .route("/wp-json/wp/v2/types", get(list_types))
        .route("/wp-json/wp/v2/types/{slug}", get(get_type))
}

/// `GET /wp-json/wp/v2/types` — returns an object keyed by type slug,
/// matching the WordPress `WP_REST_Post_Types_Controller` response format.
async fn list_types() -> Json<HashMap<String, Value>> {
    Json(builtin_types())
}

async fn get_type(Path(slug): Path<String>) -> Result<Json<Value>, StatusCode> {
    builtin_types()
        .remove(&slug)
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

fn builtin_types() -> HashMap<String, Value> {
    let mut map = HashMap::new();
    map.insert(
        "post".to_string(),
        json!({
            "description": "",
            "hierarchical": false,
            "has_archive": false,
            "name": "post",
            "slug": "post",
            "taxonomies": ["category", "post_tag"],
            "rest_base": "posts",
            "rest_namespace": "wp/v2",
            "viewable": true,
            "labels": {
                "name": "Posts",
                "singular_name": "Post",
                "add_new": "Add New Post",
                "add_new_item": "Add New Post",
                "edit_item": "Edit Post",
                "new_item": "New Post",
                "view_item": "View Post",
                "view_items": "View Posts",
                "search_items": "Search Posts",
                "not_found": "No posts found.",
                "not_found_in_trash": "No posts found in Trash.",
                "parent_item_colon": null,
                "all_items": "All Posts",
                "archives": "Post Archives",
                "attributes": "Post Attributes",
                "insert_into_item": "Insert into post",
                "uploaded_to_this_item": "Uploaded to this post",
                "featured_image": "Featured image",
                "set_featured_image": "Set featured image",
                "remove_featured_image": "Remove featured image",
                "use_featured_image": "Use as featured image",
                "filter_items_list": "Filter posts list",
                "filter_by_date": "Filter by date",
                "items_list_navigation": "Posts list navigation",
                "items_list": "Posts list",
                "item_published": "Post published.",
                "item_published_with_private": "Post published privately.",
                "item_reverted_to_draft": "Post reverted to draft.",
                "item_trashed": "Post moved to the Trash.",
                "item_scheduled": "Post scheduled.",
                "item_updated": "Post updated.",
                "menu_name": "Posts"
            },
            "supports": {
                "title": true,
                "editor": true,
                "author": true,
                "thumbnail": true,
                "excerpt": true,
                "trackbacks": true,
                "custom-fields": true,
                "comments": true,
                "revisions": true,
                "post-formats": true
            }
        }),
    );
    map.insert(
        "page".to_string(),
        json!({
            "description": "",
            "hierarchical": true,
            "has_archive": false,
            "name": "page",
            "slug": "page",
            "taxonomies": [],
            "rest_base": "pages",
            "rest_namespace": "wp/v2",
            "viewable": true,
            "labels": {
                "name": "Pages",
                "singular_name": "Page",
                "add_new": "Add New Page",
                "add_new_item": "Add New Page",
                "edit_item": "Edit Page",
                "new_item": "New Page",
                "view_item": "View Page",
                "view_items": "View Pages",
                "search_items": "Search Pages",
                "not_found": "No pages found.",
                "not_found_in_trash": "No pages found in Trash.",
                "parent_item_colon": "Parent Page:",
                "all_items": "All Pages",
                "archives": "Page Archives",
                "attributes": "Page Attributes",
                "insert_into_item": "Insert into page",
                "uploaded_to_this_item": "Uploaded to this page",
                "featured_image": "Featured image",
                "set_featured_image": "Set featured image",
                "remove_featured_image": "Remove featured image",
                "use_featured_image": "Use as featured image",
                "filter_items_list": "Filter pages list",
                "filter_by_date": "Filter by date",
                "items_list_navigation": "Pages list navigation",
                "items_list": "Pages list",
                "item_published": "Page published.",
                "item_published_with_private": "Page published privately.",
                "item_reverted_to_draft": "Page reverted to draft.",
                "item_trashed": "Page moved to the Trash.",
                "item_scheduled": "Page scheduled.",
                "item_updated": "Page updated.",
                "menu_name": "Pages"
            },
            "supports": {
                "title": true,
                "editor": true,
                "author": true,
                "thumbnail": true,
                "excerpt": false,
                "trackbacks": false,
                "custom-fields": true,
                "comments": true,
                "revisions": true,
                "page-attributes": true
            }
        }),
    );
    map.insert(
        "attachment".to_string(),
        json!({
            "description": "",
            "hierarchical": false,
            "has_archive": false,
            "name": "attachment",
            "slug": "attachment",
            "taxonomies": [],
            "rest_base": "media",
            "rest_namespace": "wp/v2",
            "viewable": true,
            "labels": {
                "name": "Media",
                "singular_name": "Media Item",
                "add_new": "Add New Media File",
                "add_new_item": "Add New Media File",
                "edit_item": "Edit Media",
                "new_item": "New Media File",
                "view_item": "View Attachment Page",
                "view_items": "View Posts",
                "search_items": "Search Media",
                "not_found": "No media found.",
                "not_found_in_trash": "No media found in Trash.",
                "parent_item_colon": null,
                "all_items": "Media Library",
                "archives": "Media",
                "attributes": "Attachment Attributes",
                "insert_into_item": "Insert into post",
                "uploaded_to_this_item": "Uploaded to this post",
                "featured_image": "Featured image",
                "set_featured_image": "Set featured image",
                "remove_featured_image": "Remove featured image",
                "use_featured_image": "Use as featured image",
                "filter_items_list": "Filter media list",
                "filter_by_date": "Filter by date",
                "items_list_navigation": "Media list navigation",
                "items_list": "Media list",
                "item_published": "Media File published.",
                "item_reverted_to_draft": "Media File reverted to draft.",
                "item_scheduled": "Media File scheduled.",
                "item_updated": "Media File updated.",
                "menu_name": "Media"
            },
            "supports": {
                "title": true,
                "author": true,
                "comments": true
            }
        }),
    );
    map
}
