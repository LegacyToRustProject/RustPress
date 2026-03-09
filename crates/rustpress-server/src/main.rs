use anyhow::Result;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tracing::info;

mod admin;
mod config;
mod error;
pub mod i18n;
pub mod media;
pub mod middleware;
pub mod nav_menu;
mod routes;
mod state;
mod telemetry;
mod templates;
pub mod widgets;

use rustpress_api::ApiState;
use rustpress_auth::{JwtManager, SessionManager};
use rustpress_cache::{ObjectCache, PageCache, RedisCache, TransientCache};
use rustpress_commerce::{CartManager, OrderManager, ProductCatalog};
use rustpress_core::hooks::HookRegistry;
use rustpress_core::nonce::NonceManager;
use rustpress_cron::CronManager;
use rustpress_db::{connection, options::OptionsManager};
use rustpress_fields::{FieldGroupRegistry, FieldStorage};
use rustpress_forms::SubmissionStore;
use rustpress_plugins::{PluginLoader, PluginRegistry, WasmHost};
use rustpress_security::{AuditLog, LoginProtection, RateLimiter, WafEngine};

use state::AppState;

#[tokio::main]
async fn main() -> Result<()> {
    // Load .env before telemetry so OTLP_ENDPOINT / SENTRY_DSN / RUST_LOG are visible.
    dotenvy::dotenv().ok();

    // Initialise OpenTelemetry, tracing-subscriber, and Sentry.
    // The guard must be kept alive until after `axum::serve` returns.
    let _telemetry = telemetry::init_telemetry(telemetry::TelemetryConfig::default());

    // Install the Prometheus recorder and publish the handle so GET /metrics works.
    if let Some(handle) = telemetry::prometheus_handle() {
        routes::metrics::PROMETHEUS_HANDLE.set(handle).ok();
        info!("Prometheus metrics endpoint active at /metrics");
    }

    // Initialize health check start time for uptime tracking
    routes::health::init_start_time();

    let config = config::AppConfig::from_env();

    info!(
        "Starting RustPress v{} on {}:{}",
        env!("CARGO_PKG_VERSION"),
        config.host,
        config.port
    );

    // Connect to database — fall back to in-memory SQLite when MySQL is unavailable.
    // Skip MySQL entirely if NO_DB=1 OR if DATABASE_URL was not set (empty string).
    let no_db_mode = {
        let v = std::env::var("NO_DB").unwrap_or_default();
        v == "1" || v == "true" || config.database_url.is_empty()
    };
    let db = if no_db_mode {
        tracing::warn!("Using in-memory SQLite stub (no DATABASE_URL / NO_DB mode)");
        rustpress_db::connection::connect_sqlite_memory().await?
    } else {
        match connection::connect(&config.database_url).await {
            Ok(conn) => conn,
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "MySQL connection failed — falling back to in-memory SQLite stub. \
                     Set DATABASE_URL to a valid MySQL URL for full functionality."
                );
                rustpress_db::connection::connect_sqlite_memory().await?
            }
        }
    };

    // Run migrations — skip in NO_DB mode or when SKIP_MIGRATIONS is set
    let skip_migrations = no_db_mode || {
        let v = std::env::var("SKIP_MIGRATIONS").unwrap_or_default();
        v == "true" || v == "1"
    };
    if skip_migrations {
        info!("Skipping database migrations (stub/no-DB mode)");
    } else {
        info!("Running database migrations...");
        rustpress_migrate::create_wp_tables(&db).await?;
        rustpress_migrate::insert_default_options(&db, &config.site_url, "RustPress").await?;

        // Create default admin user.
        // SECURITY: never print the generated password — plain-text in logs is
        // a C3-class credential exposure. Operators must set ADMIN_PASSWORD
        // before first run, or reset via CLI afterwards.
        let admin_password = std::env::var("ADMIN_PASSWORD").unwrap_or_else(|_| {
            tracing::warn!(
                "ADMIN_PASSWORD env var not set. \
                 A random admin password was generated but NOT logged. \
                 Use `rustpress user reset-password admin` or set \
                 ADMIN_PASSWORD before the first run to gain admin access."
            );
            uuid::Uuid::new_v4().to_string()
        });
        let admin_hash = rustpress_auth::PasswordHasher::hash_argon2(&admin_password)
            .expect("Failed to hash admin password");
        rustpress_migrate::create_default_admin(&db, &admin_hash).await?;

        // Insert a sample post so there's something to see
        {
            use sea_orm::{ConnectionTrait, Statement};
            let sample_post = "INSERT IGNORE INTO wp_posts (ID, post_author, post_date, post_date_gmt, post_content, post_title, post_excerpt, post_status, comment_status, ping_status, post_password, post_name, to_ping, pinged, post_modified, post_modified_gmt, post_content_filtered, post_parent, guid, menu_order, post_type, post_mime_type, comment_count) VALUES (1, 1, NOW(), NOW(), '<h2>Welcome to RustPress!</h2>\n<p>This is your first post, powered by <strong>Rust</strong>. RustPress is a WordPress-compatible CMS built entirely in Rust for blazing-fast performance.</p>\n<h3>Features</h3>\n<ul>\n<li>WordPress 6.9 database compatible</li>\n<li>WP REST API compatible (/wp-json/wp/v2/)</li>\n<li>Plugin system with WASM support</li>\n<li>Built with Axum, SeaORM, and Tera</li>\n</ul>\n<p>Edit or delete this post, then start writing!</p>', 'Hello RustPress!', 'Welcome to RustPress - a WordPress-compatible CMS built in Rust.', 'publish', 'open', 'open', '', 'hello-rustpress', '', '', NOW(), NOW(), '', 0, '', 0, 'post', '', 0)";
            db.execute(Statement::from_string(
                sea_orm::DatabaseBackend::MySql,
                sample_post.to_string(),
            ))
            .await
            .ok();
        }
    }

    info!("Database ready");

    // Initialize subsystems
    let hooks = HookRegistry::new();

    // Register WordPress-standard filters and actions
    register_default_hooks(&hooks);
    info!("WordPress-standard hooks registered");

    let options = OptionsManager::new(db.clone());
    if let Err(e) = options.load_autoload_options().await {
        tracing::warn!("Failed to load autoload options: {}", e);
    }

    let jwt = JwtManager::new(&config.jwt_secret, 24);
    let sessions = SessionManager::with_db(24, db.clone());
    sessions.load_from_db().await;

    let object_cache = ObjectCache::new(10_000, 3600);
    let page_cache = PageCache::new(1_000, 300);
    let transients = TransientCache::new(5_000);

    let plugin_registry = PluginRegistry::new();
    let plugin_loader = PluginLoader::new(&config.plugins_dir);
    if let Err(e) = plugin_loader.scan_and_register(&plugin_registry).await {
        tracing::warn!("Plugin scan failed: {}", e);
    }

    let site_url = options
        .get_siteurl()
        .await
        .unwrap_or_else(|_| config.site_url.clone());

    // Initialize i18n translations
    let wplang = options
        .get_option("WPLANG")
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| "en_US".to_string());
    let translations = i18n::Translations::new("languages", &wplang);
    info!("i18n locale: {}", wplang);

    // Determine active theme from DB or environment
    let active_theme = if let Ok(t) = std::env::var("ACTIVE_THEME") {
        t
    } else {
        options
            .get_option("template")
            .await
            .ok()
            .flatten()
            .unwrap_or_else(|| "default".to_string())
    };
    info!(theme = %active_theme, "active theme determined");

    // Initialize theme engine for frontend
    let mut theme_engine =
        templates::init_theme_engine(&config.themes_dir, &config.templates_dir, &active_theme)
            .expect("Failed to initialize theme engine");

    // Register i18n functions on the theme engine's Tera instance
    i18n::register_tera_i18n_functions(theme_engine.tera_mut(), &translations);
    let theme_engine = Arc::new(RwLock::new(theme_engine));

    // Initialize asset manager and enqueue the active theme's stylesheet.
    // This runs once at startup — equivalent to a theme's functions.php
    // calling wp_enqueue_style() on the 'wp_enqueue_scripts' action.
    let asset_manager = Arc::new(rustpress_themes::AssetManager::new());
    {
        let theme_style_url = format!(
            "/wp-content/themes/{}/style.css",
            active_theme
        );
        asset_manager.enqueue_style(
            "theme-style",
            &theme_style_url,
            &[],
            env!("CARGO_PKG_VERSION"),
            "all",
        );
        info!(theme = %active_theme, url = %theme_style_url, "theme stylesheet enqueued");

        // Enqueue theme-specific additional assets.
        match active_theme.as_str() {
            "twentyseventeen" => {
                let base = format!("/wp-content/themes/{active_theme}");
                asset_manager.enqueue_style(
                    "twentyseventeen-block-style",
                    &format!("{base}/assets/css/blocks.css"),
                    &["theme-style"],
                    env!("CARGO_PKG_VERSION"),
                    "all",
                );
                asset_manager.enqueue_style(
                    "twentyseventeen-fonts",
                    &format!("{base}/assets/fonts/font-libre-franklin.css"),
                    &[],
                    env!("CARGO_PKG_VERSION"),
                    "all",
                );
            }
            _ => {}
        }
    }

    // Initialize admin template engine
    let mut admin_tera = templates::init_admin_tera("templates/admin")
        .expect("Failed to initialize admin templates");

    // Register i18n functions on the admin Tera instance
    i18n::register_tera_i18n_functions(&mut admin_tera, &translations);

    // Register wp_nonce_field() Tera function for CSRF protection in forms.
    // Usage: {{ wp_nonce_field(nonce=wpnonce_save_post) }}
    // Or with custom field name: {{ wp_nonce_field(nonce=wpnonce_general, name="_custom_nonce") }}
    admin_tera.register_function(
        "wp_nonce_field",
        |args: &std::collections::HashMap<String, tera::Value>| -> tera::Result<tera::Value> {
            let nonce = args
                .get("nonce")
                .and_then(|v| v.as_str())
                .ok_or_else(|| tera::Error::msg("wp_nonce_field requires a 'nonce' argument"))?;
            let name = args
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("_wpnonce");
            let html = format!(
                "<input type=\"hidden\" name=\"{}\" value=\"{}\" />",
                rustpress_core::esc_attr(name),
                rustpress_core::esc_attr(nonce),
            );
            Ok(tera::Value::String(html))
        },
    );

    let admin_tera = Arc::new(admin_tera);

    // Initialize rewrite rules from permalink_structure option
    let permalink_structure = options
        .get_option("permalink_structure")
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| "/%postname%/".to_string());
    let mut rewrite_rules = rustpress_core::rewrite::RewriteRules::new();
    rewrite_rules.set_structure(&permalink_structure);
    info!(structure = %permalink_structure, "permalink structure loaded");

    // Initialize shortcode registry with built-in shortcodes
    let shortcodes = rustpress_core::shortcode::ShortcodeRegistry::new();
    register_builtin_shortcodes(&shortcodes);
    info!("shortcode registry initialized with built-in shortcodes");

    // Initialize cron system (before AppState so it can be stored in state)
    let cron = Arc::new(CronManager::new());

    // Initialize security subsystems
    let waf = Arc::new(RwLock::new(WafEngine::with_default_rules()));
    let rate_limiter = Arc::new(RwLock::new(RateLimiter::new()));
    let login_protection = Arc::new(RwLock::new(LoginProtection::new()));
    let audit_log = Arc::new(AuditLog::new(10_000));
    info!("Security subsystems initialized (WAF, rate limiter, login protection, audit log)");

    // Initialize block renderer with all core WordPress blocks
    let block_renderer = Arc::new(rustpress_blocks::create_default_renderer());
    info!("Block renderer initialized with core blocks");

    // Initialize Redis cache (in-memory fallback if no REDIS_URL configured)
    let redis_url = std::env::var("REDIS_URL").ok();
    let redis_cache = Arc::new(RedisCache::new(redis_url.clone()));
    if redis_url.is_some() {
        if let Err(e) = redis_cache.connect().await {
            tracing::warn!("Redis connection failed, using in-memory fallback: {}", e);
        }
    }

    // Initialize custom fields (ACF-compatible)
    let field_registry = Arc::new(RwLock::new(FieldGroupRegistry::new()));
    let field_storage = Arc::new(RwLock::new(FieldStorage::new()));
    info!("Custom fields subsystem initialized");

    // Initialize forms subsystem
    let form_submissions = Arc::new(SubmissionStore::new());
    info!("Forms subsystem initialized");

    // Initialize commerce subsystem
    let product_catalog = Arc::new(RwLock::new(ProductCatalog::new()));
    let cart_manager = Arc::new(RwLock::new(CartManager::new()));
    let order_manager = Arc::new(RwLock::new(OrderManager::new()));
    info!("Commerce subsystem initialized");

    // Initialize multisite support (if enabled)
    let multisite_env = std::env::var("MULTISITE").unwrap_or_default();
    let multisite_enabled = multisite_env == "true" || multisite_env == "1";
    let (multisite_resolver, multisite_network) = if multisite_enabled {
        // Extract domain from site_url for the network domain
        let network_domain = site_url
            .trim_start_matches("http://")
            .trim_start_matches("https://")
            .split('/')
            .next()
            .unwrap_or("localhost")
            .split(':')
            .next()
            .unwrap_or("localhost")
            .to_string();

        let resolver = rustpress_multisite::SiteResolver::new(
            rustpress_multisite::MultisiteMode::SubDirectory,
            network_domain.clone(),
        );

        // Register the main site (blog_id 1)
        let now = chrono::Utc::now();
        let main_site = rustpress_multisite::Site {
            blog_id: 1,
            domain: network_domain.clone(),
            path: "/".to_string(),
            site_id: 1,
            registered: now,
            last_updated: now,
            public: true,
            archived: false,
            mature: false,
            spam: false,
            deleted: false,
            lang_id: 0,
        };
        resolver.register_site(main_site);

        let network_manager = rustpress_multisite::NetworkManager::new();
        network_manager.create_network(
            network_domain.clone(),
            "/".to_string(),
            "Default Network".to_string(),
            "admin@localhost".to_string(),
        );

        info!(domain = %network_domain, "Multisite enabled (SubDirectory mode)");

        (
            Some(Arc::new(resolver)),
            Some(Arc::new(RwLock::new(network_manager))),
        )
    } else {
        (None, None)
    };

    // Initialize WASM plugin host
    let mut wasm_host = WasmHost::new();
    let wasm_plugins_dir = std::path::Path::new("plugins/wasm");
    std::fs::create_dir_all(wasm_plugins_dir).ok();
    if let Ok(entries) = std::fs::read_dir(wasm_plugins_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("wasm") {
                let plugin_name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown")
                    .to_string();
                match wasm_host.load_plugin(&plugin_name, &path) {
                    Ok(()) => {
                        if let Err(e) = wasm_host.init_plugin(&plugin_name) {
                            tracing::warn!(
                                plugin = %plugin_name,
                                error = %e,
                                "Failed to init WASM plugin"
                            );
                        }
                    }
                    Err(e) => {
                        tracing::warn!(
                            plugin = %plugin_name,
                            error = %e,
                            "Failed to load WASM plugin"
                        );
                    }
                }
            }
        }
    }
    let wasm_host = Arc::new(RwLock::new(wasm_host));
    info!("WASM plugin host initialized");

    // Build application state
    let state = Arc::new(AppState {
        db: db.clone(),
        hooks: hooks.clone(),
        options,
        jwt: jwt.clone(),
        sessions,
        object_cache,
        page_cache,
        transients,
        plugin_registry,
        site_url: site_url.clone(),
        theme_engine,
        asset_manager,
        admin_tera,
        translations,
        nonces: Arc::new(NonceManager::new(&config.jwt_secret)),
        rewrite_rules: Arc::new(RwLock::new(rewrite_rules)),
        shortcodes,
        cron: cron.clone(),
        waf,
        rate_limiter,
        login_protection,
        block_renderer,
        redis_cache,
        field_registry,
        field_storage,
        form_submissions,
        product_catalog,
        cart_manager,
        order_manager,
        login_tracker: rustpress_auth::LoginAttemptTracker::new(),
        audit_log,
        wasm_host,
        multisite_resolver,
        multisite_network,
    });

    // Register session cleanup cron job (every hour)
    {
        let sessions_clone = state.sessions.clone();
        cron.register_callback(
            "session_cleanup",
            Arc::new(move || {
                // SessionManager::cleanup_expired is sync-compatible
                let s = sessions_clone.clone();
                tokio::spawn(async move {
                    s.cleanup_expired().await;
                });
            }),
        );
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        cron.schedule_event(now + 3600, "hourly", "session_cleanup", vec![]);
    }

    // Register publish_future_posts cron job (every minute)
    // Equivalent to WordPress's wp_publish_post() called via missed schedule check
    cron.add_schedule("minutely", 60, "Once Every Minute");
    {
        use rustpress_db::entities::wp_posts;
        use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter};

        let db_clone = state.db.clone();
        cron.register_callback(
            "publish_future_posts",
            Arc::new(move || {
                let db = db_clone.clone();
                tokio::spawn(async move {
                    let now_utc = chrono::Utc::now().naive_utc();

                    // Find all posts with status 'future' whose scheduled time has passed
                    let future_posts = wp_posts::Entity::find()
                        .filter(wp_posts::Column::PostStatus.eq("future"))
                        .filter(wp_posts::Column::PostDateGmt.lte(now_utc))
                        .all(&db)
                        .await;

                    match future_posts {
                        Ok(posts) => {
                            for post in posts {
                                let post_id = post.id;
                                let mut active: wp_posts::ActiveModel = post.into();
                                active.post_status = Set("publish".to_string());
                                active.post_modified = Set(now_utc);
                                active.post_modified_gmt = Set(now_utc);
                                match active.update(&db).await {
                                    Ok(_) => {
                                        tracing::info!(
                                            post_id,
                                            "publish_future_posts: published scheduled post"
                                        );
                                    }
                                    Err(e) => {
                                        tracing::error!(
                                            post_id,
                                            error = %e,
                                            "publish_future_posts: failed to publish post"
                                        );
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            tracing::error!(
                                error = %e,
                                "publish_future_posts: failed to query future posts"
                            );
                        }
                    }
                });
            }),
        );
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        cron.schedule_event(now + 60, "minutely", "publish_future_posts", vec![]);
    }

    // Start cron background runner
    let _cron_handle = cron.clone().start_background_runner();
    info!("Cron system started");

    // Build router with all routes (includes frontend, health, API, admin dashboard)
    let mut app = routes::create_router(state.clone());

    // Mount admin CRUD API routes (protected by session/JWT)
    app = app.merge(admin::create_admin_routes(&state));

    // Mount WP REST API routes (GET = public, POST/PUT/DELETE = authenticated)
    let api_state = ApiState {
        db: db.clone(),
        hooks: hooks.clone(),
        jwt: jwt.clone(),
        sessions: state.sessions.clone(),
        site_url,
        nonces: state.nonces.clone(),
    };
    app = app.merge(rustpress_api::routes(api_state));

    // Resolve theme-aware static dir and theme.json
    let static_dir =
        templates::resolve_theme_static_dir(&config.themes_dir, &active_theme, &config.static_dir);
    let theme_json_path =
        templates::resolve_theme_json_path(&config.themes_dir, &active_theme, &config.static_dir);

    // Generate CSS from theme.json if available
    if theme_json_path.exists() {
        match rustpress_themes::theme_json::ThemeJson::from_file(&theme_json_path) {
            Ok(theme) => {
                let css = theme.generate_css_variables();
                let css_path = std::path::Path::new(&static_dir).join("theme-generated.css");
                if let Err(e) = std::fs::write(&css_path, &css) {
                    tracing::warn!("Failed to write theme-generated.css: {}", e);
                } else {
                    info!("Generated theme CSS from theme.json ({} bytes)", css.len());
                }
            }
            Err(e) => tracing::warn!("Failed to parse theme.json: {}", e),
        }
    }

    // Static file serving
    if std::path::Path::new(&static_dir).exists() {
        app = app.nest_service("/static", tower_http::services::ServeDir::new(&static_dir));
    }

    // Uploads serving — always create directory and mount
    let uploads_dir = config.uploads_dir.clone();
    std::fs::create_dir_all(&uploads_dir).ok();
    app = app.nest_service(
        "/wp-content/uploads",
        tower_http::services::ServeDir::new(&uploads_dir),
    );

    // Theme assets serving — /wp-content/themes/{name}/ → themes/{name}/static/
    let themes_dir_path = std::path::Path::new(&config.themes_dir);
    if themes_dir_path.exists() {
        if let Ok(entries) = std::fs::read_dir(themes_dir_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let slug = path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    let static_path = path.join("static");
                    if static_path.exists() {
                        let route = format!("/wp-content/themes/{slug}");
                        app = app
                            .nest_service(&route, tower_http::services::ServeDir::new(static_path));
                    }
                }
            }
        }
    }

    // WordPress core includes path — serves Gutenberg JS/CSS assets
    let wp_includes_dir = format!("{}/wp-includes", static_dir);
    if std::path::Path::new(&wp_includes_dir).exists() {
        app = app.nest_service(
            "/wp-includes",
            tower_http::services::ServeDir::new(&wp_includes_dir),
        );
    }

    // Apply global middleware (order matters: outermost layer runs first)
    // 1. Telemetry — trace spans + Prometheus metrics (outermost, covers all layers)
    // 2. Block sensitive files (.env, .git, etc.) — first line of defense
    // 3. WAF — block malicious patterns
    // 4. Rate limiter — prevent abuse
    // 5. CORS — cross-origin policy
    // 6. Security headers — defense-in-depth headers
    // 7. ETag + compression — performance
    app = app
        .layer(axum::middleware::from_fn(middleware::telemetry_trace))
        .layer(axum::middleware::from_fn(middleware::block_sensitive_files))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            middleware::waf_check,
        ))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            middleware::rate_limit,
        ))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            middleware::cors_headers,
        ))
        .layer(axum::middleware::from_fn(middleware::security_headers))
        .layer(axum::middleware::from_fn(middleware::etag_headers))
        .layer(tower_http::compression::CompressionLayer::new());

    // Apply multisite middleware (only if multisite is enabled)
    if multisite_enabled {
        app = app.layer(axum::middleware::from_fn_with_state(
            state.clone(),
            middleware::multisite_resolve,
        ));
        info!("Multisite middleware active");
    }

    // Fire init hook
    state.hooks.do_action("init", &serde_json::json!({}));

    let addr = format!("{}:{}", config.host, config.port);
    let listener = TcpListener::bind(&addr).await?;
    info!("RustPress is ready at http://{}", addr);
    info!("  Site:   http://{}/", addr);
    info!("  Admin:  http://{}/wp-admin/", addr);
    info!("  API:    http://{}/api/posts", addr);
    info!("  REST:   http://{}/wp-json/wp/v2/posts", addr);
    info!("  Health: http://{}/health", addr);
    info!("  Shop:   http://{}/shop", addr);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    // Flush and shut down OTLP exporters after the server has drained.
    telemetry::shutdown_telemetry();

    Ok(())
}

