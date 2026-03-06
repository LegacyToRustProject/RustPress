pub mod comments;
pub mod media;
pub mod plugins;
pub mod posts;
pub mod taxonomies;
pub mod users;

use axum::Router;
use sea_orm::DatabaseConnection;

use rustpress_auth::JwtManager;
use rustpress_core::hooks::HookRegistry;
use rustpress_plugins::PluginRegistry;

/// Shared state for admin routes.
#[derive(Clone)]
pub struct AdminState {
    pub db: DatabaseConnection,
    pub hooks: HookRegistry,
    pub jwt: JwtManager,
    pub plugin_registry: PluginRegistry,
}

/// Create the admin API router with all CRUD endpoints.
pub fn routes(state: AdminState) -> Router {
    Router::new()
        .merge(posts::routes())
        .merge(users::routes())
        .merge(media::routes())
        .merge(comments::routes())
        .merge(taxonomies::routes())
        .merge(plugins::routes())
        .with_state(state)
}
