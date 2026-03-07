//! WooCommerce REST API v3 compatible endpoints.
//!
//! Provides `/wp-json/wc/v3/products`, `/wp-json/wc/v3/orders`, etc.
//! These endpoints read from the same wp_posts + wp_postmeta tables
//! that WooCommerce uses, enabling existing WooCommerce clients
//! (mobile apps, POS systems, etc.) to work with RustPress.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;

use rustpress_commerce::woo_compat::{self, WooOrderData, WooProductData};
use rustpress_db::entities::wp_posts;
use rustpress_db::queries;

use crate::state::AppState;

/// WC API pagination query params.
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct WcListParams {
    pub page: Option<u64>,
    pub per_page: Option<u64>,
    pub search: Option<String>,
    pub status: Option<String>,
    pub orderby: Option<String>,
    pub order: Option<String>,
}

/// Register WooCommerce REST API v3 routes.
pub fn routes(state: Arc<AppState>) -> Router {
    Router::new()
        // Products
        .route("/wp-json/wc/v3/products", get(list_products))
        .route("/wp-json/wc/v3/products/{id}", get(get_product))
        // Orders
        .route("/wp-json/wc/v3/orders", get(list_orders))
        .route("/wp-json/wc/v3/orders/{id}", get(get_order))
        // System status
        .route("/wp-json/wc/v3/system_status", get(system_status))
        .with_state(state)
}

/// GET /wp-json/wc/v3/products
async fn list_products(
    State(state): State<Arc<AppState>>,
    Query(params): Query<WcListParams>,
) -> impl IntoResponse {
    let page = params.page.unwrap_or(1);
    let per_page = params.per_page.unwrap_or(10).min(100);

    let mut query = wp_posts::Entity::find()
        .filter(wp_posts::Column::PostType.eq(woo_compat::post_types::PRODUCT))
        .filter(wp_posts::Column::PostStatus.eq("publish"));

    if let Some(ref search) = params.search {
        query = query.filter(wp_posts::Column::PostTitle.contains(search));
    }

    let products = query
        .order_by_desc(wp_posts::Column::PostDate)
        .offset((page - 1) * per_page)
        .limit(per_page)
        .all(&state.db)
        .await
        .unwrap_or_default();

    let mut result = Vec::new();
    for post in products {
        let meta = queries::get_post_meta_map(&state.db, post.id)
            .await
            .unwrap_or_default();
        let woo = WooProductData::from_post_and_meta(
            post.id,
            &post.post_title,
            &post.post_name,
            &post.post_content,
            &post.post_excerpt,
            &meta,
        );
        result.push(product_to_json(&woo));
    }

    Json(result)
}

