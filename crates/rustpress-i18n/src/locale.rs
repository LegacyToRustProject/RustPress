//! Locale detection and management.
//!
//! Handles resolving the active locale from various sources (Accept-Language header,
//! WordPress WPLANG option, explicit configuration) and provides locale metadata.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Represents a locale with its code and display names.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Locale {
    /// Locale code, e.g. "en_US", "ja", "fr_FR".
    pub code: String,
    /// English name, e.g. "Japanese".
    pub name: String,
    /// Native name, e.g. "日本語".
    pub native_name: String,
}

/// Manages the active locale and available locales.
#[derive(Debug, Clone)]
pub struct LocaleManager {
    current: Arc<RwLock<String>>,
    available: Arc<RwLock<HashMap<String, Locale>>>,
}

impl Default for LocaleManager {
    fn default() -> Self {
        Self::new()
    }
}

impl LocaleManager {
    /// Create a new `LocaleManager` with built-in locale definitions.
    pub fn new() -> Self {
        let mut available = HashMap::new();

        // Register common locales
        let locales = vec![
            ("en_US", "English (United States)", "English"),
            ("en_GB", "English (United Kingdom)", "English"),
            ("ja", "Japanese", "日本語"),
            ("fr_FR", "French (France)", "Français"),
            ("de_DE", "German", "Deutsch"),
            ("es_ES", "Spanish (Spain)", "Español"),
            ("pt_BR", "Portuguese (Brazil)", "Português do Brasil"),
            ("zh_CN", "Chinese (Simplified)", "简体中文"),
            ("zh_TW", "Chinese (Traditional)", "繁體中文"),
            ("ko_KR", "Korean", "한국어"),
            ("ru_RU", "Russian", "Русский"),
            ("ar", "Arabic", "العربية"),
            ("it_IT", "Italian", "Italiano"),
            ("nl_NL", "Dutch", "Nederlands"),
            ("pl_PL", "Polish", "Polski"),
            ("tr_TR", "Turkish", "Türkçe"),
            ("sv_SE", "Swedish", "Svenska"),
            ("vi", "Vietnamese", "Tiếng Việt"),
            ("th", "Thai", "ไทย"),
            ("uk", "Ukrainian", "Українська"),
        ];

        for (code, name, native_name) in locales {
            available.insert(
                code.to_string(),
                Locale {
                    code: code.to_string(),
                    name: name.to_string(),
                    native_name: native_name.to_string(),
                },
            );
        }

        Self {
            current: Arc::new(RwLock::new("en_US".to_string())),
            available: Arc::new(RwLock::new(available)),
        }
    }

    /// Set the current locale.
    pub fn set_locale(&self, code: &str) {
        let mut current = self.current.write().unwrap();
        *current = code.to_string();
        tracing::info!(locale = code, "Locale changed");
    }

    /// Get the current locale code.
    pub fn get_locale(&self) -> String {
        self.current.read().unwrap().clone()
    }

    /// Get all available locales.
    pub fn available_locales(&self) -> Vec<Locale> {
        let available = self.available.read().unwrap();
        let mut locales: Vec<Locale> = available.values().cloned().collect();
        locales.sort_by(|a, b| a.code.cmp(&b.code));
        locales
    }

    /// Register a new locale.
    pub fn register_locale(&self, locale: Locale) {
        let mut available = self.available.write().unwrap();
        available.insert(locale.code.clone(), locale);
    }

    /// Check if a locale code is available.
    pub fn is_available(&self, code: &str) -> bool {
        let available = self.available.read().unwrap();
        available.contains_key(code)
    }

    /// Get locale info by code.
    pub fn get_locale_info(&self, code: &str) -> Option<Locale> {
        let available = self.available.read().unwrap();
        available.get(code).cloned()
    }
}

/// Parsed entry from an Accept-Language header.
#[derive(Debug, Clone)]
struct AcceptLanguageEntry {
    /// Language tag, e.g. "en-US", "ja".
    tag: String,
    /// Quality value (0.0 - 1.0), default 1.0.
    quality: f32,
}

/// Parse an `Accept-Language` header value into sorted entries.
///
/// Example input: `"ja,en-US;q=0.9,en;q=0.8"`
fn parse_accept_language(header: &str) -> Vec<AcceptLanguageEntry> {
    let mut entries = Vec::new();

    for part in header.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }

        let mut segments = part.splitn(2, ';');
        let tag = segments.next().unwrap_or("").trim().to_string();

        let quality = if let Some(q_part) = segments.next() {
            let q_part = q_part.trim();
            if let Some(q_val) = q_part.strip_prefix("q=") {
                q_val.trim().parse::<f32>().unwrap_or(1.0)
            } else {
                1.0
            }
        } else {
            1.0
        };

        if !tag.is_empty() {
            entries.push(AcceptLanguageEntry { tag, quality });
        }
    }

    // Sort by quality descending
    entries.sort_by(|a, b| b.quality.partial_cmp(&a.quality).unwrap_or(std::cmp::Ordering::Equal));
    entries
}

