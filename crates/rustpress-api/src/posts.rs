use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use rustpress_db::entities::{wp_postmeta, wp_posts, wp_term_relationships, wp_term_taxonomy, wp_terms, wp_users};
use rustpress_db::revisions::RevisionManager;

use crate::common::{
    filter_post_context, pagination_headers, post_links, slugify, term_links, RestContext, WpError,
};
use crate::ApiState;

/// WP REST API Post response format.
#[derive(Debug, Serialize)]
pub struct WpPost {
    pub id: u64,
    pub date: String,
    pub date_gmt: String,
    pub guid: WpRendered,
    pub modified: String,
    pub modified_gmt: String,
    pub slug: String,
    pub status: String,
    #[serde(rename = "type")]
    pub post_type: String,
    pub link: String,
    pub title: WpRendered,
    pub content: WpRendered,
    pub excerpt: WpRendered,
    pub author: u64,
    pub featured_media: u64,
    pub comment_status: String,
    pub ping_status: String,
    pub sticky: bool,
    pub template: String,
    pub format: String,
    pub meta: Vec<Value>,
    pub categories: Vec<u64>,
    pub tags: Vec<u64>,
    pub _links: Value,
}

#[derive(Debug, Serialize, Clone)]
pub struct WpRendered {
    pub rendered: String,
}

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    pub page: Option<u64>,
    pub per_page: Option<u64>,
    pub search: Option<String>,
    pub status: Option<String>,
    pub author: Option<u64>,
    pub orderby: Option<String>,
    pub order: Option<String>,
    pub include: Option<String>,
    pub exclude: Option<String>,
    pub slug: Option<String>,
    pub sticky: Option<bool>,
    pub before: Option<String>,
    pub after: Option<String>,
    pub categories: Option<String>,
    pub tags: Option<String>,
    pub categories_exclude: Option<String>,
    pub tags_exclude: Option<String>,
    pub context: Option<String>,
    pub _fields: Option<String>,
    pub _embed: Option<String>,
}

