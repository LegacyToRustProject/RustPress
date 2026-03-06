use axum::{extract::Path, http::StatusCode, routing::get, Json, Router};
use serde::Serialize;

use crate::ApiState;

/// WP REST API Taxonomy response.
///
/// Corresponds to `WP_REST_Taxonomies_Controller` — `/wp/v2/taxonomies`.
#[derive(Debug, Serialize)]
pub struct WpTaxonomy {
    pub name: String,
    pub slug: String,
    pub description: String,
    pub hierarchical: bool,
    pub rest_base: String,
    pub types: Vec<String>,
}

pub fn routes() -> Router<ApiState> {
    Router::new()
        .route("/wp-json/wp/v2/taxonomies", get(list_taxonomies))
        .route("/wp-json/wp/v2/taxonomies/{slug}", get(get_taxonomy))
}

async fn list_taxonomies() -> Json<Vec<WpTaxonomy>> {
    // Return built-in taxonomies
    Json(vec![
        WpTaxonomy {
            name: "Categories".to_string(),
            slug: "category".to_string(),
            description: "".to_string(),
            hierarchical: true,
            rest_base: "categories".to_string(),
            types: vec!["post".to_string()],
        },
        WpTaxonomy {
            name: "Tags".to_string(),
            slug: "post_tag".to_string(),
            description: "".to_string(),
            hierarchical: false,
            rest_base: "tags".to_string(),
            types: vec!["post".to_string()],
        },
    ])
}

async fn get_taxonomy(Path(slug): Path<String>) -> Result<Json<WpTaxonomy>, StatusCode> {
    match slug.as_str() {
        "category" => Ok(Json(WpTaxonomy {
            name: "Categories".to_string(),
            slug: "category".to_string(),
            description: "".to_string(),
            hierarchical: true,
            rest_base: "categories".to_string(),
            types: vec!["post".to_string()],
        })),
        "post_tag" => Ok(Json(WpTaxonomy {
            name: "Tags".to_string(),
            slug: "post_tag".to_string(),
            description: "".to_string(),
            hierarchical: false,
            rest_base: "tags".to_string(),
            types: vec!["post".to_string()],
        })),
        _ => Err(StatusCode::NOT_FOUND),
    }
}
