use axum::{extract::State, routing::get, Json, Router};
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Instant;

use crate::state::AppState;

// Store server start time
static START_TIME: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();

pub fn init_start_time() {
    START_TIME.get_or_init(Instant::now);
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/health", get(health_check))
}

async fn health_check(
    State(state): State<Arc<AppState>>,
) -> (axum::http::StatusCode, Json<Value>) {
    let uptime = START_TIME.get().map(|t| t.elapsed().as_secs()).unwrap_or(0);

    // Check DB connectivity
    use sea_orm::ConnectionTrait;
    let db_ok = state
        .db
        .execute(sea_orm::Statement::from_string(
            sea_orm::DatabaseBackend::MySql,
            "SELECT 1".to_string(),
        ))
        .await
        .is_ok();

    let status_code = if db_ok {
        axum::http::StatusCode::OK
    } else {
        axum::http::StatusCode::SERVICE_UNAVAILABLE
    };

    (
        status_code,
        Json(json!({
            "status": if db_ok { "healthy" } else { "degraded" },
            "version": env!("CARGO_PKG_VERSION"),
            "uptime_seconds": uptime,
            "db_connected": db_ok,
        })),
    )
}
