use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, PaginatorTrait,
    QueryFilter, QueryOrder, QuerySelect,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use rustpress_db::entities::{wp_term_relationships, wp_term_taxonomy, wp_terms};

use crate::common::{filter_term_context, pagination_headers, slugify, term_links, RestContext, WpError};
use crate::ApiState;

/// WordPress REST API Category response format.
#[derive(Debug, Serialize)]
pub struct WpCategory {
    pub id: u64,
    pub count: i64,
    pub description: String,
    pub link: String,
    pub name: String,
    pub slug: String,
    pub taxonomy: String,
    pub parent: u64,
    pub meta: Vec<Value>,
    pub _links: Value,
}

#[derive(Debug, Deserialize)]
pub struct ListCategoriesQuery {
    pub page: Option<u64>,
    pub per_page: Option<u64>,
    pub parent: Option<u64>,
    pub search: Option<String>,
    pub exclude: Option<String>,
    pub include: Option<String>,
    pub slug: Option<String>,
    pub hide_empty: Option<bool>,
    pub context: Option<String>,
    pub orderby: Option<String>,
    pub order: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GetCategoryQuery {
    pub context: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateCategoryRequest {
    pub name: String,
    pub slug: Option<String>,
    pub description: Option<String>,
    pub parent: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateCategoryRequest {
    pub name: Option<String>,
    pub slug: Option<String>,
    pub description: Option<String>,
    pub parent: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct DeleteQuery {
    pub force: Option<bool>,
}

/// Public read-only routes (GET) -- no authentication required.
pub fn read_routes() -> Router<ApiState> {
    Router::new()
        .route("/wp-json/wp/v2/categories", get(list_categories))
        .route("/wp-json/wp/v2/categories/{id}", get(get_category))
}

/// Protected write routes (POST/PUT/DELETE) -- authentication required.
pub fn write_routes() -> Router<ApiState> {
    Router::new()
        .route(
            "/wp-json/wp/v2/categories",
            axum::routing::post(create_category),
        )
        .route(
            "/wp-json/wp/v2/categories/{id}",
            axum::routing::put(update_category).patch(update_category).delete(delete_category),
        )
}

/// Parse a comma-separated string of u64 IDs.
fn parse_id_list(s: &str) -> Vec<u64> {
    s.split(',')
        .filter_map(|v| v.trim().parse::<u64>().ok())
        .collect()
}

async fn list_categories(
    State(state): State<ApiState>,
    Query(params): Query<ListCategoriesQuery>,
) -> Result<impl IntoResponse, WpError> {
    let page = params.page.unwrap_or(1).max(1);
    let per_page = params.per_page.unwrap_or(10).min(100);

    // Build base query for counting
    let mut count_query = wp_term_taxonomy::Entity::find()
        .filter(wp_term_taxonomy::Column::Taxonomy.eq("category"));

    // Build paginated query
    let mut query = wp_term_taxonomy::Entity::find()
        .filter(wp_term_taxonomy::Column::Taxonomy.eq("category"));

    // Filter: parent
    if let Some(parent) = params.parent {
        count_query = count_query.filter(wp_term_taxonomy::Column::Parent.eq(parent));
        query = query.filter(wp_term_taxonomy::Column::Parent.eq(parent));
    }

    // Filter: hide_empty (count > 0)
    if params.hide_empty.unwrap_or(false) {
        count_query =
            count_query.filter(wp_term_taxonomy::Column::Count.gt(0));
        query = query.filter(wp_term_taxonomy::Column::Count.gt(0));
    }

    // Filter: exclude
    if let Some(ref exclude) = params.exclude {
        let ids = parse_id_list(exclude);
        if !ids.is_empty() {
            count_query = count_query
                .filter(wp_term_taxonomy::Column::TermTaxonomyId.is_not_in(ids.clone()));
            query =
                query.filter(wp_term_taxonomy::Column::TermTaxonomyId.is_not_in(ids));
        }
    }

    // Filter: include
    if let Some(ref include) = params.include {
        let ids = parse_id_list(include);
        if !ids.is_empty() {
            count_query =
                count_query.filter(wp_term_taxonomy::Column::TermTaxonomyId.is_in(ids.clone()));
            query = query.filter(wp_term_taxonomy::Column::TermTaxonomyId.is_in(ids));
        }
    }

    // Total count for pagination headers
    let total = count_query
        .count(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))? as u64;
    let total_pages = if per_page > 0 {
        (total + per_page - 1) / per_page
    } else {
        1
    };

    // Ordering
    let order_asc = params.order.as_deref() == Some("asc");
    let orderby = params.orderby.as_deref().unwrap_or("name");
    // For name ordering, we need to order after fetching since the name is in wp_terms.
    // For id and count, we can order in the query.
    query = match orderby {
        "id" => {
            if order_asc {
                query.order_by_asc(wp_term_taxonomy::Column::TermTaxonomyId)
            } else {
                query.order_by_desc(wp_term_taxonomy::Column::TermTaxonomyId)
            }
        }
        "count" => {
            if order_asc {
                query.order_by_asc(wp_term_taxonomy::Column::Count)
            } else {
                query.order_by_desc(wp_term_taxonomy::Column::Count)
            }
        }
        // Default: order by term_id (approximation for name ordering)
        _ => {
            if order_asc {
                query.order_by_asc(wp_term_taxonomy::Column::TermId)
            } else {
                query.order_by_desc(wp_term_taxonomy::Column::TermId)
            }
        }
    };

    let taxonomies = query
        .offset((page - 1) * per_page)
        .limit(per_page)
        .all(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?;

    let mut categories = Vec::new();
    for tax in taxonomies {
        if let Some(term) = wp_terms::Entity::find_by_id(tax.term_id)
            .one(&state.db)
            .await
            .map_err(|e| WpError::internal(e.to_string()))?
        {
            // Filter: search (match on term name)
            if let Some(ref search) = params.search {
                if !term
                    .name
                    .to_lowercase()
                    .contains(&search.to_lowercase())
                {
                    continue;
                }
            }

            // Filter: slug
            if let Some(ref slug_filter) = params.slug {
                if term.slug != *slug_filter {
                    continue;
                }
            }

            categories.push(build_wp_category(&state.site_url, &term, &tax));
        }
    }

    // Sort by name if orderby=name (default)
    if orderby == "name" || orderby == "slug" {
        categories.sort_by(|a, b| {
            let cmp = a.name.to_lowercase().cmp(&b.name.to_lowercase());
            if order_asc {
                cmp
            } else {
                cmp.reverse()
            }
        });
    }

    let context = RestContext::from_option(params.context.as_deref());
    let mut json_items: Vec<Value> = categories
        .iter()
        .map(|c| serde_json::to_value(c).unwrap_or_default())
        .collect();
    if context != RestContext::View {
        for item in json_items.iter_mut() {
            filter_term_context(item, context);
        }
    }

    let headers = pagination_headers(total, total_pages);
    Ok((headers, Json(json_items)))
}

async fn get_category(
    State(state): State<ApiState>,
    Path(id): Path<u64>,
    Query(params): Query<GetCategoryQuery>,
) -> Result<Json<Value>, WpError> {
    let tax = wp_term_taxonomy::Entity::find_by_id(id)
        .filter(wp_term_taxonomy::Column::Taxonomy.eq("category"))
        .one(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?
        .ok_or(WpError::not_found("Category not found"))?;

    let term = wp_terms::Entity::find_by_id(tax.term_id)
        .one(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?
        .ok_or(WpError::not_found("Term not found"))?;

    let context = RestContext::from_option(params.context.as_deref());
    let mut val = serde_json::to_value(&build_wp_category(&state.site_url, &term, &tax))
        .unwrap_or_default();
    filter_term_context(&mut val, context);
    Ok(Json(val))
}

async fn create_category(
    State(state): State<ApiState>,
    auth: crate::AuthUser,
    Json(input): Json<CreateCategoryRequest>,
) -> Result<(StatusCode, Json<WpCategory>), WpError> {
    auth.require(&rustpress_auth::Capability::ManageCategories)?;
    let slug = input
        .slug
        .unwrap_or_else(|| slugify(&input.name));

    // Insert into wp_terms
    let new_term = wp_terms::ActiveModel {
        term_id: sea_orm::ActiveValue::NotSet,
        name: Set(input.name),
        slug: Set(slug),
        term_group: Set(0),
    };

    let term = new_term
        .insert(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?;

    // Insert into wp_term_taxonomy
    let new_taxonomy = wp_term_taxonomy::ActiveModel {
        term_taxonomy_id: sea_orm::ActiveValue::NotSet,
        term_id: Set(term.term_id),
        taxonomy: Set("category".to_string()),
        description: Set(input.description.unwrap_or_default()),
        parent: Set(input.parent.unwrap_or(0)),
        count: Set(0),
    };

    let tax = new_taxonomy
        .insert(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?;

    Ok((
        StatusCode::CREATED,
        Json(build_wp_category(&state.site_url, &term, &tax)),
    ))
}

async fn update_category(
    State(state): State<ApiState>,
    auth: crate::AuthUser,
    Path(id): Path<u64>,
    Json(input): Json<UpdateCategoryRequest>,
) -> Result<Json<WpCategory>, WpError> {
    auth.require(&rustpress_auth::Capability::ManageCategories)?;
    // Find existing taxonomy entry
    let tax = wp_term_taxonomy::Entity::find_by_id(id)
        .filter(wp_term_taxonomy::Column::Taxonomy.eq("category"))
        .one(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?
        .ok_or(WpError::not_found("Category not found"))?;

    // Find associated term
    let term = wp_terms::Entity::find_by_id(tax.term_id)
        .one(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?
        .ok_or(WpError::not_found("Term not found"))?;

    // Update wp_terms if name or slug changed
    let mut term_active: wp_terms::ActiveModel = term.into();
    if let Some(name) = input.name {
        term_active.name = Set(name);
    }
    if let Some(slug) = input.slug {
        term_active.slug = Set(slug);
    }
    let updated_term = term_active
        .update(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?;

    // Update wp_term_taxonomy if description or parent changed
    let mut tax_active: wp_term_taxonomy::ActiveModel = tax.into();
    if let Some(description) = input.description {
        tax_active.description = Set(description);
    }
    if let Some(parent) = input.parent {
        tax_active.parent = Set(parent);
    }
    let updated_tax = tax_active
        .update(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?;

    Ok(Json(build_wp_category(
        &state.site_url,
        &updated_term,
        &updated_tax,
    )))
}

async fn delete_category(
    State(state): State<ApiState>,
    auth: crate::AuthUser,
    Path(id): Path<u64>,
    Query(params): Query<DeleteQuery>,
) -> Result<Json<Value>, WpError> {
    auth.require(&rustpress_auth::Capability::ManageCategories)?;
    let force = params.force.unwrap_or(false);
    if !force {
        return Err(WpError::bad_request("Categories do not support trashing. Set force=true to delete."));
    }

    // Find the taxonomy entry
    let tax = wp_term_taxonomy::Entity::find_by_id(id)
        .filter(wp_term_taxonomy::Column::Taxonomy.eq("category"))
        .one(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?
        .ok_or(WpError::not_found("Category not found"))?;

    let term_id = tax.term_id;

    // Find term for the response
    let term = wp_terms::Entity::find_by_id(term_id)
        .one(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?
        .ok_or(WpError::not_found("Term not found"))?;

    let response_category = build_wp_category(&state.site_url, &term, &tax);

    // Delete from wp_term_relationships where term_taxonomy_id = id
    wp_term_relationships::Entity::delete_many()
        .filter(wp_term_relationships::Column::TermTaxonomyId.eq(id))
        .exec(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?;

    // Delete from wp_term_taxonomy
    let tax_active: wp_term_taxonomy::ActiveModel = tax.into();
    tax_active
        .delete(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?;

    // Delete from wp_terms
    let term_active: wp_terms::ActiveModel = term.into();
    term_active
        .delete(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?;

    Ok(Json(serde_json::json!({
        "deleted": true,
        "previous": response_category
    })))
}

/// Helper: build a WpCategory from term + taxonomy models.
fn build_wp_category(
    site_url: &str,
    term: &wp_terms::Model,
    tax: &wp_term_taxonomy::Model,
) -> WpCategory {
    WpCategory {
        id: tax.term_taxonomy_id,
        count: tax.count,
        description: tax.description.clone(),
        link: format!(
            "{}/category/{}",
            site_url.trim_end_matches('/'),
            term.slug
        ),
        name: term.name.clone(),
        slug: term.slug.clone(),
        taxonomy: "category".to_string(),
        parent: tax.parent,
        meta: vec![],
        _links: term_links(site_url, "category", tax.term_taxonomy_id),
    }
}
