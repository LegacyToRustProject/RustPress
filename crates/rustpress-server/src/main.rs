use anyhow::Result;
use tokio::net::TcpListener;
use tracing::info;
use tracing_subscriber::EnvFilter;

mod admin;
mod config;
mod error;
mod routes;
mod templates;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("rustpress=debug,tower_http=debug")),
        )
        .init();

    dotenvy::dotenv().ok();

    let config = config::AppConfig::from_env();

    info!(
        "Starting RustPress v{} on {}:{}",
        env!("CARGO_PKG_VERSION"),
        config.host,
        config.port
    );

    let app = routes::create_router();

    let addr = format!("{}:{}", config.host, config.port);
    let listener = TcpListener::bind(&addr).await?;
    info!("Listening on {}", addr);

    axum::serve(listener, app).await?;

    Ok(())
}
