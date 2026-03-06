use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Value};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// WordPress REST API `context` parameter
// ---------------------------------------------------------------------------

/// WordPress REST API context parameter.
///
/// Controls which fields are included in API responses:
/// - `View`  (default) -- standard public fields
/// - `Edit`  -- all fields including `raw` sub-fields on title/content/excerpt
/// - `Embed` -- minimal subset suitable for `_embed` responses
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestContext {
    View,
    Edit,
    Embed,
}

impl RestContext {
    /// Parse a context string (from `?context=` query param).
    /// Unknown values default to `View`.
    pub fn from_str(s: &str) -> Self {
        match s {
            "edit" => Self::Edit,
            "embed" => Self::Embed,
            _ => Self::View,
        }
    }

    /// Parse from an `Option<String>` query parameter, defaulting to `View`.
    pub fn from_option(s: Option<&str>) -> Self {
        match s {
            Some(v) => Self::from_str(v),
            None => Self::View,
        }
    }
}

/// Filter post / page fields based on context.
///
/// - `Embed`: keep only id, date, slug, link, title, excerpt, author,
///   featured_media, _links, _embedded.
/// - `Edit`: add `raw` sub-field to title/content/excerpt objects.
/// - `View`: no changes.
pub fn filter_post_context(val: &mut Value, context: RestContext) {
    match context {
        RestContext::Embed => {
            let embed_fields = [
                "id", "date", "slug", "link", "title", "excerpt",
                "author", "featured_media", "_links", "_embedded",
            ];
            if let Some(obj) = val.as_object_mut() {
                obj.retain(|k, _| embed_fields.contains(&k.as_str()));
            }
        }
        RestContext::Edit => {
            // In edit context WordPress returns a `raw` sub-field alongside
            // `rendered` for title, content, and excerpt.  The `raw` value is
            // the same as `rendered` here because we do not store the
            // unprocessed source separately (the DB already holds the raw
            // markup).
            if let Some(obj) = val.as_object_mut() {
                for key in &["title", "content", "excerpt"] {
                    if let Some(field) = obj.get_mut(*key) {
                        if let Some(inner) = field.as_object_mut() {
                            if let Some(rendered) = inner.get("rendered").cloned() {
                                inner.insert("raw".to_string(), rendered);
                            }
                        }
                    }
                }
            }
        }
        RestContext::View => { /* default -- nothing to filter */ }
    }
}

/// Filter user fields based on context.
///
/// - `Embed`: id, name, url, description, link, slug, avatar_urls, _links.
/// - `View`: remove edit-only fields (email, registered_date, roles, capabilities).
/// - `Edit`: all fields returned.
pub fn filter_user_context(val: &mut Value, context: RestContext) {
    match context {
        RestContext::Embed => {
            let embed_fields = [
                "id", "name", "url", "description", "link",
                "slug", "avatar_urls", "_links",
            ];
            if let Some(obj) = val.as_object_mut() {
                obj.retain(|k, _| embed_fields.contains(&k.as_str()));
            }
        }
        RestContext::View => {
            // Remove edit-only fields
            if let Some(obj) = val.as_object_mut() {
                obj.remove("email");
                obj.remove("registered_date");
                obj.remove("roles");
                obj.remove("capabilities");
            }
        }
        RestContext::Edit => { /* all fields */ }
    }
}

/// Filter term (category/tag) fields based on context.
///
/// - `Embed`: id, link, name, slug, taxonomy, _links.
/// - `View` / `Edit`: no filtering.
pub fn filter_term_context(val: &mut Value, context: RestContext) {
    match context {
        RestContext::Embed => {
            let embed_fields = ["id", "link", "name", "slug", "taxonomy", "_links"];
            if let Some(obj) = val.as_object_mut() {
                obj.retain(|k, _| embed_fields.contains(&k.as_str()));
            }
        }
        _ => {}
    }
}

/// Filter comment fields based on context.
///
/// - `Embed`: id, author, author_name, author_url, date, content, link, type, _links.
/// - `View` / `Edit`: no filtering.
pub fn filter_comment_context(val: &mut Value, context: RestContext) {
    match context {
        RestContext::Embed => {
            let embed_fields = [
                "id", "author", "author_name", "author_url",
                "date", "content", "link", "type", "_links",
            ];
            if let Some(obj) = val.as_object_mut() {
                obj.retain(|k, _| embed_fields.contains(&k.as_str()));
            }
        }
        _ => {}
    }
}

