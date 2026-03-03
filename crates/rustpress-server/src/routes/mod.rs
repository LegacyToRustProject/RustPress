use axum::Router;
use tower_http::trace::TraceLayer;

mod health;

pub fn create_router() -> Router {
    Router::new()
        .merge(health::routes())
        .layer(TraceLayer::new_for_http())
}
