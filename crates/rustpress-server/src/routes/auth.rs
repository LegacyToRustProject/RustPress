use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::post,
    Json, Router,
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use rustpress_auth::PasswordHasher;
use rustpress_db::entities::wp_users;

use crate::middleware::get_user_role;
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
        .route("/api/auth/logout", post(logout))
}

async fn login(
    State(state): State<Arc<AppState>>,
    Json(input): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, (StatusCode, String)> {
    // Constant-time-ish: always return the same error to prevent user enumeration
    let invalid_creds = (StatusCode::UNAUTHORIZED, "Invalid credentials".to_string());

    // Find user by login
    let user = wp_users::Entity::find()
        .filter(wp_users::Column::UserLogin.eq(&input.username))
        .one(&state.db)
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal error".to_string(),
            )
        })?
        .ok_or_else(|| invalid_creds.clone())?;

    // Verify password
    let valid = PasswordHasher::verify(&input.password, &user.user_pass).map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Internal error".to_string(),
        )
    })?;

    if !valid {
        return Err(invalid_creds);
    }

    // Look up the user's actual role from the database (NEVER hardcode)
    let role = get_user_role(user.id, &state.db)
        .await
        .unwrap_or_else(|| "subscriber".to_string());

    // Generate JWT token with the user's real role
    let token = state
        .jwt
        .generate_token(user.id, &user.user_login, &user.user_email, &role)
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal error".to_string(),
            )
        })?;

    Ok(Json(LoginResponse {
        token,
        user_id: user.id,
        user_login: user.user_login,
        display_name: user.display_name,
    }))
}

/// Invalidate the caller's JWT by adding its `jti` to the blacklist.
///
/// The token must be passed in the `Authorization: Bearer <token>` header.
/// After this call, the token is rejected by `require_auth` middleware even
/// if it has not yet expired.
async fn logout(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> (StatusCode, Json<serde_json::Value>) {
    let token = headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));

    let Some(token) = token else {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Missing Authorization header"})),
        );
    };

    match state.jwt.validate_token(token) {
        Ok(claims) => {
            rustpress_auth::blacklist_token(&claims.jti);
            (
                StatusCode::OK,
                Json(serde_json::json!({"message": "Logged out"})),
            )
        }
        Err(_) => (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": "Invalid or expired token"})),
        ),
    }
}
