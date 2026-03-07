//! WordPress Global Styles REST API
//!
//! GET /wp-json/wp/v2/global-styles/{id}
//! GET /wp-json/wp/v2/global-styles/themes/{stylesheet}
//! GET /wp-json/wp/v2/global-styles/{id}/revisions
//! GET /wp-json/wp/v2/global-styles/{id}/revisions/{revision_id}
//!
//! Returns theme global styles (theme.json).

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde_json::{json, Value};

use rustpress_db::entities::wp_options;

use crate::common::WpError;
use crate::ApiState;
use crate::AuthUser;

pub fn routes() -> Router<ApiState> {
    Router::new()
        .route(
            "/wp-json/wp/v2/global-styles/themes/{stylesheet}",
            get(get_theme_global_styles),
        )
        .route("/wp-json/wp/v2/global-styles/{id}", get(get_global_styles))
        .route(
            "/wp-json/wp/v2/global-styles/{id}/revisions",
            get(list_global_styles_revisions),
        )
        .route(
            "/wp-json/wp/v2/global-styles/{id}/revisions/{revision_id}",
            get(get_global_styles_revision),
        )
}

pub fn write_routes() -> Router<ApiState> {
    Router::new().route(
        "/wp-json/wp/v2/global-styles/{id}",
        axum::routing::put(update_global_styles).patch(update_global_styles),
    )
}

fn default_global_styles(id: &str, stylesheet: &str) -> Value {
    json!({
        "id": id,
        "title": {
            "raw": "Custom Styles",
            "rendered": "Custom Styles"
        },
        "settings": {
            "color": {
                "background": true,
                "custom": true,
                "customDuotone": true,
                "customGradient": true,
                "defaultGradients": true,
                "defaultPalette": true,
                "link": false,
                "text": true
            },
            "typography": {
                "customFontSize": true,
                "lineHeight": false
            },
            "spacing": {
                "blockGap": null,
                "margin": false,
                "padding": false
            },
            "layout": {
                "contentSize": "800px",
                "wideSize": "1200px"
            }
        },
        "styles": {},
        "stylesheet": stylesheet,
        "_links": {
            "self": [{"href": format!("/wp-json/wp/v2/global-styles/{}", id)}],
            "about": [{"href": format!("/wp-json/wp/v2/types/wp_global_styles")}]
        }
    })
}

/// GET /wp-json/wp/v2/global-styles/themes/{stylesheet}
async fn get_theme_global_styles(
    State(state): State<ApiState>,
    Path(stylesheet): Path<String>,
) -> Result<Json<Value>, WpError> {
    let id = format!("wp-global-styles-{stylesheet}");
    let base = state.site_url.trim_end_matches('/').to_string();
    let mut gs = default_global_styles(&id, &stylesheet);
    if let Some(obj) = gs.as_object_mut() {
        obj["_links"] = json!({
            "self": [{"href": format!("{}/wp-json/wp/v2/global-styles/{}", base, id)}],
            "about": [{"href": format!("{}/wp-json/wp/v2/types/wp_global_styles", base)}]
        });
    }
    Ok(Json(gs))
}

/// GET /wp-json/wp/v2/global-styles/{id}
async fn get_global_styles(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, WpError> {
    // Extract stylesheet from id like "wp-global-styles-twentytwentyfour"
    let stylesheet = id
        .strip_prefix("wp-global-styles-")
        .unwrap_or(&id)
        .to_string();

    let base = state.site_url.trim_end_matches('/').to_string();
    let mut gs = default_global_styles(&id, &stylesheet);
    if let Some(obj) = gs.as_object_mut() {
        obj["_links"] = json!({
            "self": [{"href": format!("{}/wp-json/wp/v2/global-styles/{}", base, id)}],
            "about": [{"href": format!("{}/wp-json/wp/v2/types/wp_global_styles", base)}]
        });
    }
    Ok(Json(gs))
}

/// GET /wp-json/wp/v2/global-styles/{id}/revisions
async fn list_global_styles_revisions(
    State(_state): State<ApiState>,
    Path(_id): Path<String>,
) -> Result<Json<Vec<Value>>, WpError> {
    // No revisions stored for global styles
    Ok(Json(vec![]))
}

/// GET /wp-json/wp/v2/global-styles/{id}/revisions/{revision_id}
async fn get_global_styles_revision(
    State(_state): State<ApiState>,
    Path((_id, _revision_id)): Path<(String, String)>,
) -> Result<Json<Value>, WpError> {
    Err(WpError::new(
        StatusCode::NOT_FOUND,
        "rest_post_invalid_id",
        "Invalid revision ID.",
    ))
}

/// PUT/PATCH /wp-json/wp/v2/global-styles/{id}
/// Updates global styles (theme.json customizations) stored in wp_options.
async fn update_global_styles(
    State(state): State<ApiState>,
    _auth: AuthUser,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, WpError> {
    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::Set;

    let option_key = format!("wp_global_styles_{id}");
    let serialized = serde_json::to_string(&body).unwrap_or_else(|_| "{}".to_string());

    // Upsert into wp_options
    let existing = wp_options::Entity::find()
        .filter(wp_options::Column::OptionName.eq(&option_key))
        .one(&state.db)
        .await
        .map_err(|e| WpError::internal(e.to_string()))?;

    if let Some(opt) = existing {
        let mut active: wp_options::ActiveModel = opt.into();
        active.option_value = Set(serialized);
        active
            .update(&state.db)
            .await
            .map_err(|e| WpError::internal(e.to_string()))?;
    } else {
        let new_opt = wp_options::ActiveModel {
            option_name: Set(option_key),
            option_value: Set(serialized),
            autoload: Set("no".to_string()),
            ..Default::default()
        };
        new_opt
            .insert(&state.db)
            .await
            .map_err(|e| WpError::internal(e.to_string()))?;
    }

    // Return the updated global styles
    let stylesheet = id
        .strip_prefix("wp-global-styles-")
        .unwrap_or(&id)
        .to_string();
    let base = state.site_url.trim_end_matches('/').to_string();
    let mut gs = default_global_styles(&id, &stylesheet);

    // Merge submitted settings/styles into response
    if let (Some(gs_obj), Some(body_obj)) = (gs.as_object_mut(), body.as_object()) {
        if let Some(settings) = body_obj.get("settings") {
            gs_obj.insert("settings".to_string(), settings.clone());
        }
        if let Some(styles) = body_obj.get("styles") {
            gs_obj.insert("styles".to_string(), styles.clone());
        }
        gs_obj["_links"] = json!({
            "self": [{"href": format!("{}/wp-json/wp/v2/global-styles/{}", base, id)}],
            "about": [{"href": format!("{}/wp-json/wp/v2/types/wp_global_styles", base)}]
        });
    }

    Ok(Json(gs))
}
