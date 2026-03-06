use axum::{extract::Path, http::StatusCode, routing::get, Json, Router};
use serde::Serialize;

use crate::ApiState;

/// WP REST API Post Status response.
///
/// Corresponds to `WP_REST_Post_Statuses_Controller` — `/wp/v2/statuses`.
#[derive(Debug, Serialize)]
pub struct WpPostStatus {
    pub name: String,
    pub slug: String,
    pub public: bool,
    pub queryable: bool,
    #[serde(rename = "show_in_list")]
    pub show_in_list: bool,
}

pub fn routes() -> Router<ApiState> {
    Router::new()
        .route("/wp-json/wp/v2/statuses", get(list_statuses))
        .route("/wp-json/wp/v2/statuses/{slug}", get(get_status))
}

async fn list_statuses() -> Json<Vec<WpPostStatus>> {
    Json(builtin_statuses())
}

async fn get_status(Path(slug): Path<String>) -> Result<Json<WpPostStatus>, StatusCode> {
    builtin_statuses()
        .into_iter()
        .find(|s| s.slug == slug)
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

fn builtin_statuses() -> Vec<WpPostStatus> {
    vec![
        WpPostStatus {
            name: "Published".to_string(),
            slug: "publish".to_string(),
            public: true,
            queryable: true,
            show_in_list: true,
        },
        WpPostStatus {
            name: "Future".to_string(),
            slug: "future".to_string(),
            public: false,
            queryable: false,
            show_in_list: true,
        },
        WpPostStatus {
            name: "Draft".to_string(),
            slug: "draft".to_string(),
            public: false,
            queryable: false,
            show_in_list: true,
        },
        WpPostStatus {
            name: "Pending".to_string(),
            slug: "pending".to_string(),
            public: false,
            queryable: false,
            show_in_list: true,
        },
        WpPostStatus {
            name: "Private".to_string(),
            slug: "private".to_string(),
            public: false,
            queryable: true,
            show_in_list: true,
        },
        WpPostStatus {
            name: "Trash".to_string(),
            slug: "trash".to_string(),
            public: false,
            queryable: false,
            show_in_list: false,
        },
    ]
}
