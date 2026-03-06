use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use serde::Serialize;

use rustpress_db::entities::wp_posts;
use rustpress_db::RevisionManager;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

use crate::ApiState;

/// WP REST API Revision response.
///
/// Corresponds to `WP_REST_Revisions_Controller` — `/wp/v2/posts/{id}/revisions`.
#[derive(Debug, Serialize)]
pub struct WpRevision {
    pub id: u64,
    pub author: u64,
    pub date: String,
    pub date_gmt: String,
    pub parent: u64,
    pub title: super::posts::WpRendered,
    pub content: super::posts::WpRendered,
    pub excerpt: super::posts::WpRendered,
}

pub fn routes() -> Router<ApiState> {
    Router::new()
        .route(
            "/wp-json/wp/v2/posts/{post_id}/revisions",
            get(list_revisions),
        )
        .route(
            "/wp-json/wp/v2/posts/{post_id}/revisions/{revision_id}",
            get(get_revision),
        )
}

async fn list_revisions(
    State(state): State<ApiState>,
    Path(post_id): Path<u64>,
) -> Result<Json<Vec<WpRevision>>, StatusCode> {
    let revisions = RevisionManager::get_revisions(&state.db, post_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let wp_revisions: Vec<WpRevision> = revisions
        .into_iter()
        .map(|r| WpRevision {
            id: r.id as u64,
            author: r.post_author as u64,
            date: r.post_date.to_string(),
            date_gmt: r.post_date_gmt.to_string(),
            parent: r.post_parent as u64,
            title: super::posts::WpRendered {
                rendered: rustpress_themes::apply_title_filters(&r.post_title),
            },
            content: super::posts::WpRendered {
                rendered: rustpress_themes::apply_content_filters(&r.post_content),
            },
            excerpt: super::posts::WpRendered {
                rendered: rustpress_themes::apply_excerpt_filters(&r.post_excerpt),
            },
        })
        .collect();

    Ok(Json(wp_revisions))
}

async fn get_revision(
    State(state): State<ApiState>,
    Path((post_id, revision_id)): Path<(u64, u64)>,
) -> Result<Json<WpRevision>, StatusCode> {
    let revision = wp_posts::Entity::find_by_id(revision_id)
        .filter(wp_posts::Column::PostParent.eq(post_id))
        .filter(wp_posts::Column::PostType.eq("revision"))
        .one(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(WpRevision {
        id: revision.id as u64,
        author: revision.post_author as u64,
        date: revision.post_date.to_string(),
        date_gmt: revision.post_date_gmt.to_string(),
        parent: revision.post_parent as u64,
        title: super::posts::WpRendered {
            rendered: rustpress_themes::apply_title_filters(&revision.post_title),
        },
        content: super::posts::WpRendered {
            rendered: rustpress_themes::apply_content_filters(&revision.post_content),
        },
        excerpt: super::posts::WpRendered {
            rendered: rustpress_themes::apply_excerpt_filters(&revision.post_excerpt),
        },
    }))
}