/// Register built-in WordPress shortcodes (caption, audio, video, gallery, embed).
fn register_builtin_shortcodes(registry: &rustpress_core::shortcode::ShortcodeRegistry) {
    use std::sync::Arc;

    // [caption] — wraps image with figcaption
    registry.add_shortcode(
        "caption",
        Arc::new(|attrs, content| {
            let align = attrs.get("align").cloned().unwrap_or_default();
            let align_class = if align.is_empty() {
                String::new()
            } else {
                format!(" class=\"{align}\"")
            };
            if let Some(img_end) = content.find("/>") {
                let img = &content[..img_end + 2];
                let caption_text = content[img_end + 2..].trim();
                format!(
                    "<figure{align_class}>{img}<figcaption>{caption_text}</figcaption></figure>"
                )
            } else {
                format!("<figure{align_class}>{content}</figure>")
            }
        }),
    );

    // [audio] — HTML5 audio player
    registry.add_shortcode(
        "audio",
        Arc::new(|attrs, _| {
            let src = attrs.get("src").cloned().unwrap_or_default();
            if src.is_empty() {
                return String::new();
            }
            format!(
                r#"<audio controls preload="metadata"><source src="{src}">Your browser does not support audio.</audio>"#
            )
        }),
    );

    // [video] — HTML5 video player
    registry.add_shortcode(
        "video",
        Arc::new(|attrs, _| {
            let src = attrs.get("src").cloned().unwrap_or_default();
            let width = attrs.get("width").cloned().unwrap_or_else(|| "100%".to_string());
            if src.is_empty() {
                return String::new();
            }
            format!(
                r#"<video controls preload="metadata" style="max-width:{width};height:auto"><source src="{src}">Your browser does not support video.</video>"#
            )
        }),
    );

    // [gallery] — image gallery placeholder
    registry.add_shortcode(
        "gallery",
        Arc::new(|attrs, _| {
            let ids = attrs.get("ids").cloned().unwrap_or_default();
            let columns = attrs
                .get("columns")
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(3);
            if ids.is_empty() {
                return String::new();
            }
            let img_tags: String = ids
                .split(',')
                .map(|id| id.trim())
                .filter(|id| !id.is_empty())
                .map(|_| "<div class=\"gallery-item\"></div>".to_string())
                .collect::<Vec<_>>()
                .join("\n");
            format!("<div class=\"gallery gallery-columns-{columns}\">{img_tags}</div>")
        }),
    );

    // [embed] — oEmbed / basic URL embedding
    registry.add_shortcode(
        "embed",
        Arc::new(|_, content| {
            let url = content.trim();
            if url.contains("youtube.com") || url.contains("youtu.be") {
                // Extract YouTube video ID
                let video_id = if let Some(pos) = url.find("youtu.be/") {
                    let id = &url[pos + 9..];
                    id.split(&['?', '&', '#'][..]).next().map(|s| s.to_string())
                } else if let Some(pos) = url.find("v=") {
                    let id = &url[pos + 2..];
                    id.split(&['&', '#'][..]).next().map(|s| s.to_string())
                } else {
                    None
                };
                if let Some(vid) = video_id {
                    format!(
                        r#"<div class="wp-embed"><iframe width="560" height="315" src="https://www.youtube.com/embed/{vid}" frameborder="0" allowfullscreen></iframe></div>"#
                    )
                } else {
                    format!("<a href=\"{url}\">{url}</a>")
                }
            } else if url.contains("vimeo.com") {
                // Vimeo embed
                let vid = url.rsplit('/').next().unwrap_or("");
                format!(
                    r#"<div class="wp-embed"><iframe src="https://player.vimeo.com/video/{vid}" width="560" height="315" frameborder="0" allowfullscreen></iframe></div>"#
                )
            } else {
                format!("<a href=\"{url}\">{url}</a>")
            }
        }),
    );
}

