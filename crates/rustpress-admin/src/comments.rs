use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{delete, get, put},
    Json, Router,
};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect,
};
use serde::{Deserialize, Serialize};
use tracing::info;

use rustpress_db::entities::wp_comments;

use crate::AdminState;

#[derive(Debug, Serialize)]
pub struct AdminComment {
    pub id: u64,
    pub post_id: u64,
    pub author: String,
    pub author_email: String,
    pub content: String,
    pub status: String,
    pub date: String,
    #[serde(rename = "type")]
    pub comment_type: String,
    pub parent: u64,
}

#[derive(Debug, Deserialize)]
pub struct CommentListParams {
    pub status: Option<String>,
    pub post_id: Option<u64>,
    pub page: Option<u64>,
    pub per_page: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateCommentRequest {
    pub status: Option<String>,
    pub content: Option<String>,
}

pub fn routes() -> Router<AdminState> {
    Router::new()
        .route("/admin/comments", get(list_comments))
        .route("/admin/comments/{id}", get(get_comment))
        .route("/admin/comments/{id}", put(update_comment))
        .route("/admin/comments/{id}", delete(delete_comment))
}

async fn list_comments(
    State(state): State<AdminState>,
    Query(params): Query<CommentListParams>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let per_page = params.per_page.unwrap_or(20).min(100);
    let page = params.page.unwrap_or(1).max(1);

    let mut query = wp_comments::Entity::find();

    if let Some(ref status) = params.status {
        let db_status = match status.as_str() {
            "approved" => "1",
            "pending" | "hold" => "0",
            "spam" => "spam",
            "trash" => "trash",
            _ => "1",
        };
        query = query.filter(wp_comments::Column::CommentApproved.eq(db_status));
    }

    if let Some(post_id) = params.post_id {
        query = query.filter(wp_comments::Column::CommentPostId.eq(post_id));
    }

    let total = query.clone().count(&state.db).await.unwrap_or(0);

    let comments = query
        .order_by_desc(wp_comments::Column::CommentDateGmt)
        .offset((page - 1) * per_page)
        .limit(per_page)
        .all(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let items: Vec<AdminComment> = comments.into_iter().map(|c| to_admin_comment(c)).collect();

    Ok(Json(serde_json::json!({
        "items": items,
        "total": total,
        "page": page,
        "per_page": per_page,
    })))
}

async fn get_comment(
    State(state): State<AdminState>,
    Path(id): Path<u64>,
) -> Result<Json<AdminComment>, StatusCode> {
    let comment = wp_comments::Entity::find_by_id(id)
        .one(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(to_admin_comment(comment)))
}

async fn update_comment(
    State(state): State<AdminState>,
    Path(id): Path<u64>,
    Json(body): Json<UpdateCommentRequest>,
) -> Result<Json<AdminComment>, StatusCode> {
    let comment = wp_comments::Entity::find_by_id(id)
        .one(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let mut active: wp_comments::ActiveModel = comment.into();

    if let Some(ref status) = body.status {
        let db_status = match status.as_str() {
            "approved" | "approve" => "1",
            "pending" | "hold" => "0",
            "spam" => "spam",
            "trash" => "trash",
            _ => "1",
        };
        active.comment_approved = Set(db_status.to_string());
    }

    if let Some(ref content) = body.content {
        active.comment_content = Set(content.clone());
    }

    let updated = active
        .update(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    info!(id, "comment updated");
    Ok(Json(to_admin_comment(updated)))
}

async fn delete_comment(
    State(state): State<AdminState>,
    Path(id): Path<u64>,
) -> Result<StatusCode, StatusCode> {
    wp_comments::Entity::delete_by_id(id)
        .exec(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    info!(id, "comment deleted");
    Ok(StatusCode::NO_CONTENT)
}

fn to_admin_comment(c: wp_comments::Model) -> AdminComment {
    AdminComment {
        id: c.comment_id as u64,
        post_id: c.comment_post_id as u64,
        author: c.comment_author,
        author_email: c.comment_author_email,
        content: c.comment_content,
        status: match c.comment_approved.as_str() {
            "1" => "approved".to_string(),
            "0" => "pending".to_string(),
            other => other.to_string(),
        },
        date: c.comment_date_gmt.to_string(),
        comment_type: c.comment_type,
        parent: c.comment_parent as u64,
    }
}
