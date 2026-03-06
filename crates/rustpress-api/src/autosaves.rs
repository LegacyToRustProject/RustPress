//! WordPress REST API — Autosaves sub-resource.
//!
//! Endpoints:
//! - `GET  /wp-json/wp/v2/posts/{id}/autosaves`      — list autosaves
//! - `GET  /wp-json/wp/v2/posts/{id}/autosaves/{rev}` — get single autosave
//! - `POST /wp-json/wp/v2/posts/{id}/autosaves`       — create autosave
//! - `GET  /wp-json/wp/v2/pages/{id}/autosaves`       — list page autosaves
//! - `POST /wp-json/wp/v2/pages/{id}/autosaves`       — create page autosave
//!
//! Autosaves are stored as revisions with `post_name` containing the parent ID.

use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect,
};
use serde::Deserialize;
use serde_json::{json, Value};

use rustpress_db::entities::wp_posts;

use crate::common::WpError;
use crate::{ApiState, AuthUser};

#[derive(Debug, Deserialize)]
pub struct AutosaveBody {
    pub title: Option<String>,
    pub content: Option<String>,
    pub excerpt: Option<String>,
}

pub fn read_routes() -> Router<ApiState> {
    Router::new()
        .route(
            "/wp-json/wp/v2/posts/{id}/autosaves",
            get(list_autosaves),
        )
        .route(
            "/wp-json/wp/v2/posts/{id}/autosaves/{rev_id}",
            get(get_autosave),
        )
        .route(
            "/wp-json/wp/v2/pages/{id}/autosaves",
            get(list_autosaves),
        )
        .route(
            "/wp-json/wp/v2/pages/{id}/autosaves/{rev_id}",
            get(get_autosave),
        )
}

pub fn write_routes() -> Router<ApiState> {
    Router::new()
        .route(
            "/wp-json/wp/v2/posts/{id}/autosaves",
            post(create_autosave),
        )
        .route(
            "/wp-json/wp/v2/pages/{id}/autosaves",
            post(create_autosave),
        )
}

