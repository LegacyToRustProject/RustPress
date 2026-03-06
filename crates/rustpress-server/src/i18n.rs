use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use tracing::{debug, info, warn};

/// WordPress-compatible internationalization (i18n) system.
///
/// Provides gettext-style translation through JSON language files.
/// Thread-safe via `Arc<RwLock>` so it can be shared across request handlers.
///
/// Translation files live in the `languages/` directory and follow the naming
/// convention `{locale}.json` (e.g. `ja.json`, `fr_FR.json`).  Each file
/// contains a flat JSON object mapping English source strings to their
/// translated equivalents:
///
/// ```json
/// { "Dashboard": "ダッシュボード", "Posts": "投稿" }
/// ```
#[derive(Clone)]
pub struct Translations {
    inner: Arc<RwLock<TranslationsInner>>,
}

struct TranslationsInner {
    /// Current locale code (e.g. "en_US", "ja").
    locale: String,
    /// Map of source string -> translated string for the active locale.
    strings: HashMap<String, String>,
    /// Directory where language JSON files are stored.
    languages_dir: PathBuf,
}

impl Translations {
    /// Create a new `Translations` instance.
    ///
    /// `languages_dir` is the path to the directory containing `*.json`
    /// language files.  `locale` is the initial locale code to load (e.g.
    /// `"ja"` or `"en_US"`).  If the locale file does not exist the
    /// translator will fall back to returning the original key for every
    /// lookup (i.e. English passthrough).
    pub fn new(languages_dir: &str, locale: &str) -> Self {
        let dir = PathBuf::from(languages_dir);
        let strings = Self::load_strings(&dir, locale);

        info!(locale, count = strings.len(), "i18n translations loaded");

        Self {
            inner: Arc::new(RwLock::new(TranslationsInner {
                locale: locale.to_string(),
                strings,
                languages_dir: dir,
            })),
        }
    }

    /// Load translation strings from `{languages_dir}/{locale}.json`.
    fn load_strings(dir: &Path, locale: &str) -> HashMap<String, String> {
        let file_path = dir.join(format!("{}.json", locale));

        if !file_path.exists() {
            debug!(locale, "no translation file found, using passthrough");
            return HashMap::new();
        }

        match fs::read_to_string(&file_path) {
            Ok(contents) => match serde_json::from_str::<HashMap<String, String>>(&contents) {
                Ok(map) => {
                    info!(locale, count = map.len(), path = %file_path.display(), "loaded translation file");
                    map
                }
                Err(e) => {
                    warn!(locale, error = %e, "failed to parse translation file");
                    HashMap::new()
                }
            },
            Err(e) => {
                warn!(locale, error = %e, "failed to read translation file");
                HashMap::new()
            }
        }
    }

    /// Translate a key.  Returns the translated string if one exists, or the
    /// original key as a fallback (WordPress `__()` semantics).
    pub async fn translate(&self, key: &str) -> String {
        self.translate_sync(key)
    }

    /// Synchronous translation lookup used inside Tera function closures.
    pub fn translate_sync(&self, key: &str) -> String {
        let inner = self.inner.read().unwrap();
        match inner.strings.get(key) {
            Some(translated) => translated.clone(),
            None => key.to_string(),
        }
    }

    /// Pluralization helper (WordPress `_n()` semantics).
    ///
    /// Returns `single` when `count == 1`, otherwise `plural`.
    /// Both forms are looked up in the translation table; if no translation
    /// exists the English form is returned as-is.
    pub fn translate_plural_sync(&self, single: &str, plural: &str, count: i64) -> String {
        let key = if count == 1 { single } else { plural };
        let translated = self.translate_sync(key);
        translated.replace("%d", &count.to_string())
    }

    /// Switch the active locale, reloading translations from disk.
    pub async fn set_locale(&self, locale: &str) {
        let mut inner = self.inner.write().unwrap();
        let strings = Self::load_strings(&inner.languages_dir, locale);
        info!(locale, count = strings.len(), "locale switched");
        inner.locale = locale.to_string();
        inner.strings = strings;
    }

    /// Return the currently active locale code.
    pub async fn get_locale(&self) -> String {
        let inner = self.inner.read().unwrap();
        inner.locale.clone()
    }

