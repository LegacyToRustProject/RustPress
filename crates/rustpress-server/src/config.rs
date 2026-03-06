pub struct AppConfig {
    pub host: String,
    pub port: u16,
    pub database_url: String,
    pub jwt_secret: String,
    pub site_url: String,
    pub templates_dir: String,
    pub uploads_dir: String,
    pub plugins_dir: String,
    pub static_dir: String,
}

impl AppConfig {
    pub fn from_env() -> Self {
        Self {
            host: std::env::var("RUSTPRESS_HOST").unwrap_or_else(|_| "127.0.0.1".to_string()),
            port: std::env::var("RUSTPRESS_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(3000),
            database_url: std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| "mysql://root:password@localhost:3306/wordpress".to_string()),
            jwt_secret: std::env::var("JWT_SECRET")
                .unwrap_or_else(|_| {
                    uuid::Uuid::new_v4().to_string()
                }),
            site_url: std::env::var("SITE_URL")
                .unwrap_or_else(|_| "http://localhost:3000".to_string()),
            templates_dir: std::env::var("TEMPLATES_DIR")
                .unwrap_or_else(|_| "templates".to_string()),
            uploads_dir: std::env::var("UPLOADS_DIR")
                .unwrap_or_else(|_| "wp-content/uploads".to_string()),
            plugins_dir: std::env::var("PLUGINS_DIR")
                .unwrap_or_else(|_| "wp-content/plugins".to_string()),
            static_dir: std::env::var("STATIC_DIR")
                .unwrap_or_else(|_| "static".to_string()),
        }
    }
}
