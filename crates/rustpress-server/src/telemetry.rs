//! Observability: OpenTelemetry (OTLP traces + metrics), Sentry error tracking,
//! and Prometheus metrics exposition.
//!
//! Call `init_telemetry()` once at startup (after `dotenvy::dotenv()`), before
//! any `tracing` macros are used.  Hold the returned `TelemetryGuard` for the
//! lifetime of the process.  Call `shutdown_telemetry()` during graceful
//! shutdown to flush OTLP exporters.
//!
//! # Environment variables
//! | Variable           | Effect                                               |
//! |--------------------|------------------------------------------------------|
//! | `OTLP_ENDPOINT`    | gRPC endpoint, e.g. `http://localhost:4317`          |
//! | `SENTRY_DSN`       | Sentry project DSN                                   |
//! | `RUST_LOG`         | `tracing-subscriber` filter directives               |
//! | `RUST_LOG_FORMAT`  | Set to `json` for JSON log output                    |

#![allow(dead_code)]

use opentelemetry::global;
use opentelemetry_sdk::{metrics::SdkMeterProvider, trace::TracerProvider, Resource};
use opentelemetry_semantic_conventions::resource as semconv;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer};

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for the telemetry subsystem.
pub struct TelemetryConfig {
    /// `service.name` attribute propagated to every signal.
    pub service_name: String,
    /// `service.version` attribute (defaults to the Cargo crate version).
    pub service_version: String,
    /// OTLP gRPC collector endpoint, e.g. `"http://localhost:4317"`.
    /// When `None` the OTLP exporters are disabled; only local tracing runs.
    pub otlp_endpoint: Option<String>,
    /// Sentry project DSN.  When `None` Sentry is disabled.
    pub sentry_dsn: Option<String>,
    /// Fraction of traces to sample: `0.0` = never, `1.0` = always (default).
    pub sample_rate: f64,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            service_name: "rustpress".to_string(),
            service_version: env!("CARGO_PKG_VERSION").to_string(),
            otlp_endpoint: std::env::var("OTLP_ENDPOINT").ok(),
            sentry_dsn: std::env::var("SENTRY_DSN").ok(),
            sample_rate: 1.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Guard
// ---------------------------------------------------------------------------

/// Keeps observability backends alive.  Drop to trigger a clean flush.
pub struct TelemetryGuard {
    /// Sentry client handle — `None` when Sentry is not configured.
    _sentry: Option<sentry::ClientInitGuard>,
    /// OTLP meter provider — `None` when OTLP is not configured.
    _meter_provider: Option<SdkMeterProvider>,
}

// ---------------------------------------------------------------------------
// init_telemetry
// ---------------------------------------------------------------------------

/// Initialise tracing-subscriber, OpenTelemetry (OTLP), and Sentry.
///
/// Must be called **once**, before any `tracing` macros are used.
pub fn init_telemetry(config: TelemetryConfig) -> TelemetryGuard {
    use opentelemetry::trace::TracerProvider as _;

    // -----------------------------------------------------------------------
    // OpenTelemetry Resource — shared by all signals.
    // -----------------------------------------------------------------------
    let resource = Resource::new(vec![
        opentelemetry::KeyValue::new(semconv::SERVICE_NAME, config.service_name.clone()),
        opentelemetry::KeyValue::new(semconv::SERVICE_VERSION, config.service_version.clone()),
    ]);

    // -----------------------------------------------------------------------
    // OTLP exporters (optional).
    // -----------------------------------------------------------------------
    let (otel_trace_layer, meter_provider) = if let Some(ref endpoint) = config.otlp_endpoint {
        // --- Traces ---
        let tracer_provider =
            build_tracer_provider(endpoint, resource.clone(), config.sample_rate);
        global::set_tracer_provider(tracer_provider.clone());
        let tracer = tracer_provider.tracer(config.service_name.clone());
        let otel_layer: Box<dyn tracing_subscriber::Layer<_> + Send + Sync> =
            tracing_opentelemetry::layer().with_tracer(tracer).boxed();

        // --- Metrics ---
        let mp = build_meter_provider(endpoint, resource);
        global::set_meter_provider(mp.clone());

        (Some(otel_layer), Some(mp))
    } else {
        (None, None)
    };

    // -----------------------------------------------------------------------
    // tracing-subscriber stack.
    // -----------------------------------------------------------------------
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("rustpress=debug,tower_http=debug,info"));

    let use_json = std::env::var("RUST_LOG_FORMAT")
        .map(|v| v.eq_ignore_ascii_case("json"))
        .unwrap_or(false);

