//! WordPress REST API — Navigation Menus & Menu Items.
//!
//! Endpoints:
//! - `GET  /wp-json/wp/v2/menus`           — list menus
//! - `GET  /wp-json/wp/v2/menus/{id}`      — get single menu
//! - `POST /wp-json/wp/v2/menus`           — create menu
//! - `PUT  /wp-json/wp/v2/menus/{id}`      — update menu
//! - `DELETE /wp-json/wp/v2/menus/{id}`    — delete menu
//! - `GET  /wp-json/wp/v2/menu-items`      — list menu items
//! - `GET  /wp-json/wp/v2/menu-items/{id}` — get single item
//! - `POST /wp-json/wp/v2/menu-items`      — create item
//! - `PUT  /wp-json/wp/v2/menu-items/{id}` — update item
//! - `DELETE /wp-json/wp/v2/menu-items/{id}`— delete item
//!
//! WordPress stores menus as terms in the `nav_menu` taxonomy and menu items
//! as posts of type `nav_menu_item`. RustPress currently stores menus as
//! simple text in `wp_options` (nav_menu_header / nav_menu_footer), so this
//! endpoint provides a JSON interface to that storage.

use axum::{
    extract::{Path, Query, State},
    routing::{get, post, put},
    Json, Router,
};
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter};
use serde::Deserialize;
use serde_json::{json, Value};

use rustpress_db::entities::{wp_term_taxonomy, wp_terms};

use crate::common::WpError;
use crate::ApiState;

