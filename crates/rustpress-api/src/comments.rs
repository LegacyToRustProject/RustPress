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
use std::collections::HashMap;

use rustpress_db::entities::wp_comments;
use rustpress_db::entities::wp_posts;

use crate::common::{
    avatar_urls, comment_links, filter_comment_context, pagination_headers, RestContext, WpError,
};
use crate::ApiState;

#[derive(Debug, Serialize)]
pub struct WpComment {
    pub id: u64,
    pub post: u64,
    pub parent: u64,
    pub author: u64,
    pub author_name: String,
    pub author_email: String,
    pub author_url: String,
    pub author_avatar_urls: HashMap<String, String>,
    pub date: String,
    pub date_gmt: String,
    pub content: super::posts::WpRendered,
    pub status: String,
    #[serde(rename = "type")]
    pub comment_type: String,
    pub _links: Value,
}

#[derive(Debug, Deserialize)]
pub struct ListCommentsQuery {
    pub page: Option<u64>,
    pub per_page: Option<u64>,
    pub post: Option<u64>,
    pub status: Option<String>,
    pub include: Option<String>,
    pub exclude: Option<String>,
    pub parent: Option<u64>,
    pub parent_exclude: Option<String>,
    pub author_email: Option<String>,
    pub context: Option<String>,
    pub orderby: Option<String>,
    pub order: Option<String>,
    pub _fields: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GetCommentQuery {
    pub context: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateCommentRequest {
    pub post: u64,
    pub content: String,
    pub author_name: Option<String>,
    pub author_email: Option<String>,
    pub author_url: Option<String>,
    pub parent: Option<u64>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateCommentRequest {
    pub content: Option<String>,
    pub status: Option<String>,
    pub author_name: Option<String>,
    pub author_email: Option<String>,
    pub author_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DeleteCommentQuery {
    pub force: Option<bool>,
}

/// Map WordPress REST API status names to database comment_approved values.
/// "approved" -> "1", "hold"/"pending" -> "0", "spam" -> "spam", "trash" -> "trash"
fn status_to_db(status: &str) -> &str {
    match status {
        "approved" | "approve" => "1",
        "hold" | "pending" | "unapproved" => "0",
        "spam" => "spam",
        "trash" => "trash",
        other => other,
    }
}

/// Map database comment_approved values to WordPress REST API status names.
/// "1" -> "approved", "0" -> "hold", otherwise pass through.
fn db_to_status(approved: &str) -> String {
    match approved {
        "1" => "approved".to_string(),
        "0" => "hold".to_string(),
        other => other.to_string(),
    }
}

fn build_comment(c: wp_comments::Model, site_url: &str) -> WpComment {
    let links = comment_links(site_url, c.comment_id, c.comment_post_id);
    let avatars = avatar_urls(&c.comment_author_email);
    WpComment {
        id: c.comment_id,
        post: c.comment_post_id,
        parent: c.comment_parent,
        author: c.user_id,
        author_name: c.comment_author,
        author_email: c.comment_author_email,
        author_url: c.comment_author_url,
        author_avatar_urls: avatars,
        date: c.comment_date.format("%Y-%m-%dT%H:%M:%S").to_string(),
        date_gmt: c.comment_date_gmt.format("%Y-%m-%dT%H:%M:%S").to_string(),
        content: super::posts::WpRendered {
            rendered: rustpress_themes::apply_content_filters(&c.comment_content),
        },
        status: db_to_status(&c.comment_approved),
        comment_type: if c.comment_type.is_empty() {
            "comment".to_string()
        } else {
            c.comment_type
        },
        _links: links,
    }
}

/// Public read-only routes (GET) -- no authentication required.
pub fn read_routes() -> Router<ApiState> {
    Router::new()
        .route("/wp-json/wp/v2/comments", get(list_comments))
        .route("/wp-json/wp/v2/comments/{id}", get(get_comment))
}

/// Protected write routes (POST/PUT/DELETE) -- authentication required.
pub fn write_routes() -> Router<ApiState> {
    Router::new()
        .route(
            "/wp-json/wp/v2/comments",
            axum::routing::post(create_comment),
        )
        .route(
            "/wp-json/wp/v2/comments/{id}",
            axum::routing::put(update_comment)
                .patch(update_comment)
                .delete(delete_comment),
        )
}

async fn list_comments(
    State(state): State<ApiState>,
    Query(params): Query<ListCommentsQuery>,
) -> Result<impl IntoResponse, WpError> {
    let page = params.page.unwrap_or(1);
    let per_page = params.per_page.unwrap_or(10).min(100);

    let mut query = wp_comments::Entity::find();

    // Status filter: map WordPress status names to DB values
    let status_filter = params.status.as_deref().unwrap_or("approved");
    let db_status = status_to_db(status_filter);
    query = query.filter(wp_comments::Column::CommentApproved.eq(db_status));

    // Post filter
    if let Some(post_id) = params.post {
        query = query.filter(wp_comments::Column::CommentPostId.eq(post_id));
    }

    // Parent filter
    if let Some(parent_id) = params.parent {
        query = query.filter(wp_comments::Column::CommentParent.eq(parent_id));
    }

    // Author email filter
    if let Some(ref email) = params.author_email {
        query = query.filter(wp_comments::Column::CommentAuthorEmail.eq(email.as_str()));
    }

    // Include filter (comma-separated IDs)
    if let Some(ref include) = params.include {
        let ids: Vec<u64> = include
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect();
        if !ids.is_empty() {
            query = query.filter(wp_comments::Column::CommentId.is_in(ids));
        }
    }

    // Exclude filter (comma-separated IDs)
    if let Some(ref exclude) = params.exclude {
        let ids: Vec<u64> = exclude
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect();
        if !ids.is_empty() {
            query = query.filter(wp_comments::Column::CommentId.is_not_in(ids));
        }
    }

    // Parent exclude filter (comma-separated IDs)
    if let Some(ref parent_exclude) = params.parent_exclude {
        let ids: Vec<u64> = parent_exclude
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect();
        if !ids.is_empty() {
            query = query.filter(wp_comments::Column::CommentParent.is_not_in(ids));
        }
    }

    // Get total count for pagination headers
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
        "id" => {
            if order_desc {
                query.order_by_desc(wp_comments::Column::CommentId)
            } else {
                query.order_by_asc(wp_comments::Column::CommentId)
            }
        }
        "parent" => {
            if order_desc {
                query.order_by_desc(wp_comments::Column::CommentParent)
            } else {
                query.order_by_asc(wp_comments::Column::CommentParent)
            }
        }
        // "date" or default
        _ => {
            if order_desc {
                query.order_by_desc(wp_comments::Column::CommentDate)
            } else {
                query.order_by_asc(wp_comments::Column::CommentDate)
            }
        }
    };

    let comments = query
        .offset((page - 1) * per_page)
        .limit(per_page)
        .all(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?;

    let items: Vec<WpComment> = comments
        .into_iter()
        .map(|c| build_comment(c, &state.site_url))
        .collect();

    let context = RestContext::from_option(params.context.as_deref());
    let mut json_items: Vec<Value> = items
        .iter()
        .map(|c| serde_json::to_value(c).unwrap_or_default())
        .collect();
    if context != RestContext::View {
        for item in json_items.iter_mut() {
            filter_comment_context(item, context);
        }
    }

    let headers = pagination_headers(total, total_pages);
    Ok((headers, Json(json_items)))
}

async fn get_comment(
    State(state): State<ApiState>,
    Path(id): Path<u64>,
    Query(params): Query<GetCommentQuery>,
) -> Result<Json<Value>, WpError> {
    let comment = wp_comments::Entity::find_by_id(id)
        .one(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?
        .ok_or(WpError::not_found("Comment not found"))?;

    let context = RestContext::from_option(params.context.as_deref());
    let mut val = serde_json::to_value(build_comment(comment, &state.site_url)).unwrap_or_default();
    filter_comment_context(&mut val, context);
    Ok(Json(val))
}

async fn create_comment(
    State(state): State<ApiState>,
    _auth: crate::AuthUser,
    Json(input): Json<CreateCommentRequest>,
) -> Result<(StatusCode, Json<WpComment>), WpError> {
    // Any authenticated user can create comments
    let now = chrono::Utc::now().naive_utc();

    // Map status: default to "approved" ("1" in DB)
    let db_approved = match input.status.as_deref() {
        Some(s) => status_to_db(s).to_string(),
        None => "1".to_string(),
    };

    let new_comment = wp_comments::ActiveModel {
        comment_post_id: Set(input.post),
        comment_author: Set(input.author_name.unwrap_or_else(|| "Anonymous".to_string())),
        comment_author_email: Set(input.author_email.unwrap_or_default()),
        comment_author_url: Set(input.author_url.unwrap_or_default()),
        comment_author_ip: Set(String::new()),
        comment_date: Set(now),
        comment_date_gmt: Set(now),
        comment_content: Set(rustpress_core::wp_kses_comment(&input.content)),
        comment_karma: Set(0),
        comment_approved: Set(db_approved),
        comment_agent: Set(String::new()),
        comment_type: Set("comment".to_string()),
        comment_parent: Set(input.parent.unwrap_or(0)),
        user_id: Set(0),
        ..Default::default()
    };

    let result = new_comment
        .insert(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?;

    // Fire comment-creation hooks
    let comment_id_val = serde_json::json!(result.comment_id);
    state.hooks.do_action("comment_post", &comment_id_val);
    state.hooks.do_action("wp_insert_comment", &comment_id_val);

    // Update comment_count on the target post
    update_comment_count(&state.db, input.post).await;

    Ok((
        StatusCode::CREATED,
        Json(build_comment(result, &state.site_url)),
    ))
}

async fn update_comment(
    State(state): State<ApiState>,
    auth: crate::AuthUser,
    Path(id): Path<u64>,
    Json(input): Json<UpdateCommentRequest>,
) -> Result<Json<WpComment>, WpError> {
    auth.require(&rustpress_auth::Capability::ModerateComments)?;
    let comment = wp_comments::Entity::find_by_id(id)
        .one(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?
        .ok_or(WpError::not_found("Comment not found"))?;

    let mut active: wp_comments::ActiveModel = comment.into();

    if let Some(content) = input.content {
        active.comment_content = Set(rustpress_core::wp_kses_comment(&content));
    }
    if let Some(ref status) = input.status {
        active.comment_approved = Set(status_to_db(status).to_string());
    }
    if let Some(name) = input.author_name {
        active.comment_author = Set(name);
    }
    if let Some(email) = input.author_email {
        active.comment_author_email = Set(email);
    }
    if let Some(url) = input.author_url {
        active.comment_author_url = Set(url);
    }

    let updated = active
        .update(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?;

    Ok(Json(build_comment(updated, &state.site_url)))
}

async fn delete_comment(
    State(state): State<ApiState>,
    auth: crate::AuthUser,
    Path(id): Path<u64>,
    Query(params): Query<DeleteCommentQuery>,
) -> Result<Json<WpComment>, WpError> {
    auth.require(&rustpress_auth::Capability::ModerateComments)?;
    let comment = wp_comments::Entity::find_by_id(id)
        .one(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?
        .ok_or(WpError::not_found("Comment not found"))?;

    let post_id = comment.comment_post_id;
    let response = build_comment(comment.clone(), &state.site_url);

    if params.force.unwrap_or(false) {
        // Hard delete
        wp_comments::Entity::delete_by_id(id)
            .exec(&state.db)
            .await
            .map_err(|e| WpError::internal(e.to_string()))?;
    } else {
        // Soft delete: move to trash
        let mut active: wp_comments::ActiveModel = comment.into();
        active.comment_approved = Set("trash".to_string());
        active
            .update(&state.db)
            .await
            .map_err(|e| WpError::internal(e.to_string()))?;
    }

    // Update comment_count on the post
    update_comment_count(&state.db, post_id).await;

    Ok(Json(response))
}

/// Recalculate and update the comment_count on a post.
async fn update_comment_count(db: &sea_orm::DatabaseConnection, post_id: u64) {
    let count = wp_comments::Entity::find()
        .filter(wp_comments::Column::CommentPostId.eq(post_id))
        .filter(wp_comments::Column::CommentApproved.eq("1"))
        .count(db)
        .await
        .unwrap_or(0);

    if let Ok(Some(post)) = wp_posts::Entity::find_by_id(post_id).one(db).await {
        let mut active: wp_posts::ActiveModel = post.into();
        active.comment_count = Set(count as i64);
        let _ = active.update(db).await;
    }
}
