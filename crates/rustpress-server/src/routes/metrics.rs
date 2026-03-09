//! GET /metrics — Prometheus text-format exposition endpoint.
//!
//! The Prometheus scrape target should be pointed at this path.
//! The endpoint is intentionally unauthenticated so that Prometheus can reach
//! it without credentials; restrict access at the network layer if needed.

use axum::{
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use std::sync::Arc;

use crate::state::AppState;

/// Axum router that exposes `GET /metrics`.
pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/metrics", get(metrics_handler))
}

async fn metrics_handler() -> Response {
    // Attempt to render the Prometheus text exposition via the global handle
    // that was installed by `telemetry::prometheus_handle()`.  If no handle
    // was installed (e.g. Prometheus recorder not initialised) we return a
    // 503 so the scraper is aware and will retry.
    match PROMETHEUS_HANDLE.get() {
        Some(handle) => {
            let body = handle.render();
            (
                StatusCode::OK,
                [(
                    header::CONTENT_TYPE,
                    "text/plain; version=0.0.4; charset=utf-8",
                )],
                body,
            )
                .into_response()
        }
        None => (
            StatusCode::SERVICE_UNAVAILABLE,
            "Prometheus recorder not initialised",
        )
            .into_response(),
    }
}

/// Global storage for the Prometheus handle.
///
/// Populated once by `crate::telemetry::init_prometheus()` during server
/// startup; read on every `/metrics` request.
pub static PROMETHEUS_HANDLE: std::sync::OnceLock<metrics_exporter_prometheus::PrometheusHandle> =
    std::sync::OnceLock::new();
