//! WordPress Block Renderer REST API
//!
//! GET /wp-json/wp/v2/block-renderer/{namespace}/{name}
//! POST /wp-json/wp/v2/block-renderer/{namespace}/{name}
//!
//! Renders server-side blocks (e.g. core/latest-posts, core/categories, etc.)
//! and returns the rendered HTML. Used by the Gutenberg block editor for preview.

use axum::{
    extract::{Path, Query, State},
    routing::get,
    Json, Router,
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use serde::Deserialize;
use serde_json::{json, Value};

use rustpress_db::entities::wp_posts;

use crate::common::WpError;
use crate::ApiState;

#[derive(Debug, Deserialize)]
pub struct BlockRendererQuery {
    pub post_id: Option<u64>,
    pub context: Option<String>,
    pub attributes: Option<String>,
}

pub fn routes() -> Router<ApiState> {
    Router::new().route(
        "/wp-json/wp/v2/block-renderer/{namespace}/{name}",
        get(render_block).post(render_block_post),
    )
}

/// GET /wp-json/wp/v2/block-renderer/{namespace}/{name}
async fn render_block(
    State(state): State<ApiState>,
    Path((namespace, name)): Path<(String, String)>,
    Query(params): Query<BlockRendererQuery>,
) -> Result<Json<Value>, WpError> {
    let html = render_block_html(&state, &namespace, &name, &params).await;
    Ok(Json(json!({ "rendered": html })))
}

/// POST /wp-json/wp/v2/block-renderer/{namespace}/{name}
async fn render_block_post(
    State(state): State<ApiState>,
    Path((namespace, name)): Path<(String, String)>,
    Query(params): Query<BlockRendererQuery>,
) -> Result<Json<Value>, WpError> {
    let html = render_block_html(&state, &namespace, &name, &params).await;
    Ok(Json(json!({ "rendered": html })))
}

/// Render a server-side block to HTML.
async fn render_block_html(
    state: &ApiState,
    namespace: &str,
    name: &str,
    _params: &BlockRendererQuery,
) -> String {
    let full_name = format!("{namespace}/{name}");

    match full_name.as_str() {
        "core/latest-posts" => {
            let posts = wp_posts::Entity::find()
                .filter(wp_posts::Column::PostType.eq("post"))
                .filter(wp_posts::Column::PostStatus.eq("publish"))
                .order_by_desc(wp_posts::Column::PostDate)
                .limit(5)
                .all(&state.db)
                .await
                .unwrap_or_default();

            let base = state.site_url.trim_end_matches('/');
            let items: Vec<String> = posts
                .iter()
                .map(|p| {
                    format!(
                        r#"<li><a href="{}/{}">{}</a></li>"#,
                        base, p.post_name, p.post_title
                    )
                })
                .collect();
            format!(
                "<ul class=\"wp-block-latest-posts\">{}</ul>",
                items.join("")
            )
        }
        "core/categories" => {
            use rustpress_db::entities::{wp_term_taxonomy, wp_terms};
            let terms = wp_term_taxonomy::Entity::find()
                .filter(wp_term_taxonomy::Column::Taxonomy.eq("category"))
                .all(&state.db)
                .await
                .unwrap_or_default();

            let base = state.site_url.trim_end_matches('/');
            let mut items = Vec::new();
            for tt in &terms {
                let term = wp_terms::Entity::find_by_id(tt.term_id)
                    .one(&state.db)
                    .await
                    .ok()
                    .flatten();
                if let Some(t) = term {
                    items.push(format!(
                        r#"<li class="cat-item"><a href="{}/category/{}">{}</a></li>"#,
                        base, t.slug, t.name
                    ));
                }
            }
            format!("<ul class=\"wp-block-categories\">{}</ul>", items.join(""))
        }
        "core/archives" => {
            let base = state.site_url.trim_end_matches('/');
            format!(
                r#"<ul class="wp-block-archives"><li><a href="{base}/feed/">Recent Posts</a></li></ul>"#
            )
        }
        "core/tag-cloud" => {
            use rustpress_db::entities::{wp_term_taxonomy, wp_terms};
            let terms = wp_term_taxonomy::Entity::find()
                .filter(wp_term_taxonomy::Column::Taxonomy.eq("post_tag"))
                .limit(20)
                .all(&state.db)
                .await
                .unwrap_or_default();

            let base = state.site_url.trim_end_matches('/');
            let mut tags = Vec::new();
            for tt in &terms {
                let term = wp_terms::Entity::find_by_id(tt.term_id)
                    .one(&state.db)
                    .await
                    .ok()
                    .flatten();
                if let Some(t) = term {
                    tags.push(format!(
                        r#"<a href="{}/tag/{}" class="tag-cloud-link">{}</a>"#,
                        base, t.slug, t.name
                    ));
                }
            }
            format!("<div class=\"wp-block-tag-cloud\">{}</div>", tags.join(" "))
        }
        "core/search" => {
            let base = state.site_url.trim_end_matches('/');
            format!(
                r#"<form role="search" method="get" class="wp-block-search__button-outside wp-block-search__text-button wp-block-search" action="{base}">
<div class="wp-block-search__inside-wrapper">
<input type="search" id="wp-block-search__input-1" class="wp-block-search__input" name="s" value="" placeholder="Search&hellip;" required>
<button type="submit" class="wp-block-search__button wp-element-button">Search</button>
</div></form>"#
            )
        }
        "core/latest-comments" => {
            use rustpress_db::entities::wp_comments;
            let comments = wp_comments::Entity::find()
                .filter(wp_comments::Column::CommentApproved.eq("1"))
                .order_by_desc(wp_comments::Column::CommentDate)
                .limit(5)
                .all(&state.db)
                .await
                .unwrap_or_default();

            let items: Vec<String> = comments
                .iter()
                .map(|c| format!(
                    r#"<li class="recentcomments"><span class="comment-author-link">{}</span> on <span>post</span></li>"#,
                    c.comment_author
                ))
                .collect();
            format!(
                "<ul class=\"wp-block-latest-comments\">{}</ul>",
                items.join("")
            )
        }
        // Static blocks — return empty rendered output (content provided by editor)
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_block_name_format() {
        let ns = "core";
        let name = "paragraph";
        let full = format!("{ns}/{name}");
        assert_eq!(full, "core/paragraph");
    }
}
