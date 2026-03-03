use axum::{routing::get, Json, Router};
use serde::Serialize;

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    version: &'static str,
}

pub fn routes() -> Router {
    Router::new()
        .route("/", get(index))
        .route("/health", get(health_check))
}

async fn index() -> &'static str {
    "Hello, RustPress!"
}

async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
    })
}