/// Filter media fields based on context.
///
/// - `Embed`: id, date, slug, title, author, source_url, _links, _embedded.
/// - `Edit`: add `raw` sub-field to title/caption/description objects.
/// - `View`: no filtering.
pub fn filter_media_context(val: &mut Value, context: RestContext) {
    match context {
        RestContext::Embed => {
            let embed_fields = [
                "id", "date", "slug", "title", "author",
                "source_url", "_links", "_embedded",
            ];
            if let Some(obj) = val.as_object_mut() {
                obj.retain(|k, _| embed_fields.contains(&k.as_str()));
            }
        }
        RestContext::Edit => {
            if let Some(obj) = val.as_object_mut() {
                for key in &["title", "caption", "description"] {
                    if let Some(field) = obj.get_mut(*key) {
                        if let Some(inner) = field.as_object_mut() {
                            if let Some(rendered) = inner.get("rendered").cloned() {
                                inner.insert("raw".to_string(), rendered);
                            }
                        }
                    }
                }
            }
        }
        RestContext::View => {}
    }
}

/// Apply context filtering to every element of a JSON array.
/// `filter_fn` is one of the `filter_*_context` helpers above.
pub fn apply_context_to_array(
    items: &mut Vec<Value>,
    context: RestContext,
    filter_fn: fn(&mut Value, RestContext),
) {
    if context != RestContext::View {
        for item in items.iter_mut() {
            filter_fn(item, context);
        }
    }
}

/// WordPress-compatible REST API error response.
///
/// WordPress always returns JSON from its REST API, even for errors:
/// ```json
/// {"code": "rest_error_code", "message": "Human readable message", "data": {"status": 500}}
/// ```
///
/// This type implements `IntoResponse` to return proper JSON errors instead of
/// plain text, which `@wordpress/api-fetch` requires to parse error messages.
#[derive(Debug)]
pub struct WpError {
    pub status: StatusCode,
    pub code: String,
    pub message: String,
}

impl WpError {
    pub fn new(status: StatusCode, code: &str, message: impl Into<String>) -> Self {
        Self {
            status,
            code: code.to_string(),
            message: message.into(),
        }
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, "rest_post_invalid_id", message)
    }

    pub fn unauthorized() -> Self {
        Self::new(
            StatusCode::UNAUTHORIZED,
            "rest_not_logged_in",
            "Authentication required",
        )
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::new(StatusCode::FORBIDDEN, "rest_forbidden", message)
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_server_error",
            message,
        )
    }

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "rest_invalid_param", message)
    }
}

impl IntoResponse for WpError {
    fn into_response(self) -> Response {
        let body = json!({
            "code": self.code,
            "message": self.message,
            "data": { "status": self.status.as_u16() }
        });
        (self.status, Json(body)).into_response()
    }
}

/// Convert `(StatusCode, String)` tuple to `WpError` for backward compatibility.
impl From<(StatusCode, String)> for WpError {
    fn from((status, message): (StatusCode, String)) -> Self {
        let code = match status {
            StatusCode::NOT_FOUND => "rest_post_invalid_id",
            StatusCode::UNAUTHORIZED => "rest_not_logged_in",
            StatusCode::FORBIDDEN => "rest_forbidden",
            StatusCode::BAD_REQUEST => "rest_invalid_param",
            _ => "internal_server_error",
        };
        Self::new(status, code, message)
    }
}

