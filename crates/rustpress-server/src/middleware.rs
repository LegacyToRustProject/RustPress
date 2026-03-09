use axum::{
    extract::{Request, State},
    http::{header, HeaderValue, Method, StatusCode},
    middleware::Next,
    response::{IntoResponse, Redirect, Response},
};
use base64::Engine as _;
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

/// Per-request CSP nonce injected into request extensions by `security_headers`.
///
/// Handlers that render inline `<script nonce="…">` or `<style nonce="…">` can
/// extract this from `request.extensions()`.  Because every request gets a fresh
/// nonce the value is guaranteed not to repeat.
#[derive(Clone, Debug)]
pub struct CspNonce(pub String);

/// Middleware: add security headers to all responses.
///
/// Generates a fresh 128-bit cryptographically-random nonce on every request and
/// injects it into both the request extensions (for handlers) and the
/// Content-Security-Policy response header. `unsafe-inline` and `unsafe-eval` are
/// intentionally absent from `script-src`.
pub async fn security_headers(mut request: Request, next: Next) -> Response {
    // Generate a 128-bit nonce via UUID v4 (backed by OS CSPRNG via getrandom).
    let nonce = base64::engine::general_purpose::STANDARD
        .encode(uuid::Uuid::new_v4().as_bytes());

    // Store in request extensions so handlers / templates can use it.
    request.extensions_mut().insert(CspNonce(nonce.clone()));

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
    // Nonce-based CSP: no unsafe-inline, no unsafe-eval.
    let csp = format!(
        "default-src 'self'; \
         script-src 'self' 'nonce-{nonce}'; \
         style-src 'self' 'nonce-{nonce}'; \
         img-src 'self' data: https:; \
         font-src 'self' data:; \
         object-src 'none'; \
         base-uri 'self'; \
         form-action 'self'; \
         frame-src 'self'; \
         frame-ancestors 'self'"
    );
    if let Ok(val) = HeaderValue::from_str(&csp) {
        headers.insert("Content-Security-Policy", val);
    }
    headers.insert(
        "Cross-Origin-Resource-Policy",
        HeaderValue::from_static("same-origin"),
    );
    headers.insert(
        "Cross-Origin-Embedder-Policy",
        HeaderValue::from_static("require-corp"),
    );
    headers.insert(
        "Cross-Origin-Opener-Policy",
        HeaderValue::from_static("same-origin"),
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
        || path.starts_with("/wp-content/themes/")
        || path.starts_with("/wp-includes/")
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
            let ip = extract_client_ip(&request);
            tracing::warn!(rule_id, reason, path, "WAF blocked request");
            state.audit_log.log_waf_block(&ip, &rule_id, &path);
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
    let ip = extract_client_ip(&request);
    let path = request.uri().path().to_string();

    let mut limiter = state.rate_limiter.write().await;
    let result = limiter.check(&ip, &path);
    drop(limiter);

    match result {
        rustpress_security::RateLimitResult::Limited { retry_after } => {
            tracing::warn!(ip, path, retry_after, "Rate limited");
            state.audit_log.log_rate_limited(&ip, &path);
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
    let etag = format!("\"{hash:x}\"");

    // Check If-None-Match
    if let Some(inm) = if_none_match {
        if inm == etag || inm == format!("W/{etag}") {
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

/// Middleware: block access to sensitive files (.env, .git, wp-config.php, etc.).
///
/// Returns 404 for any request attempting to access files that should never be
/// served over HTTP. This prevents information disclosure (OWASP A05).
pub async fn block_sensitive_files(request: Request, next: Next) -> Response {
    let path = request.uri().path().to_lowercase();

    // Block dotfiles and sensitive config files
    let blocked_patterns = [
        "/.env",
        "/.git",
        "/.htaccess",
        "/.htpasswd",
        "/wp-config.php",
        "/wp-config-sample.php",
        "/.user.ini",
        "/php.ini",
        "/.DS_Store",
        "/Thumbs.db",
        "/web.config",
        "/.svn",
        "/.hg",
        "/composer.json",
        "/composer.lock",
        "/package.json",
        "/yarn.lock",
        "/Cargo.toml",
        "/Cargo.lock",
        "/readme.html",
        "/license.txt",
        "/wp-includes/",
        "/wp-content/debug.log",
    ];

    for pattern in &blocked_patterns {
        if path == *pattern || path.starts_with(&format!("{pattern}/")) {
            return (StatusCode::NOT_FOUND, "").into_response();
        }
    }

    // Block backup/source files
    if path.ends_with(".bak")
        || path.ends_with(".swp")
        || path.ends_with(".swo")
        || path.ends_with("~")
        || path.ends_with(".orig")
        || path.ends_with(".sql")
        || path.ends_with(".log")
        || path.ends_with(".php")
    {
        // Allow specific PHP-like routes that are actually Axum handlers
        let allowed_php = [
            "/wp-login.php",
            "/xmlrpc.php",
            "/wp-admin/admin-seo.php",
            "/wp-admin/admin-acf.php",
            "/wp-admin/admin-cf7.php",
            "/wp-admin/admin-security.php",
        ];
        if !allowed_php.contains(&path.as_str()) {
            return (StatusCode::NOT_FOUND, "").into_response();
        }
    }

    next.run(request).await
}

/// Middleware: CORS (Cross-Origin Resource Sharing) control.
///
/// Only allows requests from the configured site URL origin.
/// API routes with Bearer token auth don't need CORS cookies, but we still
/// restrict origins to prevent unauthorized cross-origin access (OWASP A01).
pub async fn cors_headers(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Response {
    let origin = request
        .headers()
        .get(header::ORIGIN)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    // Handle preflight requests
    if request.method() == Method::OPTIONS {
        let mut response = StatusCode::NO_CONTENT.into_response();
        let headers = response.headers_mut();
        if let Some(ref origin) = origin {
            if is_allowed_origin(origin, &state.site_url) {
                headers.insert(
                    header::ACCESS_CONTROL_ALLOW_ORIGIN,
                    HeaderValue::from_str(origin).unwrap_or(HeaderValue::from_static("null")),
                );
            }
        }
        headers.insert(
            header::ACCESS_CONTROL_ALLOW_METHODS,
            HeaderValue::from_static("GET, POST, PUT, DELETE, OPTIONS"),
        );
        headers.insert(
            header::ACCESS_CONTROL_ALLOW_HEADERS,
            HeaderValue::from_static("Content-Type, Authorization, X-WP-Nonce"),
        );
        headers.insert(
            header::ACCESS_CONTROL_MAX_AGE,
            HeaderValue::from_static("3600"),
        );
        return response;
    }

    let mut response = next.run(request).await;

    // Add CORS headers to the response
    if let Some(ref origin) = origin {
        if is_allowed_origin(origin, &state.site_url) {
            let headers = response.headers_mut();
            headers.insert(
                header::ACCESS_CONTROL_ALLOW_ORIGIN,
                HeaderValue::from_str(origin).unwrap_or(HeaderValue::from_static("null")),
            );
            headers.insert(
                header::ACCESS_CONTROL_ALLOW_CREDENTIALS,
                HeaderValue::from_static("true"),
            );
            headers.insert(
                header::ACCESS_CONTROL_EXPOSE_HEADERS,
                HeaderValue::from_static("X-WP-Total, X-WP-TotalPages"),
            );
        }
    }

    response
}

/// Extract and lower-case the `scheme://host:port` authority from a URL.
///
/// Strips any path, query, or fragment so that `http://example.com/path` and
/// `http://example.com` both normalise to `http://example.com`.  This prevents
/// sub-domain bypass (e.g. `http://example.com.evil.com` differs from
/// `http://example.com` after normalisation).
fn origin_authority(url: &str) -> String {
    let lower = url.trim().to_lowercase();
    if let Some(after_scheme) = lower.find("://") {
        let rest = &lower[after_scheme + 3..];
        // Authority ends at the first '/', '?', or '#'
        let authority_len = rest
            .find(|c| c == '/' || c == '?' || c == '#')
            .unwrap_or(rest.len());
        let scheme = &lower[..after_scheme];
        let authority = &rest[..authority_len];
        // Strip default ports (80 for http, 443 for https) to normalise
        let authority = match (scheme, authority.rsplit_once(':')) {
            ("http", Some((host, "80"))) => host,
            ("https", Some((host, "443"))) => host,
            _ => authority,
        };
        format!("{scheme}://{authority}")
    } else {
        lower
    }
}

/// Check if an origin is allowed.
///
/// Compares normalised `scheme://host:port` representations so that path
/// suffixes and default ports cannot be exploited to bypass the check.
fn is_allowed_origin(origin: &str, site_url: &str) -> bool {
    let norm_origin = origin_authority(origin);
    if norm_origin == origin_authority(site_url) {
        return true;
    }

    // Allow explicitly configured origins from the environment.
    if let Ok(allowed) = std::env::var("CORS_ALLOWED_ORIGINS") {
        for allowed_origin in allowed.split(',') {
            if norm_origin == origin_authority(allowed_origin.trim()) {
                return true;
            }
        }
    }

    false
}

/// Middleware: verify CSRF nonce on state-changing requests to admin endpoints.
///
/// All POST/PUT/DELETE requests to /wp-admin/* must include a valid nonce
/// either as a form field `_wpnonce` or header `X-WP-Nonce`.
/// API endpoints using Bearer token auth are exempt (the token itself is CSRF protection).
pub async fn csrf_nonce_check(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Response {
    let method = request.method().clone();
    let path = request.uri().path().to_string();

    // Only check state-changing methods
    if method == Method::GET || method == Method::HEAD || method == Method::OPTIONS {
        return next.run(request).await;
    }

    // Only check admin form endpoints (not API endpoints which use JWT)
    if !path.starts_with("/wp-admin/") {
        return next.run(request).await;
    }

    // Skip login POST (no session yet)
    if path == "/wp-login.php" {
        return next.run(request).await;
    }

    // Extract session to get user_id for nonce verification
    let session = request.extensions().get::<Session>().cloned();
    let user_id = session.as_ref().map(|s| s.user_id).unwrap_or(0);

    if user_id == 0 {
        // No session means the admin session middleware will handle redirect
        return next.run(request).await;
    }

    // Check for nonce in X-WP-Nonce header
    let nonce = request
        .headers()
        .get("X-WP-Nonce")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    // If no header nonce, we'll check the form body nonce in the handler
    // For now, if X-WP-Nonce is present, validate it
    if let Some(ref nonce_value) = nonce {
        let action = format!("admin_{}", path.trim_start_matches("/wp-admin/"));
        if state
            .nonces
            .verify_nonce(nonce_value, &action, user_id)
            .is_none()
        {
            return (StatusCode::FORBIDDEN, "Invalid or expired security token").into_response();
        }
    }

    // If no nonce header, allow the request through (handlers will check form nonce)
    next.run(request).await
}

/// Extract the client IP address from the request.
///
/// Checks X-Forwarded-For first (for reverse proxy setups), falls back to
/// X-Real-IP, then to the peer address.
pub fn extract_client_ip(request: &Request) -> String {
    request
        .headers()
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.split(',').next().unwrap_or("unknown").trim().to_string())
        .or_else(|| {
            request
                .headers()
                .get("x-real-ip")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| "unknown".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request as HttpRequest;
    use axum::routing::get;
    use axum::Router;
    use tower::ServiceExt;

    fn security_headers_app() -> Router {
        Router::new()
            .route("/test", get(|| async { "ok" }))
            .layer(axum::middleware::from_fn(security_headers))
    }

    #[tokio::test]
    async fn test_security_headers_present() {
        let app = security_headers_app();
        let req = HttpRequest::builder()
            .uri("/test")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(
            resp.headers().get("X-Content-Type-Options").unwrap(),
            "nosniff"
        );
        assert_eq!(resp.headers().get("X-Frame-Options").unwrap(), "SAMEORIGIN");
    }

    #[tokio::test]
    async fn test_security_headers_referrer_and_permissions() {
        let app = security_headers_app();
        let req = HttpRequest::builder()
            .uri("/test")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(
            resp.headers().get("Referrer-Policy").unwrap(),
            "strict-origin-when-cross-origin"
        );
        assert_eq!(
            resp.headers().get("Permissions-Policy").unwrap(),
            "camera=(), microphone=(), geolocation=()"
        );
    }

    #[tokio::test]
    async fn test_security_headers_csp() {
        let app = security_headers_app();
        let req = HttpRequest::builder()
            .uri("/test")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        let csp = resp
            .headers()
            .get("Content-Security-Policy")
            .unwrap()
            .to_str()
            .unwrap();
        assert!(csp.contains("default-src 'self'"));
        assert!(csp.contains("object-src 'none'"));
        assert!(csp.contains("base-uri 'self'"));
    }

    #[tokio::test]
    async fn test_security_headers_corp_coep_coop() {
        let app = security_headers_app();
        let req = HttpRequest::builder()
            .uri("/test")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(
            resp.headers().get("Cross-Origin-Resource-Policy").unwrap(),
            "same-origin"
        );
        assert_eq!(
            resp.headers().get("Cross-Origin-Embedder-Policy").unwrap(),
            "require-corp"
        );
        assert_eq!(
            resp.headers().get("Cross-Origin-Opener-Policy").unwrap(),
            "same-origin"
        );
    }

    #[test]
    fn test_extract_role_from_serialized_php() {
        assert_eq!(
            extract_role_from_serialized(r#"a:1:{s:13:"administrator";b:1;}"#),
            Some("administrator".to_string())
        );
        assert_eq!(
            extract_role_from_serialized(r#"a:1:{s:6:"editor";b:1;}"#),
            Some("editor".to_string())
        );
        assert_eq!(extract_role_from_serialized("garbage"), None);
    }
}

// ---------------------------------------------------------------------------
// Telemetry middleware — records HTTP request metrics and tracing spans
// ---------------------------------------------------------------------------

/// Axum middleware that wraps every request in a tracing span and records
/// Prometheus metrics via `telemetry::record_http_request`.
pub async fn telemetry_trace(request: Request, next: Next) -> Response {
    use std::time::Instant;

    let method = request.method().to_string();
    // Normalise the path: strip query-string so cardinality stays bounded.
    let path = request
        .uri()
        .path()
        .to_string();

    let span = tracing::info_span!(
        "http_request",
        http.method = %method,
        http.path   = %path,
        http.status = tracing::field::Empty,
    );

    let start = Instant::now();

    let response = {
        let _enter = span.enter();
        next.run(request).await
    };

    let status = response.status().as_u16();
    let duration_ms = start.elapsed().as_millis() as u64;

    span.record("http.status", status);

    crate::telemetry::record_http_request(&method, &path, status, duration_ms);

    response
}
