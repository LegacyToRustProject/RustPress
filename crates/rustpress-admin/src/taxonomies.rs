use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{delete, get, post, put},
    Json, Router,
};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect,
};
use serde::{Deserialize, Serialize};
use tracing::info;

use rustpress_db::entities::{wp_term_taxonomy, wp_terms};

use crate::AdminState;

#[derive(Debug, Serialize)]
pub struct AdminTerm {
    pub term_id: u64,
    pub name: String,
    pub slug: String,
    pub taxonomy: Option<String>,
    pub description: Option<String>,
    pub parent: Option<u64>,
    pub count: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct TermListParams {
    pub taxonomy: Option<String>,
    pub search: Option<String>,
    pub page: Option<u64>,
    pub per_page: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct CreateTermRequest {
    pub name: String,
    pub slug: Option<String>,
    pub taxonomy: String,
    pub description: Option<String>,
    pub parent: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateTermRequest {
    pub name: Option<String>,
    pub slug: Option<String>,
    pub description: Option<String>,
}

pub fn routes() -> Router<AdminState> {
    Router::new()
        .route("/admin/terms", get(list_terms))
        .route("/admin/terms", post(create_term))
        .route("/admin/terms/{id}", get(get_term))
        .route("/admin/terms/{id}", put(update_term))
        .route("/admin/terms/{id}", delete(delete_term))
}

async fn list_terms(
    State(state): State<AdminState>,
    Query(params): Query<TermListParams>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let per_page = params.per_page.unwrap_or(100).min(200);
    let page = params.page.unwrap_or(1).max(1);

    // If taxonomy filter is provided, query via term_taxonomy first
    if let Some(ref taxonomy) = params.taxonomy {
        let tt_query = wp_term_taxonomy::Entity::find()
            .filter(wp_term_taxonomy::Column::Taxonomy.eq(taxonomy.as_str()));

        let tt_records = tt_query
            .order_by_asc(wp_term_taxonomy::Column::TermId)
            .all(&state.db)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        if tt_records.is_empty() {
            return Ok(Json(serde_json::json!({
                "items": Vec::<AdminTerm>::new(),
                "total": 0,
                "page": page,
                "per_page": per_page,
            })));
        }

        let term_ids: Vec<u64> = tt_records.iter().map(|tt| tt.term_id).collect();
        let tt_map: std::collections::HashMap<u64, &wp_term_taxonomy::Model> =
            tt_records.iter().map(|tt| (tt.term_id, tt)).collect();

        let mut term_query =
            wp_terms::Entity::find().filter(wp_terms::Column::TermId.is_in(term_ids));

        if let Some(ref search) = params.search {
            let pattern = format!("%{search}%");
            term_query = term_query.filter(wp_terms::Column::Name.like(&pattern));
        }

        let total = term_query.clone().count(&state.db).await.unwrap_or(0);

        let terms = term_query
            .order_by_asc(wp_terms::Column::Name)
            .offset((page - 1) * per_page)
            .limit(per_page)
            .all(&state.db)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let items: Vec<AdminTerm> = terms
            .into_iter()
            .map(|t| {
                let tt = tt_map.get(&t.term_id);
                AdminTerm {
                    term_id: t.term_id,
                    name: t.name,
                    slug: t.slug,
                    taxonomy: Some(taxonomy.clone()),
                    description: tt.map(|tt| tt.description.clone()),
                    parent: tt.map(|tt| tt.parent),
                    count: tt.map(|tt| tt.count as u64),
                }
            })
            .collect();

        return Ok(Json(serde_json::json!({
            "items": items,
            "total": total,
            "page": page,
            "per_page": per_page,
        })));
    }

    // No taxonomy filter — return all terms
    let mut query = wp_terms::Entity::find();

    if let Some(ref search) = params.search {
        let pattern = format!("%{search}%");
        query = query.filter(wp_terms::Column::Name.like(&pattern));
    }

    let total = query.clone().count(&state.db).await.unwrap_or(0);

    let terms = query
        .order_by_asc(wp_terms::Column::Name)
        .offset((page - 1) * per_page)
        .limit(per_page)
        .all(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let items: Vec<AdminTerm> = terms
        .into_iter()
        .map(|t| AdminTerm {
            term_id: t.term_id,
            name: t.name,
            slug: t.slug,
            taxonomy: None,
            description: None,
            parent: None,
            count: None,
        })
        .collect();

    Ok(Json(serde_json::json!({
        "items": items,
        "total": total,
        "page": page,
        "per_page": per_page,
    })))
}

async fn get_term(
    State(state): State<AdminState>,
    Path(id): Path<u64>,
) -> Result<Json<AdminTerm>, StatusCode> {
    let term = wp_terms::Entity::find_by_id(id)
        .one(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(AdminTerm {
        term_id: term.term_id,
        name: term.name,
        slug: term.slug,
        taxonomy: None,
        description: None,
        parent: None,
        count: None,
    }))
}

async fn create_term(
    State(state): State<AdminState>,
    Json(body): Json<CreateTermRequest>,
) -> Result<(StatusCode, Json<AdminTerm>), StatusCode> {
    let slug = body.slug.unwrap_or_else(|| slugify(&body.name));

    let term = wp_terms::ActiveModel {
        term_id: sea_orm::ActiveValue::NotSet,
        name: Set(body.name.clone()),
        slug: Set(slug.clone()),
        term_group: Set(0),
    };

    let result = term
        .insert(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Create term_taxonomy entry
    let tt = wp_term_taxonomy::ActiveModel {
        term_taxonomy_id: sea_orm::ActiveValue::NotSet,
        term_id: Set(result.term_id),
        taxonomy: Set(body.taxonomy.clone()),
        description: Set(body.description.unwrap_or_default()),
        parent: Set(body.parent.unwrap_or(0)),
        count: Set(0),
    };
    let _ = tt.insert(&state.db).await;

    info!(term_id = result.term_id, name = body.name, "term created");

    Ok((
        StatusCode::CREATED,
        Json(AdminTerm {
            term_id: result.term_id,
            name: result.name,
            slug: result.slug,
            taxonomy: Some(body.taxonomy),
            description: None,
            parent: body.parent,
            count: Some(0),
        }),
    ))
}

async fn update_term(
    State(state): State<AdminState>,
    Path(id): Path<u64>,
    Json(body): Json<UpdateTermRequest>,
) -> Result<Json<AdminTerm>, StatusCode> {
    let term = wp_terms::Entity::find_by_id(id)
        .one(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let mut active: wp_terms::ActiveModel = term.into();

    if let Some(ref name) = body.name {
        active.name = Set(name.clone());
    }
    if let Some(ref slug) = body.slug {
        active.slug = Set(slug.clone());
    }

    let updated = active
        .update(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    info!(id, "term updated");
    Ok(Json(AdminTerm {
        term_id: updated.term_id,
        name: updated.name,
        slug: updated.slug,
        taxonomy: None,
        description: None,
        parent: None,
        count: None,
    }))
}

async fn delete_term(
    State(state): State<AdminState>,
    Path(id): Path<u64>,
) -> Result<StatusCode, StatusCode> {
    // Delete term_taxonomy entries first
    wp_term_taxonomy::Entity::delete_many()
        .filter(wp_term_taxonomy::Column::TermId.eq(id))
        .exec(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    wp_terms::Entity::delete_by_id(id)
        .exec(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    info!(id, "term deleted");
    Ok(StatusCode::NO_CONTENT)
}

/// Simple slugify function.
fn slugify(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}