    /// List all available locales by scanning the languages directory for
    /// `*.json` files.  Always includes `en_US` even if no file exists
    /// (English is the built-in default).
    pub fn available_locales(&self) -> Vec<LocaleInfo> {
        let inner = self.inner.read().unwrap();
        let mut locales = Vec::new();

        // en_US is always available (passthrough)
        locales.push(LocaleInfo {
            code: "en_US".to_string(),
            name: "English (United States)".to_string(),
        });

        if inner.languages_dir.exists() {
            if let Ok(entries) = fs::read_dir(&inner.languages_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|e| e.to_str()) == Some("json") {
                        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                            if stem == "en_US" {
                                continue; // already added above
                            }
                            let name = locale_display_name(stem);
                            locales.push(LocaleInfo {
                                code: stem.to_string(),
                                name,
                            });
                        }
                    }
                }
            }
        }

        locales.sort_by(|a, b| a.name.cmp(&b.name));
        locales
    }
}

/// Human-readable information about a locale.
#[derive(Debug, Clone, serde::Serialize)]
pub struct LocaleInfo {
    pub code: String,
    pub name: String,
}

/// Map well-known locale codes to their display names.
fn locale_display_name(code: &str) -> String {
    match code {
        "ja" => "Japanese (日本語)".to_string(),
        "fr_FR" => "French (Français)".to_string(),
        "de_DE" => "German (Deutsch)".to_string(),
        "es_ES" => "Spanish (Español)".to_string(),
        "it_IT" => "Italian (Italiano)".to_string(),
        "pt_BR" => "Portuguese - Brazil (Português do Brasil)".to_string(),
        "pt_PT" => "Portuguese (Português)".to_string(),
        "ko_KR" => "Korean (한국어)".to_string(),
        "zh_CN" => "Chinese - Simplified (简体中文)".to_string(),
        "zh_TW" => "Chinese - Traditional (繁體中文)".to_string(),
        "ru_RU" => "Russian (Русский)".to_string(),
        "ar" => "Arabic (العربية)".to_string(),
        "nl_NL" => "Dutch (Nederlands)".to_string(),
        "sv_SE" => "Swedish (Svenska)".to_string(),
        "pl_PL" => "Polish (Polski)".to_string(),
        "tr_TR" => "Turkish (Türkçe)".to_string(),
        "th" => "Thai (ไทย)".to_string(),
        "vi" => "Vietnamese (Tiếng Việt)".to_string(),
        "he_IL" => "Hebrew (עברית)".to_string(),
        "en_GB" => "English (UK)".to_string(),
        "en_AU" => "English (Australia)".to_string(),
        "en_CA" => "English (Canada)".to_string(),
        other => other.to_string(),
    }
}

/// Register the `__()` and `_n()` translation functions on a `tera::Tera`
/// instance so templates can use `{{ __("Dashboard") }}` and
/// `{{ _n("1 post", "%d posts", count) }}`.
pub fn register_tera_i18n_functions(tera: &mut tera::Tera, translations: &Translations) {
    // __ (double underscore) — simple translation
    let t = translations.clone();
    tera.register_function(
        "__",
        move |args: &HashMap<String, tera::Value>| -> tera::Result<tera::Value> {
            // Tera calls positional arguments as "0", "1", etc. when using
            // the shorthand `{{ __("key") }}`, but named `key` if the caller
            // writes `{{ __(key="Dashboard") }}`.  Support both.
            let key = args
                .get("0")
                .or_else(|| args.get("key"))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let translated = t.translate_sync(key);
            Ok(tera::Value::String(translated))
        },
    );

    // _n — pluralization
    let t2 = translations.clone();
    tera.register_function(
        "_n",
        move |args: &HashMap<String, tera::Value>| -> tera::Result<tera::Value> {
            let single = args
                .get("0")
                .or_else(|| args.get("single"))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let plural = args
                .get("1")
                .or_else(|| args.get("plural"))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let count = args
                .get("2")
                .or_else(|| args.get("count"))
                .and_then(|v| v.as_i64())
                .unwrap_or(0);

            let translated = t2.translate_plural_sync(single, plural, count);
            Ok(tera::Value::String(translated))
        },
    );
}