/// Filter JSON response fields based on `_fields` query parameter.
/// WordPress `_fields` parameter: `?_fields=id,title,slug`
/// If fields is None or empty, returns the value unchanged.
pub fn filter_fields(value: Value, fields: Option<&str>) -> Value {
    match fields {
        Some(f) if !f.is_empty() => {
            let requested: Vec<&str> = f.split(',').map(|s| s.trim()).collect();
            match value {
                Value::Object(map) => {
                    let mut result = serde_json::Map::new();
                    for field in &requested {
                        // Handle nested fields like "title.rendered"
                        let parts: Vec<&str> = field.splitn(2, '.').collect();
                        let top_key = parts[0];

                        if let Some(val) = map.get(top_key) {
                            if parts.len() == 2 {
                                // Nested field: keep only the nested key
                                if let Value::Object(inner) = val {
                                    let mut nested = match result.get(top_key) {
                                        Some(Value::Object(existing)) => existing.clone(),
                                        _ => serde_json::Map::new(),
                                    };
                                    if let Some(inner_val) = inner.get(parts[1]) {
                                        nested
                                            .insert(parts[1].to_string(), inner_val.clone());
                                    }
                                    result
                                        .insert(top_key.to_string(), Value::Object(nested));
                                } else {
                                    result.insert(top_key.to_string(), val.clone());
                                }
                            } else {
                                result.insert(top_key.to_string(), val.clone());
                            }
                        }
                    }
                    // Always include _links if present (WordPress behavior)
                    if let Some(links) = map.get("_links") {
                        if !result.contains_key("_links") {
                            result.insert("_links".to_string(), links.clone());
                        }
                    }
                    Value::Object(result)
                }
                Value::Array(arr) => {
                    let filtered: Vec<Value> = arr
                        .into_iter()
                        .map(|v| filter_fields(v, Some(f)))
                        .collect();
                    Value::Array(filtered)
                }
                other => other,
            }
        }
        _ => value,
    }
}

/// Build `_links` object for a post or page.
/// Returns HATEOAS links matching WordPress format:
/// self, collection, about, author (embeddable), replies (embeddable),
/// version-history, wp:featuredmedia (embeddable), wp:attachment,
/// wp:term (embeddable, for category and post_tag), curies.
pub fn post_links(site_url: &str, post_id: u64, post_type: &str, author_id: u64) -> Value {
    let base = site_url.trim_end_matches('/');
    let collection_path = if post_type == "page" {
        "pages"
    } else {
        "posts"
    };

    json!({
        "self": [{
            "href": format!("{}/wp-json/wp/v2/{}/{}", base, collection_path, post_id)
        }],
        "collection": [{
            "href": format!("{}/wp-json/wp/v2/{}", base, collection_path)
        }],
        "about": [{
            "href": format!("{}/wp-json/wp/v2/types/{}", base, post_type)
        }],
        "author": [{
            "embeddable": true,
            "href": format!("{}/wp-json/wp/v2/users/{}", base, author_id)
        }],
        "replies": [{
            "embeddable": true,
            "href": format!("{}/wp-json/wp/v2/comments?post={}", base, post_id)
        }],
        "version-history": [{
            "count": 0,
            "href": format!("{}/wp-json/wp/v2/{}/{}/revisions", base, collection_path, post_id)
        }],
        "wp:featuredmedia": [{
            "embeddable": true,
            "href": format!("{}/wp-json/wp/v2/media/0", base)
        }],
        "wp:attachment": [{
            "href": format!("{}/wp-json/wp/v2/media?parent={}", base, post_id)
        }],
        "wp:term": [
            {
                "taxonomy": "category",
                "embeddable": true,
                "href": format!("{}/wp-json/wp/v2/categories?post={}", base, post_id)
            },
            {
                "taxonomy": "post_tag",
                "embeddable": true,
                "href": format!("{}/wp-json/wp/v2/tags?post={}", base, post_id)
            }
        ],
        "curies": wp_curies()
    })
}

/// Build `_links` for a taxonomy term (category or tag).
pub fn term_links(site_url: &str, taxonomy: &str, term_taxonomy_id: u64) -> Value {
    let base = site_url.trim_end_matches('/');
    let collection_slug = if taxonomy == "category" {
        "categories"
    } else {
        "tags"
    };

    json!({
        "self": [
            { "href": format!("{}/wp-json/wp/v2/{}/{}", base, collection_slug, term_taxonomy_id) }
        ],
        "collection": [
            { "href": format!("{}/wp-json/wp/v2/{}", base, collection_slug) }
        ],
        "about": [
            { "href": format!("{}/wp-json/wp/v2/taxonomies/{}", base, taxonomy) }
        ],
        "wp:post_type": [
            { "href": format!("{}/wp-json/wp/v2/posts?{}={}", base, collection_slug, term_taxonomy_id) }
        ],
        "curies": wp_curies()
    })
}

