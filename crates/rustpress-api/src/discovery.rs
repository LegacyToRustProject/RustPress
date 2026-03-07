use axum::{extract::State, routing::get, Json, Router};
use serde_json::{json, Value};

use crate::ApiState;

pub fn routes() -> Router<ApiState> {
    Router::new()
        .route("/wp-json", get(api_root))
        .route("/wp-json/", get(api_root))
    // block-types routes are registered in block_types module
}

/// WP REST API discovery root - tells clients what namespaces and routes are available.
async fn api_root(State(state): State<ApiState>) -> Json<Value> {
    let mut routes = serde_json::Map::new();

    // Helper to add a simple route entry
    let route =
        |ns: &str, methods: &[&str]| -> Value { json!({"namespace": ns, "methods": methods}) };

    // Route keys must NOT include the /wp-json prefix — clients prepend
    // the API root URL (e.g. http://site/wp-json/) to these paths.
    routes.insert("/wp/v2/posts".into(), route("wp/v2", &["GET", "POST"]));
    routes.insert(
        "/wp/v2/posts/(?P<id>[\\d]+)".into(),
        route("wp/v2", &["GET", "PUT", "DELETE"]),
    );
    routes.insert("/wp/v2/pages".into(), route("wp/v2", &["GET", "POST"]));
    routes.insert(
        "/wp/v2/pages/(?P<id>[\\d]+)".into(),
        route("wp/v2", &["GET", "PUT", "DELETE"]),
    );
    routes.insert("/wp/v2/media".into(), route("wp/v2", &["GET", "POST"]));
    routes.insert(
        "/wp/v2/media/(?P<id>[\\d]+)".into(),
        route("wp/v2", &["GET", "PUT", "DELETE"]),
    );
    routes.insert("/wp/v2/categories".into(), route("wp/v2", &["GET", "POST"]));
    routes.insert(
        "/wp/v2/categories/(?P<id>[\\d]+)".into(),
        route("wp/v2", &["GET", "PUT", "DELETE"]),
    );
    routes.insert("/wp/v2/tags".into(), route("wp/v2", &["GET", "POST"]));
    routes.insert(
        "/wp/v2/tags/(?P<id>[\\d]+)".into(),
        route("wp/v2", &["GET", "PUT", "DELETE"]),
    );
    routes.insert("/wp/v2/users".into(), route("wp/v2", &["GET", "POST"]));
    routes.insert(
        "/wp/v2/users/(?P<id>[\\d]+)".into(),
        route("wp/v2", &["GET", "PUT", "DELETE"]),
    );
    routes.insert("/wp/v2/users/me".into(), route("wp/v2", &["GET"]));
    routes.insert("/wp/v2/comments".into(), route("wp/v2", &["GET", "POST"]));
    routes.insert(
        "/wp/v2/comments/(?P<id>[\\d]+)".into(),
        route("wp/v2", &["GET", "PUT", "DELETE"]),
    );
    routes.insert("/wp/v2/settings".into(), route("wp/v2", &["GET", "PUT"]));
    routes.insert("/wp/v2/types".into(), route("wp/v2", &["GET"]));
    routes.insert(
        "/wp/v2/types/(?P<slug>[\\w-]+)".into(),
        route("wp/v2", &["GET"]),
    );
    routes.insert("/wp/v2/taxonomies".into(), route("wp/v2", &["GET"]));
    routes.insert(
        "/wp/v2/taxonomies/(?P<slug>[\\w-]+)".into(),
        route("wp/v2", &["GET"]),
    );
    routes.insert("/wp/v2/statuses".into(), route("wp/v2", &["GET"]));
    routes.insert("/wp/v2/search".into(), route("wp/v2", &["GET"]));
    routes.insert("/wp/v2/block-types".into(), route("wp/v2", &["GET"]));
    routes.insert("/wp/v2/menus".into(), route("wp/v2", &["GET", "POST"]));
    routes.insert(
        "/wp/v2/menus/(?P<id>[\\d]+)".into(),
        route("wp/v2", &["GET", "PUT", "DELETE"]),
    );
    routes.insert("/wp/v2/menu-items".into(), route("wp/v2", &["GET", "POST"]));
    routes.insert(
        "/wp/v2/menu-items/(?P<id>[\\d]+)".into(),
        route("wp/v2", &["GET", "PUT", "DELETE"]),
    );
    routes.insert("/wp/v2/themes".into(), route("wp/v2", &["GET"]));
    routes.insert(
        "/wp/v2/themes/(?P<slug>[\\w-]+)".into(),
        route("wp/v2", &["GET"]),
    );
    routes.insert("/wp/v2/plugins".into(), route("wp/v2", &["GET"]));
    routes.insert(
        "/wp/v2/plugins/(?P<slug>[\\w-]+)".into(),
        route("wp/v2", &["GET"]),
    );
    routes.insert("/wp/v2/sidebars".into(), route("wp/v2", &["GET"]));
    routes.insert(
        "/wp/v2/sidebars/(?P<id>[\\w-]+)".into(),
        route("wp/v2", &["GET"]),
    );
    routes.insert("/wp/v2/widgets".into(), route("wp/v2", &["GET"]));
    routes.insert(
        "/wp/v2/widgets/(?P<id>[\\w-]+)".into(),
        route("wp/v2", &["GET"]),
    );
    routes.insert(
        "/wp/v2/posts/(?P<id>[\\d]+)/revisions".into(),
        route("wp/v2", &["GET"]),
    );
    routes.insert(
        "/wp/v2/posts/(?P<id>[\\d]+)/autosaves".into(),
        route("wp/v2", &["GET", "POST"]),
    );
    routes.insert(
        "/wp/v2/pages/(?P<id>[\\d]+)/autosaves".into(),
        route("wp/v2", &["GET", "POST"]),
    );
    routes.insert("/oembed/1.0/embed".into(), route("oembed/1.0", &["GET"]));
    routes.insert("/batch/v1".into(), route("", &["POST"]));

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
