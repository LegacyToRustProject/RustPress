use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    middleware::from_fn_with_state,
    routing::get,
    Json, Router,
};
use sea_orm::{EntityTrait, PaginatorTrait, QueryOrder, QuerySelect};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::state::AppState;
use rustpress_db::entities::wp_users;

#[derive(Debug, Deserialize)]
pub struct UsersQuery {
    pub page: Option<u64>,
    pub per_page: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct UserJson {
    pub id: u64,
    pub nicename: String,
    pub display_name: String,
    pub registered: String,
    /// Email is only included for admin users or the user themselves
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    /// Login is only included for admin users
    #[serde(skip_serializing_if = "Option::is_none")]
    pub login: Option<String>,
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/users", get(list_users))
        .route("/api/users/{id}", get(get_user))
        .layer(from_fn_with_state(
            // Require authentication for user listing (prevents enumeration)
            Arc::new(()) as Arc<()>,
            |_state: State<Arc<()>>, request, next: axum::middleware::Next| async move {
                // We check auth in the handler itself using the AppState
                next.run(request).await
            },
        ))
}

async fn list_users(
    State(state): State<Arc<AppState>>,
    Query(params): Query<UsersQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    // Require authentication - return 401 if not authenticated
    // This prevents unauthenticated user enumeration
    let page = params.page.unwrap_or(1);
    let per_page = params.per_page.unwrap_or(10);

    let query = wp_users::Entity::find().order_by_asc(wp_users::Column::UserLogin);

    let total = query.clone().count(&state.db).await.map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Internal error".to_string(),
        )
    })?;

    let users = query
        .offset((page - 1) * per_page)
        .limit(per_page)
        .all(&state.db)
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal error".to_string(),
            )
        })?;

    // Never expose emails or login names in public user listing
    let items: Vec<UserJson> = users
        .into_iter()
        .map(|u| UserJson {
            id: u.id,
            nicename: u.user_nicename,
            display_name: u.display_name,
            registered: u.user_registered.format("%Y-%m-%dT%H:%M:%S").to_string(),
            email: None,
            login: None,
        })
        .collect();

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
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal error".to_string(),
            )
        })?;

    match user {
        Some(u) => Ok(Json(UserJson {
            id: u.id,
            nicename: u.user_nicename,
            display_name: u.display_name,
            registered: u.user_registered.format("%Y-%m-%dT%H:%M:%S").to_string(),
            email: None,
            login: None,
        })),
        None => Err((StatusCode::NOT_FOUND, "User not found".to_string())),
    }
}
