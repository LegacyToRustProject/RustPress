use axum::{extract::State, routing::get, Json, Router};
use serde_json::{json, Value};

use crate::ApiState;

pub fn routes() -> Router<ApiState> {
    Router::new()
        .route("/wp-json", get(api_root))
        .route("/wp-json/", get(api_root))
        .route("/wp-json/wp/v2/block-types", get(block_types))
        .route(
            "/wp-json/wp/v2/block-types/{namespace}/{name}",
            get(block_type_single),
        )
}

/// WP REST API discovery root - tells clients what namespaces and routes are available.
async fn api_root(State(state): State<ApiState>) -> Json<Value> {
    let mut routes = serde_json::Map::new();

    // Helper to add a simple route entry
    let route = |ns: &str, methods: &[&str]| -> Value {
        json!({"namespace": ns, "methods": methods})
    };

    routes.insert("/wp-json/wp/v2/posts".into(), route("wp/v2", &["GET", "POST"]));
    routes.insert("/wp-json/wp/v2/posts/(?P<id>[\\d]+)".into(), route("wp/v2", &["GET", "PUT", "DELETE"]));
    routes.insert("/wp-json/wp/v2/pages".into(), route("wp/v2", &["GET", "POST"]));
    routes.insert("/wp-json/wp/v2/pages/(?P<id>[\\d]+)".into(), route("wp/v2", &["GET", "PUT", "DELETE"]));
    routes.insert("/wp-json/wp/v2/media".into(), route("wp/v2", &["GET", "POST"]));
    routes.insert("/wp-json/wp/v2/media/(?P<id>[\\d]+)".into(), route("wp/v2", &["GET", "PUT", "DELETE"]));
    routes.insert("/wp-json/wp/v2/categories".into(), route("wp/v2", &["GET", "POST"]));
    routes.insert("/wp-json/wp/v2/categories/(?P<id>[\\d]+)".into(), route("wp/v2", &["GET", "PUT", "DELETE"]));
    routes.insert("/wp-json/wp/v2/tags".into(), route("wp/v2", &["GET", "POST"]));
    routes.insert("/wp-json/wp/v2/tags/(?P<id>[\\d]+)".into(), route("wp/v2", &["GET", "PUT", "DELETE"]));
    routes.insert("/wp-json/wp/v2/users".into(), route("wp/v2", &["GET", "POST"]));
    routes.insert("/wp-json/wp/v2/users/(?P<id>[\\d]+)".into(), route("wp/v2", &["GET", "PUT", "DELETE"]));
    routes.insert("/wp-json/wp/v2/users/me".into(), route("wp/v2", &["GET"]));
    routes.insert("/wp-json/wp/v2/comments".into(), route("wp/v2", &["GET", "POST"]));
    routes.insert("/wp-json/wp/v2/comments/(?P<id>[\\d]+)".into(), route("wp/v2", &["GET", "PUT", "DELETE"]));
    routes.insert("/wp-json/wp/v2/settings".into(), route("wp/v2", &["GET", "PUT"]));
    routes.insert("/wp-json/wp/v2/types".into(), route("wp/v2", &["GET"]));
    routes.insert("/wp-json/wp/v2/types/(?P<slug>[\\w-]+)".into(), route("wp/v2", &["GET"]));
    routes.insert("/wp-json/wp/v2/taxonomies".into(), route("wp/v2", &["GET"]));
    routes.insert("/wp-json/wp/v2/taxonomies/(?P<slug>[\\w-]+)".into(), route("wp/v2", &["GET"]));
    routes.insert("/wp-json/wp/v2/statuses".into(), route("wp/v2", &["GET"]));
    routes.insert("/wp-json/wp/v2/search".into(), route("wp/v2", &["GET"]));
    routes.insert("/wp-json/wp/v2/block-types".into(), route("wp/v2", &["GET"]));
    routes.insert("/wp-json/wp/v2/menus".into(), route("wp/v2", &["GET", "POST"]));
    routes.insert("/wp-json/wp/v2/menus/(?P<id>[\\d]+)".into(), route("wp/v2", &["GET", "PUT", "DELETE"]));
    routes.insert("/wp-json/wp/v2/menu-items".into(), route("wp/v2", &["GET", "POST"]));
    routes.insert("/wp-json/wp/v2/menu-items/(?P<id>[\\d]+)".into(), route("wp/v2", &["GET", "PUT", "DELETE"]));
    routes.insert("/wp-json/wp/v2/themes".into(), route("wp/v2", &["GET"]));
    routes.insert("/wp-json/wp/v2/themes/(?P<slug>[\\w-]+)".into(), route("wp/v2", &["GET"]));
    routes.insert("/wp-json/wp/v2/plugins".into(), route("wp/v2", &["GET"]));
    routes.insert("/wp-json/wp/v2/plugins/(?P<slug>[\\w-]+)".into(), route("wp/v2", &["GET"]));
    routes.insert("/wp-json/wp/v2/sidebars".into(), route("wp/v2", &["GET"]));
    routes.insert("/wp-json/wp/v2/sidebars/(?P<id>[\\w-]+)".into(), route("wp/v2", &["GET"]));
    routes.insert("/wp-json/wp/v2/widgets".into(), route("wp/v2", &["GET"]));
    routes.insert("/wp-json/wp/v2/widgets/(?P<id>[\\w-]+)".into(), route("wp/v2", &["GET"]));
    routes.insert("/wp-json/wp/v2/posts/(?P<id>[\\d]+)/revisions".into(), route("wp/v2", &["GET"]));
    routes.insert("/wp-json/wp/v2/posts/(?P<id>[\\d]+)/autosaves".into(), route("wp/v2", &["GET", "POST"]));
    routes.insert("/wp-json/wp/v2/pages/(?P<id>[\\d]+)/autosaves".into(), route("wp/v2", &["GET", "POST"]));
    routes.insert("/wp-json/oembed/1.0/embed".into(), route("oembed/1.0", &["GET"]));
    routes.insert("/wp-json/batch/v1".into(), route("", &["POST"]));

    Json(json!({
        "name": "RustPress",
        "description": "WordPress-compatible CMS built in Rust",
        "url": state.site_url,
        "home": state.site_url,
        "gmt_offset": "0",
        "timezone_string": "",
        "namespaces": ["wp/v2", "oembed/1.0"],
        "authentication": {
            "cookie": {
                "name": "rustpress_session"
            }
        },
        "routes": Value::Object(routes)
    }))
}

