//! WordPress oEmbed provider endpoint.
//!
//! Implements `GET /wp-json/oembed/1.0/embed?url={url}` which returns
//! oEmbed JSON (or XML) for posts and pages hosted on this site.
//! This allows external sites to embed RustPress content.
//!
//! Reference: <https://developer.wordpress.org/reference/functions/get_oembed_response_data/>

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::Deserialize;
use serde_json::{json, Value};

use rustpress_db::entities::wp_posts;

use crate::ApiState;

#[derive(Debug, Deserialize)]
pub struct OEmbedQuery {
    /// The URL to retrieve oEmbed data for.
    pub url: String,
    /// Response format: "json" (default) or "xml".
    pub format: Option<String>,
    /// Maximum width of the embed in pixels (default 600).
    #[serde(rename = "maxwidth")]
    pub max_width: Option<u32>,
    /// Maximum height of the embed in pixels.
    #[serde(rename = "maxheight")]
    pub max_height: Option<u32>,
}

pub fn routes() -> Router<ApiState> {
    Router::new()
        .route("/wp-json/oembed/1.0/embed", get(oembed_handler))
        .route("/wp-json/oembed/1.0/embed/", get(oembed_handler))
        .route("/wp-json/oembed/1.0", get(oembed_discovery))
}

/// oEmbed provider endpoint.
///
/// WordPress equivalent: `wp-includes/rest-api/endpoints/class-wp-rest-oembed-controller.php`
async fn oembed_handler(
    State(state): State<ApiState>,
    Query(query): Query<OEmbedQuery>,
) -> Response {
    let format = query.format.as_deref().unwrap_or("json");
    let max_width = query.max_width.unwrap_or(600).min(1200).max(200);

    // Extract slug from the URL
    let slug = extract_slug_from_url(&query.url, &state.site_url);
    let slug = match slug {
        Some(s) => s,
        None => {
            return (StatusCode::NOT_FOUND, Json(json!({
                "code": "oembed_invalid_url",
                "message": "Not Found",
                "data": {"status": 404}
            }))).into_response();
        }
    };

    // Find the post/page by slug
    let post = wp_posts::Entity::find()
        .filter(wp_posts::Column::PostName.eq(&slug))
        .filter(wp_posts::Column::PostStatus.eq("publish"))
        .one(&state.db)
        .await
        .ok()
        .flatten();

    let post = match post {
        Some(p) => p,
        None => {
            return (StatusCode::NOT_FOUND, Json(json!({
                "code": "oembed_invalid_url",
                "message": "Not Found",
                "data": {"status": 404}
            }))).into_response();
        }
    };

    // Don't serve oEmbed for password-protected posts
    if !post.post_password.is_empty() {
        return (StatusCode::FORBIDDEN, Json(json!({
            "code": "oembed_forbidden",
            "message": "Forbidden",
            "data": {"status": 403}
        }))).into_response();
    }

    let site_name = "RustPress"; // Could be fetched from options
    let post_url = format!("{}/{}", state.site_url.trim_end_matches('/'), post.post_name);

    // Build excerpt (strip HTML, limit to 150 chars)
    let excerpt = if !post.post_excerpt.is_empty() {
        post.post_excerpt.clone()
    } else {
        strip_html(&post.post_content)
    };
    let _excerpt = if excerpt.len() > 150 {
        format!("{}…", &excerpt[..150])
    } else {
        excerpt
    };

    // Build the embedded HTML (an iframe pointing to the embed endpoint)
    let embed_url = format!("{}?embed=true", post_url);
    let height = query.max_height.unwrap_or((max_width as f64 * 0.5625) as u32).max(200);

    let html = format!(
        r#"<blockquote class="wp-embedded-content" data-secret="rs{post_id}"><a href="{url}">{title}</a></blockquote><iframe sandbox="allow-scripts" security="restricted" src="{embed_url}" width="{width}" height="{height}" title="{title_attr}" frameborder="0" marginwidth="0" marginheight="0" scrolling="no" class="wp-embedded-content"></iframe>"#,
        post_id = post.id,
        url = post_url,
        title = html_escape(&post.post_title),
        embed_url = embed_url,
        width = max_width,
        height = height,
        title_attr = html_escape(&post.post_title),
    );

    let response_data = json!({
        "version": "1.0",
        "provider_name": site_name,
        "provider_url": state.site_url,
        "author_name": "",
        "author_url": "",
        "title": post.post_title,
        "type": "rich",
        "width": max_width,
        "height": height,
        "html": html,
    });

    if format == "xml" {
        let xml = oembed_to_xml(&response_data);
        (
            StatusCode::OK,
            [("content-type", "text/xml; charset=UTF-8")],
            xml,
        ).into_response()
    } else {
        Json(response_data).into_response()
    }
}