/// List autosaves for a post/page.
///
/// Autosaves are revisions where `post_name` is `{parent_id}-autosave-v1`.
async fn list_autosaves(
    State(state): State<ApiState>,
    Path(parent_id): Path<u64>,
) -> Result<Json<Vec<Value>>, WpError> {
    // Verify parent exists
    let parent = wp_posts::Entity::find_by_id(parent_id)
        .one(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?
        .ok_or_else(|| WpError::not_found("Post not found"))?;

    let autosave_name = format!("{}-autosave-v1", parent_id);

    let revisions = wp_posts::Entity::find()
        .filter(wp_posts::Column::PostParent.eq(parent_id))
        .filter(wp_posts::Column::PostType.eq("revision"))
        .filter(wp_posts::Column::PostName.eq(&autosave_name))
        .order_by_desc(wp_posts::Column::PostModified)
        .limit(10)
        .all(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?;

    let items: Vec<Value> = revisions
        .iter()
        .map(|r| autosave_to_json(r, &parent, &state.site_url))
        .collect();

    Ok(Json(items))
}

/// Get a single autosave revision.
async fn get_autosave(
    State(state): State<ApiState>,
    Path((parent_id, rev_id)): Path<(u64, u64)>,
) -> Result<Json<Value>, WpError> {
    let parent = wp_posts::Entity::find_by_id(parent_id)
        .one(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?
        .ok_or_else(|| WpError::not_found("Post not found"))?;

    let revision = wp_posts::Entity::find_by_id(rev_id)
        .filter(wp_posts::Column::PostParent.eq(parent_id))
        .filter(wp_posts::Column::PostType.eq("revision"))
        .one(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?
        .ok_or_else(|| WpError::not_found("Autosave not found"))?;

    Ok(Json(autosave_to_json(&revision, &parent, &state.site_url)))
}

/// Create a new autosave for a post/page.
///
/// WordPress stores one autosave per user per post. If an autosave already
/// exists for this user, it is updated rather than creating a new one.
async fn create_autosave(
    State(state): State<ApiState>,
    Path(parent_id): Path<u64>,
    user: AuthUser,
    Json(body): Json<AutosaveBody>,
) -> Result<Json<Value>, WpError> {
    let parent = wp_posts::Entity::find_by_id(parent_id)
        .one(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?
        .ok_or_else(|| WpError::not_found("Post not found"))?;

    let now = chrono::Utc::now().naive_utc();
    let autosave_name = format!("{}-autosave-v1", parent_id);

    let title = body.title.unwrap_or_else(|| parent.post_title.clone());
    let content = body.content.unwrap_or_else(|| parent.post_content.clone());
    let excerpt = body.excerpt.unwrap_or_else(|| parent.post_excerpt.clone());

    // Check if an autosave already exists for this user
    let existing = wp_posts::Entity::find()
        .filter(wp_posts::Column::PostParent.eq(parent_id))
        .filter(wp_posts::Column::PostType.eq("revision"))
        .filter(wp_posts::Column::PostName.eq(&autosave_name))
        .filter(wp_posts::Column::PostAuthor.eq(user.user_id))
        .one(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?;

    let revision = if let Some(existing) = existing {
        // Update existing autosave
        let mut active: wp_posts::ActiveModel = existing.into();
        active.post_title = Set(title);
        active.post_content = Set(content);
        active.post_excerpt = Set(excerpt);
        active.post_modified = Set(now);
        active.post_modified_gmt = Set(now);
        active.update(&state.db).await
            .map_err(|e| WpError::internal(e.to_string()))?
    } else {
        // Create new autosave revision
        let revision = wp_posts::ActiveModel {
            id: sea_orm::ActiveValue::NotSet,
            post_author: Set(user.user_id),
            post_date: Set(now),
            post_date_gmt: Set(now),
            post_content: Set(content),
            post_title: Set(title),
            post_excerpt: Set(excerpt),
            post_status: Set("inherit".to_string()),
            comment_status: Set("closed".to_string()),
            ping_status: Set("closed".to_string()),
            post_password: Set(String::new()),
            post_name: Set(autosave_name),
            to_ping: Set(String::new()),
            pinged: Set(String::new()),
            post_modified: Set(now),
            post_modified_gmt: Set(now),
            post_content_filtered: Set(String::new()),
            post_parent: Set(parent_id),
            guid: Set(String::new()),
            menu_order: Set(0),
            post_type: Set("revision".to_string()),
            post_mime_type: Set(String::new()),
            comment_count: Set(0),
        };
        revision.insert(&state.db).await
            .map_err(|e| WpError::internal(e.to_string()))?
    };

    Ok(Json(autosave_to_json(&revision, &parent, &state.site_url)))
}

fn autosave_to_json(
    revision: &wp_posts::Model,
    parent: &wp_posts::Model,
    site_url: &str,
) -> Value {
    let base = site_url.trim_end_matches('/');
    let parent_type = if parent.post_type == "page" { "pages" } else { "posts" };

    json!({
        "author": revision.post_author,
        "date": revision.post_date.format("%Y-%m-%dT%H:%M:%S").to_string(),
        "date_gmt": revision.post_date_gmt.format("%Y-%m-%dT%H:%M:%S").to_string(),
        "id": revision.id,
        "parent": parent.id,
        "modified": revision.post_modified.format("%Y-%m-%dT%H:%M:%S").to_string(),
        "modified_gmt": revision.post_modified_gmt.format("%Y-%m-%dT%H:%M:%S").to_string(),
        "slug": revision.post_name,
        "title": {"rendered": revision.post_title},
        "content": {"rendered": revision.post_content},
        "excerpt": {"rendered": revision.post_excerpt},
        "_links": {
            "self": [{"href": format!("{}/wp-json/wp/v2/{}/{}/autosaves/{}", base, parent_type, parent.id, revision.id)}],
            "parent": [{"href": format!("{}/wp-json/wp/v2/{}/{}", base, parent_type, parent.id)}],
            "curies": [{"name": "wp", "href": "https://api.w.org/{rel}", "templated": true}]
        }
    })
}
