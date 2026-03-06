use axum::{extract::Path, http::StatusCode, routing::get, Json, Router};
use serde::Serialize;

use crate::ApiState;

/// WP REST API Post Type response.
///
/// Corresponds to `WP_REST_Post_Types_Controller` — `/wp/v2/types`.
#[derive(Debug, Serialize)]
pub struct WpPostType {
    pub name: String,
    pub slug: String,
    pub description: String,
    pub hierarchical: bool,
    pub rest_base: String,
    pub has_archive: bool,
    pub taxonomies: Vec<String>,
}

pub fn routes() -> Router<ApiState> {
    Router::new()
        .route("/wp-json/wp/v2/types", get(list_types))
        .route("/wp-json/wp/v2/types/{slug}", get(get_type))
}

async fn list_types() -> Json<Vec<WpPostType>> {
    Json(builtin_types())
}

async fn get_type(Path(slug): Path<String>) -> Result<Json<WpPostType>, StatusCode> {
    builtin_types()
        .into_iter()
        .find(|pt| pt.slug == slug)
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

fn builtin_types() -> Vec<WpPostType> {
    vec![
        WpPostType {
            name: "Posts".to_string(),
            slug: "post".to_string(),
            description: "".to_string(),
            hierarchical: false,
            rest_base: "posts".to_string(),
            has_archive: true,
            taxonomies: vec!["category".to_string(), "post_tag".to_string()],
        },
        WpPostType {
            name: "Pages".to_string(),
            slug: "page".to_string(),
            description: "".to_string(),
            hierarchical: true,
            rest_base: "pages".to_string(),
            has_archive: false,
            taxonomies: vec![],
        },
        WpPostType {
            name: "Media".to_string(),
            slug: "attachment".to_string(),
            description: "".to_string(),
            hierarchical: false,
            rest_base: "media".to_string(),
            has_archive: false,
            taxonomies: vec![],
        },
    ]
}
