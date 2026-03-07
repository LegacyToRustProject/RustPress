#![recursion_limit = "256"]

pub mod application_passwords;
pub mod autosaves;
pub mod batch;
pub mod block_patterns;
pub mod block_renderer;
pub mod block_types;
pub mod categories;
pub mod comments;
pub mod common;
pub mod discovery;
pub mod global_styles;
pub mod media;
pub mod menus;
pub mod navigation;
pub mod oembed;
pub mod pages;
pub mod post_types;
pub mod posts;
pub mod revisions;
pub mod search;
pub mod settings;
pub mod statuses;
pub mod tags;
pub mod taxonomies;
pub mod templates;
pub mod themes;
pub mod users;
pub mod wp_plugins;
pub mod wp_widgets;

use axum::{
    extract::{FromRequestParts, Request},
    http::{header, request::Parts},
    middleware::Next,
    response::{IntoResponse, Response},
    Router,
};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};

use rustpress_auth::{Capability, JwtManager, Role, SessionManager};
use rustpress_core::hooks::HookRegistry;
use rustpress_db::entities::{wp_usermeta, wp_users};

/// Shared state for WP REST API routes.
#[derive(Clone)]
pub struct ApiState {
    pub db: DatabaseConnection,
    pub hooks: HookRegistry,
    pub jwt: JwtManager,
    pub sessions: SessionManager,
    pub site_url: String,
    pub nonces: std::sync::Arc<rustpress_core::nonce::NonceManager>,
}

/// Authenticated user info injected by the auth middleware into request extensions.
/// Handlers can extract this to perform capability checks.
#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user_id: u64,
    pub login: String,
    pub role: String,
}

impl AuthUser {
    /// Check if this user has a specific WordPress capability.
    pub fn can(&self, capability: &Capability) -> bool {
        Role::from_str(&self.role)
            .map(|r| r.can(capability))
            .unwrap_or(false)
    }

    /// Require a capability or return a WpError::forbidden.
    pub fn require(&self, capability: &Capability) -> Result<(), common::WpError> {
        if self.can(capability) {
            Ok(())
        } else {
            Err(common::WpError::forbidden(
                "Sorry, you are not allowed to do that.",
            ))
        }
    }
}

/// Axum extractor: pull AuthUser from request extensions.
impl<S: Send + Sync> FromRequestParts<S> for AuthUser {
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<AuthUser>()
            .cloned()
            .ok_or_else(|| common::WpError::unauthorized().into_response())
    }
}

/// Middleware: require JWT bearer token or session cookie for REST API write endpoints.
/// On success, injects `AuthUser` into request extensions for capability checks.
async fn require_api_auth(
    axum::extract::State(state): axum::extract::State<ApiState>,
    mut request: Request,
    next: Next,
) -> Response {
    // Try JWT
    if let Some(token) = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
    {
        if let Ok(claims) = state.jwt.validate_token(token) {
            let user_id = claims.sub;
            request.extensions_mut().insert(AuthUser {
                user_id,
                login: claims.login,
                role: claims.role,
            });
            let mut resp = next.run(request).await;
            let nonce = state.nonces.create_nonce("wp_rest", user_id);
            if let Ok(val) = header::HeaderValue::from_str(&nonce) {
                resp.headers_mut().insert("x-wp-nonce", val);
            }
            return resp;
        }
    }

    // Try Application Password (HTTP Basic Auth)
    if let Some(auth_value) = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Basic "))
        .map(|s| s.to_string())
    {
        use base64::Engine;
        if let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(&auth_value) {
            if let Ok(credentials) = std::str::from_utf8(&decoded) {
                if let Some((username, app_password)) = credentials.split_once(':') {
                    if let Some(auth_user) =
                        verify_app_password(&state.db, username, app_password).await
                    {
                        let user_id = auth_user.user_id;
                        request.extensions_mut().insert(auth_user);
                        let mut resp = next.run(request).await;
                        let nonce = state.nonces.create_nonce("wp_rest", user_id);
                        if let Ok(val) = header::HeaderValue::from_str(&nonce) {
                            resp.headers_mut().insert("x-wp-nonce", val);
                        }
                        return resp;
                    }
                }
            }
        }
    }

    // Try session cookie (+ optional X-WP-Nonce for CSRF protection)
    if let Some(sid) = request
        .headers()
        .get(header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(|cookies| {
            cookies.split(';').find_map(|c| {
                c.trim()
                    .strip_prefix("rustpress_session=")
                    .map(|v| v.to_string())
            })
        })
    {
        if let Some(session) = state.sessions.get_session(&sid).await {
            let user_id = session.user_id;

            // If X-WP-Nonce header is present, validate it for CSRF protection.
            // Gutenberg's api-fetch always sends this header; curl / direct API
            // calls omit it and continue to pass via session cookie alone.
            let nonce_header = request
                .headers()
                .get("x-wp-nonce")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string());
            if let Some(nonce_val) = nonce_header {
                if state
                    .nonces
                    .verify_nonce(&nonce_val, "wp_rest", user_id)
                    .is_none()
                {
                    return common::WpError::forbidden("Invalid or expired nonce").into_response();
                }
            }

            request.extensions_mut().insert(AuthUser {
                user_id,
                login: session.login,
                role: session.role,
            });
            let mut resp = next.run(request).await;
            let nonce = state.nonces.create_nonce("wp_rest", user_id);
            if let Ok(val) = header::HeaderValue::from_str(&nonce) {
                resp.headers_mut().insert("x-wp-nonce", val);
            }
            return resp;
        }
    }

    common::WpError::unauthorized().into_response()
}