/// Request body for creating/updating posts via WP REST API.
#[derive(Debug, Deserialize)]
pub struct WpPostWrite {
    pub title: Option<String>,
    pub content: Option<String>,
    pub excerpt: Option<String>,
    pub status: Option<String>,
    pub slug: Option<String>,
    pub author: Option<u64>,
    pub post_type: Option<String>,
    pub featured_media: Option<u64>,
    pub date: Option<String>,
    pub sticky: Option<bool>,
    pub categories: Option<Vec<u64>>,
    pub tags: Option<Vec<u64>>,
    pub comment_status: Option<String>,
    pub ping_status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DeletePostQuery {
    pub force: Option<bool>,
}

/// Build a WpPost from a database model, loading categories and tags.
pub async fn build_post(
    p: wp_posts::Model,
    site_url: &str,
    db: &sea_orm::DatabaseConnection,
) -> WpPost {
    let links = post_links(site_url, p.id, "post", p.post_author);
    let link = format!("{}/{}", site_url.trim_end_matches('/'), p.post_name);
    let guid_str = if p.guid.is_empty() {
        format!("{}/?p={}", site_url.trim_end_matches('/'), p.id)
    } else {
        p.guid.clone()
    };

    // Load categories and tags via wp_term_relationships + wp_term_taxonomy
    let (categories, tags) = load_post_terms(db, p.id).await;

    // Load featured_media from postmeta
    let featured_media = load_featured_media(db, p.id).await;

    // Check sticky status from wp_options
    let sticky = check_sticky(db, p.id).await;

    // Apply WordPress content filters (shortcodes, wpautop, wptexturize)
    let rendered_content = rustpress_themes::apply_content_filters(&p.post_content);
    let rendered_title = rustpress_themes::apply_title_filters(&p.post_title);
    let rendered_excerpt = if p.post_excerpt.is_empty() {
        String::new()
    } else {
        rustpress_themes::apply_excerpt_filters(&p.post_excerpt)
    };

    WpPost {
        id: p.id,
        date: p.post_date.format("%Y-%m-%dT%H:%M:%S").to_string(),
        date_gmt: p.post_date_gmt.format("%Y-%m-%dT%H:%M:%S").to_string(),
        guid: WpRendered {
            rendered: guid_str,
        },
        modified: p.post_modified.format("%Y-%m-%dT%H:%M:%S").to_string(),
        modified_gmt: p.post_modified_gmt.format("%Y-%m-%dT%H:%M:%S").to_string(),
        slug: p.post_name,
        status: p.post_status,
        post_type: "post".to_string(),
        link,
        title: WpRendered {
            rendered: rendered_title,
        },
        content: WpRendered {
            rendered: rendered_content,
        },
        excerpt: WpRendered {
            rendered: rendered_excerpt,
        },
        author: p.post_author,
        featured_media,
        comment_status: p.comment_status,
        ping_status: p.ping_status,
        sticky,
        template: String::new(),
        format: "standard".to_string(),
        meta: vec![],
        categories,
        tags,
        _links: links,
    }
}

/// Build the `_embedded` object for a post when `?_embed` is requested.
/// Resolves: author (user), wp:term (categories + tags).
async fn build_embedded(
    db: &sea_orm::DatabaseConnection,
    site_url: &str,
    post: &WpPost,
) -> Value {
    use serde_json::json;

    // 1. Embed author
    let author_embed = if let Ok(Some(user)) = wp_users::Entity::find_by_id(post.author)
        .one(db)
        .await
    {
        let wp_user = crate::users::build_wp_user(db, &user, site_url, false).await;
        serde_json::to_value(&wp_user).unwrap_or(json!({}))
    } else {
        json!({})
    };

    // 2. Embed wp:term (categories and tags)
    let mut term_groups: Vec<Value> = Vec::new();

    // Categories
    let mut cat_terms = Vec::new();
    for &cat_id in &post.categories {
        if let Ok(Some(tt)) = wp_term_taxonomy::Entity::find()
            .filter(wp_term_taxonomy::Column::TermId.eq(cat_id))
            .filter(wp_term_taxonomy::Column::Taxonomy.eq("category"))
            .one(db)
            .await
        {
            if let Ok(Some(term)) = wp_terms::Entity::find_by_id(cat_id).one(db).await {
                let links = term_links(site_url, "category", cat_id);
                cat_terms.push(json!({
                    "id": cat_id,
                    "link": format!("{}/category/{}", site_url.trim_end_matches('/'), term.slug),
                    "name": term.name,
                    "slug": term.slug,
                    "taxonomy": "category",
                    "parent": tt.parent,
                    "_links": links,
                }));
            }
        }
    }
    term_groups.push(Value::Array(cat_terms));

    // Tags
    let mut tag_terms = Vec::new();
    for &tag_id in &post.tags {
        if let Ok(Some(_tt)) = wp_term_taxonomy::Entity::find()
            .filter(wp_term_taxonomy::Column::TermId.eq(tag_id))
            .filter(wp_term_taxonomy::Column::Taxonomy.eq("post_tag"))
            .one(db)
            .await
        {
            if let Ok(Some(term)) = wp_terms::Entity::find_by_id(tag_id).one(db).await {
                let links = term_links(site_url, "post_tag", tag_id);
                tag_terms.push(json!({
                    "id": tag_id,
                    "link": format!("{}/tag/{}", site_url.trim_end_matches('/'), term.slug),
                    "name": term.name,
                    "slug": term.slug,
                    "taxonomy": "post_tag",
                    "_links": links,
                }));
            }
        }
    }
    term_groups.push(Value::Array(tag_terms));

    json!({
        "author": [author_embed],
        "wp:term": term_groups,
    })
}

/// Load category and tag term_taxonomy_ids for a post.
async fn load_post_terms(db: &sea_orm::DatabaseConnection, post_id: u64) -> (Vec<u64>, Vec<u64>) {
    let mut categories = Vec::new();
    let mut tags = Vec::new();

    let rels = wp_term_relationships::Entity::find()
        .filter(wp_term_relationships::Column::ObjectId.eq(post_id))
        .all(db)
        .await
        .unwrap_or_default();

    if rels.is_empty() {
        return (categories, tags);
    }

    let tt_ids: Vec<u64> = rels.iter().map(|r| r.term_taxonomy_id).collect();
    let taxonomies = wp_term_taxonomy::Entity::find()
        .filter(wp_term_taxonomy::Column::TermTaxonomyId.is_in(tt_ids))
        .all(db)
        .await
        .unwrap_or_default();

    for tt in taxonomies {
        match tt.taxonomy.as_str() {
            "category" => categories.push(tt.term_id),
            "post_tag" => tags.push(tt.term_id),
            _ => {}
        }
    }

    (categories, tags)
}

/// Load featured media (thumbnail) ID from postmeta.
async fn load_featured_media(db: &sea_orm::DatabaseConnection, post_id: u64) -> u64 {
    let meta = wp_postmeta::Entity::find()
        .filter(wp_postmeta::Column::PostId.eq(post_id))
        .filter(wp_postmeta::Column::MetaKey.eq(Some("_thumbnail_id".to_string())))
        .one(db)
        .await
        .ok()
        .flatten();

    meta.and_then(|m| m.meta_value.and_then(|v| v.parse().ok()))
        .unwrap_or(0)
}

/// Check if a post is sticky by reading wp_options sticky_posts.
async fn check_sticky(db: &sea_orm::DatabaseConnection, post_id: u64) -> bool {
    use rustpress_db::entities::wp_options;
    let opt = wp_options::Entity::find()
        .filter(wp_options::Column::OptionName.eq("sticky_posts"))
        .one(db)
        .await
        .ok()
        .flatten();

    if let Some(o) = opt {
        // sticky_posts is stored as PHP serialized array; check if post_id appears
        o.option_value.contains(&post_id.to_string())
    } else {
        false
    }
}

/// Apply a term (category/tag) filter at the SQL level by resolving matching post IDs.
/// When `exclude` is true, filters OUT posts with the given terms.
async fn apply_term_filter(
    query: sea_orm::Select<wp_posts::Entity>,
    db: &sea_orm::DatabaseConnection,
    param: &Option<String>,
    taxonomy: &str,
    exclude: bool,
) -> sea_orm::Select<wp_posts::Entity> {
    let ids_str = match param {
        Some(s) if !s.is_empty() => s,
        _ => return query,
    };

    let term_ids: Vec<u64> = ids_str
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();
    if term_ids.is_empty() {
        return query;
    }

    // Resolve term_ids → term_taxonomy_ids
    let tt_ids: Vec<u64> = wp_term_taxonomy::Entity::find()
        .filter(wp_term_taxonomy::Column::TermId.is_in(term_ids))
        .filter(wp_term_taxonomy::Column::Taxonomy.eq(taxonomy))
        .all(db)
        .await
        .unwrap_or_default()
        .iter()
        .map(|tt| tt.term_taxonomy_id)
        .collect();

    if tt_ids.is_empty() {
        return if exclude {
            query // excluding nothing → keep all
        } else {
            query.filter(wp_posts::Column::Id.eq(0u64)) // including nothing → empty result
        };
    }

    // Resolve term_taxonomy_ids → post IDs
    let post_ids: Vec<u64> = wp_term_relationships::Entity::find()
        .filter(wp_term_relationships::Column::TermTaxonomyId.is_in(tt_ids))
        .all(db)
        .await
        .unwrap_or_default()
        .iter()
        .map(|r| r.object_id)
        .collect();

    if exclude {
        if !post_ids.is_empty() {
            query.filter(wp_posts::Column::Id.is_not_in(post_ids))
        } else {
            query
        }
    } else if !post_ids.is_empty() {
        query.filter(wp_posts::Column::Id.is_in(post_ids))
    } else {
        query.filter(wp_posts::Column::Id.eq(0u64)) // no matches
    }
}

/// Public read-only routes (GET) — no authentication required.
pub fn read_routes() -> Router<ApiState> {
    Router::new()
        .route("/wp-json/wp/v2/posts", get(list_posts))
        .route("/wp-json/wp/v2/posts/{id}", get(get_post))
}

/// Protected write routes (POST/PUT/DELETE) — authentication required.
pub fn write_routes() -> Router<ApiState> {
    Router::new()
        .route("/wp-json/wp/v2/posts", axum::routing::post(create_post))
        .route(
            "/wp-json/wp/v2/posts/{id}",
            axum::routing::put(update_post).patch(update_post).delete(delete_post),
        )
}

async fn list_posts(
    State(state): State<ApiState>,
    Query(params): Query<ListQuery>,
) -> Result<impl IntoResponse, WpError> {
    let page = params.page.unwrap_or(1);
    let per_page = params.per_page.unwrap_or(10).min(100);

    let mut query = wp_posts::Entity::find()
        .filter(wp_posts::Column::PostType.eq("post"));

    // Status filter (default: "publish")
    let status = params.status.as_deref().unwrap_or("publish");
    query = query.filter(wp_posts::Column::PostStatus.eq(status));

    // Search filter
    if let Some(ref search) = params.search {
        query = query.filter(wp_posts::Column::PostTitle.like(&format!("%{}%", search)));
    }

    // Author filter
    if let Some(author) = params.author {
        query = query.filter(wp_posts::Column::PostAuthor.eq(author));
    }

    // Slug filter
    if let Some(ref slug) = params.slug {
        query = query.filter(wp_posts::Column::PostName.eq(slug.as_str()));
    }

    // Include filter (comma-separated IDs)
    if let Some(ref include) = params.include {
        let ids: Vec<u64> = include
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect();
        if !ids.is_empty() {
            query = query.filter(wp_posts::Column::Id.is_in(ids));
        }
    }

    // Exclude filter (comma-separated IDs)
    if let Some(ref exclude) = params.exclude {
        let ids: Vec<u64> = exclude
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect();
        if !ids.is_empty() {
            query = query.filter(wp_posts::Column::Id.is_not_in(ids));
        }
    }

    // Before/after date filters
    if let Some(ref before) = params.before {
        if let Ok(dt) =
            chrono::NaiveDateTime::parse_from_str(before, "%Y-%m-%dT%H:%M:%S")
                .or_else(|_| chrono::NaiveDateTime::parse_from_str(before, "%Y-%m-%dT%H:%M"))
        {
            query = query.filter(wp_posts::Column::PostDate.lt(dt));
        }
    }
    if let Some(ref after) = params.after {
        if let Ok(dt) =
            chrono::NaiveDateTime::parse_from_str(after, "%Y-%m-%dT%H:%M:%S")
                .or_else(|_| chrono::NaiveDateTime::parse_from_str(after, "%Y-%m-%dT%H:%M"))
        {
            query = query.filter(wp_posts::Column::PostDate.gt(dt));
        }
    }

    // Category/tag filters — resolved to post IDs at the SQL level for correct pagination
    query = apply_term_filter(query, &state.db, &params.categories, "category", false).await;
    query = apply_term_filter(query, &state.db, &params.tags, "post_tag", false).await;
    query = apply_term_filter(query, &state.db, &params.categories_exclude, "category", true).await;
    query = apply_term_filter(query, &state.db, &params.tags_exclude, "post_tag", true).await;

    // Get total count for pagination
    let total = query
        .clone()
        .count(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?;
    let total_pages = if per_page > 0 {
        (total + per_page - 1) / per_page
    } else {
        1
    };

    // Ordering
    let order_desc = params.order.as_deref() != Some("asc");
    let orderby = params.orderby.as_deref().unwrap_or("date");
    query = match orderby {
        "title" => {
            if order_desc {
                query.order_by_desc(wp_posts::Column::PostTitle)
            } else {
                query.order_by_asc(wp_posts::Column::PostTitle)
            }
        }
        "id" => {
            if order_desc {
                query.order_by_desc(wp_posts::Column::Id)
            } else {
                query.order_by_asc(wp_posts::Column::Id)
            }
        }
        "modified" => {
            if order_desc {
                query.order_by_desc(wp_posts::Column::PostModified)
            } else {
                query.order_by_asc(wp_posts::Column::PostModified)
            }
        }
        "slug" => {
            if order_desc {
                query.order_by_desc(wp_posts::Column::PostName)
            } else {
                query.order_by_asc(wp_posts::Column::PostName)
            }
        }
        // "date" or default
        _ => {
            if order_desc {
                query.order_by_desc(wp_posts::Column::PostDate)
            } else {
                query.order_by_asc(wp_posts::Column::PostDate)
            }
        }
    };

    let posts = query
        .offset((page - 1) * per_page)
        .limit(per_page)
        .all(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?;

    let mut items = Vec::new();
    for p in posts {
        items.push(build_post(p, &state.site_url, &state.db).await);
    }

    let headers = pagination_headers(total, total_pages);
    let context = RestContext::from_option(params.context.as_deref());

    if params._embed.is_some() {
        let mut embedded_items = Vec::new();
        for post in &items {
            let mut val = serde_json::to_value(post).unwrap_or_default();
            let embedded = build_embedded(&state.db, &state.site_url, post).await;
            val.as_object_mut().map(|o| o.insert("_embedded".to_string(), embedded));
            filter_post_context(&mut val, context);
            embedded_items.push(val);
        }
        Ok((headers, Json(Value::Array(embedded_items))).into_response())
    } else {
        let mut json_items: Vec<Value> = items
            .iter()
            .map(|p| serde_json::to_value(p).unwrap_or_default())
            .collect();
        if context != RestContext::View {
            for item in json_items.iter_mut() {
                filter_post_context(item, context);
            }
        }
        Ok((headers, Json(Value::Array(json_items))).into_response())
    }
}

#[derive(Debug, Deserialize)]
pub struct GetPostQuery {
    pub context: Option<String>,
    pub _embed: Option<String>,
    pub _fields: Option<String>,
}

async fn get_post(
    State(state): State<ApiState>,
    Path(id): Path<u64>,
    Query(params): Query<GetPostQuery>,
) -> Result<impl IntoResponse, WpError> {
    let post = wp_posts::Entity::find_by_id(id)
        .filter(wp_posts::Column::PostType.eq("post"))
        .one(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?
        .ok_or(WpError::not_found("Post not found"))?;

    let wp_post = build_post(post, &state.site_url, &state.db).await;
    let context = RestContext::from_option(params.context.as_deref());

    let mut val = serde_json::to_value(&wp_post).unwrap_or_default();
    if params._embed.is_some() {
        let embedded = build_embedded(&state.db, &state.site_url, &wp_post).await;
        val.as_object_mut().map(|o| o.insert("_embedded".to_string(), embedded));
    }
    filter_post_context(&mut val, context);
    Ok(Json(val))
}

async fn create_post(
    State(state): State<ApiState>,
    auth: crate::AuthUser,
    Json(input): Json<WpPostWrite>,
) -> Result<(StatusCode, Json<WpPost>), WpError> {
    auth.require(&rustpress_auth::Capability::EditPosts)?;
    let now = chrono::Utc::now().naive_utc();
    let title = input.title.unwrap_or_default();
    let slug = input.slug.unwrap_or_else(|| slugify(&title));
    let status = input.status.unwrap_or_else(|| "draft".to_string());
    let post_type = input.post_type.unwrap_or_else(|| "post".to_string());
    let featured_media = input.featured_media;

    // Parse scheduled date if provided
    let post_date = if let Some(ref date_str) = input.date {
        chrono::NaiveDateTime::parse_from_str(date_str, "%Y-%m-%dT%H:%M:%S")
            .or_else(|_| chrono::NaiveDateTime::parse_from_str(date_str, "%Y-%m-%dT%H:%M"))
            .unwrap_or(now)
    } else {
        now
    };

    let new_post = wp_posts::ActiveModel {
        post_author: Set(input.author.unwrap_or(1)),
        post_date: Set(post_date),
        post_date_gmt: Set(post_date),
        post_content: Set(rustpress_core::wp_kses_post(&input.content.unwrap_or_default())),
        post_title: Set(title),
        post_excerpt: Set(rustpress_core::wp_kses_post(&input.excerpt.unwrap_or_default())),
        post_status: Set(status),
        comment_status: Set(input.comment_status.unwrap_or_else(|| "open".to_string())),
        ping_status: Set(input.ping_status.unwrap_or_else(|| "open".to_string())),
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

    let result = new_post
        .insert(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?;

    // Fire post-creation hooks
    let post_id_val = serde_json::json!(result.id);
    state.hooks.do_action("save_post", &post_id_val);
    state.hooks.do_action("wp_insert_post", &post_id_val);

    // Save featured image as _thumbnail_id postmeta
    if let Some(media_id) = featured_media {
        if media_id > 0 {
            save_thumbnail_meta(&state.db, result.id, media_id).await;
        }
    }

    // Save category/tag relationships
    if let Some(ref cat_ids) = input.categories {
        save_term_relationships(&state.db, result.id, cat_ids, "category").await;
    }
    if let Some(ref tag_ids) = input.tags {
        save_term_relationships(&state.db, result.id, tag_ids, "post_tag").await;
    }

    // Handle sticky
    if input.sticky.unwrap_or(false) {
        set_sticky(&state.db, result.id, true).await;
    }

    let wp_post = build_post(result, &state.site_url, &state.db).await;
    Ok((StatusCode::CREATED, Json(wp_post)))
}

async fn update_post(
    State(state): State<ApiState>,
    auth: crate::AuthUser,
    Path(id): Path<u64>,
    Json(input): Json<WpPostWrite>,
) -> Result<Json<WpPost>, WpError> {
    auth.require(&rustpress_auth::Capability::EditPosts)?;
    let post = wp_posts::Entity::find_by_id(id)
        .filter(wp_posts::Column::PostType.eq("post"))
        .one(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?
        .ok_or(WpError::not_found("Post not found"))?;

    // Save revision before updating (skip for revisions and auto-drafts)
    if post.post_type != "revision" && post.post_status != "auto-draft" {
        let _ = RevisionManager::save_revision(&state.db, &post).await;
    }

    let mut active: wp_posts::ActiveModel = post.into();
    let now = chrono::Utc::now().naive_utc();

    if let Some(title) = input.title {
        active.post_title = Set(title);
    }
    if let Some(content) = input.content {
        active.post_content = Set(rustpress_core::wp_kses_post(&content));
    }
    if let Some(excerpt) = input.excerpt {
        active.post_excerpt = Set(rustpress_core::wp_kses_post(&excerpt));
    }
    if let Some(status) = input.status {
        active.post_status = Set(status);
    }
    if let Some(slug) = input.slug {
        active.post_name = Set(slug);
    }
    if let Some(author) = input.author {
        active.post_author = Set(author);
    }
    if let Some(comment_status) = input.comment_status {
        active.comment_status = Set(comment_status);
    }
    if let Some(ping_status) = input.ping_status {
        active.ping_status = Set(ping_status);
    }

    // Handle scheduled date
    if let Some(ref date_str) = input.date {
        if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(date_str, "%Y-%m-%dT%H:%M:%S")
            .or_else(|_| chrono::NaiveDateTime::parse_from_str(date_str, "%Y-%m-%dT%H:%M"))
        {
            active.post_date = Set(dt);
            active.post_date_gmt = Set(dt);
        }
    }

    active.post_modified = Set(now);
    active.post_modified_gmt = Set(now);

    let updated = active
        .update(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?;

    // Fire post-update hooks
    let post_id_val = serde_json::json!(id);
    state.hooks.do_action("save_post", &post_id_val);
    state.hooks.do_action("edit_post", &post_id_val);

    // Save featured image
    if let Some(media_id) = input.featured_media {
        save_thumbnail_meta(&state.db, id, media_id).await;
    }

    // Update category/tag relationships
    if let Some(ref cat_ids) = input.categories {
        clear_term_relationships(&state.db, id, "category").await;
        save_term_relationships(&state.db, id, cat_ids, "category").await;
    }
    if let Some(ref tag_ids) = input.tags {
        clear_term_relationships(&state.db, id, "post_tag").await;
        save_term_relationships(&state.db, id, tag_ids, "post_tag").await;
    }

    // Handle sticky
    if let Some(sticky) = input.sticky {
        set_sticky(&state.db, id, sticky).await;
    }

    let wp_post = build_post(updated, &state.site_url, &state.db).await;
    Ok(Json(wp_post))
}

async fn delete_post(
    State(state): State<ApiState>,
    auth: crate::AuthUser,
    Path(id): Path<u64>,
    Query(params): Query<DeletePostQuery>,
) -> Result<Json<WpPost>, WpError> {
    auth.require(&rustpress_auth::Capability::DeletePosts)?;
    let post = wp_posts::Entity::find_by_id(id)
        .filter(wp_posts::Column::PostType.eq("post"))
        .one(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?
        .ok_or(WpError::not_found("Post not found"))?;

    let response = build_post(post.clone(), &state.site_url, &state.db).await;

    let force = params.force.unwrap_or(false);
    let post_id_val = serde_json::json!(id);

    // Fire pre-delete hook
    state.hooks.do_action("delete_post", &post_id_val);

    if force {
        // Hard delete
        wp_posts::Entity::delete_by_id(id)
            .exec(&state.db)
            .await
            .map_err(|e| WpError::internal(e.to_string()))?;
        state.hooks.do_action("deleted_post", &post_id_val);
    } else {
        // Soft delete: move to trash
        let mut active: wp_posts::ActiveModel = post.into();
        active.post_status = Set("trash".to_string());
        active
            .update(&state.db)
            .await
            .map_err(|e| WpError::internal(e.to_string()))?;
        state.hooks.do_action("trashed_post", &post_id_val);
    }

    Ok(Json(response))
}

async fn save_thumbnail_meta(db: &sea_orm::DatabaseConnection, post_id: u64, media_id: u64) {
    // Delete existing _thumbnail_id
    let _ = wp_postmeta::Entity::delete_many()
        .filter(wp_postmeta::Column::PostId.eq(post_id))
        .filter(wp_postmeta::Column::MetaKey.eq(Some("_thumbnail_id".to_string())))
        .exec(db)
        .await;

    if media_id > 0 {
        let meta = wp_postmeta::ActiveModel {
            meta_id: sea_orm::ActiveValue::NotSet,
            post_id: Set(post_id),
            meta_key: Set(Some("_thumbnail_id".to_string())),
            meta_value: Set(Some(media_id.to_string())),
        };
        let _ = meta.insert(db).await;
    }
}

/// Save term relationships for a post (categories or tags).
async fn save_term_relationships(
    db: &sea_orm::DatabaseConnection,
    post_id: u64,
    term_ids: &[u64],
    taxonomy: &str,
) {
    for &term_id in term_ids {
        // Look up term_taxonomy_id for this term_id + taxonomy
        let tt = wp_term_taxonomy::Entity::find()
            .filter(wp_term_taxonomy::Column::TermId.eq(term_id))
            .filter(wp_term_taxonomy::Column::Taxonomy.eq(taxonomy))
            .one(db)
            .await
            .ok()
            .flatten();

        if let Some(tt) = tt {
            let rel = wp_term_relationships::ActiveModel {
                object_id: Set(post_id),
                term_taxonomy_id: Set(tt.term_taxonomy_id),
                term_order: Set(0),
            };
            let _ = rel.insert(db).await;
        }
    }
}

/// Clear term relationships of a specific taxonomy for a post.
async fn clear_term_relationships(
    db: &sea_orm::DatabaseConnection,
    post_id: u64,
    taxonomy: &str,
) {
    // Get all term_taxonomy_ids for this taxonomy
    let tts = wp_term_taxonomy::Entity::find()
        .filter(wp_term_taxonomy::Column::Taxonomy.eq(taxonomy))
        .all(db)
        .await
        .unwrap_or_default();

    let tt_ids: Vec<u64> = tts.iter().map(|t| t.term_taxonomy_id).collect();
    if !tt_ids.is_empty() {
        let _ = wp_term_relationships::Entity::delete_many()
            .filter(wp_term_relationships::Column::ObjectId.eq(post_id))
            .filter(wp_term_relationships::Column::TermTaxonomyId.is_in(tt_ids))
            .exec(db)
            .await;
    }
}

/// Set or unset a post as sticky in wp_options.
async fn set_sticky(db: &sea_orm::DatabaseConnection, post_id: u64, sticky: bool) {
    use rustpress_db::options::OptionsManager;

    let options = OptionsManager::new(db.clone());
    let current = options
        .get_option("sticky_posts")
        .await
        .ok()
        .flatten()
        .unwrap_or_default();

    // Parse as comma-separated IDs (simplified from PHP serialization)
    let mut ids: Vec<u64> = current
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();

    if sticky {
        if !ids.contains(&post_id) {
            ids.push(post_id);
        }
    } else {
        ids.retain(|&id| id != post_id);
    }

    let new_value = ids
        .iter()
        .map(|id| id.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let _ = options.update_option("sticky_posts", &new_value).await;
}
