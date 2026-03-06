use axum::{
    extract::State,
    http::StatusCode,
    routing::post,
    Json, Router,
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use rustpress_auth::PasswordHasher;
use rustpress_db::entities::wp_users;

use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub user_id: u64,
    pub user_login: String,
    pub display_name: String,
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/auth/login", post(login))
}

async fn login(
    State(state): State<Arc<AppState>>,
    Json(input): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, (StatusCode, String)> {
    // Find user by login
    let user = wp_users::Entity::find()
        .filter(wp_users::Column::UserLogin.eq(&input.username))
        .one(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::UNAUTHORIZED, "Invalid credentials".to_string()))?;

    // Verify password
    let valid = PasswordHasher::verify(&input.password, &user.user_pass)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if !valid {
        return Err((StatusCode::UNAUTHORIZED, "Invalid credentials".to_string()));
    }

    // Generate JWT token
    let token = state
        .jwt
        .generate_token(user.id, &user.user_login, &user.user_email, "administrator")
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(LoginResponse {
        token,
        user_id: user.id,
        user_login: user.user_login,
        display_name: user.display_name,
    }))
}