/// Verify an Application Password for HTTP Basic Auth.
/// Returns `Some(AuthUser)` if the username + app_password are valid.
async fn verify_app_password(
    db: &DatabaseConnection,
    username: &str,
    app_password: &str,
) -> Option<AuthUser> {
    // Find user by login or email
    let user = wp_users::Entity::find()
        .filter(wp_users::Column::UserLogin.eq(username))
        .one(db)
        .await
        .ok()
        .flatten()
        .or(None);

    // Also try by email if not found by login
    let user = if user.is_none() {
        wp_users::Entity::find()
            .filter(wp_users::Column::UserEmail.eq(username))
            .one(db)
            .await
            .ok()
            .flatten()
    } else {
        user
    }?;

    // Normalize: remove spaces from the provided password (WordPress formatted passwords have spaces)
    let normalized = app_password.replace(' ', "");

    // Load application passwords from usermeta
    let meta = wp_usermeta::Entity::find()
        .filter(wp_usermeta::Column::UserId.eq(user.id))
        .filter(wp_usermeta::Column::MetaKey.eq("_application_passwords"))
        .one(db)
        .await
        .ok()
        .flatten()?;

    let passwords: Vec<serde_json::Value> = meta
        .meta_value
        .and_then(|v| serde_json::from_str(&v).ok())
        .unwrap_or_default();

    // Check each stored password (stored as raw 24-char string)
    for entry in &passwords {
        let stored = entry.get("password")?.as_str()?;
        if stored == normalized || stored == app_password {
            // Determine role from usermeta wp_capabilities
            let role = get_user_role(db, user.id).await;
            return Some(AuthUser {
                user_id: user.id,
                login: user.user_login.clone(),
                role,
            });
        }
    }

    None
}

/// Get the primary role of a user from wp_usermeta.
async fn get_user_role(db: &DatabaseConnection, user_id: u64) -> String {
    let meta = wp_usermeta::Entity::find()
        .filter(wp_usermeta::Column::UserId.eq(user_id))
        .filter(wp_usermeta::Column::MetaKey.eq("wp_capabilities"))
        .one(db)
        .await
        .ok()
        .flatten();

    if let Some(m) = meta {
        if let Some(val) = m.meta_value {
            // PHP serialized: a:1:{s:13:"administrator";b:1;}
            if val.contains("administrator") {
                return "administrator".to_string();
            } else if val.contains("editor") {
                return "editor".to_string();
            } else if val.contains("author") {
                return "author".to_string();
            } else if val.contains("contributor") {
                return "contributor".to_string();
            }
        }
    }
    "subscriber".to_string()
}

/// Create the WP REST API compatible router.
/// GET endpoints are public; POST/PUT/DELETE require authentication.
pub fn routes(state: ApiState) -> Router {
    // Public read-only routes (GET)
    let public = Router::new()
        .merge(discovery::routes())
        .merge(oembed::routes())
        .merge(posts::read_routes())
        .merge(pages::read_routes())
        .merge(users::read_routes())
        .merge(categories::read_routes())
        .merge(tags::read_routes())
        .merge(media::read_routes())
        .merge(comments::read_routes())
        .merge(settings::read_routes())
        .merge(taxonomies::routes())
        .merge(post_types::routes())
        .merge(statuses::routes())
        .merge(revisions::routes())
        .merge(search::routes())
        .merge(menus::read_routes())
        .merge(autosaves::read_routes())
        .merge(application_passwords::read_routes())
        .merge(themes::routes())
        .merge(wp_plugins::routes())
        .merge(wp_widgets::sidebar_routes())
        .merge(wp_widgets::widget_routes())
        .merge(block_types::routes())
        .merge(block_renderer::routes())
        .merge(block_patterns::routes())
        .merge(global_styles::routes())
        .merge(navigation::read_routes())
        .merge(templates::read_routes())
        .with_state(state.clone());

    // Protected write routes (POST/PUT/DELETE) — require auth
    let protected = Router::new()
        .merge(posts::write_routes())
        .merge(pages::write_routes())
        .merge(categories::write_routes())
        .merge(tags::write_routes())
        .merge(users::write_routes())
        .merge(comments::write_routes())
        .merge(media::write_routes())
        .merge(settings::write_routes())
        .merge(batch::write_routes())
        .merge(menus::write_routes())
        .merge(autosaves::write_routes())
        .merge(application_passwords::write_routes())
        .merge(navigation::write_routes())
        .merge(templates::write_routes())
        .merge(global_styles::write_routes())
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            require_api_auth,
        ))
        .with_state(state);

    public.merge(protected)
}