/// GET /wp-json/wc/v3/products/:id
async fn get_product(
    State(state): State<Arc<AppState>>,
    Path(id): Path<u64>,
) -> Result<impl IntoResponse, StatusCode> {
    let post = wp_posts::Entity::find_by_id(id)
        .filter(wp_posts::Column::PostType.eq(woo_compat::post_types::PRODUCT))
        .one(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let meta = queries::get_post_meta_map(&state.db, post.id)
        .await
        .unwrap_or_default();

    let woo = WooProductData::from_post_and_meta(
        post.id,
        &post.post_title,
        &post.post_name,
        &post.post_content,
        &post.post_excerpt,
        &meta,
    );

    Ok(Json(product_to_json(&woo)))
}

/// GET /wp-json/wc/v3/orders
async fn list_orders(
    State(state): State<Arc<AppState>>,
    Query(params): Query<WcListParams>,
) -> impl IntoResponse {
    let page = params.page.unwrap_or(1);
    let per_page = params.per_page.unwrap_or(10).min(100);

    let status_filter = params.status.as_deref().map(|s| {
        if s.starts_with("wc-") {
            s.to_string()
        } else {
            format!("wc-{s}")
        }
    });

    let mut query = wp_posts::Entity::find()
        .filter(wp_posts::Column::PostType.eq(woo_compat::post_types::ORDER));

    if let Some(ref status) = status_filter {
        query = query.filter(wp_posts::Column::PostStatus.eq(status.as_str()));
    } else {
        // Exclude trashed orders
        query = query.filter(wp_posts::Column::PostStatus.ne("trash"));
    }

    let orders = query
        .order_by_desc(wp_posts::Column::PostDate)
        .offset((page - 1) * per_page)
        .limit(per_page)
        .all(&state.db)
        .await
        .unwrap_or_default();

    let mut result = Vec::new();
    for post in orders {
        let meta = queries::get_post_meta_map(&state.db, post.id)
            .await
            .unwrap_or_default();
        let woo = WooOrderData::from_post_and_meta(post.id, &post.post_status, &meta);

        // Fetch order line items
        let items = queries::get_order_items(&state.db, post.id)
            .await
            .unwrap_or_default();

        let mut line_items = Vec::new();
        for item in &items {
            let item_meta = queries::get_order_item_meta(&state.db, item.order_item_id)
                .await
                .unwrap_or_default();
            line_items.push(order_item_to_json(item, &item_meta));
        }

        let mut order_json = order_to_json(&woo, &post);
        order_json
            .as_object_mut()
            .unwrap()
            .insert("line_items".into(), serde_json::json!(line_items));

        result.push(order_json);
    }

    Json(result)
}

/// GET /wp-json/wc/v3/orders/:id
async fn get_order(
    State(state): State<Arc<AppState>>,
    Path(id): Path<u64>,
) -> Result<impl IntoResponse, StatusCode> {
    let post = wp_posts::Entity::find_by_id(id)
        .filter(wp_posts::Column::PostType.eq(woo_compat::post_types::ORDER))
        .one(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let meta = queries::get_post_meta_map(&state.db, post.id)
        .await
        .unwrap_or_default();
    let woo = WooOrderData::from_post_and_meta(post.id, &post.post_status, &meta);

    let items = queries::get_order_items(&state.db, post.id)
        .await
        .unwrap_or_default();

    let mut line_items = Vec::new();
    for item in &items {
        let item_meta = queries::get_order_item_meta(&state.db, item.order_item_id)
            .await
            .unwrap_or_default();
        line_items.push(order_item_to_json(item, &item_meta));
    }

    let mut order_json = order_to_json(&woo, &post);
    order_json
        .as_object_mut()
        .unwrap()
        .insert("line_items".into(), serde_json::json!(line_items));

    Ok(Json(order_json))
}

/// GET /wp-json/wc/v3/system_status
async fn system_status(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let blogname = state
        .options
        .get_option_or("blogname", "RustPress")
        .await
        .unwrap_or_else(|_| "RustPress".into());

    Json(serde_json::json!({
        "environment": {
            "home_url": state.site_url,
            "site_url": state.site_url,
            "version": "9.0.0",
            "wp_version": "6.9",
            "server_info": "RustPress/Axum",
        },
        "theme": {
            "name": blogname,
        },
        "settings": {
            "currency": "USD",
            "currency_position": "left",
        }
    }))
}

/// Convert WooProductData to WC API v3 JSON format.
fn product_to_json(woo: &WooProductData) -> serde_json::Value {
    serde_json::json!({
        "id": woo.post_id,
        "name": woo.name,
        "slug": woo.slug,
        "type": if woo.product_type.is_empty() { "simple" } else { &woo.product_type },
        "status": "publish",
        "description": woo.description,
        "short_description": woo.short_description,
        "sku": woo.sku,
        "price": format!("{:.2}", woo.price),
        "regular_price": format!("{:.2}", woo.regular_price),
        "sale_price": woo.sale_price.map(|p| format!("{p:.2}")).unwrap_or_default(),
        "stock_quantity": woo.stock_quantity,
        "stock_status": woo.stock_status,
        "manage_stock": woo.manage_stock,
        "weight": woo.weight.map(|w| format!("{w}")).unwrap_or_default(),
        "dimensions": {
            "length": woo.length.map(|v| format!("{v}")).unwrap_or_default(),
            "width": woo.width.map(|v| format!("{v}")).unwrap_or_default(),
            "height": woo.height.map(|v| format!("{v}")).unwrap_or_default(),
        },
        "virtual": woo.is_virtual,
        "downloadable": woo.is_downloadable,
        "tax_status": woo.tax_status,
        "tax_class": woo.tax_class,
        "backorders": woo.backorders,
        "images": woo.image_gallery.iter().map(|id| {
            serde_json::json!({"id": id})
        }).collect::<Vec<_>>(),
    })
}

/// Convert WooOrderData to WC API v3 JSON format.
fn order_to_json(woo: &WooOrderData, post: &wp_posts::Model) -> serde_json::Value {
    let status = woo.status.strip_prefix("wc-").unwrap_or(&woo.status);

    serde_json::json!({
        "id": woo.post_id,
        "status": status,
        "currency": woo.currency,
        "date_created": post.post_date.format("%Y-%m-%dT%H:%M:%S").to_string(),
        "total": format!("{:.2}", woo.total),
        "total_tax": format!("{:.2}", woo.tax_total),
        "shipping_total": format!("{:.2}", woo.shipping_total),
        "discount_total": format!("{:.2}", woo.discount_total),
        "payment_method": woo.payment_method,
        "payment_method_title": woo.payment_method_title,
        "customer_id": woo.customer_id.unwrap_or(0),
        "billing": {
            "first_name": woo.billing.first_name,
            "last_name": woo.billing.last_name,
            "company": woo.billing.company,
            "address_1": woo.billing.address_1,
            "address_2": woo.billing.address_2,
            "city": woo.billing.city,
            "state": woo.billing.state,
            "postcode": woo.billing.postcode,
            "country": woo.billing.country,
            "email": woo.billing.email,
            "phone": woo.billing.phone,
        },
        "shipping": {
            "first_name": woo.shipping.first_name,
            "last_name": woo.shipping.last_name,
            "company": woo.shipping.company,
            "address_1": woo.shipping.address_1,
            "address_2": woo.shipping.address_2,
            "city": woo.shipping.city,
            "state": woo.shipping.state,
            "postcode": woo.shipping.postcode,
            "country": woo.shipping.country,
        },
    })
}

/// Convert a WooCommerce order item to JSON.
fn order_item_to_json(
    item: &rustpress_db::entities::wc_order_items::Model,
    meta: &HashMap<String, String>,
) -> serde_json::Value {
    serde_json::json!({
        "id": item.order_item_id,
        "name": item.order_item_name,
        "product_id": meta.get("_product_id").and_then(|v| v.parse::<u64>().ok()).unwrap_or(0),
        "variation_id": meta.get("_variation_id").and_then(|v| v.parse::<u64>().ok()).unwrap_or(0),
        "quantity": meta.get("_qty").and_then(|v| v.parse::<i64>().ok()).unwrap_or(0),
        "subtotal": meta.get("_line_subtotal").unwrap_or(&"0".to_string()).clone(),
        "total": meta.get("_line_total").unwrap_or(&"0".to_string()).clone(),
        "total_tax": meta.get("_line_tax").unwrap_or(&"0".to_string()).clone(),
        "sku": meta.get("_sku").unwrap_or(&String::new()).clone(),
        "price": meta.get("_line_total").unwrap_or(&"0".to_string()).clone(),
    })
}
