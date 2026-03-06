use axum::{
    extract::{Request, State},
    http::{header, HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Redirect, Response},
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use std::sync::Arc;

use rustpress_auth::roles::{Capability, Role};
use rustpress_auth::session::Session;
use rustpress_db::entities::wp_usermeta;

use crate::state::AppState;

/// Extract session ID from the `rustpress_session` cookie.
fn extract_session_cookie(request: &Request) -> Option<String> {
    request
        .headers()
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .find_map(|cookie| {
            let cookie = cookie.trim();
            cookie
                .strip_prefix("rustpress_session=")
                .map(|v| v.to_string())
        })
}

/// Extract Bearer token from the Authorization header.
fn extract_bearer_token(request: &Request) -> Option<String> {
    request
        .headers()
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
        .map(|v| v.to_string())
}

/// Middleware: require a valid admin session cookie. Redirects to login if invalid.
pub async fn require_admin_session(
    State(state): State<Arc<AppState>>,
    mut request: Request,
    next: Next,
) -> Response {
    if let Some(sid) = extract_session_cookie(&request) {
        if let Some(session) = state.sessions.get_session(&sid).await {
            request.extensions_mut().insert(session);
            return next.run(request).await;
        }
    }

    Redirect::to("/wp-login.php").into_response()
}

/// Middleware: accept either JWT bearer token OR session cookie for API auth.
/// Returns 401 if neither is valid.
pub async fn require_auth_jwt_or_session(
    State(state): State<Arc<AppState>>,
    mut request: Request,
    next: Next,
) -> Response {
    // Try JWT first
    if let Some(token) = extract_bearer_token(&request) {
        if let Ok(claims) = state.jwt.validate_token(&token) {
            request.extensions_mut().insert(claims);
            return next.run(request).await;
        }
    }

    // Try session cookie
    if let Some(sid) = extract_session_cookie(&request) {
        if let Some(session) = state.sessions.get_session(&sid).await {
            request.extensions_mut().insert(session);
            return next.run(request).await;
        }
    }

    (StatusCode::UNAUTHORIZED, "Authentication required").into_response()
}

/// Look up the WordPress role string for a user from wp_usermeta.
///
/// WordPress stores serialized PHP in `wp_capabilities`, e.g.
/// `a:1:{s:13:"administrator";b:1;}`.  We extract the role name from that
/// serialized blob.  If the meta row is missing we fall back to the role
/// stored in the session (which is set at login time).
pub async fn get_user_role(
    user_id: u64,
    db: &sea_orm::DatabaseConnection,
) -> Option<String> {
    // Try the standard WordPress meta key first
    let meta = wp_usermeta::Entity::find()
        .filter(wp_usermeta::Column::UserId.eq(user_id))
        .filter(wp_usermeta::Column::MetaKey.eq("wp_capabilities"))
        .one(db)
        .await
        .ok()
        .flatten();

    if let Some(ref row) = meta {
        if let Some(ref value) = row.meta_value {
            // WordPress serializes this as e.g. a:1:{s:13:"administrator";b:1;}
            // We do a simple extraction: find the quoted role name.
            if let Some(role_str) = extract_role_from_serialized(value) {
                return Some(role_str);
            }
        }
    }

    // Fallback: try a simpler `wp_user_role` custom meta key that RustPress
    // may use for newly-created users.
    let fallback = wp_usermeta::Entity::find()
        .filter(wp_usermeta::Column::UserId.eq(user_id))
        .filter(wp_usermeta::Column::MetaKey.eq("wp_user_role"))
        .one(db)
        .await
        .ok()
        .flatten();

    if let Some(ref row) = fallback {
        if let Some(ref value) = row.meta_value {
            let trimmed = value.trim().to_lowercase();
            if Role::from_str(&trimmed).is_some() {
                return Some(trimmed);
            }
        }
    }

    None
}

/// Extract a role name from a PHP-serialized wp_capabilities value.
///
/// Typical shapes:
///   `a:1:{s:13:"administrator";b:1;}`
///   `a:1:{s:6:"editor";b:1;}`
///
/// We look for the first `s:NN:"<role>";` pattern where `<role>` is one of
/// the known WordPress roles.
fn extract_role_from_serialized(serialized: &str) -> Option<String> {
    // Quick regex-free approach: split on `"` and check known role names.
    let known = [
        "administrator",
        "editor",
        "author",
        "contributor",
        "subscriber",
    ];
    for part in serialized.split('"') {
        let lower = part.trim().to_lowercase();
        if known.contains(&lower.as_str()) {
            return Some(lower);
        }
    }
    None
}

/// Resolve the effective role for the current request.
///
/// 1. Check wp_usermeta in the database (authoritative).
/// 2. Fall back to the role cached in the session.
///
/// Returns the `Role` enum value if it could be resolved.
pub async fn resolve_session_role(
    session: &Session,
    db: &sea_orm::DatabaseConnection,
) -> Option<Role> {
    // Try the database first
    if let Some(role_str) = get_user_role(session.user_id, db).await {
        if let Some(role) = Role::from_str(&role_str) {
            return Some(role);
        }
    }
    // Fall back to session
    Role::from_str(&session.role)
}

/// Middleware factory: require that the logged-in user possesses a specific
/// WordPress capability.  Returns 403 Forbidden if the capability is missing.
///
/// Usage in route definitions:
/// ```ignore
/// .layer(axum::middleware::from_fn_with_state(
///     state.clone(),
///     |s, r, n| require_capability(s, r, n, Capability::ManageOptions),
/// ))
/// ```
pub async fn require_capability(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
    capability: Capability,
) -> Response {
    // The session must already be inserted by `require_admin_session`.
    let session = match request.extensions().get::<Session>() {
        Some(s) => s.clone(),
        None => return Redirect::to("/wp-login.php").into_response(),
    };

    let role = resolve_session_role(&session, &state.db).await;

    match role {
        Some(r) if r.can(&capability) => next.run(request).await,
        _ => {
            (
                StatusCode::FORBIDDEN,
                "Sorry, you are not allowed to access this page.",
            )
                .into_response()
        }
    }
}

/// Convenience middleware: require `manage_options` capability (admin only).
pub async fn require_admin(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Response {
    require_capability(State(state), request, next, Capability::ManageOptions).await
}

/// Convenience middleware: require at least Editor-level access.
/// We check for `edit_others_posts` which editors and admins have but
/// authors / contributors / subscribers do not.
pub async fn require_editor_or_above(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Response {
    require_capability(State(state), request, next, Capability::EditOthersPosts).await
}

/// Convenience middleware: require `list_users` capability (admin only).
pub async fn require_list_users(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Response {
    require_capability(State(state), request, next, Capability::ListUsers).await
}

/// Convenience middleware: require `activate_plugins` capability (admin only).
pub async fn require_activate_plugins(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Response {
    require_capability(State(state), request, next, Capability::ActivatePlugins).await
}

/// Middleware: add security headers to all responses.
pub async fn security_headers(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();

    headers.insert(
        "X-Content-Type-Options",
        HeaderValue::from_static("nosniff"),
    );
    headers.insert("X-Frame-Options", HeaderValue::from_static("SAMEORIGIN"));
    headers.insert(
        "X-XSS-Protection",
        HeaderValue::from_static("1; mode=block"),
    );
    headers.insert(
        "Referrer-Policy",
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    );
    headers.insert(
        "Permissions-Policy",
        HeaderValue::from_static("camera=(), microphone=(), geolocation=()"),
    );

    response
}
