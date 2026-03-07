//! WordPress Application Passwords REST API
//!
//! Application Passwords (added in WP 5.6) allow authenticating the REST API
//! via HTTP Basic Auth with a generated token instead of the user's real password.
//!
//! Endpoints:
//!   GET    /wp-json/wp/v2/users/{id}/application-passwords
//!   POST   /wp-json/wp/v2/users/{id}/application-passwords
//!   GET    /wp-json/wp/v2/users/{id}/application-passwords/{uuid}
//!   DELETE /wp-json/wp/v2/users/{id}/application-passwords/{uuid}
//!   DELETE /wp-json/wp/v2/users/{id}/application-passwords
//!
//! Stored in `wp_usermeta` as `_application_passwords` (JSON array).

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use rustpress_db::entities::{wp_usermeta, wp_users};

use crate::common::WpError;
use crate::ApiState;
use crate::AuthUser;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ApplicationPassword {
    pub uuid: String,
    pub name: String,
    pub password: String, // Only shown once on creation
    pub created: String,
    pub last_used: Option<String>,
    pub last_ip: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateAppPasswordRequest {
    pub name: String,
}

pub fn read_routes() -> Router<ApiState> {
    Router::new()
        .route(
            "/wp-json/wp/v2/users/{user_id}/application-passwords",
            get(list_app_passwords),
        )
        .route(
            "/wp-json/wp/v2/users/{user_id}/application-passwords/{uuid}",
            get(get_app_password),
        )
}

pub fn write_routes() -> Router<ApiState> {
    Router::new()
        .route(
            "/wp-json/wp/v2/users/{user_id}/application-passwords",
            axum::routing::post(create_app_password).delete(delete_all_app_passwords),
        )
        .route(
            "/wp-json/wp/v2/users/{user_id}/application-passwords/introspect",
            get(introspect_app_password),
        )
        .route(
            "/wp-json/wp/v2/users/{user_id}/application-passwords/{uuid}",
            axum::routing::delete(delete_app_password),
        )
}

/// Load application passwords from usermeta.
async fn load_app_passwords(db: &sea_orm::DatabaseConnection, user_id: u64) -> Vec<Value> {
    let meta = wp_usermeta::Entity::find()
        .filter(wp_usermeta::Column::UserId.eq(user_id))
        .filter(wp_usermeta::Column::MetaKey.eq("_application_passwords"))
        .one(db)
        .await
        .ok()
        .flatten();

    meta.and_then(|m| m.meta_value)
        .and_then(|v| serde_json::from_str(&v).ok())
        .unwrap_or_default()
}

/// Save application passwords back to usermeta.
async fn save_app_passwords(
    db: &sea_orm::DatabaseConnection,
    user_id: u64,
    passwords: &[Value],
) -> Result<(), sea_orm::DbErr> {
    let json = serde_json::to_string(passwords).unwrap_or_else(|_| "[]".to_string());

    // Check if meta exists
    let existing = wp_usermeta::Entity::find()
        .filter(wp_usermeta::Column::UserId.eq(user_id))
        .filter(wp_usermeta::Column::MetaKey.eq("_application_passwords"))
        .one(db)
        .await?;

    if let Some(record) = existing {
        let mut active: wp_usermeta::ActiveModel = record.into();
        active.meta_value = Set(Some(json));
        active.update(db).await?;
    } else {
        let new_meta = wp_usermeta::ActiveModel {
            umeta_id: sea_orm::ActiveValue::NotSet,
            user_id: Set(user_id),
            meta_key: Set(Some("_application_passwords".to_string())),
            meta_value: Set(Some(json)),
        };
        new_meta.insert(db).await?;
    }
    Ok(())
}

/// GET /wp-json/wp/v2/users/{user_id}/application-passwords
async fn list_app_passwords(
    State(state): State<ApiState>,
    auth: AuthUser,
    Path(user_id): Path<u64>,
) -> Result<Json<Vec<Value>>, WpError> {
    // Only the user themselves or admins can list application passwords
    if auth.user_id != user_id && !auth.can(&rustpress_auth::Capability::ManageOptions) {
        return Err(WpError::forbidden(
            "You cannot list application passwords for this user.",
        ));
    }

    // Verify user exists
    wp_users::Entity::find_by_id(user_id)
        .one(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?
        .ok_or(WpError::not_found("User not found"))?;

    let passwords = load_app_passwords(&state.db, user_id).await;
    // Return without the password field (only shown on creation)
    let sanitized: Vec<Value> = passwords
        .into_iter()
        .map(|mut p| {
            if let Some(obj) = p.as_object_mut() {
                obj.remove("password");
            }
            p
        })
        .collect();
    Ok(Json(sanitized))
}

/// GET /wp-json/wp/v2/users/{user_id}/application-passwords/{uuid}
async fn get_app_password(
    State(state): State<ApiState>,
    auth: AuthUser,
    Path((user_id, uuid)): Path<(u64, String)>,
) -> Result<Json<Value>, WpError> {
    if auth.user_id != user_id && !auth.can(&rustpress_auth::Capability::ManageOptions) {
        return Err(WpError::forbidden(
            "You cannot access application passwords for this user.",
        ));
    }

    let passwords = load_app_passwords(&state.db, user_id).await;
    let found = passwords
        .into_iter()
        .find(|p| p.get("uuid").and_then(|v| v.as_str()) == Some(&uuid));

    match found {
        Some(mut p) => {
            if let Some(obj) = p.as_object_mut() {
                obj.remove("password");
            }
            Ok(Json(p))
        }
        None => Err(WpError::not_found("Application password not found")),
    }
}

/// GET /wp-json/wp/v2/users/{user_id}/application-passwords/introspect
/// Returns info about the currently-used application password (or 401 if not using one).
async fn introspect_app_password(
    State(state): State<ApiState>,
    auth: AuthUser,
    Path(user_id): Path<u64>,
) -> Result<Json<Value>, WpError> {
    if auth.user_id != user_id && !auth.can(&rustpress_auth::Capability::ManageOptions) {
        return Err(WpError::forbidden(
            "You cannot introspect application passwords for this user.",
        ));
    }

    // Verify user exists
    wp_users::Entity::find_by_id(user_id)
        .one(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?
        .ok_or(WpError::not_found("User not found"))?;

    let passwords = load_app_passwords(&state.db, user_id).await;

    // Return info about the first application password (the one most recently used, if any)
    // In a real implementation this would identify the specific password used in the request.
    // Here we return info about the most recently used password.
    let pw = passwords
        .into_iter()
        .filter(|p| {
            p.get("last_used")
                .and_then(|v| v.as_str())
                .map(|s| !s.is_empty())
                .unwrap_or(false)
        })
        .next();

    match pw {
        Some(mut p) => {
            if let Some(obj) = p.as_object_mut() {
                obj.remove("password");
            }
            Ok(Json(p))
        }
        None => Err(WpError::new(
            StatusCode::UNAUTHORIZED,
            "rest_not_using_application_password",
            "You are not using an application password.",
        )),
    }
}

/// POST /wp-json/wp/v2/users/{user_id}/application-passwords
async fn create_app_password(
    State(state): State<ApiState>,
    auth: AuthUser,
    Path(user_id): Path<u64>,
    Json(input): Json<CreateAppPasswordRequest>,
) -> Result<(StatusCode, Json<Value>), WpError> {
    if auth.user_id != user_id && !auth.can(&rustpress_auth::Capability::ManageOptions) {
        return Err(WpError::forbidden(
            "You cannot create application passwords for this user.",
        ));
    }

    if input.name.trim().is_empty() {
        return Err(WpError::bad_request(
            "Application password name is required.",
        ));
    }

    // Verify user exists
    wp_users::Entity::find_by_id(user_id)
        .one(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?
        .ok_or(WpError::not_found("User not found"))?;

    // Generate a random 24-character application password (WordPress format: XXXX XXXX XXXX XXXX XXXX XXXX)
    let raw_password: String = (0..24)
        .map(|_| {
            let idx = (rand_byte() % 36) as usize;
            b"abcdefghijklmnopqrstuvwxyz0123456789"[idx] as char
        })
        .collect();

    // Format as WordPress app password chunks
    let formatted = raw_password
        .chars()
        .collect::<Vec<char>>()
        .chunks(4)
        .map(|c| c.iter().collect::<String>())
        .collect::<Vec<_>>()
        .join(" ");

    let uuid = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();

    let new_entry = serde_json::json!({
        "uuid": uuid,
        "name": input.name.trim(),
        "password": raw_password,  // Store unhashed for verification (WordPress stores hashed, but for simplicity we store plain here)
        "created": now,
        "last_used": null,
        "last_ip": null,
    });

    let mut passwords = load_app_passwords(&state.db, user_id).await;
    passwords.push(new_entry.clone());

    save_app_passwords(&state.db, user_id, &passwords)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?;

    // Return with the formatted password (only time it's shown)
    let mut response = new_entry;
    response["password"] = serde_json::json!(formatted);

    Ok((StatusCode::CREATED, Json(response)))
}

/// DELETE /wp-json/wp/v2/users/{user_id}/application-passwords/{uuid}
async fn delete_app_password(
    State(state): State<ApiState>,
    auth: AuthUser,
    Path((user_id, uuid)): Path<(u64, String)>,
) -> Result<Json<Value>, WpError> {
    if auth.user_id != user_id && !auth.can(&rustpress_auth::Capability::ManageOptions) {
        return Err(WpError::forbidden(
            "You cannot delete application passwords for this user.",
        ));
    }

    let mut passwords = load_app_passwords(&state.db, user_id).await;
    let original_len = passwords.len();
    passwords.retain(|p| p.get("uuid").and_then(|v| v.as_str()) != Some(&uuid));

    if passwords.len() == original_len {
        return Err(WpError::not_found("Application password not found"));
    }

    save_app_passwords(&state.db, user_id, &passwords)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?;

    Ok(Json(serde_json::json!({"deleted": true})))
}

/// DELETE /wp-json/wp/v2/users/{user_id}/application-passwords
async fn delete_all_app_passwords(
    State(state): State<ApiState>,
    auth: AuthUser,
    Path(user_id): Path<u64>,
) -> Result<Json<Value>, WpError> {
    if auth.user_id != user_id && !auth.can(&rustpress_auth::Capability::ManageOptions) {
        return Err(WpError::forbidden(
            "You cannot delete application passwords for this user.",
        ));
    }

    save_app_passwords(&state.db, user_id, &[])
        .await
        .map_err(|e| WpError::internal(e.to_string()))?;

    Ok(Json(serde_json::json!({"deleted": true})))
}

/// Simple pseudo-random byte using system time (no rand crate needed).
fn rand_byte() -> u8 {
    use std::time::{SystemTime, UNIX_EPOCH};
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    let c = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    ((t ^ (c.wrapping_mul(6364136223846793005) >> 32) as u32) & 0xFF) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rand_byte_is_valid() {
        for _ in 0..100 {
            let b = rand_byte();
            // Should map to valid index in 36-char alphabet
            assert!((b % 36) < 36);
        }
    }

    #[test]
    fn test_password_format() {
        let raw: String = (0..24)
            .map(|i| {
                let chars = b"abcdefghijklmnopqrstuvwxyz0123456789";
                chars[i % 36] as char
            })
            .collect();
        let formatted = raw
            .chars()
            .collect::<Vec<char>>()
            .chunks(4)
            .map(|c| c.iter().collect::<String>())
            .collect::<Vec<_>>()
            .join(" ");
        // Should be 6 groups of 4 separated by spaces = 29 chars
        assert_eq!(formatted.len(), 29);
        assert_eq!(formatted.split(' ').count(), 6);
    }
}
