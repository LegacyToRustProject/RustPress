use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect, PaginatorTrait};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use rustpress_db::entities::wp_posts;

use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct PostsQuery {
    pub page: Option<u64>,
    pub per_page: Option<u64>,
    #[serde(rename = "type")]
    pub post_type: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PostJson {
    pub id: u64,
    pub title: String,
    pub content: String,
    pub excerpt: String,
    pub slug: String,
    pub status: String,
    pub post_type: String,
    pub date: String,
    pub author: u64,
    pub comment_count: i64,
}

impl From<wp_posts::Model> for PostJson {
    fn from(p: wp_posts::Model) -> Self {
        Self {
            id: p.id,
            title: p.post_title,
            content: p.post_content,
            excerpt: p.post_excerpt,
            slug: p.post_name,
            status: p.post_status,
            post_type: p.post_type,
            date: p.post_date.format("%Y-%m-%dT%H:%M:%S").to_string(),
            author: p.post_author,
            comment_count: p.comment_count,
        }
    }
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/posts", get(list_posts))
        .route("/api/posts/{id}", get(get_post_by_id))
        .route("/api/posts/slug/{slug}", get(get_post_by_slug))
}

async fn list_posts(
    State(state): State<Arc<AppState>>,
    Query(params): Query<PostsQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let page = params.page.unwrap_or(1);
    let per_page = params.per_page.unwrap_or(10);
    let post_type = params.post_type.as_deref().unwrap_or("post");
    let status = params.status.as_deref().unwrap_or("publish");

    let query = wp_posts::Entity::find()
        .filter(wp_posts::Column::PostType.eq(post_type))
        .filter(wp_posts::Column::PostStatus.eq(status))
        .order_by_desc(wp_posts::Column::PostDate);

    let total = query.clone().count(&state.db).await.map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    let posts = query
        .offset((page - 1) * per_page)
        .limit(per_page)
        .all(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let items: Vec<PostJson> = posts.into_iter().map(PostJson::from).collect();

    Ok(Json(serde_json::json!({
        "items": items,
        "total": total,
        "page": page,
        "per_page": per_page,
    })))
}

async fn get_post_by_id(
    State(state): State<Arc<AppState>>,
    Path(id): Path<u64>,
) -> Result<Json<PostJson>, (StatusCode, String)> {
    let post = wp_posts::Entity::find_by_id(id)
        .one(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    match post {
        Some(p) => Ok(Json(PostJson::from(p))),
        None => Err((StatusCode::NOT_FOUND, "Post not found".to_string())),
    }
}

async fn get_post_by_slug(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
) -> Result<Json<PostJson>, (StatusCode, String)> {
    let post = wp_posts::Entity::find()
        .filter(wp_posts::Column::PostName.eq(&slug))
        .filter(wp_posts::Column::PostStatus.eq("publish"))
        .one(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    match post {
        Some(p) => Ok(Json(PostJson::from(p))),
        None => Err((StatusCode::NOT_FOUND, "Post not found".to_string())),
    }
}
