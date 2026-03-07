use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use sea_orm::{ColumnTrait, Condition, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use serde::{Deserialize, Serialize};

use rustpress_db::entities::wp_posts;

use crate::ApiState;

/// WP REST API Search result.
///
/// Corresponds to `WP_REST_Search_Controller` — `/wp/v2/search`.
#[derive(Debug, Serialize)]
pub struct WpSearchResult {
    pub id: u64,
    pub title: String,
    pub url: String,
    #[serde(rename = "type")]
    pub result_type: String,
    pub subtype: String,
}

#[derive(Debug, Deserialize)]
pub struct SearchParams {
    pub search: Option<String>,
    pub page: Option<u64>,
    pub per_page: Option<u64>,
    #[serde(rename = "type")]
    pub result_type: Option<String>,
    pub subtype: Option<String>,
}

pub fn routes() -> Router<ApiState> {
    Router::new().route("/wp-json/wp/v2/search", get(search))
}

async fn search(
    State(state): State<ApiState>,
    Query(params): Query<SearchParams>,
) -> Json<Vec<WpSearchResult>> {
    let search_term = params.search.unwrap_or_default();
    if search_term.is_empty() {
        return Json(vec![]);
    }

    let per_page = params.per_page.unwrap_or(10).min(100);
    let page = params.page.unwrap_or(1).max(1);
    let subtype = params.subtype.unwrap_or_else(|| "any".to_string());

    let pattern = format!("%{}%", search_term);

    let mut condition = Condition::all().add(
        Condition::any()
            .add(wp_posts::Column::PostTitle.like(&pattern))
            .add(wp_posts::Column::PostContent.like(&pattern)),
    );

    condition = condition.add(wp_posts::Column::PostStatus.eq("publish"));

    if subtype != "any" {
        condition = condition.add(wp_posts::Column::PostType.eq(&subtype));
    } else {
        condition = condition.add(wp_posts::Column::PostType.is_in(vec!["post", "page"]));
    }

    let results = wp_posts::Entity::find()
        .filter(condition)
        .order_by_desc(wp_posts::Column::PostDate)
        .offset((page - 1) * per_page)
        .limit(per_page)
        .all(&state.db)
        .await
        .unwrap_or_default();

    let search_results: Vec<WpSearchResult> = results
        .into_iter()
        .map(|p| WpSearchResult {
            id: p.id,
            title: p.post_title,
            url: format!("{}/{}", state.site_url, p.post_name),
            result_type: "post".to_string(),
            subtype: p.post_type,
        })
        .collect();

    Json(search_results)
}
