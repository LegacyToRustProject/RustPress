use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use sea_orm::{EntityTrait, QueryOrder, QuerySelect, PaginatorTrait};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use rustpress_db::entities::wp_users;

use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct UsersQuery {
    pub page: Option<u64>,
    pub per_page: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct UserJson {
    pub id: u64,
    pub login: String,
    pub nicename: String,
    pub email: String,
    pub display_name: String,
    pub registered: String,
}

impl From<wp_users::Model> for UserJson {
    fn from(u: wp_users::Model) -> Self {
        Self {
            id: u.id,
            login: u.user_login,
            nicename: u.user_nicename,
            email: u.user_email,
            display_name: u.display_name,
            registered: u.user_registered.format("%Y-%m-%dT%H:%M:%S").to_string(),
        }
    }
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/users", get(list_users))
        .route("/api/users/{id}", get(get_user))
}

async fn list_users(
    State(state): State<Arc<AppState>>,
    Query(params): Query<UsersQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let page = params.page.unwrap_or(1);
    let per_page = params.per_page.unwrap_or(10);

    let query = wp_users::Entity::find().order_by_asc(wp_users::Column::UserLogin);

    let total = query.clone().count(&state.db).await.map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    let users = query
        .offset((page - 1) * per_page)
        .limit(per_page)
        .all(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let items: Vec<UserJson> = users.into_iter().map(UserJson::from).collect();

    Ok(Json(serde_json::json!({
        "items": items,
        "total": total,
        "page": page,
        "per_page": per_page,
    })))
}

async fn get_user(
    State(state): State<Arc<AppState>>,
    Path(id): Path<u64>,
) -> Result<Json<UserJson>, (StatusCode, String)> {
    let user = wp_users::Entity::find_by_id(id)
        .one(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    match user {
        Some(u) => Ok(Json(UserJson::from(u))),
        None => Err((StatusCode::NOT_FOUND, "User not found".to_string())),
    }
}
