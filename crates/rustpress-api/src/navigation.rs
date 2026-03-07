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

use crate::common::{
    envelope_response, filter_post_context, pagination_headers_with_link, post_links, slugify,
    RestContext, WpError,
};
use crate::posts::WpRendered;
use crate::ApiState;

/// WP REST API Navigation response format.
/// Represents a `wp_navigation` post type used for navigation menus.
#[derive(Debug, Serialize)]
pub struct WpNavigation {
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
    pub title: WpRendered,
    pub content: WpRendered,
    pub _links: Value,
}

#[derive(Debug, Deserialize)]
pub struct ListNavigationQuery {
    pub page: Option<u64>,
    pub per_page: Option<u64>,
    pub search: Option<String>,
    pub status: Option<String>,
    pub slug: Option<String>,
    pub orderby: Option<String>,
    pub order: Option<String>,
    pub context: Option<String>,
    pub _fields: Option<String>,
    pub _embed: Option<String>,
    pub _envelope: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GetNavigationQuery {
    pub context: Option<String>,
    pub _fields: Option<String>,
    pub _embed: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateNavigationRequest {
    pub title: Option<String>,
    pub content: Option<String>,
    pub status: Option<String>,
    pub slug: Option<String>,
    pub date: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateNavigationRequest {
    pub title: Option<String>,
    pub content: Option<String>,
    pub status: Option<String>,
    pub slug: Option<String>,
    pub date: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DeleteNavigationQuery {
    pub force: Option<bool>,
}

pub fn build_navigation(p: wp_posts::Model, site_url: &str) -> WpNavigation {
    let links = post_links(site_url, p.id, "wp_navigation", p.post_author);

    let rendered_content = rustpress_themes::apply_content_filters(&p.post_content);
    let rendered_title = rustpress_themes::apply_title_filters(&p.post_title);

    WpNavigation {
        id: p.id,
        date: p.post_date.format("%Y-%m-%dT%H:%M:%S").to_string(),
        date_gmt: p.post_date_gmt.format("%Y-%m-%dT%H:%M:%S").to_string(),
        guid: WpRendered { rendered: p.guid },
        modified: p.post_modified.format("%Y-%m-%dT%H:%M:%S").to_string(),
        modified_gmt: p.post_modified_gmt.format("%Y-%m-%dT%H:%M:%S").to_string(),
        slug: p.post_name,
        status: p.post_status,
        post_type: "wp_navigation".to_string(),
        title: WpRendered {
            rendered: rendered_title,
        },
        content: WpRendered {
            rendered: rendered_content,
        },
        _links: links,
    }
}

/// Public read-only routes (GET) -- no authentication required.
pub fn read_routes() -> Router<ApiState> {
    Router::new()
        .route("/wp-json/wp/v2/navigation", get(list_navigation))
        .route("/wp-json/wp/v2/navigation/{id}", get(get_navigation))
}

/// Protected write routes (POST/PUT/DELETE) -- authentication required.
pub fn write_routes() -> Router<ApiState> {
    Router::new()
        .route(
            "/wp-json/wp/v2/navigation",
            axum::routing::post(create_navigation),
        )
        .route(
            "/wp-json/wp/v2/navigation/{id}",
            axum::routing::put(update_navigation)
                .patch(update_navigation)
                .delete(delete_navigation),
        )
}

async fn list_navigation(
    State(state): State<ApiState>,
    Query(params): Query<ListNavigationQuery>,
) -> Result<impl IntoResponse, WpError> {
    let page = params.page.unwrap_or(1);
    let per_page = params.per_page.unwrap_or(10).min(100);

    let mut query = wp_posts::Entity::find().filter(wp_posts::Column::PostType.eq("wp_navigation"));

    // Status filter (default: "publish")
    let status = params.status.as_deref().unwrap_or("publish");
    query = query.filter(wp_posts::Column::PostStatus.eq(status));

    // Search filter
    if let Some(ref search) = params.search {
        query = query.filter(wp_posts::Column::PostTitle.like(format!("%{}%", search)));
    }

    // Slug filter
    if let Some(ref slug) = params.slug {
        query = query.filter(wp_posts::Column::PostName.eq(slug.as_str()));
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

    let items: Vec<WpNavigation> = posts
        .into_iter()
        .map(|p| build_navigation(p, &state.site_url))
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

    let base_url = format!("{}/wp-json/wp/v2/navigation", state.site_url);
    let headers = pagination_headers_with_link(total, total_pages, page, &base_url);

    let body = Value::Array(json_items);
    if params._envelope.is_some() {
        Ok(Json(envelope_response(200, &headers, body)).into_response())
    } else {
        Ok((headers, Json(body)).into_response())
    }
}

async fn get_navigation(
    State(state): State<ApiState>,
    Path(id): Path<u64>,
    Query(params): Query<GetNavigationQuery>,
) -> Result<Json<Value>, WpError> {
    let post = wp_posts::Entity::find_by_id(id)
        .filter(wp_posts::Column::PostType.eq("wp_navigation"))
        .one(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?
        .ok_or(WpError::not_found("Navigation not found"))?;

    let context = RestContext::from_option(params.context.as_deref());
    let mut val = serde_json::to_value(build_navigation(post, &state.site_url)).unwrap_or_default();
    filter_post_context(&mut val, context);
    Ok(Json(val))
}

async fn create_navigation(
    State(state): State<ApiState>,
    auth: crate::AuthUser,
    Json(input): Json<CreateNavigationRequest>,
) -> Result<(StatusCode, Json<WpNavigation>), WpError> {
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

    let new_nav = wp_posts::ActiveModel {
        post_author: Set(1),
        post_date: Set(post_date),
        post_date_gmt: Set(post_date),
        post_content: Set(rustpress_core::wp_kses_post(
            &input.content.unwrap_or_default(),
        )),
        post_title: Set(title),
        post_excerpt: Set(String::new()),
        post_status: Set(status),
        comment_status: Set("closed".to_string()),
        ping_status: Set("closed".to_string()),
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
        post_type: Set("wp_navigation".to_string()),
        post_mime_type: Set(String::new()),
        comment_count: Set(0),
        ..Default::default()
    };

    let result = new_nav
        .insert(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?;

    Ok((
        StatusCode::CREATED,
        Json(build_navigation(result, &state.site_url)),
    ))
}

async fn update_navigation(
    State(state): State<ApiState>,
    auth: crate::AuthUser,
    Path(id): Path<u64>,
    Json(input): Json<UpdateNavigationRequest>,
) -> Result<Json<WpNavigation>, WpError> {
    auth.require(&rustpress_auth::Capability::EditPages)?;
    let post = wp_posts::Entity::find_by_id(id)
        .filter(wp_posts::Column::PostType.eq("wp_navigation"))
        .one(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?
        .ok_or(WpError::not_found("Navigation not found"))?;

    let mut active: wp_posts::ActiveModel = post.into();
    let now = chrono::Utc::now().naive_utc();

    if let Some(title) = input.title {
        active.post_title = Set(title);
    }
    if let Some(content) = input.content {
        active.post_content = Set(rustpress_core::wp_kses_post(&content));
    }
    if let Some(status) = input.status {
        active.post_status = Set(status);
    }
    if let Some(slug) = input.slug {
        active.post_name = Set(slug);
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

    Ok(Json(build_navigation(updated, &state.site_url)))
}

async fn delete_navigation(
    State(state): State<ApiState>,
    auth: crate::AuthUser,
    Path(id): Path<u64>,
    Query(params): Query<DeleteNavigationQuery>,
) -> Result<Json<WpNavigation>, WpError> {
    auth.require(&rustpress_auth::Capability::DeletePages)?;
    let post = wp_posts::Entity::find_by_id(id)
        .filter(wp_posts::Column::PostType.eq("wp_navigation"))
        .one(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?
        .ok_or(WpError::not_found("Navigation not found"))?;

    let response = build_navigation(post.clone(), &state.site_url);

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
