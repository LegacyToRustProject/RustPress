use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect,
};
use serde::{Deserialize, Serialize};

use rustpress_db::entities::{wp_postmeta, wp_posts, wp_term_relationships, wp_term_taxonomy, wp_terms};

use crate::AdminState;

#[derive(Debug, Deserialize)]
pub struct ListPostsQuery {
    pub page: Option<u64>,
    pub per_page: Option<u64>,
    pub post_type: Option<String>,
    pub status: Option<String>,
    pub search: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreatePostRequest {
    pub title: String,
    pub content: String,
    pub excerpt: Option<String>,
    pub status: Option<String>,
    pub post_type: Option<String>,
    pub author: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePostRequest {
    pub title: Option<String>,
    pub content: Option<String>,
    pub excerpt: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PostResponse {
    pub id: u64,
    pub title: String,
    pub content: String,
    pub excerpt: String,
    pub status: String,
    pub post_type: String,
    pub slug: String,
    pub date: String,
    pub modified: String,
    pub author: u64,
}

impl From<wp_posts::Model> for PostResponse {
    fn from(p: wp_posts::Model) -> Self {
        Self {
            id: p.id,
            title: p.post_title,
            content: p.post_content,
            excerpt: p.post_excerpt,
            status: p.post_status,
            post_type: p.post_type,
            slug: p.post_name,
            date: p.post_date.format("%Y-%m-%dT%H:%M:%S").to_string(),
            modified: p.post_modified.format("%Y-%m-%dT%H:%M:%S").to_string(),
            author: p.post_author,
        }
    }
}

pub fn routes() -> Router<AdminState> {
    Router::new()
        .route("/admin/posts", get(list_posts).post(create_post))
        // Static path must be registered before the parameterized {id} catch-all
        .route("/admin/posts/meta-keys", get(list_all_meta_keys))
        .route(
            "/admin/posts/{id}",
            get(get_post).put(update_post).delete(delete_post),
        )
        .route(
            "/admin/posts/{id}/terms",
            get(get_post_terms).put(set_post_terms),
        )
        .route(
            "/admin/posts/{id}/meta",
            get(list_post_meta).post(create_post_meta),
        )
        .route(
            "/admin/posts/{id}/meta/{meta_id}",
            axum::routing::put(update_post_meta).delete(delete_post_meta),
        )
}

#[derive(Debug, Deserialize)]
pub struct SetPostTermsRequest {
    pub taxonomy: String,
    pub term_ids: Vec<u64>,
}

async fn list_posts(
    State(state): State<AdminState>,
    Query(params): Query<ListPostsQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let page = params.page.unwrap_or(1);
    let per_page = params.per_page.unwrap_or(20);
    let post_type = params.post_type.as_deref().unwrap_or("post");
    let status = params.status.as_deref().unwrap_or("publish");

    let mut query = wp_posts::Entity::find()
        .filter(wp_posts::Column::PostType.eq(post_type))
        .filter(wp_posts::Column::PostStatus.eq(status))
        .order_by_desc(wp_posts::Column::PostDate);

    if let Some(ref search) = params.search {
        query = query.filter(wp_posts::Column::PostTitle.like(&format!("%{}%", search)));
    }

    let total = query.clone().count(&state.db).await.map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    let posts = query
        .offset((page - 1) * per_page)
        .limit(per_page)
        .all(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let items: Vec<PostResponse> = posts.into_iter().map(PostResponse::from).collect();

    Ok(Json(serde_json::json!({
        "items": items,
        "total": total,
        "page": page,
        "per_page": per_page,
    })))
}

async fn get_post(
    State(state): State<AdminState>,
    Path(id): Path<u64>,
) -> Result<Json<PostResponse>, (StatusCode, String)> {
    let post = wp_posts::Entity::find_by_id(id)
        .one(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    match post {
        Some(p) => Ok(Json(PostResponse::from(p))),
        None => Err((StatusCode::NOT_FOUND, "Post not found".to_string())),
    }
}

async fn create_post(
    State(state): State<AdminState>,
    Json(input): Json<CreatePostRequest>,
) -> Result<(StatusCode, Json<PostResponse>), (StatusCode, String)> {
    let now = chrono::Utc::now().naive_utc();
    let slug = slugify(&input.title);
    let status = input.status.unwrap_or_else(|| "draft".to_string());
    let post_type = input.post_type.unwrap_or_else(|| "post".to_string());

    let new_post = wp_posts::ActiveModel {
        post_author: Set(input.author.unwrap_or(1)),
        post_date: Set(now),
        post_date_gmt: Set(now),
        post_content: Set(input.content),
        post_title: Set(input.title),
        post_excerpt: Set(input.excerpt.unwrap_or_default()),
        post_status: Set(status),
        comment_status: Set("open".to_string()),
        ping_status: Set("open".to_string()),
        post_password: Set(String::new()),
        post_name: Set(slug),
        to_ping: Set(String::new()),
        pinged: Set(String::new()),
        post_modified: Set(now),
        post_modified_gmt: Set(now),
        post_content_filtered: Set(String::new()),
        post_parent: Set(0),
        guid: Set(String::new()),
        menu_order: Set(0),
        post_type: Set(post_type),
        post_mime_type: Set(String::new()),
        comment_count: Set(0),
        ..Default::default()
    };

    let result = new_post.insert(&state.db).await.map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    Ok((StatusCode::CREATED, Json(PostResponse::from(result))))
}

async fn update_post(
    State(state): State<AdminState>,
    Path(id): Path<u64>,
    Json(input): Json<UpdatePostRequest>,
) -> Result<Json<PostResponse>, (StatusCode, String)> {
    let post = wp_posts::Entity::find_by_id(id)
        .one(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Post not found".to_string()))?;

    let mut active: wp_posts::ActiveModel = post.into();
    let now = chrono::Utc::now().naive_utc();

    if let Some(title) = input.title {
        active.post_title = Set(title);
    }
    if let Some(content) = input.content {
        active.post_content = Set(content);
    }
    if let Some(excerpt) = input.excerpt {
        active.post_excerpt = Set(excerpt);
    }
    if let Some(status) = input.status {
        active.post_status = Set(status);
    }
    active.post_modified = Set(now);
    active.post_modified_gmt = Set(now);

    let updated = active.update(&state.db).await.map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    Ok(Json(PostResponse::from(updated)))
}

async fn delete_post(
    State(state): State<AdminState>,
    Path(id): Path<u64>,
) -> Result<StatusCode, (StatusCode, String)> {
    // Move to trash instead of hard delete
    let post = wp_posts::Entity::find_by_id(id)
        .one(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Post not found".to_string()))?;

    let mut active: wp_posts::ActiveModel = post.into();
    active.post_status = Set("trash".to_string());
    active.update(&state.db).await.map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    Ok(StatusCode::NO_CONTENT)
}

// ---- Post-Term Relationships ----

async fn get_post_terms(
    State(state): State<AdminState>,
    Path(id): Path<u64>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    // Get all term_taxonomy_ids for this post
    let relationships = wp_term_relationships::Entity::find()
        .filter(wp_term_relationships::Column::ObjectId.eq(id))
        .all(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let tt_ids: Vec<u64> = relationships.iter().map(|r| r.term_taxonomy_id).collect();

    if tt_ids.is_empty() {
        return Ok(Json(serde_json::json!({
            "categories": [],
            "tags": [],
        })));
    }

    // Get term_taxonomy records
    let taxonomies = wp_term_taxonomy::Entity::find()
        .filter(wp_term_taxonomy::Column::TermTaxonomyId.is_in(tt_ids))
        .all(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let term_ids: Vec<u64> = taxonomies.iter().map(|tt| tt.term_id).collect();

    // Get terms
    let terms = if term_ids.is_empty() {
        vec![]
    } else {
        wp_terms::Entity::find()
            .filter(wp_terms::Column::TermId.is_in(term_ids.clone()))
            .all(&state.db)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    };

    let term_map: std::collections::HashMap<u64, &wp_terms::Model> =
        terms.iter().map(|t| (t.term_id, t)).collect();

    let mut categories = vec![];
    let mut tags = vec![];

    for tt in &taxonomies {
        if let Some(term) = term_map.get(&tt.term_id) {
            let item = serde_json::json!({
                "term_id": term.term_id,
                "name": term.name,
                "slug": term.slug,
            });
            match tt.taxonomy.as_str() {
                "category" => categories.push(item),
                "post_tag" => tags.push(item),
                _ => {}
            }
        }
    }

    Ok(Json(serde_json::json!({
        "categories": categories,
        "tags": tags,
    })))
}

async fn set_post_terms(
    State(state): State<AdminState>,
    Path(id): Path<u64>,
    Json(input): Json<SetPostTermsRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    // Get term_taxonomy_ids for the given taxonomy and term_ids
    let tt_records = wp_term_taxonomy::Entity::find()
        .filter(wp_term_taxonomy::Column::Taxonomy.eq(&input.taxonomy))
        .filter(wp_term_taxonomy::Column::TermId.is_in(input.term_ids.clone()))
        .all(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let new_tt_ids: Vec<u64> = tt_records.iter().map(|tt| tt.term_taxonomy_id).collect();

    // Get all existing relationships for this post in this taxonomy
    let all_tt_for_taxonomy = wp_term_taxonomy::Entity::find()
        .filter(wp_term_taxonomy::Column::Taxonomy.eq(&input.taxonomy))
        .all(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let taxonomy_tt_ids: Vec<u64> = all_tt_for_taxonomy
        .iter()
        .map(|tt| tt.term_taxonomy_id)
        .collect();

    // Delete existing relationships for this post in this taxonomy
    if !taxonomy_tt_ids.is_empty() {
        wp_term_relationships::Entity::delete_many()
            .filter(wp_term_relationships::Column::ObjectId.eq(id))
            .filter(wp_term_relationships::Column::TermTaxonomyId.is_in(taxonomy_tt_ids))
            .exec(&state.db)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }

    // Insert new relationships
    for (order, tt_id) in new_tt_ids.iter().enumerate() {
        let rel = wp_term_relationships::ActiveModel {
            object_id: Set(id),
            term_taxonomy_id: Set(*tt_id),
            term_order: Set(order as i32),
        };
        let _ = rel.insert(&state.db).await;
    }

    Ok(Json(serde_json::json!({ "ok": true })))
}

// ---- Post Meta (Custom Fields) ----

#[derive(Debug, Serialize)]
pub struct PostMetaResponse {
    pub meta_id: u64,
    pub key: String,
    pub value: String,
}

#[derive(Debug, Deserialize)]
pub struct CreatePostMetaRequest {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePostMetaRequest {
    pub key: Option<String>,
    pub value: Option<String>,
}

/// Returns true if a meta key is internal (starts with `_`).
fn is_internal_meta_key(key: &str) -> bool {
    key.starts_with('_')
}

/// GET /admin/posts/{id}/meta - List all non-internal postmeta for a post.
async fn list_post_meta(
    State(state): State<AdminState>,
    Path(id): Path<u64>,
) -> Result<Json<Vec<PostMetaResponse>>, (StatusCode, String)> {
    // Verify the post exists
    let _post = wp_posts::Entity::find_by_id(id)
        .one(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Post not found".to_string()))?;

    let metas = wp_postmeta::Entity::find()
        .filter(wp_postmeta::Column::PostId.eq(id))
        .all(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let items: Vec<PostMetaResponse> = metas
        .into_iter()
        .filter(|m| {
            m.meta_key
                .as_deref()
                .map(|k| !is_internal_meta_key(k))
                .unwrap_or(false)
        })
        .map(|m| PostMetaResponse {
            meta_id: m.meta_id,
            key: m.meta_key.unwrap_or_default(),
            value: m.meta_value.unwrap_or_default(),
        })
        .collect();

    Ok(Json(items))
}

/// POST /admin/posts/{id}/meta - Add a new meta field to a post.
async fn create_post_meta(
    State(state): State<AdminState>,
    Path(id): Path<u64>,
    Json(input): Json<CreatePostMetaRequest>,
) -> Result<(StatusCode, Json<PostMetaResponse>), (StatusCode, String)> {
    // Verify the post exists
    let _post = wp_posts::Entity::find_by_id(id)
        .one(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Post not found".to_string()))?;

    // Reject internal keys
    if is_internal_meta_key(&input.key) {
        return Err((
            StatusCode::BAD_REQUEST,
            "Internal meta keys (starting with _) cannot be created through this endpoint"
                .to_string(),
        ));
    }

    if input.key.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Meta key cannot be empty".to_string()));
    }

    let new_meta = wp_postmeta::ActiveModel {
        post_id: Set(id),
        meta_key: Set(Some(input.key.clone())),
        meta_value: Set(Some(input.value.clone())),
        ..Default::default()
    };

    let result = new_meta.insert(&state.db).await.map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    Ok((
        StatusCode::CREATED,
        Json(PostMetaResponse {
            meta_id: result.meta_id,
            key: result.meta_key.unwrap_or_default(),
            value: result.meta_value.unwrap_or_default(),
        }),
    ))
}

/// PUT /admin/posts/{id}/meta/{meta_id} - Update an existing meta field.
async fn update_post_meta(
    State(state): State<AdminState>,
    Path((id, meta_id)): Path<(u64, u64)>,
    Json(input): Json<UpdatePostMetaRequest>,
) -> Result<Json<PostMetaResponse>, (StatusCode, String)> {
    let meta = wp_postmeta::Entity::find_by_id(meta_id)
        .one(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Meta field not found".to_string()))?;

    // Verify it belongs to the correct post
    if meta.post_id != id {
        return Err((StatusCode::NOT_FOUND, "Meta field not found for this post".to_string()));
    }

    // Block updates to internal meta keys
    if meta
        .meta_key
        .as_deref()
        .map(is_internal_meta_key)
        .unwrap_or(false)
    {
        return Err((
            StatusCode::BAD_REQUEST,
            "Internal meta keys cannot be modified through this endpoint".to_string(),
        ));
    }

    let mut active: wp_postmeta::ActiveModel = meta.into();

    if let Some(key) = input.key {
        if is_internal_meta_key(&key) {
            return Err((
                StatusCode::BAD_REQUEST,
                "Cannot change key to an internal meta key".to_string(),
            ));
        }
        if key.trim().is_empty() {
            return Err((StatusCode::BAD_REQUEST, "Meta key cannot be empty".to_string()));
        }
        active.meta_key = Set(Some(key));
    }
    if let Some(value) = input.value {
        active.meta_value = Set(Some(value));
    }

    let updated = active.update(&state.db).await.map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    Ok(Json(PostMetaResponse {
        meta_id: updated.meta_id,
        key: updated.meta_key.unwrap_or_default(),
        value: updated.meta_value.unwrap_or_default(),
    }))
}

/// DELETE /admin/posts/{id}/meta/{meta_id} - Delete a meta field.
async fn delete_post_meta(
    State(state): State<AdminState>,
    Path((id, meta_id)): Path<(u64, u64)>,
) -> Result<StatusCode, (StatusCode, String)> {
    let meta = wp_postmeta::Entity::find_by_id(meta_id)
        .one(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Meta field not found".to_string()))?;

    // Verify it belongs to the correct post
    if meta.post_id != id {
        return Err((StatusCode::NOT_FOUND, "Meta field not found for this post".to_string()));
    }

    // Block deletion of internal meta keys
    if meta
        .meta_key
        .as_deref()
        .map(is_internal_meta_key)
        .unwrap_or(false)
    {
        return Err((
            StatusCode::BAD_REQUEST,
            "Internal meta keys cannot be deleted through this endpoint".to_string(),
        ));
    }

    wp_postmeta::Entity::delete_by_id(meta_id)
        .exec(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}

/// GET /admin/posts/meta-keys - List all distinct non-internal meta keys for the dropdown.
async fn list_all_meta_keys(
    State(state): State<AdminState>,
) -> Result<Json<Vec<String>>, (StatusCode, String)> {
    let metas = wp_postmeta::Entity::find()
        .all(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut keys: Vec<String> = metas
        .into_iter()
        .filter_map(|m| m.meta_key)
        .filter(|k| !is_internal_meta_key(k) && !k.trim().is_empty())
        .collect();

    keys.sort();
    keys.dedup();

    Ok(Json(keys))
}

/// Generate a URL-safe slug from a title.
fn slugify(title: &str) -> String {
    title
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' {
                c
            } else if c.is_whitespace() {
                '-'
            } else {
                '_'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slugify_simple() {
        assert_eq!(slugify("Hello World"), "hello-world");
    }

    #[test]
    fn test_slugify_special_chars() {
        assert_eq!(slugify("Hello, World!"), "hello_-world_");
    }

    #[test]
    fn test_slugify_multiple_spaces() {
        assert_eq!(slugify("Hello   World"), "hello-world");
    }

    #[test]
    fn test_slugify_already_lowercase() {
        assert_eq!(slugify("already-a-slug"), "already-a-slug");
    }

    #[test]
    fn test_slugify_uppercase() {
        assert_eq!(slugify("UPPERCASE TITLE"), "uppercase-title");
    }

    #[test]
    fn test_slugify_numbers() {
        assert_eq!(slugify("Post Number 42"), "post-number-42");
    }
}
