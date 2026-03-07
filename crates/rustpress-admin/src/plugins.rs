use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::post,
    Json, Router,
};
use serde::Serialize;

use crate::AdminState;

#[derive(Debug, Serialize)]
pub struct PluginActionResponse {
    pub ok: bool,
    pub name: String,
    pub status: String,
    pub message: String,
}

pub fn routes() -> Router<AdminState> {
    Router::new()
        .route("/admin/plugins/{name}/activate", post(activate_plugin))
        .route("/admin/plugins/{name}/deactivate", post(deactivate_plugin))
}

async fn activate_plugin(
    State(state): State<AdminState>,
    Path(name): Path<String>,
) -> Result<Json<PluginActionResponse>, (StatusCode, String)> {
    state
        .plugin_registry
        .activate(&name)
        .await
        .map_err(|e| (StatusCode::NOT_FOUND, e))?;

    Ok(Json(PluginActionResponse {
        ok: true,
        name: name.clone(),
        status: "Active".to_string(),
        message: format!("Plugin '{name}' activated successfully."),
    }))
}

async fn deactivate_plugin(
    State(state): State<AdminState>,
    Path(name): Path<String>,
) -> Result<Json<PluginActionResponse>, (StatusCode, String)> {
    state
        .plugin_registry
        .deactivate(&name)
        .await
        .map_err(|e| (StatusCode::NOT_FOUND, e))?;

    Ok(Json(PluginActionResponse {
        ok: true,
        name: name.clone(),
        status: "Inactive".to_string(),
        message: format!("Plugin '{name}' deactivated successfully."),
    }))
}
