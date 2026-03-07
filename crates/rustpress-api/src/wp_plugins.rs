//! WordPress REST API — Plugins endpoint.
//!
//! `GET  /wp-json/wp/v2/plugins`         — list installed plugins
//! `GET  /wp-json/wp/v2/plugins/{slug}`  — get single plugin

use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use serde_json::{json, Value};

use crate::common::WpError;
use crate::ApiState;

pub fn routes() -> Router<ApiState> {
    Router::new()
        .route("/wp-json/wp/v2/plugins", get(list_plugins))
        .route("/wp-json/wp/v2/plugins/{slug}", get(get_plugin))
}

/// List installed plugins.
///
/// WordPress stores active plugins in `active_plugins` option.
async fn list_plugins(State(state): State<ApiState>) -> Json<Vec<Value>> {
    let active_list = get_option(&state, "active_plugins")
        .await
        .unwrap_or_default();
    let active_slugs: Vec<&str> = active_list
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    // Return a built-in "hello" plugin like WordPress ships with
    let plugins = vec![plugin_json(
        "hello-rustpress",
        "Hello RustPress",
        "1.0",
        "A demo plugin.",
        active_slugs.contains(&"hello-rustpress"),
        &state.site_url,
    )];

    Json(plugins)
}

/// Get a single plugin by slug.
async fn get_plugin(
    State(state): State<ApiState>,
    Path(slug): Path<String>,
) -> Result<Json<Value>, WpError> {
    let active_list = get_option(&state, "active_plugins")
        .await
        .unwrap_or_default();
    let is_active = active_list.split(',').any(|s| s.trim() == slug);

    if slug == "hello-rustpress" {
        Ok(Json(plugin_json(
            "hello-rustpress",
            "Hello RustPress",
            "1.0",
            "A demo plugin.",
            is_active,
            &state.site_url,
        )))
    } else {
        Err(WpError::not_found("Plugin not found"))
    }
}

fn plugin_json(
    slug: &str,
    name: &str,
    version: &str,
    description: &str,
    active: bool,
    site_url: &str,
) -> Value {
    let base = site_url.trim_end_matches('/');
    let status = if active { "active" } else { "inactive" };
    json!({
        "plugin": format!("{}/{}.php", slug, slug),
        "status": status,
        "name": name,
        "plugin_uri": "",
        "author": "RustPress",
        "author_uri": base,
        "description": {"raw": description, "rendered": description},
        "version": version,
        "network_only": false,
        "requires_wp": "",
        "requires_php": "",
        "textdomain": slug,
        "_links": {
            "self": [{"href": format!("{}/wp-json/wp/v2/plugins/{}", base, slug)}],
            "collection": [{"href": format!("{}/wp-json/wp/v2/plugins", base)}],
            "curies": [{"name": "wp", "href": "https://api.w.org/{rel}", "templated": true}]
        }
    })
}

async fn get_option(state: &ApiState, key: &str) -> Option<String> {
    use rustpress_db::entities::wp_options;
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

    wp_options::Entity::find()
        .filter(wp_options::Column::OptionName.eq(key))
        .one(&state.db)
        .await
        .ok()
        .flatten()
        .and_then(|o| {
            let val = o.option_value.trim().to_string();
            if val.is_empty() {
                None
            } else {
                Some(val)
            }
        })
}
