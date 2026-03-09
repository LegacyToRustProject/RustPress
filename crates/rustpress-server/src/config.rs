pub struct AppConfig {
    pub host: String,
    pub port: u16,
    /// MySQL/MariaDB connection URL.  Empty string means "no explicit DB" —
    /// main.rs will fall back to the in-memory SQLite stub automatically.
    pub database_url: String,
    pub jwt_secret: String,
    pub site_url: String,
    pub templates_dir: String,
    pub themes_dir: String,
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
            // C5: no hardcoded credentials — empty string triggers SQLite fallback in main.rs.
            database_url: std::env::var("DATABASE_URL").unwrap_or_else(|_| {
                tracing::warn!(
                    "DATABASE_URL not set — will fall back to in-memory SQLite stub. \
                     Set DATABASE_URL (e.g. mysql://user:pass@host/db) for full functionality."
                );
                String::new()
            }),
            // C4: generate 256-bit random secret via two UUID v4 values (each backed by
            // the OS CSPRNG via getrandom, 128 bits each = 256 bits total).
            // Tokens will NOT survive restarts — set JWT_SECRET for production.
            jwt_secret: std::env::var("JWT_SECRET").unwrap_or_else(|_| {
                tracing::warn!(
                    "JWT_SECRET not set — using a per-process random secret. \
                     All existing session tokens will be invalidated on restart. \
                     Set JWT_SECRET with a 256-bit (32+ byte) random value for production."
                );
                // Two UUIDs = 32 bytes = 256 bits of CSPRNG-backed entropy.
                format!("{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple())
            }),
            site_url: std::env::var("SITE_URL")
                .unwrap_or_else(|_| "http://localhost:3000".to_string()),
            templates_dir: std::env::var("TEMPLATES_DIR")
                .unwrap_or_else(|_| "templates".to_string()),
            themes_dir: std::env::var("THEMES_DIR")
                .unwrap_or_else(|_| "themes".to_string()),
            uploads_dir: std::env::var("UPLOADS_DIR")
                .unwrap_or_else(|_| "wp-content/uploads".to_string()),
            plugins_dir: std::env::var("PLUGINS_DIR")
                .unwrap_or_else(|_| "wp-content/plugins".to_string()),
            static_dir: std::env::var("STATIC_DIR").unwrap_or_else(|_| "static".to_string()),
        }
    }
}
