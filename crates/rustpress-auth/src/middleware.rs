use axum::{
    extract::Request,
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

use crate::jwt::JwtManager;

/// Extract the bearer token from Authorization header.
fn extract_bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
}

/// Authentication middleware layer.
/// Validates JWT tokens from the Authorization header.
#[derive(Clone)]
pub struct AuthLayer {
    #[allow(dead_code)]
    jwt_manager: JwtManager,
}

impl AuthLayer {
    pub fn new(jwt_manager: JwtManager) -> Self {
        Self { jwt_manager }
    }
}

/// Middleware function that requires authentication.
/// Adds Claims to request extensions if authentication succeeds.
pub async fn require_auth(headers: HeaderMap, mut request: Request, next: Next) -> Response {
    let jwt_manager = request.extensions().get::<JwtManager>().cloned();

    let jwt_manager = match jwt_manager {
        Some(m) => m,
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Auth not configured"})),
            )
                .into_response();
        }
    };

    let token = match extract_bearer_token(&headers) {
        Some(t) => t,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Missing authorization token"})),
            )
                .into_response();
        }
    };

    match jwt_manager.validate_token(token) {
        Ok(claims) => {
            request.extensions_mut().insert(claims);
            next.run(request).await
        }
        Err(_) => (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "Invalid or expired token"})),
        )
            .into_response(),
    }
}

/// Middleware function that optionally extracts authentication.
/// Adds Claims to request extensions if a valid token is present, but doesn't require it.
pub async fn optional_auth(headers: HeaderMap, mut request: Request, next: Next) -> Response {
    if let Some(jwt_manager) = request.extensions().get::<JwtManager>().cloned() {
        if let Some(token) = extract_bearer_token(&headers) {
            if let Ok(claims) = jwt_manager.validate_token(token) {
                request.extensions_mut().insert(claims);
            }
        }
    }

    next.run(request).await
}