/// oEmbed discovery endpoint (lists capabilities).
async fn oembed_discovery(State(_state): State<ApiState>) -> Json<Value> {
    Json(json!({
        "namespace": "oembed/1.0",
        "routes": {
            "/oembed/1.0/embed": {
                "namespace": "oembed/1.0",
                "methods": ["GET"],
                "endpoints": [{
                    "methods": ["GET"],
                    "args": {
                        "url": {"required": true, "type": "string"},
                        "format": {"type": "string", "default": "json"},
                        "maxwidth": {"type": "integer", "default": 600},
                        "maxheight": {"type": "integer"}
                    }
                }]
            },
            "/oembed/1.0/proxy": {
                "namespace": "oembed/1.0",
                "methods": ["GET"]
            }
        }
    }))
}

/// Extract slug from a URL like `http://example.com/my-post` → `my-post`.
fn extract_slug_from_url(url: &str, site_url: &str) -> Option<String> {
    let base = site_url.trim_end_matches('/');
    let path = if url.starts_with(base) {
        &url[base.len()..]
    } else {
        // Try stripping protocol differences
        let url_path = url.find("://").map(|i| &url[i + 3..]).unwrap_or(url);
        let base_path = base.find("://").map(|i| &base[i + 3..]).unwrap_or(base);
        if url_path.starts_with(base_path) {
            &url_path[base_path.len()..]
        } else {
            return None;
        }
    };

    let path = path.trim_matches('/');
    if path.is_empty() {
        return None;
    }

    // Handle paths like /2025/01/my-post or just /my-post
    // Take the last path segment as the slug
    let slug = path.rsplit('/').next().unwrap_or(path);
    // Remove query params
    let slug = slug.split('?').next().unwrap_or(slug);
    let slug = slug.split('#').next().unwrap_or(slug);

    if slug.is_empty() {
        None
    } else {
        Some(slug.to_string())
    }
}

fn strip_html(html: &str) -> String {
    html.chars()
        .fold((String::new(), false), |(mut acc, in_tag), c| {
            if c == '<' {
                (acc, true)
            } else if c == '>' {
                (acc, false)
            } else if !in_tag {
                acc.push(c);
                (acc, false)
            } else {
                (acc, true)
            }
        })
        .0
        .trim()
        .to_string()
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn oembed_to_xml(data: &Value) -> String {
    let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<oembed>\n");
    if let Some(obj) = data.as_object() {
        for (key, val) in obj {
            let text = match val {
                Value::String(s) => s.clone(),
                Value::Number(n) => n.to_string(),
                _ => val.to_string(),
            };
            xml.push_str(&format!("\t<{}>{}</{}>\n", key, html_escape(&text), key));
        }
    }
    xml.push_str("</oembed>");
    xml
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_slug_simple() {
        let slug = extract_slug_from_url("http://localhost:8080/hello-world", "http://localhost:8080");
        assert_eq!(slug, Some("hello-world".to_string()));
    }

    #[test]
    fn test_extract_slug_with_trailing_slash() {
        let slug = extract_slug_from_url("http://localhost:8080/hello-world/", "http://localhost:8080");
        assert_eq!(slug, Some("hello-world".to_string()));
    }

    #[test]
    fn test_extract_slug_date_permalink() {
        let slug = extract_slug_from_url("http://localhost:8080/2025/01/my-post", "http://localhost:8080");
        assert_eq!(slug, Some("my-post".to_string()));
    }

    #[test]
    fn test_extract_slug_homepage() {
        let slug = extract_slug_from_url("http://localhost:8080/", "http://localhost:8080");
        assert_eq!(slug, None);
    }

    #[test]
    fn test_extract_slug_query_params() {
        let slug = extract_slug_from_url("http://localhost:8080/hello?p=1", "http://localhost:8080");
        assert_eq!(slug, Some("hello".to_string()));
    }

    #[test]
    fn test_strip_html() {
        assert_eq!(strip_html("<p>Hello <strong>world</strong></p>"), "Hello world");
    }

    #[test]
    fn test_html_escape() {
        assert_eq!(html_escape("a & b < c"), "a &amp; b &lt; c");
    }

    #[test]
    fn test_oembed_to_xml() {
        let data = json!({"version": "1.0", "type": "rich"});
        let xml = oembed_to_xml(&data);
        assert!(xml.contains("<version>1.0</version>"));
        assert!(xml.contains("<type>rich</type>"));
        assert!(xml.starts_with("<?xml"));
    }
}
