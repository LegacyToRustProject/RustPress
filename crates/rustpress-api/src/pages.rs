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

use rustpress_db::entities::wp_posts;
use rustpress_db::revisions::RevisionManager;

use crate::common::{
    filter_post_context, pagination_headers, post_links, slugify, RestContext, WpError,
};
use crate::posts::WpRendered;
use crate::ApiState;

/// WP REST API Page response format.
/// Similar to WpPost but includes parent and menu_order fields.
#[derive(Debug, Serialize)]
pub struct WpPage {
    pub id: u64,
    pub date: String,
    pub date_gmt: String,
    pub modified: String,
    pub modified_gmt: String,
    pub slug: String,
    pub status: String,
    #[serde(rename = "type")]
    pub post_type: String,
    pub title: WpRendered,
    pub content: WpRendered,
    pub excerpt: WpRendered,
    pub author: u64,
    pub parent: u64,
    pub menu_order: i32,
    pub featured_media: u64,
    pub comment_status: String,
    pub ping_status: String,
    pub link: String,
    pub _links: Value,
}

#[derive(Debug, Deserialize)]
pub struct ListPagesQuery {
    pub page: Option<u64>,
    pub per_page: Option<u64>,
    pub search: Option<String>,
    pub status: Option<String>,
    pub author: Option<u64>,
    pub parent: Option<u64>,
    pub menu_order: Option<i32>,
    pub orderby: Option<String>,
    pub order: Option<String>,
    pub include: Option<String>,
    pub exclude: Option<String>,
    pub context: Option<String>,
    pub _fields: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreatePageRequest {
    pub title: Option<String>,
    pub content: Option<String>,
    pub excerpt: Option<String>,
    pub status: Option<String>,
    pub slug: Option<String>,
    pub author: Option<u64>,
    pub parent: Option<u64>,
    pub menu_order: Option<i32>,
    pub featured_media: Option<u64>,
    pub comment_status: Option<String>,
    pub ping_status: Option<String>,
    pub date: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePageRequest {
    pub title: Option<String>,
    pub content: Option<String>,
    pub excerpt: Option<String>,
    pub status: Option<String>,
    pub slug: Option<String>,
    pub author: Option<u64>,
    pub parent: Option<u64>,
    pub menu_order: Option<i32>,
    pub featured_media: Option<u64>,
    pub comment_status: Option<String>,
    pub ping_status: Option<String>,
    pub date: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GetPageQuery {
    pub context: Option<String>,
    pub _fields: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DeletePageQuery {
    pub force: Option<bool>,
}

pub fn build_page(p: wp_posts::Model, site_url: &str) -> WpPage {
    let links = post_links(site_url, p.id, "page", p.post_author);
    let link = format!("{}/{}", site_url.trim_end_matches('/'), p.post_name);

    let rendered_content = rustpress_themes::apply_content_filters(&p.post_content);
    let rendered_title = rustpress_themes::apply_title_filters(&p.post_title);
    let rendered_excerpt = if p.post_excerpt.is_empty() {
        String::new()
    } else {
        rustpress_themes::apply_excerpt_filters(&p.post_excerpt)
    };

    WpPage {
        id: p.id,
        date: p.post_date.format("%Y-%m-%dT%H:%M:%S").to_string(),
        date_gmt: p.post_date_gmt.format("%Y-%m-%dT%H:%M:%S").to_string(),
        modified: p.post_modified.format("%Y-%m-%dT%H:%M:%S").to_string(),
        modified_gmt: p.post_modified_gmt.format("%Y-%m-%dT%H:%M:%S").to_string(),
        slug: p.post_name,
        status: p.post_status,
        post_type: "page".to_string(),
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
        parent: p.post_parent,
        menu_order: p.menu_order,
        featured_media: 0,
        comment_status: p.comment_status,
        ping_status: p.ping_status,
        link,
        _links: links,
    }
}

/// Public read-only routes (GET) -- no authentication required.
pub fn read_routes() -> Router<ApiState> {
    Router::new()
        .route("/wp-json/wp/v2/pages", get(list_pages))
        .route("/wp-json/wp/v2/pages/{id}", get(get_page))
}

/// Protected write routes (POST/PUT/DELETE) -- authentication required.
pub fn write_routes() -> Router<ApiState> {
    Router::new()
        .route("/wp-json/wp/v2/pages", axum::routing::post(create_page))
        .route(
            "/wp-json/wp/v2/pages/{id}",
            axum::routing::put(update_page)
                .patch(update_page)
                .delete(delete_page),
        )
}

async fn list_pages(
    State(state): State<ApiState>,
    Query(params): Query<ListPagesQuery>,
) -> Result<impl IntoResponse, WpError> {
    let page = params.page.unwrap_or(1);
    let per_page = params.per_page.unwrap_or(10).min(100);

    let mut query = wp_posts::Entity::find().filter(wp_posts::Column::PostType.eq("page"));

    // Status filter (default: "publish")
    let status = params.status.as_deref().unwrap_or("publish");
    query = query.filter(wp_posts::Column::PostStatus.eq(status));

    // Search filter
    if let Some(ref search) = params.search {
        query = query.filter(wp_posts::Column::PostTitle.like(format!("%{}%", search)));
    }

    // Author filter
    if let Some(author) = params.author {
        query = query.filter(wp_posts::Column::PostAuthor.eq(author));
    }

    // Parent filter
    if let Some(parent) = params.parent {
        query = query.filter(wp_posts::Column::PostParent.eq(parent));
    }

    // Menu order filter
    if let Some(menu_order) = params.menu_order {
        query = query.filter(wp_posts::Column::MenuOrder.eq(menu_order));
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

    // Get total count for pagination
    let total = query
        .clone()
        .count(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?;
    let total_pages = if per_page > 0 {
        total.div_ceil(per_page)
    } else {
        1
    };

    // Ordering
    let order_desc = params.order.as_deref() != Some("asc");
    let orderby = params.orderby.as_deref().unwrap_or("menu_order");
    query = match orderby {
        "date" => {
            if order_desc {
                query.order_by_desc(wp_posts::Column::PostDate)
            } else {
                query.order_by_asc(wp_posts::Column::PostDate)
            }
        }
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
        // "menu_order" or default
        _ => {
            if order_desc {
                query.order_by_desc(wp_posts::Column::MenuOrder)
            } else {
                query.order_by_asc(wp_posts::Column::MenuOrder)
            }
        }
    };

    let posts = query
        .offset((page - 1) * per_page)
        .limit(per_page)
        .all(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?;

    let items: Vec<WpPage> = posts
        .into_iter()
        .map(|p| build_page(p, &state.site_url))
        .collect();

    let context = RestContext::from_option(params.context.as_deref());
    let mut json_items: Vec<Value> = items
        .iter()
        .map(|p| serde_json::to_value(p).unwrap_or_default())
        .collect();
    if context != RestContext::View {
        for item in json_items.iter_mut() {
            filter_post_context(item, context);
        }
    }

    let headers = pagination_headers(total, total_pages);
    Ok((headers, Json(Value::Array(json_items))))
}

async fn get_page(
    State(state): State<ApiState>,
    Path(id): Path<u64>,
    Query(params): Query<GetPageQuery>,
) -> Result<Json<Value>, WpError> {
    let post = wp_posts::Entity::find_by_id(id)
        .filter(wp_posts::Column::PostType.eq("page"))
        .one(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?
        .ok_or(WpError::not_found("Page not found"))?;

    let context = RestContext::from_option(params.context.as_deref());
    let mut val = serde_json::to_value(build_page(post, &state.site_url)).unwrap_or_default();
    filter_post_context(&mut val, context);
    Ok(Json(val))
}

async fn create_page(
    State(state): State<ApiState>,
    auth: crate::AuthUser,
    Json(input): Json<CreatePageRequest>,
) -> Result<(StatusCode, Json<WpPage>), WpError> {
    auth.require(&rustpress_auth::Capability::EditPages)?;
    let now = chrono::Utc::now().naive_utc();
    let title = input.title.unwrap_or_default();
    let slug = input.slug.unwrap_or_else(|| slugify(&title));
    let status = input.status.unwrap_or_else(|| "draft".to_string());

    // Parse scheduled date if provided
    let post_date = if let Some(ref date_str) = input.date {
        chrono::NaiveDateTime::parse_from_str(date_str, "%Y-%m-%dT%H:%M:%S")
            .or_else(|_| chrono::NaiveDateTime::parse_from_str(date_str, "%Y-%m-%dT%H:%M"))
            .unwrap_or(now)
    } else {
        now
    };

    let new_page = wp_posts::ActiveModel {
        post_author: Set(input.author.unwrap_or(1)),
        post_date: Set(post_date),
        post_date_gmt: Set(post_date),
        post_content: Set(rustpress_core::wp_kses_post(
            &input.content.unwrap_or_default(),
        )),
        post_title: Set(title),
        post_excerpt: Set(rustpress_core::wp_kses_post(
            &input.excerpt.unwrap_or_default(),
        )),
        post_status: Set(status),
        comment_status: Set(input.comment_status.unwrap_or_else(|| "closed".to_string())),
        ping_status: Set(input.ping_status.unwrap_or_else(|| "closed".to_string())),
        post_password: Set(String::new()),
        post_name: Set(slug),
        to_ping: Set(String::new()),
        pinged: Set(String::new()),
        post_modified: Set(now),
        post_modified_gmt: Set(now),
        post_content_filtered: Set(String::new()),
        post_parent: Set(input.parent.unwrap_or(0)),
        guid: Set(String::new()),
        menu_order: Set(input.menu_order.unwrap_or(0)),
        post_type: Set("page".to_string()),
        post_mime_type: Set(String::new()),
        comment_count: Set(0),
        ..Default::default()
    };

    let result = new_page
        .insert(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?;

    Ok((
        StatusCode::CREATED,
        Json(build_page(result, &state.site_url)),
    ))
}

async fn update_page(
    State(state): State<ApiState>,
    auth: crate::AuthUser,
    Path(id): Path<u64>,
    Json(input): Json<UpdatePageRequest>,
) -> Result<Json<WpPage>, WpError> {
    auth.require(&rustpress_auth::Capability::EditPages)?;
    let post = wp_posts::Entity::find_by_id(id)
        .filter(wp_posts::Column::PostType.eq("page"))
        .one(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?
        .ok_or(WpError::not_found("Page not found"))?;

    // Save revision before updating (skip for auto-drafts)
    if post.post_status != "auto-draft" {
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
    if let Some(parent) = input.parent {
        active.post_parent = Set(parent);
    }
    if let Some(menu_order) = input.menu_order {
        active.menu_order = Set(menu_order);
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

    Ok(Json(build_page(updated, &state.site_url)))
}

async fn delete_page(
    State(state): State<ApiState>,
    auth: crate::AuthUser,
    Path(id): Path<u64>,
    Query(params): Query<DeletePageQuery>,
) -> Result<Json<WpPage>, WpError> {
    auth.require(&rustpress_auth::Capability::DeletePages)?;
    let post = wp_posts::Entity::find_by_id(id)
        .filter(wp_posts::Column::PostType.eq("page"))
        .one(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?
        .ok_or(WpError::not_found("Page not found"))?;

    let response = build_page(post.clone(), &state.site_url);

    if params.force.unwrap_or(false) {
        // Hard delete
        wp_posts::Entity::delete_by_id(id)
            .exec(&state.db)
            .await
            .map_err(|e| WpError::internal(e.to_string()))?;
    } else {
        // Soft delete: move to trash
        let mut active: wp_posts::ActiveModel = post.into();
        active.post_status = Set("trash".to_string());
        active
            .update(&state.db)
            .await
            .map_err(|e| WpError::internal(e.to_string()))?;
    }

    Ok(Json(response))
}