/// Build `_links` for a user.
pub fn user_links(site_url: &str, user_id: u64) -> Value {
    let base = site_url.trim_end_matches('/');
    json!({
        "self": [
            { "href": format!("{}/wp-json/wp/v2/users/{}", base, user_id) }
        ],
        "collection": [
            { "href": format!("{}/wp-json/wp/v2/users", base) }
        ],
        "curies": wp_curies()
    })
}

/// Build `_links` for a comment resource.
pub fn comment_links(site_url: &str, comment_id: u64, post_id: u64) -> Value {
    let base = site_url.trim_end_matches('/');
    json!({
        "self": [
            { "href": format!("{}/wp-json/wp/v2/comments/{}", base, comment_id) }
        ],
        "collection": [
            { "href": format!("{}/wp-json/wp/v2/comments", base) }
        ],
        "up": [
            {
                "href": format!("{}/wp-json/wp/v2/posts/{}", base, post_id),
                "embeddable": true,
                "post_type": "post"
            }
        ],
        "curies": wp_curies()
    })
}

/// Build `_links` for a media resource.
pub fn media_links(site_url: &str, media_id: u64, author_id: u64) -> Value {
    let base = site_url.trim_end_matches('/');
    json!({
        "self": [
            { "href": format!("{}/wp-json/wp/v2/media/{}", base, media_id) }
        ],
        "collection": [
            { "href": format!("{}/wp-json/wp/v2/media", base) }
        ],
        "about": [
            { "href": format!("{}/wp-json/wp/v2/types/attachment", base) }
        ],
        "author": [
            {
                "href": format!("{}/wp-json/wp/v2/users/{}", base, author_id),
                "embeddable": true
            }
        ],
        "replies": [
            {
                "href": format!("{}/wp-json/wp/v2/comments?post={}", base, media_id),
                "embeddable": true
            }
        ],
        "curies": wp_curies()
    })
}

/// Build standard WordPress pagination headers (`X-WP-Total`, `X-WP-TotalPages`).
pub fn pagination_headers(total: u64, total_pages: u64) -> HeaderMap {
    let mut headers = HeaderMap::new();
    if let Ok(v) = total.to_string().parse() {
        headers.insert("X-WP-Total", v);
    }
    if let Ok(v) = total_pages.to_string().parse() {
        headers.insert("X-WP-TotalPages", v);
    }
    headers
}

/// WordPress curies (compact URIs) - standard for all `_links`.
fn wp_curies() -> Value {
    json!([{
        "name": "wp",
        "href": "https://api.w.org/{rel}",
        "templated": true
    }])
}