#[derive(Debug, Deserialize)]
pub struct MenuQuery {
    pub context: Option<String>,
    pub _fields: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct MenuItemQuery {
    pub menus: Option<u64>,
    pub context: Option<String>,
    pub _fields: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateMenu {
    pub name: String,
    pub slug: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateMenu {
    pub name: Option<String>,
    pub slug: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateMenuItem {
    pub title: String,
    pub url: String,
    pub menus: Option<u64>,
    pub menu_order: Option<i32>,
    pub parent: Option<u64>,
    #[serde(rename = "type")]
    pub item_type: Option<String>,
    pub object: Option<String>,
    pub object_id: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateMenuItem {
    pub title: Option<String>,
    pub url: Option<String>,
    pub menu_order: Option<i32>,
    pub parent: Option<u64>,
}

pub fn read_routes() -> Router<ApiState> {
    Router::new()
        .route("/wp-json/wp/v2/menus", get(list_menus))
        .route("/wp-json/wp/v2/menus/{id}", get(get_menu))
        .route("/wp-json/wp/v2/menu-items", get(list_menu_items))
        .route("/wp-json/wp/v2/menu-items/{id}", get(get_menu_item))
}

pub fn write_routes() -> Router<ApiState> {
    Router::new()
        .route("/wp-json/wp/v2/menus", post(create_menu))
        .route(
            "/wp-json/wp/v2/menus/{id}",
            put(update_menu).patch(update_menu).delete(delete_menu),
        )
        .route("/wp-json/wp/v2/menu-items", post(create_menu_item))
        .route(
            "/wp-json/wp/v2/menu-items/{id}",
            put(update_menu_item)
                .patch(update_menu_item)
                .delete(delete_menu_item),
        )
}

/// List all registered navigation menus.
///
/// WordPress stores menus as terms in the `nav_menu` taxonomy.
async fn list_menus(
    State(state): State<ApiState>,
    Query(_query): Query<MenuQuery>,
) -> Result<Json<Vec<Value>>, WpError> {
    let taxonomies = wp_term_taxonomy::Entity::find()
        .filter(wp_term_taxonomy::Column::Taxonomy.eq("nav_menu"))
        .all(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?;

    let mut result = Vec::new();
    for tt in &taxonomies {
        if let Ok(Some(term)) = wp_terms::Entity::find_by_id(tt.term_id)
            .one(&state.db)
            .await
        {
            result.push(menu_to_json(&term, tt, &state.site_url));
        }
    }

    Ok(Json(result))
}

/// Get a single menu by term_id.
async fn get_menu(
    State(state): State<ApiState>,
    Path(id): Path<u64>,
) -> Result<Json<Value>, WpError> {
    let (tt, term) = find_menu_by_id(&state, id).await?;
    Ok(Json(menu_to_json(&term, &tt, &state.site_url)))
}

/// Create a new navigation menu.
async fn create_menu(
    State(state): State<ApiState>,
    Json(body): Json<CreateMenu>,
) -> Result<Json<Value>, WpError> {
    let slug = body
        .slug
        .unwrap_or_else(|| crate::common::slugify(&body.name));

    // Create term
    let term = wp_terms::ActiveModel {
        term_id: sea_orm::ActiveValue::NotSet,
        name: Set(body.name.clone()),
        slug: Set(slug),
        term_group: Set(0),
    };
    let term = term
        .insert(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?;

    // Create term_taxonomy
    let tt = wp_term_taxonomy::ActiveModel {
        term_taxonomy_id: sea_orm::ActiveValue::NotSet,
        term_id: Set(term.term_id),
        taxonomy: Set("nav_menu".to_string()),
        description: Set(body.description.unwrap_or_default()),
        parent: Set(0),
        count: Set(0),
    };
    let tt = tt
        .insert(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?;

    Ok(Json(menu_to_json(&term, &tt, &state.site_url)))
}

/// Update a navigation menu.
async fn update_menu(
    State(state): State<ApiState>,
    Path(id): Path<u64>,
    Json(body): Json<UpdateMenu>,
) -> Result<Json<Value>, WpError> {
    let (tt, term) = find_menu_by_id(&state, id).await?;

    let mut active_term: wp_terms::ActiveModel = term.into();
    if let Some(name) = &body.name {
        active_term.name = Set(name.clone());
    }
    if let Some(slug) = &body.slug {
        active_term.slug = Set(slug.clone());
    }
    let term = active_term
        .update(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?;

    let mut active_tt: wp_term_taxonomy::ActiveModel = tt.into();
    if let Some(desc) = &body.description {
        active_tt.description = Set(desc.clone());
    }
    let tt = active_tt
        .update(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?;

    Ok(Json(menu_to_json(&term, &tt, &state.site_url)))
}

/// Delete a navigation menu.
async fn delete_menu(
    State(state): State<ApiState>,
    Path(id): Path<u64>,
) -> Result<Json<Value>, WpError> {
    let (tt, term) = find_menu_by_id(&state, id).await?;
    let response = menu_to_json(&term, &tt, &state.site_url);

    // Delete menu items (posts with nav_menu_item type linked to this menu)
    use rustpress_db::entities::{wp_posts, wp_term_relationships};
    let relationships = wp_term_relationships::Entity::find()
        .filter(wp_term_relationships::Column::TermTaxonomyId.eq(tt.term_taxonomy_id))
        .all(&state.db)
        .await
        .unwrap_or_default();
    for rel in &relationships {
        let _ = wp_posts::Entity::delete_by_id(rel.object_id)
            .exec(&state.db)
            .await;
    }
    let _ = wp_term_relationships::Entity::delete_many()
        .filter(wp_term_relationships::Column::TermTaxonomyId.eq(tt.term_taxonomy_id))
        .exec(&state.db)
        .await;

    // Delete taxonomy + term
    let _ = wp_term_taxonomy::Entity::delete_by_id(tt.term_taxonomy_id)
        .exec(&state.db)
        .await;
    let _ = wp_terms::Entity::delete_by_id(term.term_id)
        .exec(&state.db)
        .await;

    Ok(Json(json!({"deleted": true, "previous": response})))
}

/// List menu items, optionally filtered by menu ID.
async fn list_menu_items(
    State(state): State<ApiState>,
    Query(query): Query<MenuItemQuery>,
) -> Result<Json<Vec<Value>>, WpError> {
    use rustpress_db::entities::{wp_postmeta, wp_posts, wp_term_relationships};

    let mut items_query = wp_posts::Entity::find()
        .filter(wp_posts::Column::PostType.eq("nav_menu_item"))
        .filter(wp_posts::Column::PostStatus.eq("publish"));

    // If menus filter provided, join through term_relationships
    if let Some(menu_id) = query.menus {
        // Find term_taxonomy_id for this menu
        let tt = wp_term_taxonomy::Entity::find()
            .filter(wp_term_taxonomy::Column::TermId.eq(menu_id))
            .filter(wp_term_taxonomy::Column::Taxonomy.eq("nav_menu"))
            .one(&state.db)
            .await
            .map_err(|e| WpError::internal(e.to_string()))?;

        if let Some(tt) = tt {
            let rels = wp_term_relationships::Entity::find()
                .filter(wp_term_relationships::Column::TermTaxonomyId.eq(tt.term_taxonomy_id))
                .all(&state.db)
                .await
                .unwrap_or_default();
            let ids: Vec<u64> = rels.iter().map(|r| r.object_id).collect();
            if ids.is_empty() {
                return Ok(Json(vec![]));
            }
            items_query = items_query.filter(wp_posts::Column::Id.is_in(ids));
        } else {
            return Ok(Json(vec![]));
        }
    }

    let items = items_query
        .all(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?;

    let mut result = Vec::new();
    for item in &items {
        let meta = wp_postmeta::Entity::find()
            .filter(wp_postmeta::Column::PostId.eq(item.id))
            .all(&state.db)
            .await
            .unwrap_or_default();
        result.push(menu_item_to_json(item, &meta, &state.site_url));
    }

    Ok(Json(result))
}

/// Get a single menu item.
async fn get_menu_item(
    State(state): State<ApiState>,
    Path(id): Path<u64>,
) -> Result<Json<Value>, WpError> {
    use rustpress_db::entities::{wp_postmeta, wp_posts};

    let item = wp_posts::Entity::find_by_id(id)
        .filter(wp_posts::Column::PostType.eq("nav_menu_item"))
        .one(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?
        .ok_or_else(|| WpError::not_found("Menu item not found"))?;

    let meta = wp_postmeta::Entity::find()
        .filter(wp_postmeta::Column::PostId.eq(id))
        .all(&state.db)
        .await
        .unwrap_or_default();

    Ok(Json(menu_item_to_json(&item, &meta, &state.site_url)))
}

/// Create a new menu item.
async fn create_menu_item(
    State(state): State<ApiState>,
    Json(body): Json<CreateMenuItem>,
) -> Result<Json<Value>, WpError> {
    use rustpress_db::entities::{wp_postmeta, wp_posts, wp_term_relationships};

    let now = chrono::Utc::now().naive_utc();
    let item = wp_posts::ActiveModel {
        id: sea_orm::ActiveValue::NotSet,
        post_author: Set(0),
        post_date: Set(now),
        post_date_gmt: Set(now),
        post_content: Set(String::new()),
        post_title: Set(body.title.clone()),
        post_excerpt: Set(String::new()),
        post_status: Set("publish".to_string()),
        comment_status: Set("closed".to_string()),
        ping_status: Set("closed".to_string()),
        post_password: Set(String::new()),
        post_name: Set(crate::common::slugify(&body.title)),
        to_ping: Set(String::new()),
        pinged: Set(String::new()),
        post_modified: Set(now),
        post_modified_gmt: Set(now),
        post_content_filtered: Set(String::new()),
        post_parent: Set(body.parent.unwrap_or(0)),
        guid: Set(String::new()),
        menu_order: Set(body.menu_order.unwrap_or(0)),
        post_type: Set("nav_menu_item".to_string()),
        post_mime_type: Set(String::new()),
        comment_count: Set(0),
    };

    let item = item
        .insert(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?;

    // Save menu item metadata
    let meta_entries = vec![
        (
            "_menu_item_type",
            body.item_type.as_deref().unwrap_or("custom"),
        ),
        ("_menu_item_url", &body.url),
    ];
    for (key, val) in meta_entries {
        let meta = wp_postmeta::ActiveModel {
            meta_id: sea_orm::ActiveValue::NotSet,
            post_id: Set(item.id),
            meta_key: Set(Some(key.to_string())),
            meta_value: Set(Some(val.to_string())),
        };
        let _ = meta.insert(&state.db).await;
    }
    if let Some(obj_id) = body.object_id {
        let meta = wp_postmeta::ActiveModel {
            meta_id: sea_orm::ActiveValue::NotSet,
            post_id: Set(item.id),
            meta_key: Set(Some("_menu_item_object_id".to_string())),
            meta_value: Set(Some(obj_id.to_string())),
        };
        let _ = meta.insert(&state.db).await;
    }
    if let Some(ref obj) = body.object {
        let meta = wp_postmeta::ActiveModel {
            meta_id: sea_orm::ActiveValue::NotSet,
            post_id: Set(item.id),
            meta_key: Set(Some("_menu_item_object".to_string())),
            meta_value: Set(Some(obj.clone())),
        };
        let _ = meta.insert(&state.db).await;
    }

    // Link to menu via term_relationships
    if let Some(menu_id) = body.menus {
        let tt = wp_term_taxonomy::Entity::find()
            .filter(wp_term_taxonomy::Column::TermId.eq(menu_id))
            .filter(wp_term_taxonomy::Column::Taxonomy.eq("nav_menu"))
            .one(&state.db)
            .await
            .ok()
            .flatten();
        if let Some(tt) = tt {
            let rel = wp_term_relationships::ActiveModel {
                object_id: Set(item.id),
                term_taxonomy_id: Set(tt.term_taxonomy_id),
                term_order: Set(0),
            };
            let _ = rel.insert(&state.db).await;
        }
    }

    let meta = wp_postmeta::Entity::find()
        .filter(wp_postmeta::Column::PostId.eq(item.id))
        .all(&state.db)
        .await
        .unwrap_or_default();

    Ok(Json(menu_item_to_json(&item, &meta, &state.site_url)))
}

/// Update a menu item.
async fn update_menu_item(
    State(state): State<ApiState>,
    Path(id): Path<u64>,
    Json(body): Json<UpdateMenuItem>,
) -> Result<Json<Value>, WpError> {
    use rustpress_db::entities::{wp_postmeta, wp_posts};

    let item = wp_posts::Entity::find_by_id(id)
        .filter(wp_posts::Column::PostType.eq("nav_menu_item"))
        .one(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?
        .ok_or_else(|| WpError::not_found("Menu item not found"))?;

    let mut active: wp_posts::ActiveModel = item.into();
    if let Some(title) = &body.title {
        active.post_title = Set(title.clone());
    }
    if let Some(parent) = body.parent {
        active.post_parent = Set(parent);
    }
    if let Some(order) = body.menu_order {
        active.menu_order = Set(order);
    }
    let now = chrono::Utc::now().naive_utc();
    active.post_modified = Set(now);
    active.post_modified_gmt = Set(now);
    let item = active
        .update(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?;

    // Update URL meta if provided
    if let Some(url) = &body.url {
        update_postmeta(&state.db, id, "_menu_item_url", url).await;
    }

    let meta = wp_postmeta::Entity::find()
        .filter(wp_postmeta::Column::PostId.eq(id))
        .all(&state.db)
        .await
        .unwrap_or_default();

    Ok(Json(menu_item_to_json(&item, &meta, &state.site_url)))
}

/// Delete a menu item.
async fn delete_menu_item(
    State(state): State<ApiState>,
    Path(id): Path<u64>,
) -> Result<Json<Value>, WpError> {
    use rustpress_db::entities::{wp_postmeta, wp_posts, wp_term_relationships};

    let item = wp_posts::Entity::find_by_id(id)
        .filter(wp_posts::Column::PostType.eq("nav_menu_item"))
        .one(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?
        .ok_or_else(|| WpError::not_found("Menu item not found"))?;

    // Clean up meta and relationships
    let _ = wp_postmeta::Entity::delete_many()
        .filter(wp_postmeta::Column::PostId.eq(id))
        .exec(&state.db)
        .await;
    let _ = wp_term_relationships::Entity::delete_many()
        .filter(wp_term_relationships::Column::ObjectId.eq(id))
        .exec(&state.db)
        .await;
    let _ = wp_posts::Entity::delete_by_id(id).exec(&state.db).await;

    Ok(Json(
        json!({"deleted": true, "previous": {"id": id, "title": item.post_title}}),
    ))
}

// ---- Helpers ----

async fn find_menu_by_id(
    state: &ApiState,
    id: u64,
) -> Result<(wp_term_taxonomy::Model, wp_terms::Model), WpError> {
    let tt = wp_term_taxonomy::Entity::find()
        .filter(wp_term_taxonomy::Column::TermId.eq(id))
        .filter(wp_term_taxonomy::Column::Taxonomy.eq("nav_menu"))
        .one(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?
        .ok_or_else(|| WpError::not_found("Menu not found"))?;

    let term = wp_terms::Entity::find_by_id(id)
        .one(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?
        .ok_or_else(|| WpError::not_found("Menu not found"))?;

    Ok((tt, term))
}

fn menu_to_json(term: &wp_terms::Model, tt: &wp_term_taxonomy::Model, site_url: &str) -> Value {
    let base = site_url.trim_end_matches('/');
    json!({
        "id": term.term_id,
        "name": term.name,
        "slug": term.slug,
        "description": tt.description,
        "count": tt.count,
        "meta": [],
        "_links": {
            "self": [{"href": format!("{}/wp-json/wp/v2/menus/{}", base, term.term_id)}],
            "collection": [{"href": format!("{}/wp-json/wp/v2/menus", base)}],
            "curies": [{"name": "wp", "href": "https://api.w.org/{rel}", "templated": true}]
        }
    })
}

fn menu_item_to_json(
    item: &rustpress_db::entities::wp_posts::Model,
    meta: &[rustpress_db::entities::wp_postmeta::Model],
    site_url: &str,
) -> Value {
    let get_meta = |key: &str| -> String {
        meta.iter()
            .find(|m| m.meta_key.as_deref() == Some(key))
            .and_then(|m| m.meta_value.clone())
            .unwrap_or_default()
    };

    let base = site_url.trim_end_matches('/');
    json!({
        "id": item.id,
        "title": {"rendered": item.post_title},
        "status": item.post_status,
        "url": get_meta("_menu_item_url"),
        "type": get_meta("_menu_item_type"),
        "object": get_meta("_menu_item_object"),
        "object_id": get_meta("_menu_item_object_id").parse::<u64>().unwrap_or(0),
        "parent": item.post_parent,
        "menu_order": item.menu_order,
        "menus": [],
        "meta": [],
        "_links": {
            "self": [{"href": format!("{}/wp-json/wp/v2/menu-items/{}", base, item.id)}],
            "collection": [{"href": format!("{}/wp-json/wp/v2/menu-items", base)}],
            "curies": [{"name": "wp", "href": "https://api.w.org/{rel}", "templated": true}]
        }
    })
}

async fn update_postmeta(db: &sea_orm::DatabaseConnection, post_id: u64, key: &str, value: &str) {
    use rustpress_db::entities::wp_postmeta;

    let existing = wp_postmeta::Entity::find()
        .filter(wp_postmeta::Column::PostId.eq(post_id))
        .filter(wp_postmeta::Column::MetaKey.eq(key))
        .one(db)
        .await
        .ok()
        .flatten();

    if let Some(m) = existing {
        let mut active: wp_postmeta::ActiveModel = m.into();
        active.meta_value = Set(Some(value.to_string()));
        let _ = active.update(db).await;
    } else {
        let meta = wp_postmeta::ActiveModel {
            meta_id: sea_orm::ActiveValue::NotSet,
            post_id: Set(post_id),
            meta_key: Set(Some(key.to_string())),
            meta_value: Set(Some(value.to_string())),
        };
        let _ = meta.insert(db).await;
    }
}
