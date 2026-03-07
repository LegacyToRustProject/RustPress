//! WordPress REST API — Themes endpoint.
//!
//! `GET  /wp-json/wp/v2/themes`         — list installed themes
//! `GET  /wp-json/wp/v2/themes/{slug}`  — get single theme

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
        .route("/wp-json/wp/v2/themes", get(list_themes))
        .route("/wp-json/wp/v2/themes/{slug}", get(get_theme))
}

/// List installed themes.
///
/// WordPress equivalent: `GET /wp-json/wp/v2/themes`
async fn list_themes(State(state): State<ApiState>) -> Json<Vec<Value>> {
    // Read active theme from options
    let active_theme = get_option(&state, "template")
        .await
        .unwrap_or_else(|| "default".to_string());

    let themes = vec![theme_json(
        "default",
        "RustPress Default",
        "1.0",
        true,
        &active_theme,
        &state.site_url,
    )];

    Json(themes)
}

/// Get a single theme by stylesheet slug.
async fn get_theme(
    State(state): State<ApiState>,
    Path(slug): Path<String>,
) -> Result<Json<Value>, WpError> {
    let active_theme = get_option(&state, "template")
        .await
        .unwrap_or_else(|| "default".to_string());

    if slug == "default" {
        Ok(Json(theme_json(
            "default",
            "RustPress Default",
            "1.0",
            true,
            &active_theme,
            &state.site_url,
        )))
    } else {
        Err(WpError::not_found("Theme not found"))
    }
}

fn theme_json(
    slug: &str,
    name: &str,
    version: &str,
    _active: bool,
    active_theme: &str,
    site_url: &str,
) -> Value {
    let base = site_url.trim_end_matches('/');
    let status = if slug == active_theme {
        "active"
    } else {
        "inactive"
    };
    json!({
        "stylesheet": slug,
        "template": slug,
        "requires_php": "7.0",
        "textdomain": slug,
        "version": version,
        "screenshot": format!("{}/wp-content/themes/{}/screenshot.png", base, slug),
        "author": {"raw": "RustPress", "rendered": "RustPress"},
        "author_uri": {"raw": base, "rendered": base},
        "description": {"raw": format!("{} theme", name), "rendered": format!("{} theme", name)},
        "name": {"raw": name, "rendered": name},
        "tags": {"raw": [], "rendered": ""},
        "theme_uri": {"raw": base, "rendered": base},
        "status": status,
        "theme_supports": {
            "align-wide": true,
            "responsive-embeds": true,
            "editor-styles": true,
            "wp-block-styles": true,
            "editor-color-palette": [],
            "editor-font-sizes": [],
            "color-palette": [],
            "custom-line-height": false,
            "custom-spacing": false,
            "custom-units": []
        },
        "_links": {
            "self": [{"href": format!("{}/wp-json/wp/v2/themes/{}", base, slug)}],
            "collection": [{"href": format!("{}/wp-json/wp/v2/themes", base)}],
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
