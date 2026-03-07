use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Json},
    routing::{get, post},
    Router,
};
use serde::Deserialize;
use std::sync::Arc;

use crate::state::AppState;

#[derive(Deserialize)]
pub struct AddToCartRequest {
    pub product_id: u64,
    pub quantity: Option<u32>,
}

pub fn routes(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/shop", get(shop_page))
        .route("/shop/product/{id}", get(product_page))
        .route("/cart", get(cart_page))
        .route("/api/cart/add", post(cart_add))
        .route("/api/cart/remove/{product_id}", post(cart_remove))
        .route("/api/cart", get(cart_get))
        .route("/api/products", get(products_list))
        .route("/api/products/{id}", get(product_detail))
        .with_state(state)
}

async fn shop_page(State(state): State<Arc<AppState>>) -> Html<String> {
    let catalog = state.product_catalog.read().await;
    let products = catalog.list_products();
    let mut html = String::from("<h1>Shop</h1><div class=\"products\">");
    for product in products {
        html.push_str(&format!(
            r#"<div class="product"><h2><a href="/shop/product/{}">{}</a></h2><p class="price">${:.2}</p><p>{}</p></div>"#,
            product.id, product.name, product.price, product.short_description
        ));
    }
    html.push_str("</div>");
    Html(html)
}

async fn product_page(
    State(state): State<Arc<AppState>>,
    Path(id): Path<u64>,
) -> Result<Html<String>, StatusCode> {
    let catalog = state.product_catalog.read().await;
    let product = catalog.get_product(id).ok_or(StatusCode::NOT_FOUND)?;
    let html = format!(
        r#"<div class="single-product"><h1>{}</h1><p class="price">${:.2}</p><div class="description">{}</div><button onclick="fetch('/api/cart/add', {{method:'POST',headers:{{'Content-Type':'application/json'}},body:JSON.stringify({{product_id:{},quantity:1}})}})">Add to Cart</button></div>"#,
        product.name, product.price, product.description, product.id
    );
    Ok(Html(html))
}

async fn cart_page(State(_state): State<Arc<AppState>>) -> Html<String> {
    Html("<h1>Shopping Cart</h1><div id=\"cart-items\"></div><script>fetch('/api/cart').then(r=>r.json()).then(d=>document.getElementById('cart-items').innerHTML=JSON.stringify(d))</script>".to_string())
}

async fn cart_add(
    State(state): State<Arc<AppState>>,
    Json(req): Json<AddToCartRequest>,
) -> impl IntoResponse {
    let quantity = req.quantity.unwrap_or(1);
    let catalog = state.product_catalog.read().await;
    if let Some(product) = catalog.get_product(req.product_id) {
        let session_id = "default_session";
        let mut cart_mgr = state.cart_manager.write().await;
        let cart = cart_mgr.get_or_create_cart(session_id);
        cart.add_item(rustpress_commerce::CartItem {
            product_id: req.product_id,
            name: product.name.clone(),
            price: product.price,
            quantity,
            variation_id: None,
        });
        (StatusCode::OK, Json(serde_json::json!({"status": "added", "product_id": req.product_id, "quantity": quantity})))
    } else {
        (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Product not found"})))
    }
}

async fn cart_remove(
    State(state): State<Arc<AppState>>,
    Path(product_id): Path<u64>,
) -> impl IntoResponse {
    let session_id = "default_session";
    let mut cart_mgr = state.cart_manager.write().await;
    if let Some(cart) = cart_mgr.get_cart_mut(session_id) {
        cart.remove_item(product_id, None);
    }
    Json(serde_json::json!({"status": "removed", "product_id": product_id}))
}

async fn cart_get(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let session_id = "default_session";
    let cart_mgr = state.cart_manager.read().await;
    if let Some(cart) = cart_mgr.get_cart(session_id) {
        Json(serde_json::json!({
            "items": cart.items,
            "total": cart.get_total()
        }))
    } else {
        Json(serde_json::json!({"items": Vec::<()>::new(), "total": 0.0}))
    }
}

async fn products_list(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let catalog = state.product_catalog.read().await;
    let products: Vec<_> = catalog.list_products().into_iter().cloned().collect();
    Json(serde_json::json!(products))
}

async fn product_detail(
    State(state): State<Arc<AppState>>,
    Path(id): Path<u64>,
) -> Result<impl IntoResponse, StatusCode> {
    let catalog = state.product_catalog.read().await;
    let product = catalog.get_product(id).ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(serde_json::json!(product)))
}