/// Convert an HTTP language tag (e.g. "en-US") to a WordPress locale code (e.g. "en_US").
fn http_tag_to_wp_locale(tag: &str) -> String {
    // Replace '-' with '_' and normalize casing
    let parts: Vec<&str> = tag.splitn(2, '-').collect();
    match parts.len() {
        1 => parts[0].to_lowercase(),
        2 => {
            let lang = parts[0].to_lowercase();
            let region = parts[1].to_uppercase();
            format!("{lang}_{region}")
        }
        _ => tag.to_string(),
    }
}

/// Determine the best locale to use, considering multiple sources.
///
/// Priority:
/// 1. `wp_lang` option (WPLANG from wp_options) - if set, this takes precedence
/// 2. `Accept-Language` header - browser preference
/// 3. Default: "en_US"
///
/// # Arguments
/// * `accept_language` - Value of the Accept-Language HTTP header
/// * `wp_lang` - Value of the WPLANG option from WordPress database (if any)
///
/// # Returns
/// The resolved locale code string (e.g. "ja", "en_US", "fr_FR").
pub fn determine_locale(accept_language: &str, wp_lang: Option<&str>) -> String {
    // WordPress WPLANG setting takes priority
    if let Some(lang) = wp_lang {
        let lang = lang.trim();
        if !lang.is_empty() {
            return lang.to_string();
        }
    }

    // Fall back to Accept-Language header
    let entries = parse_accept_language(accept_language);

    for entry in &entries {
        let locale = http_tag_to_wp_locale(&entry.tag);
        // Return the first matching locale
        // In practice, the caller should check if .mo files exist for this locale
        if !locale.is_empty() {
            return locale;
        }
    }

    // Default
    "en_US".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_accept_language() {
        let entries = parse_accept_language("ja,en-US;q=0.9,en;q=0.8");
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].tag, "ja");
        assert_eq!(entries[0].quality, 1.0);
        assert_eq!(entries[1].tag, "en-US");
        assert_eq!(entries[1].quality, 0.9);
        assert_eq!(entries[2].tag, "en");
        assert_eq!(entries[2].quality, 0.8);
    }

    #[test]
    fn test_determine_locale_wp_lang_priority() {
        let locale = determine_locale("en-US", Some("ja"));
        assert_eq!(locale, "ja");
    }

    #[test]
    fn test_determine_locale_accept_language() {
        let locale = determine_locale("fr-FR,fr;q=0.9,en;q=0.8", None);
        assert_eq!(locale, "fr_FR");
    }

    #[test]
    fn test_determine_locale_empty_wp_lang() {
        let locale = determine_locale("de-DE", Some(""));
        assert_eq!(locale, "de_DE");
    }

    #[test]
    fn test_determine_locale_default() {
        let locale = determine_locale("", None);
        assert_eq!(locale, "en_US");
    }

    #[test]
    fn test_http_tag_to_wp_locale() {
        assert_eq!(http_tag_to_wp_locale("en-US"), "en_US");
        assert_eq!(http_tag_to_wp_locale("ja"), "ja");
        assert_eq!(http_tag_to_wp_locale("pt-BR"), "pt_BR");
        assert_eq!(http_tag_to_wp_locale("zh-cn"), "zh_CN");
    }

    #[test]
    fn test_locale_manager() {
        let manager = LocaleManager::new();
        assert_eq!(manager.get_locale(), "en_US");

        manager.set_locale("ja");
        assert_eq!(manager.get_locale(), "ja");

        assert!(manager.is_available("ja"));
        assert!(manager.is_available("en_US"));
        assert!(!manager.is_available("xx_XX"));

        let info = manager.get_locale_info("ja").unwrap();
        assert_eq!(info.native_name, "日本語");
    }

    #[test]
    fn test_register_locale() {
        let manager = LocaleManager::new();
        assert!(!manager.is_available("eo"));

        manager.register_locale(Locale {
            code: "eo".to_string(),
            name: "Esperanto".to_string(),
            native_name: "Esperanto".to_string(),
        });

        assert!(manager.is_available("eo"));
    }

    #[test]
    fn test_available_locales_sorted() {
        let manager = LocaleManager::new();
        let locales = manager.available_locales();
        let codes: Vec<&str> = locales.iter().map(|l| l.code.as_str()).collect();
        let mut sorted = codes.clone();
        sorted.sort();
        assert_eq!(codes, sorted);
    }
}
