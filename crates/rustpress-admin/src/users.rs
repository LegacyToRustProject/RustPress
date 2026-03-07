use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect,
};
use serde::{Deserialize, Serialize};

use rustpress_auth::PasswordHasher;
use rustpress_db::entities::wp_users;

use crate::AdminState;

#[derive(Debug, Deserialize)]
pub struct ListUsersQuery {
    pub page: Option<u64>,
    pub per_page: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    pub login: String,
    pub email: String,
    pub password: String,
    pub display_name: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct UserResponse {
    pub id: u64,
    pub login: String,
    pub email: String,
    pub nicename: String,
    pub display_name: String,
    pub url: String,
    pub registered: String,
}

impl From<wp_users::Model> for UserResponse {
    fn from(u: wp_users::Model) -> Self {
        Self {
            id: u.id,
            login: u.user_login,
            email: u.user_email,
            nicename: u.user_nicename,
            display_name: u.display_name,
            url: u.user_url,
            registered: u.user_registered.format("%Y-%m-%dT%H:%M:%S").to_string(),
        }
    }
}

pub fn routes() -> Router<AdminState> {
    Router::new()
        .route("/admin/users", get(list_users).post(create_user))
        .route("/admin/users/{id}", get(get_user))
}

async fn list_users(
    State(state): State<AdminState>,
    Query(params): Query<ListUsersQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let page = params.page.unwrap_or(1);
    let per_page = params.per_page.unwrap_or(20);

    let query = wp_users::Entity::find().order_by_asc(wp_users::Column::UserLogin);

    let total = query
        .clone()
        .count(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let users = query
        .offset((page - 1) * per_page)
        .limit(per_page)
        .all(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let items: Vec<UserResponse> = users.into_iter().map(UserResponse::from).collect();

    Ok(Json(serde_json::json!({
        "items": items,
        "total": total,
        "page": page,
        "per_page": per_page,
    })))
}

async fn get_user(
    State(state): State<AdminState>,
    Path(id): Path<u64>,
) -> Result<Json<UserResponse>, (StatusCode, String)> {
    let user = wp_users::Entity::find_by_id(id)
        .one(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    match user {
        Some(u) => Ok(Json(UserResponse::from(u))),
        None => Err((StatusCode::NOT_FOUND, "User not found".to_string())),
    }
}

async fn create_user(
    State(state): State<AdminState>,
    Json(input): Json<CreateUserRequest>,
) -> Result<(StatusCode, Json<UserResponse>), (StatusCode, String)> {
    // Check if user already exists
    let existing = wp_users::Entity::find()
        .filter(wp_users::Column::UserLogin.eq(&input.login))
        .one(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if existing.is_some() {
        return Err((StatusCode::CONFLICT, "User already exists".to_string()));
    }

    let password_hash = PasswordHasher::hash_argon2(&input.password)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let now = chrono::Utc::now().naive_utc();
    let nicename = input.login.to_lowercase().replace(' ', "-");
    let display_name = input.display_name.unwrap_or_else(|| input.login.clone());

    let new_user = wp_users::ActiveModel {
        user_login: Set(input.login),
        user_pass: Set(password_hash),
        user_nicename: Set(nicename),
        user_email: Set(input.email),
        user_url: Set(String::new()),
        user_registered: Set(now),
        user_activation_key: Set(String::new()),
        user_status: Set(0),
        display_name: Set(display_name),
        ..Default::default()
    };

    let result = new_user
        .insert(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok((StatusCode::CREATED, Json(UserResponse::from(result))))
}
