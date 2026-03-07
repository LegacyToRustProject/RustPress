//! WordPress REST API — Widgets & Sidebars endpoints.
//!
//! Endpoints:
//! - `GET  /wp-json/wp/v2/sidebars`         — list registered sidebars
//! - `GET  /wp-json/wp/v2/sidebars/{id}`     — get single sidebar
//! - `GET  /wp-json/wp/v2/widgets`           — list widgets
//! - `GET  /wp-json/wp/v2/widgets/{id}`      — get single widget

use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use serde_json::{json, Value};

use crate::common::WpError;
use crate::ApiState;

pub fn sidebar_routes() -> Router<ApiState> {
    Router::new()
        .route("/wp-json/wp/v2/sidebars", get(list_sidebars))
        .route("/wp-json/wp/v2/sidebars/{id}", get(get_sidebar))
}

pub fn widget_routes() -> Router<ApiState> {
    Router::new()
        .route("/wp-json/wp/v2/widgets", get(list_widgets))
        .route("/wp-json/wp/v2/widgets/{id}", get(get_widget))
}

/// Registered sidebar definitions.
const SIDEBARS: &[(&str, &str, &str)] = &[
    (
        "sidebar-1",
        "Main Sidebar",
        "Add widgets here to appear in your sidebar.",
    ),
    (
        "footer-1",
        "Footer 1",
        "Add widgets here to appear in footer column 1.",
    ),
    (
        "footer-2",
        "Footer 2",
        "Add widgets here to appear in footer column 2.",
    ),
];

/// List all registered widget areas (sidebars).
async fn list_sidebars(State(state): State<ApiState>) -> Json<Vec<Value>> {
    let config = load_widget_config(&state).await;
    let sidebars: Vec<Value> = SIDEBARS
        .iter()
        .map(|(id, name, desc)| sidebar_json(id, name, desc, &config, &state.site_url))
        .collect();
    Json(sidebars)
}

/// Get a single sidebar by ID.
async fn get_sidebar(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, WpError> {
    let config = load_widget_config(&state).await;
    SIDEBARS
        .iter()
        .find(|(sid, _, _)| *sid == id.as_str())
        .map(|(id, name, desc)| Json(sidebar_json(id, name, desc, &config, &state.site_url)))
        .ok_or_else(|| WpError::not_found("Sidebar not found"))
}

/// List all widgets across all sidebars.
async fn list_widgets(State(state): State<ApiState>) -> Json<Vec<Value>> {
    let config = load_widget_config(&state).await;
    let widgets = parse_widgets_from_config(&config, &state.site_url);
    Json(widgets)
}

/// Get a single widget by ID.
async fn get_widget(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, WpError> {
    let config = load_widget_config(&state).await;
    let widgets = parse_widgets_from_config(&config, &state.site_url);
    widgets
        .into_iter()
        .find(|w| w.get("id").and_then(|v| v.as_str()) == Some(id.as_str()))
        .map(Json)
        .ok_or_else(|| WpError::not_found("Widget not found"))
}

// ---- Helpers ----

fn sidebar_json(id: &str, name: &str, description: &str, config: &Value, site_url: &str) -> Value {
    let base = site_url.trim_end_matches('/');

    // Count widgets in this sidebar
    let widgets: Vec<&str> = config
        .get(id)
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|w| w.get("type").and_then(|t| t.as_str()))
                .collect()
        })
        .unwrap_or_default();

    json!({
        "id": id,
        "name": name,
        "description": description,
        "class": "",
        "before_widget": "",
        "after_widget": "",
        "before_title": "<h2 class=\"widget-title\">",
        "after_title": "</h2>",
        "status": if widgets.is_empty() { "inactive" } else { "active" },
        "widgets": widgets,
        "_links": {
            "self": [{"href": format!("{}/wp-json/wp/v2/sidebars/{}", base, id)}],
            "collection": [{"href": format!("{}/wp-json/wp/v2/sidebars", base)}],
            "curies": [{"name": "wp", "href": "https://api.w.org/{rel}", "templated": true}]
        }
    })
}

fn parse_widgets_from_config(config: &Value, site_url: &str) -> Vec<Value> {
    let base = site_url.trim_end_matches('/');
    let mut widgets = Vec::new();
    let mut idx = 0u32;

    if let Some(obj) = config.as_object() {
        for (sidebar_id, area) in obj {
            if let Some(arr) = area.as_array() {
                for w in arr {
                    let widget_type = w.get("type").and_then(|t| t.as_str()).unwrap_or("unknown");
                    let _title = w.get("title").and_then(|t| t.as_str()).unwrap_or("");
                    let id = format!("{}-{}", widget_type.to_lowercase().replace(' ', "_"), idx);

                    widgets.push(json!({
                        "id": id,
                        "id_base": widget_type.to_lowercase().replace(' ', "_"),
                        "sidebar": sidebar_id,
                        "rendered": "",
                        "instance": {
                            "raw": w,
                            "encoded": ""
                        },
                        "form_data": "",
                        "_links": {
                            "self": [{"href": format!("{}/wp-json/wp/v2/widgets/{}", base, id)}],
                            "collection": [{"href": format!("{}/wp-json/wp/v2/widgets", base)}],
                            "about": [{"href": format!("{}/wp-json/wp/v2/sidebars/{}", base, sidebar_id)}],
                            "curies": [{"name": "wp", "href": "https://api.w.org/{rel}", "templated": true}]
                        }
                    }));
                    idx += 1;
                }
            }
        }
    }
    widgets
}

async fn load_widget_config(state: &ApiState) -> Value {
    use rustpress_db::entities::wp_options;
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

    let config_str = wp_options::Entity::find()
        .filter(wp_options::Column::OptionName.eq("widget_config"))
        .one(&state.db)
        .await
        .ok()
        .flatten()
        .map(|o| o.option_value.clone())
        .unwrap_or_else(|| "{}".to_string());

    serde_json::from_str(&config_str).unwrap_or_else(|_| json!({}))
}
