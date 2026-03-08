use std::sync::Arc;

use sea_orm::DatabaseConnection;
use tokio::sync::RwLock;

use rustpress_auth::{JwtManager, LoginAttemptTracker, SessionManager};
use rustpress_blocks::BlockRenderer;
use rustpress_cache::{ObjectCache, PageCache, RedisCache, TransientCache};
use rustpress_commerce::{CartManager, OrderManager, ProductCatalog};
use rustpress_core::hooks::HookRegistry;
use rustpress_core::nonce::NonceManager;
use rustpress_core::rewrite::RewriteRules;
use rustpress_core::shortcode::ShortcodeRegistry;
use rustpress_cron::CronManager;
use rustpress_db::options::OptionsManager;
use rustpress_fields::{FieldGroupRegistry, FieldStorage};
use rustpress_forms::SubmissionStore;
use rustpress_plugins::{PluginRegistry, WasmHost};
use rustpress_security::{AuditLog, LoginProtection, RateLimiter, WafEngine};
use rustpress_themes::ThemeEngine;

use crate::i18n::Translations;

/// Application state shared across all request handlers.
#[derive(Clone)]
pub struct AppState {
    pub db: DatabaseConnection,
    pub hooks: HookRegistry,
    pub options: OptionsManager,
    pub jwt: JwtManager,
    pub sessions: SessionManager,
    pub object_cache: ObjectCache,
    pub page_cache: PageCache,
    pub transients: TransientCache,
    pub plugin_registry: PluginRegistry,
    pub site_url: String,
    pub theme_engine: Arc<RwLock<ThemeEngine>>,
    pub admin_tera: Arc<tera::Tera>,
    pub translations: Translations,
    pub nonces: Arc<NonceManager>,
    pub rewrite_rules: Arc<RwLock<RewriteRules>>,
    pub shortcodes: ShortcodeRegistry,
    pub cron: Arc<CronManager>,
    // Integrated subsystems
    pub waf: Arc<RwLock<WafEngine>>,
    pub rate_limiter: Arc<RwLock<RateLimiter>>,
    pub login_protection: Arc<RwLock<LoginProtection>>,
    pub block_renderer: Arc<BlockRenderer>,
    pub redis_cache: Arc<RedisCache>,
    pub field_registry: Arc<RwLock<FieldGroupRegistry>>,
    pub field_storage: Arc<RwLock<FieldStorage>>,
    pub form_submissions: Arc<SubmissionStore>,
    pub product_catalog: Arc<RwLock<ProductCatalog>>,
    pub cart_manager: Arc<RwLock<CartManager>>,
    pub order_manager: Arc<RwLock<OrderManager>>,
    // Login attempt rate limiter (moka-based, per-IP)
    pub login_tracker: LoginAttemptTracker,
    // Security audit log
    pub audit_log: Arc<AuditLog>,
    // WASM plugin host
    pub wasm_host: Arc<RwLock<WasmHost>>,
    // Multisite support
    pub multisite_resolver: Option<Arc<rustpress_multisite::SiteResolver>>,
    pub multisite_network: Option<Arc<RwLock<rustpress_multisite::NetworkManager>>>,
}