/// Register WordPress-standard filters and action placeholders.
///
/// This mirrors the default hooks that WordPress registers in wp-includes/default-filters.php.
/// Filters transform content through a pipeline; actions fire side effects at specific points.
fn register_default_hooks(hooks: &HookRegistry) {
    use rustpress_themes::formatting;
    use serde_json::Value;
    use std::sync::Arc;

    // ============================================================
    // the_content filters — applied to post content before display
    // ============================================================

    // Priority 10: wpautop — converts double line breaks to <p> tags
    hooks.add_filter(
        "the_content",
        Arc::new(|value: Value| {
            if let Value::String(s) = value {
                Value::String(formatting::wpautop(&s))
            } else {
                value
            }
        }),
        10,
    );

    // Priority 11: shortcode_unautop — removes <p> wrapping around shortcodes
    hooks.add_filter(
        "the_content",
        Arc::new(|value: Value| {
            if let Value::String(s) = value {
                Value::String(formatting::shortcode_unautop(&s))
            } else {
                value
            }
        }),
        11,
    );

    // Priority 12: wptexturize — converts straight quotes to curly, etc.
    hooks.add_filter(
        "the_content",
        Arc::new(|value: Value| {
            if let Value::String(s) = value {
                Value::String(formatting::wptexturize(&s))
            } else {
                value
            }
        }),
        12,
    );

    // Priority 13: convert_smilies — converts text emoticons to emoji
    hooks.add_filter(
        "the_content",
        Arc::new(|value: Value| {
            if let Value::String(s) = value {
                Value::String(formatting::convert_smilies(&s))
            } else {
                value
            }
        }),
        13,
    );

    // ============================================================
    // the_title filters — applied to post titles before display
    // ============================================================

    // Priority 10: wptexturize
    hooks.add_filter(
        "the_title",
        Arc::new(|value: Value| {
            if let Value::String(s) = value {
                Value::String(formatting::wptexturize(&s))
            } else {
                value
            }
        }),
        10,
    );

    // Priority 11: convert_chars
    hooks.add_filter(
        "the_title",
        Arc::new(|value: Value| {
            if let Value::String(s) = value {
                Value::String(formatting::convert_chars(&s))
            } else {
                value
            }
        }),
        11,
    );

    // Priority 12: trim
    hooks.add_filter(
        "the_title",
        Arc::new(|value: Value| {
            if let Value::String(s) = value {
                Value::String(s.trim().to_string())
            } else {
                value
            }
        }),
        12,
    );

    // ============================================================
    // the_excerpt filters — applied to post excerpts before display
    // ============================================================

    // Priority 10: wp_trim_excerpt
    hooks.add_filter(
        "the_excerpt",
        Arc::new(|value: Value| {
            if let Value::String(s) = value {
                Value::String(formatting::wp_trim_excerpt(&s))
            } else {
                value
            }
        }),
        10,
    );

    // Priority 11: wpautop
    hooks.add_filter(
        "the_excerpt",
        Arc::new(|value: Value| {
            if let Value::String(s) = value {
                Value::String(formatting::wpautop(&s))
            } else {
                value
            }
        }),
        11,
    );

    // ============================================================
    // comment_text filters — applied to comment content
    // ============================================================

    // Priority 10: wpautop
    hooks.add_filter(
        "comment_text",
        Arc::new(|value: Value| {
            if let Value::String(s) = value {
                Value::String(formatting::wpautop(&s))
            } else {
                value
            }
        }),
        10,
    );

    // Priority 12: wptexturize
    hooks.add_filter(
        "comment_text",
        Arc::new(|value: Value| {
            if let Value::String(s) = value {
                Value::String(formatting::wptexturize(&s))
            } else {
                value
            }
        }),
        12,
    );

    // Priority 13: convert_smilies
    hooks.add_filter(
        "comment_text",
        Arc::new(|value: Value| {
            if let Value::String(s) = value {
                Value::String(formatting::convert_smilies(&s))
            } else {
                value
            }
        }),
        13,
    );

    // ============================================================
    // widget_text filters — applied to text widget content
    // ============================================================

    // Priority 10: wpautop
    hooks.add_filter(
        "widget_text",
        Arc::new(|value: Value| {
            if let Value::String(s) = value {
                Value::String(formatting::wpautop(&s))
            } else {
                value
            }
        }),
        10,
    );

    // Priority 11: shortcode_unautop
    hooks.add_filter(
        "widget_text",
        Arc::new(|value: Value| {
            if let Value::String(s) = value {
                Value::String(formatting::shortcode_unautop(&s))
            } else {
                value
            }
        }),
        11,
    );

    // ============================================================
    // the_content_feed filters — applied to content in RSS feeds
    // ============================================================

    // Priority 10: strip HTML tags for feed safety
    hooks.add_filter(
        "the_content_feed",
        Arc::new(|value: Value| {
            if let Value::String(s) = value {
                // Simple tag stripping for feed content
                let mut result = String::with_capacity(s.len());
                let mut in_tag = false;
                for ch in s.chars() {
                    match ch {
                        '<' => in_tag = true,
                        '>' => in_tag = false,
                        _ if !in_tag => result.push(ch),
                        _ => {}
                    }
                }
                Value::String(result)
            } else {
                value
            }
        }),
        10,
    );

    // ============================================================
    // Standard WordPress action placeholders
    //
    // These are registered so plugins can hook into them. The actions
    // are fired at the appropriate points in the request lifecycle.
    // ============================================================

    // init — fires on server startup (already called in main)
    hooks.add_action_default(
        "init",
        Arc::new(|_| {
            tracing::trace!("init action fired");
        }),
    );

    // wp_head — fires when rendering <head> section
    hooks.add_action_default(
        "wp_head",
        Arc::new(|_| {
            tracing::trace!("wp_head action fired");
        }),
    );

    // wp_footer — fires when rendering footer
    hooks.add_action_default(
        "wp_footer",
        Arc::new(|_| {
            tracing::trace!("wp_footer action fired");
        }),
    );

    // wp_enqueue_scripts — for script/style registration
    hooks.add_action_default(
        "wp_enqueue_scripts",
        Arc::new(|_| {
            tracing::trace!("wp_enqueue_scripts action fired");
        }),
    );

    // save_post — fires after post save (create or update)
    hooks.add_action_default(
        "save_post",
        Arc::new(|_| {
            tracing::trace!("save_post action fired");
        }),
    );

    // delete_post — fires before post delete
    hooks.add_action_default(
        "delete_post",
        Arc::new(|_| {
            tracing::trace!("delete_post action fired");
        }),
    );

    // wp_login — fires on successful login
    hooks.add_action_default(
        "wp_login",
        Arc::new(|_| {
            tracing::trace!("wp_login action fired");
        }),
    );

    // wp_logout — fires on logout
    hooks.add_action_default(
        "wp_logout",
        Arc::new(|_| {
            tracing::trace!("wp_logout action fired");
        }),
    );
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("Shutdown signal received, starting graceful shutdown...");
}
