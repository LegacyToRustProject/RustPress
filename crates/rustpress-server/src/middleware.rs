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
pub async fn get_user_role(user_id: u64, db: &sea_orm::DatabaseConnection) -> Option<String> {
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
        _ => (
            StatusCode::FORBIDDEN,
            "Sorry, you are not allowed to access this page.",
        )
            .into_response(),
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

/// The resolved blog ID inserted into request extensions by the multisite middleware.
#[derive(Debug, Clone, Copy)]
pub struct ResolvedBlogId(pub u64);

/// Middleware: resolve the current blog in a multisite installation.
///
/// Extracts the Host header and request path, uses the SiteResolver to find
/// the matching site, and inserts the blog_id into request extensions as
/// `ResolvedBlogId`. If multisite is not enabled or no site matches, the
/// request proceeds without modification (defaults to blog_id 1).
pub async fn multisite_resolve(
    State(state): State<Arc<AppState>>,
    mut request: Request,
    next: Next,
) -> Response {
    if let Some(ref resolver) = state.multisite_resolver {
        let host = request
            .headers()
            .get(header::HOST)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("localhost")
            .to_string();

        let path = request.uri().path().to_string();

        let blog_id = resolver
            .resolve_site(&host, &path)
            .map(|site| site.blog_id)
            .unwrap_or(1);

        request.extensions_mut().insert(ResolvedBlogId(blog_id));
        tracing::debug!(blog_id, host = %host, path = %path, "Multisite resolved");
    }

    next.run(request).await
}

/// Middleware: WAF (Web Application Firewall) check on incoming requests.
/// Blocks requests matching malicious patterns (SQL injection, XSS, path traversal, etc.).
pub async fn waf_check(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Response {
    let method = request.method().to_string();
    let path = request.uri().path().to_string();
    let query = request.uri().query().unwrap_or("").to_string();

    // Skip WAF for static assets and well-known safe paths
    if path.starts_with("/static/")
        || path.starts_with("/wp-content/uploads/")
        || path == "/favicon.ico"
    {
        return next.run(request).await;
    }

    // Collect headers for WAF inspection
    let header_map: std::collections::HashMap<String, String> = request
        .headers()
        .iter()
        .filter_map(|(k, v)| v.to_str().ok().map(|val| (k.to_string(), val.to_string())))
        .collect();

    let waf = state.waf.read().await;
    let result = waf.check_request(&method, &path, &query, "", &header_map);
    drop(waf);

    match result {
        rustpress_security::WafResult::Block { rule_id, reason } => {
            tracing::warn!(rule_id, reason, path, "WAF blocked request");
            (StatusCode::FORBIDDEN, "Forbidden").into_response()
        }
        _ => next.run(request).await,
    }
}

/// Middleware: rate limiting based on client IP and endpoint category.
pub async fn rate_limit(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Response {
    let ip = request
        .headers()
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.split(',').next().unwrap_or("unknown").trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let path = request.uri().path().to_string();

    let mut limiter = state.rate_limiter.write().await;
    let result = limiter.check(&ip, &path);
    drop(limiter);

    match result {
        rustpress_security::RateLimitResult::Limited { retry_after } => {
            tracing::warn!(ip, path, retry_after, "Rate limited");
            let mut response =
                (StatusCode::TOO_MANY_REQUESTS, "Rate limit exceeded").into_response();
            response.headers_mut().insert(
                "Retry-After",
                HeaderValue::from_str(&retry_after.to_string()).unwrap(),
            );
            response
        }
        rustpress_security::RateLimitResult::Allowed { .. } => next.run(request).await,
    }
}

/// Middleware: compute ETag for GET responses and return 304 Not Modified
/// when the client sends a matching `If-None-Match` header.
pub async fn etag_headers(request: Request, next: Next) -> Response {
    use axum::http::Method;
    use md5::{Digest, Md5};

    // Only apply to GET requests
    if request.method() != Method::GET {
        return next.run(request).await;
    }

    let if_none_match = request
        .headers()
        .get(header::IF_NONE_MATCH)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let response = next.run(request).await;

    // Only compute ETag for successful HTML/JSON responses
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if !content_type.contains("text/html") && !content_type.contains("application/json") {
        return response;
    }

    // Extract body bytes
    let (parts, body) = response.into_parts();
    let bytes = match axum::body::to_bytes(body, 10_000_000).await {
        Ok(b) => b,
        Err(_) => return axum::http::Response::from_parts(parts, axum::body::Body::empty()),
    };

    // Compute ETag
    let mut hasher = Md5::new();
    hasher.update(&bytes);
    let hash = hasher.finalize();
    let etag = format!("\"{:x}\"", hash);

    // Check If-None-Match
    if let Some(inm) = if_none_match {
        if inm == etag || inm == format!("W/{}", etag) {
            return (StatusCode::NOT_MODIFIED, [(header::ETAG, etag)]).into_response();
        }
    }

    // Rebuild response with ETag header
    let mut response = axum::http::Response::from_parts(parts, axum::body::Body::from(bytes));
    response
        .headers_mut()
        .insert(header::ETAG, HeaderValue::from_str(&etag).unwrap());
    response
}