    let fmt_layer: Box<dyn tracing_subscriber::Layer<_> + Send + Sync> = if use_json {
        tracing_subscriber::fmt::layer().json().boxed()
    } else {
        tracing_subscriber::fmt::layer().pretty().boxed()
    };

    // Build the Sentry tracing layer (if DSN is set) and add it to the stack.
    // Note: SentryLayer is a tracing Layer, not a sentry Integration.
    let sentry_layer: Option<Box<dyn tracing_subscriber::Layer<_> + Send + Sync>> =
        if config.sentry_dsn.is_some() {
            Some(sentry_tracing::layer().boxed())
        } else {
            None
        };

    let registry = tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer)
        .with(otel_trace_layer)
        .with(sentry_layer);

    registry.init();

    // -----------------------------------------------------------------------
    // Sentry client init (optional — after subscriber so sentry_tracing works).
    // -----------------------------------------------------------------------
    let sentry_guard = config.sentry_dsn.map(|dsn| {
        sentry::init((
            dsn,
            sentry::ClientOptions {
                release: sentry::release_name!(),
                traces_sample_rate: config.sample_rate as f32,
                ..Default::default()
            },
        ))
    });

    TelemetryGuard {
        _sentry: sentry_guard,
        _meter_provider: meter_provider,
    }
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Build an OTLP gRPC span exporter and wrap it in a `TracerProvider`.
fn build_tracer_provider(endpoint: &str, resource: Resource, sample_rate: f64) -> TracerProvider {
    use opentelemetry_otlp::WithExportConfig;
    use opentelemetry_sdk::{
        runtime,
        trace::{RandomIdGenerator, Sampler},
    };

    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .with_endpoint(endpoint)
        .build()
        .unwrap_or_else(|e| panic!("Failed to build OTLP span exporter: {e}"));

    TracerProvider::builder()
        .with_resource(resource)
        .with_sampler(Sampler::TraceIdRatioBased(sample_rate))
        .with_id_generator(RandomIdGenerator::default())
        .with_batch_exporter(exporter, runtime::Tokio)
        .build()
}

/// Build an OTLP gRPC metric exporter wrapped in a `SdkMeterProvider`.
fn build_meter_provider(endpoint: &str, resource: Resource) -> SdkMeterProvider {
    use opentelemetry_otlp::WithExportConfig;
    use opentelemetry_sdk::{metrics::PeriodicReader, runtime};

    let exporter = opentelemetry_otlp::MetricExporter::builder()
        .with_tonic()
        .with_endpoint(endpoint)
        .build()
        .unwrap_or_else(|e| panic!("Failed to build OTLP metric exporter: {e}"));

    let reader = PeriodicReader::builder(exporter, runtime::Tokio).build();

    SdkMeterProvider::builder()
        .with_resource(resource)
        .with_reader(reader)
        .build()
}

// ---------------------------------------------------------------------------
// Shutdown
// ---------------------------------------------------------------------------

/// Flush and shut down the global OpenTelemetry tracer provider.
///
/// Call this **after** the HTTP server has stopped accepting connections so
/// in-flight spans are exported before the process exits.
pub fn shutdown_telemetry() {
    global::shutdown_tracer_provider();
}

// ---------------------------------------------------------------------------
// Prometheus metrics endpoint helper
// ---------------------------------------------------------------------------

/// Install the global Prometheus recorder and return a handle.
///
/// The handle's `render()` method produces the Prometheus text-exposition
/// format consumed by the `GET /metrics` route.
///
/// Returns `None` if the recorder was already installed (idempotent).
pub fn prometheus_handle() -> Option<metrics_exporter_prometheus::PrometheusHandle> {
    metrics_exporter_prometheus::PrometheusBuilder::new()
        .install_recorder()
        .ok()
}

// ---------------------------------------------------------------------------
// Custom application-level metrics helpers
// ---------------------------------------------------------------------------

/// Record a completed HTTP request: increments counter and records latency.
pub fn record_http_request(method: &str, path: &str, status: u16, duration_ms: u64) {
    metrics::counter!(
        "http_requests_total",
        "method" => method.to_string(),
        "path"   => path.to_string(),
        "status" => status.to_string()
    )
    .increment(1);

    metrics::histogram!(
        "http_request_duration_ms",
        "method" => method.to_string()
    )
    .record(duration_ms as f64);
}

/// Record a completed database query: increments counter and records latency.
pub fn record_db_query(operation: &str, table: &str, duration_ms: u64, success: bool) {
    metrics::counter!(
        "db_queries_total",
        "operation" => operation.to_string(),
        "table"     => table.to_string(),
        "success"   => success.to_string()
    )
    .increment(1);

    metrics::histogram!(
        "db_query_duration_ms",
        "operation" => operation.to_string()
    )
    .record(duration_ms as f64);
}
