use axum::Router;
use std::sync::Arc;
use tower_http::trace::TraceLayer;

mod auth;
mod frontend;
mod health;
mod posts;
mod users;
pub mod wp_admin;
pub mod xmlrpc;

use crate::state::AppState;

pub fn create_router(state: Arc<AppState>) -> Router {
    // Admin dashboard HTML routes (must be before frontend catch-all)
    let admin_html = wp_admin::routes(state.clone());

    // XML-RPC endpoint (must be before frontend catch-all)
    let xmlrpc_router = Router::new()
        .merge(xmlrpc::routes())
        .with_state(state.clone());

    // Core router with API, health, auth
    let api_router = Router::new()
        .merge(health::routes())
        .merge(posts::routes())
        .merge(users::routes())
        .merge(auth::routes())
        .with_state(state.clone());

    // Frontend routes (includes /{slug} catch-all, must come last)
    let frontend_router = Router::new()
        .merge(frontend::routes())
        .with_state(state);

    api_router
        .merge(admin_html)
        .merge(xmlrpc_router)
        .merge(frontend_router)
        .layer(TraceLayer::new_for_http())
}
