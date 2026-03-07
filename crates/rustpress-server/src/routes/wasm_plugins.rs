use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::Serialize;
use std::sync::Arc;

use crate::state::AppState;

#[derive(Serialize)]
struct PluginInfo {
    name: String,
}

#[derive(Serialize)]
struct PluginListResponse {
    plugins: Vec<PluginInfo>,
    count: usize,
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/wasm/plugins", get(list_plugins))
        .route(
            "/api/wasm/plugins/{name}/call/{function}",
            post(call_plugin_function),
        )
}

async fn list_plugins(
    State(state): State<Arc<AppState>>,
) -> Json<PluginListResponse> {
    let host = state.wasm_host.read().await;
    let plugin_names = host.loaded_plugins();
    let plugins: Vec<PluginInfo> = plugin_names
        .into_iter()
        .map(|name| PluginInfo {
            name: name.to_string(),
        })
        .collect();
    let count = plugins.len();
    Json(PluginListResponse { plugins, count })
}

async fn call_plugin_function(
    State(state): State<Arc<AppState>>,
    Path((name, function)): Path<(String, String)>,
    body: Option<Json<serde_json::Value>>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let args = body.map(|b| b.0).unwrap_or(serde_json::json!({}));
    let host = state.wasm_host.read().await;
    match host.call_plugin(&name, &function, &args) {
        Ok(result) => Ok(Json(result)),
        Err(e) => {
            let status = match &e {
                rustpress_plugins::wasm_host::WasmError::NotFound(_) => StatusCode::NOT_FOUND,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            };
            Err((
                status,
                Json(serde_json::json!({
                    "error": e.to_string(),
                })),
            ))
        }
    }
}
