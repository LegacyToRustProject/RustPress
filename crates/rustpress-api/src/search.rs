//! WordPress REST API Search
//!
//! GET /wp-json/wp/v2/search
//!
//! Corresponds to `WP_REST_Search_Controller`.
//! Supports type=post|term and subtype filters.

use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use sea_orm::{ColumnTrait, Condition, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use serde::{Deserialize, Serialize};
use serde_json::json;

use rustpress_db::entities::{wp_posts, wp_term_taxonomy, wp_terms};

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
    #[serde(rename = "_links")]
    pub links: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct SearchParams {
    pub search: Option<String>,
    pub page: Option<u64>,
    pub per_page: Option<u64>,
    #[serde(rename = "type")]
    pub result_type: Option<String>,
    pub subtype: Option<String>,
    pub exclude: Option<String>,
    pub include: Option<String>,
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
    let result_type = params.result_type.unwrap_or_else(|| "post".to_string());
    let subtype = params.subtype.unwrap_or_else(|| "any".to_string());
    let base = state.site_url.trim_end_matches('/').to_string();

    let results = match result_type.as_str() {
        "term" => search_terms(&state, &search_term, &subtype, per_page, page, &base).await,
        _ => search_posts(&state, &search_term, &subtype, per_page, page, &base).await,
    };

    Json(results)
}

async fn search_posts(
    state: &ApiState,
    term: &str,
    subtype: &str,
    per_page: u64,
    page: u64,
    base: &str,
) -> Vec<WpSearchResult> {
    let pattern = format!("%{term}%");

    let post_types: Vec<&str> = match subtype {
        "post" => vec!["post"],
        "page" => vec!["page"],
        _ => vec!["post", "page"],
    };

    let condition = Condition::all()
        .add(wp_posts::Column::PostStatus.eq("publish"))
        .add(
            Condition::any()
                .add(wp_posts::Column::PostTitle.like(&pattern))
                .add(wp_posts::Column::PostContent.like(&pattern)),
        )
        .add(wp_posts::Column::PostType.is_in(post_types));

    let rows = wp_posts::Entity::find()
        .filter(condition)
        .order_by_desc(wp_posts::Column::PostDate)
        .offset((page - 1) * per_page)
        .limit(per_page)
        .all(&state.db)
        .await
        .unwrap_or_default();

    rows.into_iter()
        .map(|p| {
            let rest_base = if p.post_type == "page" { "pages" } else { "posts" };
            let url = format!("{base}/{}", p.post_name);
            WpSearchResult {
                id: p.id,
                title: p.post_title.clone(),
                url,
                result_type: "post".to_string(),
                subtype: p.post_type.clone(),
                links: json!({
                    "self": [{"embeddable": true, "href": format!("{base}/wp-json/wp/v2/{rest_base}/{}", p.id)}],
                    "about": [{"href": format!("{base}/wp-json/wp/v2/types/{}", p.post_type)}]
                }),
            }
        })
        .collect()
}

async fn search_terms(
    state: &ApiState,
    term: &str,
    subtype: &str,
    per_page: u64,
    page: u64,
    base: &str,
) -> Vec<WpSearchResult> {
    let pattern = format!("%{term}%");

    // Step 1: fetch matching term IDs from wp_terms
    let matched_terms = wp_terms::Entity::find()
        .filter(wp_terms::Column::Name.like(&pattern))
        .order_by_desc(wp_terms::Column::TermId)
        .offset((page - 1) * per_page)
        .limit(per_page)
        .all(&state.db)
        .await
        .unwrap_or_default();

    if matched_terms.is_empty() {
        return vec![];
    }

    let term_ids: Vec<u64> = matched_terms.iter().map(|t| t.term_id).collect();

    // Step 2: get taxonomy info from wp_term_taxonomy
    let taxonomies_filter: Vec<&str> = match subtype {
        "category" => vec!["category"],
        "post_tag" | "tag" => vec!["post_tag"],
        _ => vec!["category", "post_tag"],
    };

    let tax_rows = wp_term_taxonomy::Entity::find()
        .filter(wp_term_taxonomy::Column::TermId.is_in(term_ids))
        .filter(wp_term_taxonomy::Column::Taxonomy.is_in(taxonomies_filter))
        .all(&state.db)
        .await
        .unwrap_or_default();

    // Build a map of term_id -> taxonomy
    let tax_map: std::collections::HashMap<u64, String> = tax_rows
        .into_iter()
        .map(|t| (t.term_id, t.taxonomy))
        .collect();

    matched_terms
        .into_iter()
        .filter_map(|t| {
            let taxonomy = tax_map.get(&t.term_id)?;
            let (rest_base, subtype_str) = if taxonomy == "category" {
                ("categories", "category")
            } else {
                ("tags", "post_tag")
            };
            Some(WpSearchResult {
                id: t.term_id,
                title: t.name.clone(),
                url: format!("{base}/?{taxonomy}={}", t.slug),
                result_type: "term".to_string(),
                subtype: subtype_str.to_string(),
                links: json!({
                    "self": [{"embeddable": true, "href": format!("{base}/wp-json/wp/v2/{rest_base}/{}", t.term_id)}],
                    "about": [{"href": format!("{base}/wp-json/wp/v2/taxonomies/{taxonomy}")}]
                }),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_params_defaults() {
        let s = SearchParams {
            search: Some("hello".into()),
            page: None,
            per_page: None,
            result_type: None,
            subtype: None,
            exclude: None,
            include: None,
        };
        assert_eq!(s.search.as_deref(), Some("hello"));
    }
}
