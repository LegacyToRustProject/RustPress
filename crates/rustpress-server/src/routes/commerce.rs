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

#[derive(Deserialize)]
pub struct CheckoutFormData {
    pub name: String,
    pub email: String,
    pub address: String,
    pub city: String,
    pub state: String,
    pub zip: String,
    pub country: String,
    pub payment_method: String,
}

pub fn routes(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/shop", get(shop_page))
        .route("/shop/product/{id}", get(product_page))
        .route("/cart", get(cart_page))
        .route("/checkout", get(checkout_page))
        .route("/checkout/thank-you/{order_id}", get(thank_you_page))
        .route("/api/cart/add", post(cart_add))
        .route("/api/cart/remove/{product_id}", post(cart_remove))
        .route("/api/cart", get(cart_get))
        .route("/api/checkout", post(checkout_submit))
        .route("/api/orders", get(orders_list))
        .route("/api/orders/{id}", get(order_detail))
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
        (
            StatusCode::OK,
            Json(
                serde_json::json!({"status": "added", "product_id": req.product_id, "quantity": quantity}),
            ),
        )
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Product not found"})),
        )
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

async fn checkout_page(State(state): State<Arc<AppState>>) -> Html<String> {
    let session_id = "default_session";
    let cart_mgr = state.cart_manager.read().await;
    let (items_html, total) = if let Some(cart) = cart_mgr.get_cart(session_id) {
        let mut html = String::new();
        for item in &cart.items {
            let line_total = item.price * item.quantity as f64;
            html.push_str(&format!(
                r#"<tr><td>{}</td><td>{}</td><td>${:.2}</td><td>${:.2}</td></tr>"#,
                item.name, item.quantity, item.price, line_total
            ));
        }
        (html, cart.get_total())
    } else {
        (String::new(), 0.0)
    };

    let page = format!(
        r#"<!DOCTYPE html>
<html>
<head><title>Checkout - RustPress</title>
<style>
body {{ font-family: sans-serif; max-width: 900px; margin: 0 auto; padding: 20px; }}
h1 {{ border-bottom: 2px solid #333; padding-bottom: 10px; }}
table {{ width: 100%; border-collapse: collapse; margin-bottom: 20px; }}
th, td {{ padding: 8px 12px; border: 1px solid #ddd; text-align: left; }}
th {{ background: #f5f5f5; }}
.checkout-form {{ display: grid; grid-template-columns: 1fr 1fr; gap: 12px; }}
.checkout-form label {{ display: block; font-weight: bold; margin-bottom: 4px; }}
.checkout-form input, .checkout-form select {{ width: 100%; padding: 8px; border: 1px solid #ccc; border-radius: 4px; box-sizing: border-box; }}
.full-width {{ grid-column: 1 / -1; }}
.total-row {{ font-weight: bold; font-size: 1.2em; }}
button[type="submit"] {{ padding: 12px 24px; background: #0073aa; color: white; border: none; border-radius: 4px; font-size: 1.1em; cursor: pointer; margin-top: 16px; }}
button[type="submit"]:hover {{ background: #005a87; }}
.empty-cart {{ padding: 40px; text-align: center; color: #666; }}
</style>
</head>
<body>
<h1>Checkout</h1>
{cart_section}
</body>
</html>"#,
        cart_section = if total == 0.0 {
            r#"<div class="empty-cart"><p>Your cart is empty.</p><p><a href="/shop">Continue Shopping</a></p></div>"#.to_string()
        } else {
            format!(
                r#"<h2>Order Summary</h2>
<table>
<thead><tr><th>Product</th><th>Qty</th><th>Price</th><th>Total</th></tr></thead>
<tbody>{items_html}</tbody>
<tfoot><tr class="total-row"><td colspan="3">Total</td><td>${total:.2}</td></tr></tfoot>
</table>

<h2>Billing Details</h2>
<form id="checkout-form" class="checkout-form">
  <div><label for="name">Full Name</label><input type="text" id="name" name="name" required></div>
  <div><label for="email">Email</label><input type="email" id="email" name="email" required></div>
  <div class="full-width"><label for="address">Address</label><input type="text" id="address" name="address" required></div>
  <div><label for="city">City</label><input type="text" id="city" name="city" required></div>
  <div><label for="state">State / Province</label><input type="text" id="state" name="state" required></div>
  <div><label for="zip">ZIP / Postal Code</label><input type="text" id="zip" name="zip" required></div>
  <div><label for="country">Country</label><input type="text" id="country" name="country" required value="US"></div>
  <div class="full-width">
    <label for="payment_method">Payment Method</label>
    <select id="payment_method" name="payment_method" required>
      <option value="credit_card">Credit Card</option>
      <option value="paypal">PayPal</option>
      <option value="bank_transfer">Bank Transfer</option>
    </select>
  </div>
  <div class="full-width">
    <button type="submit">Place Order</button>
  </div>
</form>
<script>
document.getElementById('checkout-form').addEventListener('submit', function(e) {{
  e.preventDefault();
  const form = e.target;
  const data = {{
    name: form.name.value,
    email: form.email.value,
    address: form.address.value,
    city: form.city.value,
    state: form.state.value,
    zip: form.zip.value,
    country: form.country.value,
    payment_method: form.payment_method.value
  }};
  fetch('/api/checkout', {{
    method: 'POST',
    headers: {{'Content-Type': 'application/json'}},
    body: JSON.stringify(data)
  }})
  .then(r => r.json())
  .then(d => {{
    if (d.order_id) {{
      window.location.href = '/checkout/thank-you/' + d.order_id;
    }} else {{
      alert(d.error || 'Checkout failed');
    }}
  }})
  .catch(err => alert('Error: ' + err));
}});
</script>"#
            )
        }
    );
    Html(page)
}

async fn checkout_submit(
    State(state): State<Arc<AppState>>,
    Json(form): Json<CheckoutFormData>,
) -> impl IntoResponse {
    let session_id = "default_session";

    // Read cart items
    let cart_mgr = state.cart_manager.read().await;
    let cart = match cart_mgr.get_cart(session_id) {
        Some(c) if !c.items.is_empty() => c.clone(),
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "Cart is empty"})),
            );
        }
    };
    drop(cart_mgr);

    // Build order items from cart
    let order_items: Vec<rustpress_commerce::OrderItem> = cart
        .items
        .iter()
        .map(|item| rustpress_commerce::OrderItem {
            product_id: item.product_id,
            name: item.name.clone(),
            quantity: item.quantity,
            price: item.price,
            total: item.price * item.quantity as f64,
        })
        .collect();

    // Parse name into first/last
    let name_parts: Vec<&str> = form.name.splitn(2, ' ').collect();
    let first_name = name_parts.first().unwrap_or(&"").to_string();
    let last_name = if name_parts.len() > 1 {
        name_parts[1].to_string()
    } else {
        String::new()
    };

    let billing = rustpress_commerce::Address {
        first_name: first_name.clone(),
        last_name: last_name.clone(),
        company: String::new(),
        address_1: form.address.clone(),
        address_2: String::new(),
        city: form.city.clone(),
        state: form.state.clone(),
        postcode: form.zip.clone(),
        country: form.country.clone(),
        email: form.email.clone(),
        phone: String::new(),
    };

    // Create order
    let mut order_mgr = state.order_manager.write().await;
    let order_id = order_mgr.create_order(
        order_items,
        billing.clone(),
        billing, // use billing as shipping for simplicity
        &form.payment_method,
        0.0, // shipping_total
        0.0, // tax_total
        0.0, // discount_total
        None,
        "",
    );

    let order = order_mgr.get_order(order_id).unwrap();
    let total = order.total;
    let status = order.status.to_string();

    // Mark as processing (payment stub succeeds)
    order_mgr.update_status(order_id, rustpress_commerce::OrderStatus::Processing);
    drop(order_mgr);

    // Clear the cart
    let mut cart_mgr = state.cart_manager.write().await;
    if let Some(cart) = cart_mgr.get_cart_mut(session_id) {
        cart.clear();
    }

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "order_id": order_id,
            "status": status,
            "total": total
        })),
    )
}

async fn thank_you_page(
    State(state): State<Arc<AppState>>,
    Path(order_id): Path<u64>,
) -> Result<Html<String>, StatusCode> {
    let order_mgr = state.order_manager.read().await;
    let order = order_mgr.get_order(order_id).ok_or(StatusCode::NOT_FOUND)?;

    let mut items_html = String::new();
    for item in &order.items {
        items_html.push_str(&format!(
            r#"<tr><td>{}</td><td>{}</td><td>${:.2}</td><td>${:.2}</td></tr>"#,
            item.name, item.quantity, item.price, item.total
        ));
    }

    let billing_name = format!(
        "{} {}",
        order.billing_address.first_name, order.billing_address.last_name
    );

    let html = format!(
        r#"<!DOCTYPE html>
<html>
<head><title>Order Confirmed - RustPress</title>
<style>
body {{ font-family: sans-serif; max-width: 800px; margin: 0 auto; padding: 20px; }}
h1 {{ color: #2e7d32; }}
table {{ width: 100%; border-collapse: collapse; margin: 20px 0; }}
th, td {{ padding: 8px 12px; border: 1px solid #ddd; text-align: left; }}
th {{ background: #f5f5f5; }}
.order-meta {{ background: #f9f9f9; padding: 16px; border-radius: 8px; margin: 16px 0; }}
.order-meta p {{ margin: 4px 0; }}
.total-row {{ font-weight: bold; }}
</style>
</head>
<body>
<h1>Thank You for Your Order!</h1>
<div class="order-meta">
  <p><strong>Order Number:</strong> {order_number}</p>
  <p><strong>Order ID:</strong> {order_id}</p>
  <p><strong>Status:</strong> {status}</p>
  <p><strong>Payment Method:</strong> {payment_method}</p>
  <p><strong>Date:</strong> {date}</p>
</div>

<h2>Order Details</h2>
<table>
<thead><tr><th>Product</th><th>Qty</th><th>Price</th><th>Total</th></tr></thead>
<tbody>{items_html}</tbody>
<tfoot><tr class="total-row"><td colspan="3">Order Total</td><td>${total:.2}</td></tr></tfoot>
</table>

<h2>Billing Address</h2>
<p>{billing_name}<br>{billing_address}<br>{billing_city}, {billing_state} {billing_zip}<br>{billing_country}<br>{billing_email}</p>

<p><a href="/shop">Continue Shopping</a></p>
</body>
</html>"#,
        order_number = order.order_number,
        order_id = order.id,
        status = order.status,
        payment_method = order.payment_method,
        date = order.created_at.format("%B %e, %Y %H:%M"),
        items_html = items_html,
        total = order.total,
        billing_name = billing_name,
        billing_address = order.billing_address.address_1,
        billing_city = order.billing_address.city,
        billing_state = order.billing_address.state,
        billing_zip = order.billing_address.postcode,
        billing_country = order.billing_address.country,
        billing_email = order.billing_address.email,
    );

    Ok(Html(html))
}

async fn orders_list(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let order_mgr = state.order_manager.read().await;
    let orders = order_mgr.list_orders(None);
    let orders_json: Vec<serde_json::Value> = orders
        .iter()
        .map(|o| {
            serde_json::json!({
                "id": o.id,
                "order_number": o.order_number,
                "status": o.status,
                "total": o.total,
                "payment_method": o.payment_method,
                "created_at": o.created_at.to_rfc3339(),
                "item_count": o.items.len(),
            })
        })
        .collect();
    Json(serde_json::json!(orders_json))
}

async fn order_detail(
    State(state): State<Arc<AppState>>,
    Path(id): Path<u64>,
) -> Result<impl IntoResponse, StatusCode> {
    let order_mgr = state.order_manager.read().await;
    let order = order_mgr.get_order(id).ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(serde_json::json!(order)))
}
