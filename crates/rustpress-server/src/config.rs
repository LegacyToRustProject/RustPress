pub struct AppConfig {
    pub host: String,
    pub port: u16,
    pub database_url: Option<String>,
}

impl AppConfig {
    pub fn from_env() -> Self {
        Self {
            host: std::env::var("RUSTPRESS_HOST").unwrap_or_else(|_| "127.0.0.1".to_string()),
            port: std::env::var("RUSTPRESS_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(3000),
            database_url: std::env::var("DATABASE_URL").ok(),
        }
    }
}
