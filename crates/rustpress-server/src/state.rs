use std::sync::Arc;

use sea_orm::DatabaseConnection;
use tokio::sync::RwLock;

use rustpress_auth::{JwtManager, SessionManager};
use rustpress_cache::{ObjectCache, PageCache, TransientCache};
use rustpress_core::hooks::HookRegistry;
use rustpress_core::nonce::NonceManager;
use rustpress_core::rewrite::RewriteRules;
use rustpress_core::shortcode::ShortcodeRegistry;
use rustpress_cron::CronManager;
use rustpress_db::options::OptionsManager;
use rustpress_plugins::PluginRegistry;
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
}