/// Return registered block types.
/// Gutenberg uses this to know which blocks are available.
async fn block_types() -> Json<Vec<Value>> {
    let blocks = core_block_types();
    Json(blocks)
}

async fn block_type_single(
    axum::extract::Path((namespace, name)): axum::extract::Path<(String, String)>,
) -> Result<Json<Value>, axum::http::StatusCode> {
    let full_name = format!("{}/{}", namespace, name);
    core_block_types()
        .into_iter()
        .find(|b| b["name"].as_str() == Some(&full_name))
        .map(Json)
        .ok_or(axum::http::StatusCode::NOT_FOUND)
}

fn core_block_types() -> Vec<Value> {
    vec![
        block_def("core/paragraph", "Paragraph", "text", &["title", "content"]),
        block_def("core/heading", "Heading", "text", &["title", "content"]),
        block_def("core/image", "Image", "media", &["title"]),
        block_def("core/list", "List", "text", &["title", "content"]),
        block_def("core/list-item", "List Item", "text", &[]),
        block_def("core/quote", "Quote", "text", &["title", "content"]),
        block_def("core/code", "Code", "text", &["title", "content"]),
        block_def("core/preformatted", "Preformatted", "text", &["title", "content"]),
        block_def("core/pullquote", "Pullquote", "text", &["title", "content"]),
        block_def("core/table", "Table", "text", &["title"]),
        block_def("core/verse", "Verse", "text", &["title", "content"]),
        block_def("core/separator", "Separator", "design", &[]),
        block_def("core/spacer", "Spacer", "design", &[]),
        block_def("core/columns", "Columns", "design", &[]),
        block_def("core/column", "Column", "design", &[]),
        block_def("core/group", "Group", "design", &[]),
        block_def("core/buttons", "Buttons", "design", &[]),
        block_def("core/button", "Button", "design", &["title"]),
        block_def("core/cover", "Cover", "media", &["title"]),
        block_def("core/gallery", "Gallery", "media", &["title"]),
        block_def("core/video", "Video", "media", &["title"]),
        block_def("core/audio", "Audio", "media", &["title"]),
        block_def("core/file", "File", "media", &["title"]),
        block_def("core/html", "Custom HTML", "widgets", &["title", "content"]),
        block_def("core/shortcode", "Shortcode", "widgets", &["title"]),
        block_def("core/embed", "Embed", "embed", &["title"]),
        block_def("core/freeform", "Classic", "text", &["title", "content"]),
    ]
}

fn block_def(name: &str, title: &str, category: &str, keywords: &[&str]) -> Value {
    json!({
        "api_version": 3,
        "name": name,
        "title": title,
        "category": category,
        "keywords": keywords,
        "supports": {
            "anchor": true,
            "className": true,
            "html": true
        },
        "is_dynamic": false
    })
}
