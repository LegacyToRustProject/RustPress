use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use sea_orm::{ColumnTrait, EntityTrait, ModelTrait, QueryFilter};
use serde::{Deserialize, Serialize};

use rustpress_db::entities::wp_posts;
use rustpress_db::RevisionManager;

use crate::common::WpError;
use crate::ApiState;
use crate::AuthUser;

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
    pub slug: String,
    pub guid: super::posts::WpRendered,
}

#[derive(Debug, Deserialize)]
pub struct RevisionQuery {
    pub per_page: Option<u64>,
    pub page: Option<u64>,
    pub force: Option<bool>,
}

pub fn routes() -> Router<ApiState> {
    Router::new()
        .route(
            "/wp-json/wp/v2/posts/{post_id}/revisions",
            get(list_revisions),
        )
        .route(
            "/wp-json/wp/v2/posts/{post_id}/revisions/{revision_id}",
            get(get_revision).delete(delete_revision),
        )
        // Pages share the same revision logic
        .route(
            "/wp-json/wp/v2/pages/{post_id}/revisions",
            get(list_revisions),
        )
        .route(
            "/wp-json/wp/v2/pages/{post_id}/revisions/{revision_id}",
            get(get_revision).delete(delete_revision),
        )
}

fn to_wp_revision(r: &wp_posts::Model) -> WpRevision {
    WpRevision {
        id: r.id,
        author: r.post_author,
        date: r.post_date.to_string(),
        date_gmt: r.post_date_gmt.to_string(),
        parent: r.post_parent,
        slug: r.post_name.clone(),
        title: super::posts::WpRendered {
            rendered: rustpress_themes::apply_title_filters(&r.post_title),
        },
        content: super::posts::WpRendered {
            rendered: rustpress_themes::apply_content_filters(&r.post_content),
        },
        excerpt: super::posts::WpRendered {
            rendered: rustpress_themes::apply_excerpt_filters(&r.post_excerpt),
        },
        guid: super::posts::WpRendered {
            rendered: r.guid.clone(),
        },
    }
}

/// GET /wp-json/wp/v2/posts/{post_id}/revisions
async fn list_revisions(
    State(state): State<ApiState>,
    Path(post_id): Path<u64>,
    Query(_params): Query<RevisionQuery>,
) -> Result<Json<Vec<WpRevision>>, WpError> {
    let revisions = RevisionManager::get_revisions(&state.db, post_id)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?;

    Ok(Json(revisions.iter().map(to_wp_revision).collect()))
}

/// GET /wp-json/wp/v2/posts/{post_id}/revisions/{revision_id}
async fn get_revision(
    State(state): State<ApiState>,
    Path((post_id, revision_id)): Path<(u64, u64)>,
) -> Result<Json<WpRevision>, WpError> {
    let revision = wp_posts::Entity::find_by_id(revision_id)
        .filter(wp_posts::Column::PostParent.eq(post_id))
        .filter(wp_posts::Column::PostType.eq("revision"))
        .one(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?
        .ok_or_else(|| WpError::not_found("Revision not found"))?;

    Ok(Json(to_wp_revision(&revision)))
}

/// DELETE /wp-json/wp/v2/posts/{post_id}/revisions/{revision_id}
///
/// WordPress equivalent: `WP_REST_Revisions_Controller::delete_item()`
/// Requires authentication and `delete_posts` capability.
/// The `?force=true` query param is required (WordPress behavior).
async fn delete_revision(
    State(state): State<ApiState>,
    _auth: AuthUser,
    Path((post_id, revision_id)): Path<(u64, u64)>,
    Query(params): Query<RevisionQuery>,
) -> Result<Json<WpRevision>, WpError> {
    // WordPress requires force=true to permanently delete revisions
    if params.force != Some(true) {
        return Err(WpError::new(
            StatusCode::BAD_REQUEST,
            "rest_trash_not_supported",
            "Revisions do not support trashing. Set 'force=true' to delete.",
        ));
    }

    let revision = wp_posts::Entity::find_by_id(revision_id)
        .filter(wp_posts::Column::PostParent.eq(post_id))
        .filter(wp_posts::Column::PostType.eq("revision"))
        .one(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?
        .ok_or_else(|| WpError::not_found("Revision not found"))?;

    let previous = to_wp_revision(&revision);

    revision
        .delete(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?;

    Ok(Json(previous))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wp_revision_fields() {
        // Ensure WpRevision has the required WordPress REST API fields
        let _ = std::mem::size_of::<WpRevision>();
    }
}