/// Generate a URL-friendly slug from a name string.
pub fn slugify(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' {
                c
            } else if c.is_whitespace() {
                '-'
            } else {
                '_'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

/// Compute MD5 hex digest (for Gravatar URLs).
pub fn md5_hex(input: &str) -> String {
    use md5::Digest;
    let hash = md5::Md5::digest(input.as_bytes());
    hash.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Generate Gravatar avatar URLs for an email address.
/// Returns a map with keys "24", "48", "96" pointing to Gravatar URLs.
pub fn avatar_urls(email: &str) -> HashMap<String, String> {
    let hash = md5_hex(&email.to_lowercase());
    let base = format!("https://secure.gravatar.com/avatar/{}", hash);
    let mut urls = HashMap::new();
    urls.insert("24".to_string(), format!("{}?s=24&d=mm&r=g", base));
    urls.insert("48".to_string(), format!("{}?s=48&d=mm&r=g", base));
    urls.insert("96".to_string(), format!("{}?s=96&d=mm&r=g", base));
    urls
}

/// Extract user ID from the request by checking JWT Bearer token or session cookie.
/// Returns None if no valid authentication is found.
pub async fn extract_user_id(
    jwt: &rustpress_auth::JwtManager,
    sessions: &rustpress_auth::SessionManager,
    headers: &HeaderMap,
) -> Option<u64> {
    // Try JWT
    if let Some(token) = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
    {
        if let Ok(claims) = jwt.validate_token(token) {
            return Some(claims.sub);
        }
    }

    // Try session cookie
    if let Some(sid) = headers
        .get(axum::http::header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(|cookies| {
            cookies.split(';').find_map(|c| {
                c.trim()
                    .strip_prefix("rustpress_session=")
                    .map(|v| v.to_string())
            })
        })
    {
        if let Some(session) = sessions.get_session(&sid).await {
            return Some(session.user_id);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_fields_basic() {
        let val = json!({
            "id": 1,
            "title": {"rendered": "Hello"},
            "slug": "hello",
            "content": {"rendered": "<p>world</p>"}
        });
        let filtered = filter_fields(val, Some("id,slug"));
        assert_eq!(filtered.get("id").unwrap(), &json!(1));
        assert_eq!(filtered.get("slug").unwrap(), &json!("hello"));
        assert!(filtered.get("title").is_none());
        assert!(filtered.get("content").is_none());
    }

    #[test]
    fn test_filter_fields_nested() {
        let val = json!({
            "id": 1,
            "title": {"rendered": "Hello", "raw": "Hello"},
        });
        let filtered = filter_fields(val, Some("id,title.rendered"));
        assert_eq!(filtered.get("id").unwrap(), &json!(1));
        let title = filtered.get("title").unwrap();
        assert!(title.get("rendered").is_some());
        assert!(title.get("raw").is_none());
    }

    #[test]
    fn test_filter_fields_array() {
        let val = json!([
            {"id": 1, "slug": "a", "title": {"rendered": "A"}},
            {"id": 2, "slug": "b", "title": {"rendered": "B"}},
        ]);
        let filtered = filter_fields(val, Some("id,slug"));
        let arr = filtered.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert!(arr[0].get("title").is_none());
        assert_eq!(arr[0].get("id").unwrap(), &json!(1));
    }

    #[test]
    fn test_filter_fields_none() {
        let val = json!({"id": 1, "title": "Hello"});
        let filtered = filter_fields(val.clone(), None);
        assert_eq!(filtered, val);
    }

    #[test]
    fn test_filter_fields_empty() {
        let val = json!({"id": 1, "title": "Hello"});
        let filtered = filter_fields(val.clone(), Some(""));
        assert_eq!(filtered, val);
    }

    #[test]
    fn test_filter_fields_preserves_links() {
        let val = json!({
            "id": 1,
            "title": {"rendered": "Hello"},
            "_links": {"self": [{"href": "http://example.com"}]}
        });
        let filtered = filter_fields(val, Some("id"));
        assert!(filtered.get("_links").is_some());
        assert_eq!(filtered.get("id").unwrap(), &json!(1));
    }

    #[test]
    fn test_pagination_headers() {
        let headers = pagination_headers(42, 5);
        assert_eq!(headers.get("X-WP-Total").unwrap(), "42");
        assert_eq!(headers.get("X-WP-TotalPages").unwrap(), "5");
    }

    #[test]
    fn test_post_links_structure() {
        let links = post_links("http://localhost:8080", 1, "post", 1);
        assert!(links.get("self").is_some());
        assert!(links.get("collection").is_some());
        assert!(links.get("about").is_some());
        assert!(links.get("author").is_some());
        assert!(links.get("replies").is_some());
        assert!(links.get("curies").is_some());
        assert!(links.get("wp:term").is_some());
        assert!(links.get("wp:featuredmedia").is_some());
        assert!(links.get("wp:attachment").is_some());
        assert!(links.get("version-history").is_some());
    }

    #[test]
    fn test_post_links_page_type() {
        let links = post_links("http://localhost:8080", 5, "page", 1);
        let self_href = links["self"][0]["href"].as_str().unwrap();
        assert!(self_href.contains("/pages/5"));
    }

    #[test]
    fn test_term_links_category() {
        let links = term_links("http://localhost:8080", "category", 5);
        assert!(links.get("self").is_some());
        assert!(links.get("collection").is_some());
        assert!(links.get("about").is_some());
        assert!(links.get("wp:post_type").is_some());
        assert!(links.get("curies").is_some());
        let self_href = links["self"][0]["href"].as_str().unwrap();
        assert!(self_href.contains("/categories/5"));
    }

    #[test]
    fn test_term_links_tag() {
        let links = term_links("http://localhost:8080", "post_tag", 3);
        let self_href = links["self"][0]["href"].as_str().unwrap();
        assert!(self_href.contains("/tags/3"));
    }

    #[test]
    fn test_user_links_structure() {
        let links = user_links("http://localhost:8080", 1);
        assert!(links.get("self").is_some());
        assert!(links.get("collection").is_some());
        assert!(links.get("curies").is_some());
    }

    #[test]
    fn test_comment_links_structure() {
        let links = comment_links("http://localhost:8080", 10, 1);
        assert!(links.get("self").is_some());
        assert!(links.get("collection").is_some());
        assert!(links.get("up").is_some());
        assert!(links.get("curies").is_some());
    }

    #[test]
    fn test_media_links_structure() {
        let links = media_links("http://localhost:8080", 7, 1);
        assert!(links.get("self").is_some());
        assert!(links.get("collection").is_some());
        assert!(links.get("about").is_some());
        assert!(links.get("author").is_some());
        assert!(links.get("replies").is_some());
        assert!(links.get("curies").is_some());
    }

    #[test]
    fn test_slugify() {
        assert_eq!(slugify("Hello World"), "hello-world");
        assert_eq!(slugify("My  Post  Title"), "my-post-title");
        assert_eq!(slugify("test-slug"), "test-slug");
    }

    // -----------------------------------------------------------------------
    // RestContext tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_rest_context_from_str() {
        assert_eq!(RestContext::from_str("view"), RestContext::View);
        assert_eq!(RestContext::from_str("edit"), RestContext::Edit);
        assert_eq!(RestContext::from_str("embed"), RestContext::Embed);
        assert_eq!(RestContext::from_str("unknown"), RestContext::View);
        assert_eq!(RestContext::from_str(""), RestContext::View);
    }

    #[test]
    fn test_rest_context_from_option() {
        assert_eq!(RestContext::from_option(None), RestContext::View);
        assert_eq!(RestContext::from_option(Some("edit")), RestContext::Edit);
        assert_eq!(RestContext::from_option(Some("embed")), RestContext::Embed);
    }

    #[test]
    fn test_filter_post_context_embed() {
        let mut val = json!({
            "id": 1,
            "date": "2025-01-01T00:00:00",
            "slug": "hello-world",
            "link": "http://example.com/hello-world",
            "title": {"rendered": "Hello World"},
            "content": {"rendered": "<p>Body</p>"},
            "excerpt": {"rendered": "<p>Summary</p>"},
            "author": 1,
            "featured_media": 0,
            "status": "publish",
            "sticky": false,
            "format": "standard",
            "meta": [],
            "categories": [1],
            "tags": [],
            "_links": {"self": [{"href": "http://example.com"}]}
        });
        filter_post_context(&mut val, RestContext::Embed);
        let obj = val.as_object().unwrap();
        // Should keep only embed fields
        assert!(obj.contains_key("id"));
        assert!(obj.contains_key("date"));
        assert!(obj.contains_key("slug"));
        assert!(obj.contains_key("link"));
        assert!(obj.contains_key("title"));
        assert!(obj.contains_key("excerpt"));
        assert!(obj.contains_key("author"));
        assert!(obj.contains_key("featured_media"));
        assert!(obj.contains_key("_links"));
        // Should remove non-embed fields
        assert!(!obj.contains_key("content"));
        assert!(!obj.contains_key("status"));
        assert!(!obj.contains_key("sticky"));
        assert!(!obj.contains_key("format"));
        assert!(!obj.contains_key("meta"));
        assert!(!obj.contains_key("categories"));
        assert!(!obj.contains_key("tags"));
    }

    #[test]
    fn test_filter_post_context_edit() {
        let mut val = json!({
            "id": 1,
            "title": {"rendered": "Hello World"},
            "content": {"rendered": "<p>Body</p>"},
            "excerpt": {"rendered": "<p>Summary</p>"},
            "status": "publish"
        });
        filter_post_context(&mut val, RestContext::Edit);
        // title, content, excerpt should now have both "rendered" and "raw"
        let title = val.get("title").unwrap().as_object().unwrap();
        assert!(title.contains_key("rendered"));
        assert!(title.contains_key("raw"));
        assert_eq!(title.get("raw").unwrap(), "Hello World");

        let content = val.get("content").unwrap().as_object().unwrap();
        assert!(content.contains_key("raw"));

        let excerpt = val.get("excerpt").unwrap().as_object().unwrap();
        assert!(excerpt.contains_key("raw"));

        // Other fields should still be present
        assert!(val.get("status").is_some());
    }

    #[test]
    fn test_filter_post_context_view() {
        let mut val = json!({
            "id": 1,
            "title": {"rendered": "Hello"},
            "content": {"rendered": "<p>Body</p>"},
            "status": "publish"
        });
        let original = val.clone();
        filter_post_context(&mut val, RestContext::View);
        // View context should not modify anything
        assert_eq!(val, original);
    }

    #[test]
    fn test_filter_user_context_embed() {
        let mut val = json!({
            "id": 1,
            "name": "admin",
            "url": "http://example.com",
            "description": "Site admin",
            "link": "http://example.com/author/admin",
            "slug": "admin",
            "avatar_urls": {"24": "https://gravatar.com/24"},
            "meta": [],
            "_links": {},
            "email": "admin@example.com",
            "registered_date": "2025-01-01T00:00:00",
            "roles": ["administrator"]
        });
        filter_user_context(&mut val, RestContext::Embed);
        let obj = val.as_object().unwrap();
        assert!(obj.contains_key("id"));
        assert!(obj.contains_key("name"));
        assert!(obj.contains_key("url"));
        assert!(obj.contains_key("description"));
        assert!(obj.contains_key("link"));
        assert!(obj.contains_key("slug"));
        assert!(obj.contains_key("avatar_urls"));
        assert!(obj.contains_key("_links"));
        // Should remove these
        assert!(!obj.contains_key("email"));
        assert!(!obj.contains_key("registered_date"));
        assert!(!obj.contains_key("roles"));
        assert!(!obj.contains_key("meta"));
    }

    #[test]
    fn test_filter_user_context_view() {
        let mut val = json!({
            "id": 1,
            "name": "admin",
            "email": "admin@example.com",
            "registered_date": "2025-01-01T00:00:00",
            "roles": ["administrator"],
            "capabilities": {}
        });
        filter_user_context(&mut val, RestContext::View);
        let obj = val.as_object().unwrap();
        assert!(obj.contains_key("id"));
        assert!(obj.contains_key("name"));
        // View removes edit-only fields
        assert!(!obj.contains_key("email"));
        assert!(!obj.contains_key("registered_date"));
        assert!(!obj.contains_key("roles"));
        assert!(!obj.contains_key("capabilities"));
    }

    #[test]
    fn test_filter_user_context_edit() {
        let mut val = json!({
            "id": 1,
            "name": "admin",
            "email": "admin@example.com",
            "roles": ["administrator"]
        });
        let original = val.clone();
        filter_user_context(&mut val, RestContext::Edit);
        // Edit should not remove anything
        assert_eq!(val, original);
    }

    #[test]
    fn test_filter_term_context_embed() {
        let mut val = json!({
            "id": 5,
            "count": 10,
            "description": "Test category",
            "link": "http://example.com/category/test",
            "name": "Test",
            "slug": "test",
            "taxonomy": "category",
            "parent": 0,
            "meta": [],
            "_links": {}
        });
        filter_term_context(&mut val, RestContext::Embed);
        let obj = val.as_object().unwrap();
        assert!(obj.contains_key("id"));
        assert!(obj.contains_key("link"));
        assert!(obj.contains_key("name"));
        assert!(obj.contains_key("slug"));
        assert!(obj.contains_key("taxonomy"));
        assert!(obj.contains_key("_links"));
        assert!(!obj.contains_key("count"));
        assert!(!obj.contains_key("description"));
        assert!(!obj.contains_key("parent"));
        assert!(!obj.contains_key("meta"));
    }

    #[test]
    fn test_filter_term_context_view() {
        let mut val = json!({
            "id": 5,
            "count": 10,
            "name": "Test",
            "slug": "test"
        });
        let original = val.clone();
        filter_term_context(&mut val, RestContext::View);
        assert_eq!(val, original);
    }

    #[test]
    fn test_filter_comment_context_embed() {
        let mut val = json!({
            "id": 1,
            "post": 10,
            "parent": 0,
            "author": 1,
            "author_name": "Admin",
            "author_email": "admin@example.com",
            "author_url": "http://example.com",
            "date": "2025-01-01T00:00:00",
            "content": {"rendered": "<p>Great post!</p>"},
            "status": "approved",
            "type": "comment",
            "link": "http://example.com/post#comment-1",
            "_links": {}
        });
        filter_comment_context(&mut val, RestContext::Embed);
        let obj = val.as_object().unwrap();
        assert!(obj.contains_key("id"));
        assert!(obj.contains_key("author"));
        assert!(obj.contains_key("author_name"));
        assert!(obj.contains_key("author_url"));
        assert!(obj.contains_key("date"));
        assert!(obj.contains_key("content"));
        assert!(obj.contains_key("type"));
        assert!(obj.contains_key("_links"));
        assert!(!obj.contains_key("post"));
        assert!(!obj.contains_key("parent"));
        assert!(!obj.contains_key("author_email"));
        assert!(!obj.contains_key("status"));
    }

    #[test]
    fn test_filter_media_context_embed() {
        let mut val = json!({
            "id": 7,
            "date": "2025-01-01T00:00:00",
            "slug": "test-image",
            "title": {"rendered": "Test Image"},
            "author": 1,
            "source_url": "http://example.com/test.jpg",
            "mime_type": "image/jpeg",
            "media_type": "image",
            "media_details": {},
            "_links": {}
        });
        filter_media_context(&mut val, RestContext::Embed);
        let obj = val.as_object().unwrap();
        assert!(obj.contains_key("id"));
        assert!(obj.contains_key("date"));
        assert!(obj.contains_key("slug"));
        assert!(obj.contains_key("title"));
        assert!(obj.contains_key("author"));
        assert!(obj.contains_key("source_url"));
        assert!(obj.contains_key("_links"));
        assert!(!obj.contains_key("mime_type"));
        assert!(!obj.contains_key("media_type"));
        assert!(!obj.contains_key("media_details"));
    }

    #[test]
    fn test_filter_media_context_edit() {
        let mut val = json!({
            "id": 7,
            "title": {"rendered": "Test Image"},
            "caption": {"rendered": "A caption"},
            "description": {"rendered": "A description"}
        });
        filter_media_context(&mut val, RestContext::Edit);
        let title = val.get("title").unwrap().as_object().unwrap();
        assert!(title.contains_key("raw"));
        assert_eq!(title.get("raw").unwrap(), "Test Image");
        let caption = val.get("caption").unwrap().as_object().unwrap();
        assert!(caption.contains_key("raw"));
    }

    #[test]
    fn test_apply_context_to_array() {
        let mut items = vec![
            json!({
                "id": 1,
                "date": "2025-01-01",
                "slug": "a",
                "link": "http://example.com/a",
                "title": {"rendered": "A"},
                "excerpt": {"rendered": "Ex A"},
                "author": 1,
                "featured_media": 0,
                "status": "publish",
                "_links": {}
            }),
            json!({
                "id": 2,
                "date": "2025-01-02",
                "slug": "b",
                "link": "http://example.com/b",
                "title": {"rendered": "B"},
                "excerpt": {"rendered": "Ex B"},
                "author": 1,
                "featured_media": 0,
                "status": "draft",
                "_links": {}
            }),
        ];
        apply_context_to_array(&mut items, RestContext::Embed, filter_post_context);
        for item in &items {
            let obj = item.as_object().unwrap();
            assert!(obj.contains_key("id"));
            assert!(obj.contains_key("title"));
            assert!(!obj.contains_key("status"));
        }
    }

    #[test]
    fn test_apply_context_to_array_view_noop() {
        let mut items = vec![json!({"id": 1, "status": "publish"})];
        let original = items.clone();
        apply_context_to_array(&mut items, RestContext::View, filter_post_context);
        assert_eq!(items, original);
    }
}
