//! WordPress-compatible translation functions.
//!
//! Provides the Rust equivalents of WordPress translation functions:
//! - `__()` / `translate()` - basic translation
//! - `_e()` - echo translation (in Rust, same as `__()` since we return strings)
//! - `_n()` - plural-aware translation
//! - `_x()` - translation with context
//! - `_nx()` - plural translation with context

use std::collections::HashMap;
use std::path::Path;
use std::sync::RwLock;

use crate::mo_parser::{self, MoError, MoFile};
use crate::plural::{self, PluralExpression};

/// Context separator used in .mo files for msgctxt lookups.
/// WordPress uses "\x04" (EOT) to separate context from the message ID.
const CONTEXT_SEPARATOR: &str = "\x04";

/// Errors from the translation system.
#[derive(Debug, thiserror::Error)]
pub enum TranslatorError {
    #[error("failed to read .mo file: {0}")]
    IoError(#[from] std::io::Error),

    #[error("failed to parse .mo file: {0}")]
    ParseError(#[from] MoError),
}

/// Domain-specific translation data: the parsed .mo file and its plural expression.
#[derive(Debug, Clone)]
struct DomainData {
    mo_file: MoFile,
    plural_expr: PluralExpression,
}

/// The main translator, managing translations for multiple text domains.
///
/// A "text domain" is a WordPress concept that isolates translations for different
/// plugins/themes. The default domain is typically "default".
#[derive(Debug)]
pub struct Translator {
    /// Per-domain translation data: domain name -> DomainData
    domains: RwLock<HashMap<String, DomainData>>,
    /// Current locale code (e.g. "ja", "en_US").
    pub current_locale: RwLock<String>,
}

impl Default for Translator {
    fn default() -> Self {
        Self::new()
    }
}

impl Translator {
    /// Create a new empty `Translator`.
    pub fn new() -> Self {
        Self {
            domains: RwLock::new(HashMap::new()),
            current_locale: RwLock::new("en_US".to_string()),
        }
    }

    /// Set the current locale.
    pub fn set_locale(&self, locale: &str) {
        let mut current = self.current_locale.write().unwrap();
        *current = locale.to_string();
    }

    /// Get the current locale.
    pub fn get_locale(&self) -> String {
        self.current_locale.read().unwrap().clone()
    }

    /// Load a .mo file for a given text domain.
    ///
    /// This is the equivalent of WordPress's `load_textdomain()`.
    ///
    /// # Arguments
    /// * `domain` - The text domain (e.g. "default", "my-plugin")
    /// * `mo_path` - Path to the .mo file
    pub fn load_textdomain(&self, domain: &str, mo_path: &Path) -> Result<(), TranslatorError> {
        let data = std::fs::read(mo_path)?;
        self.load_textdomain_from_bytes(domain, &data)?;
        tracing::info!(domain = domain, path = %mo_path.display(), "Loaded text domain");
        Ok(())
    }

    /// Load a text domain from raw .mo file bytes (useful for testing and embedded translations).
    pub fn load_textdomain_from_bytes(
        &self,
        domain: &str,
        data: &[u8],
    ) -> Result<(), TranslatorError> {
        let mo_file = mo_parser::parse_mo(data)?;

        // Extract plural expression from metadata
        let plural_expr = if let Some(plural_forms) = mo_file.metadata.get("Plural-Forms") {
            plural::parse_plural_expression(plural_forms)
        } else {
            // Default to English-style plurals
            let locale = self.get_locale();
            plural::default_plural_expression(&locale)
        };

        let mut domains = self.domains.write().unwrap();
        domains.insert(
            domain.to_string(),
            DomainData {
                mo_file,
                plural_expr,
            },
        );

        Ok(())
    }

    /// Unload a text domain, removing its translations from memory.
    pub fn unload_textdomain(&self, domain: &str) -> bool {
        let mut domains = self.domains.write().unwrap();
        domains.remove(domain).is_some()
    }

    /// Check if a text domain is loaded.
    pub fn is_textdomain_loaded(&self, domain: &str) -> bool {
        let domains = self.domains.read().unwrap();
        domains.contains_key(domain)
    }

    /// Translate a string. Equivalent to WordPress `__()`.
    ///
    /// If no translation is found, returns the original text.
    pub fn translate(&self, text: &str, domain: &str) -> String {
        let domains = self.domains.read().unwrap();
        if let Some(domain_data) = domains.get(domain) {
            if let Some(translated) = domain_data.mo_file.translations.get(text) {
                return translated.clone();
            }
        }
        text.to_string()
    }

    /// Alias for `translate()`. WordPress `__()` function.
    pub fn __(&self, text: &str, domain: &str) -> String {
        self.translate(text, domain)
    }

    /// WordPress `_e()` function. In PHP this echoes; in Rust we return the string.
    pub fn _e(&self, text: &str, domain: &str) -> String {
        self.translate(text, domain)
    }

    /// Plural-aware translation. WordPress `_n()` function.
    ///
    /// # Arguments
    /// * `single` - Singular form (e.g. "%d item")
    /// * `plural` - Plural form (e.g. "%d items")
    /// * `number` - The count determining which form to use
    /// * `domain` - Text domain
    pub fn _n(&self, single: &str, plural: &str, number: u64, domain: &str) -> String {
        let domains = self.domains.read().unwrap();
        if let Some(domain_data) = domains.get(domain) {
            if let Some(forms) = domain_data.mo_file.plural_translations.get(single) {
                let index = domain_data.plural_expr.evaluate(number);
                if index < forms.len() {
                    return forms[index].clone();
                }
            }
        }

        // Fallback: use the English logic
        if number == 1 {
            single.to_string()
        } else {
            plural.to_string()
        }
    }

    /// Contextual translation. WordPress `_x()` function.
    ///
    /// Context disambiguates identical source strings that have different meanings.
    /// In .mo files, the key is stored as "context\x04text".
    ///
    /// # Arguments
    /// * `text` - The text to translate
    /// * `context` - Disambiguation context (e.g. "post type", "navigation")
    /// * `domain` - Text domain
    pub fn _x(&self, text: &str, context: &str, domain: &str) -> String {
        let context_key = format!("{context}{CONTEXT_SEPARATOR}{text}");
        let domains = self.domains.read().unwrap();
        if let Some(domain_data) = domains.get(domain) {
            if let Some(translated) = domain_data.mo_file.translations.get(&context_key) {
                return translated.clone();
            }
        }
        text.to_string()
    }

    /// Contextual plural translation. WordPress `_nx()` function.
    ///
    /// Combines `_n()` and `_x()` for plural forms with context disambiguation.
    ///
    /// # Arguments
    /// * `single` - Singular form
    /// * `plural` - Plural form
    /// * `number` - Count
    /// * `context` - Disambiguation context
    /// * `domain` - Text domain
    pub fn _nx(
        &self,
        single: &str,
        plural: &str,
        number: u64,
        context: &str,
        domain: &str,
    ) -> String {
        let context_key = format!("{context}{CONTEXT_SEPARATOR}{single}");
        let domains = self.domains.read().unwrap();
        if let Some(domain_data) = domains.get(domain) {
            if let Some(forms) = domain_data.mo_file.plural_translations.get(&context_key) {
                let index = domain_data.plural_expr.evaluate(number);
                if index < forms.len() {
                    return forms[index].clone();
                }
            }
        }

        // Fallback
        if number == 1 {
            single.to_string()
        } else {
            plural.to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mo_parser::MO_MAGIC_LE;

    /// Build a .mo file in memory for testing.
    fn build_test_mo(entries: &[(&[u8], &[u8])], metadata: Option<&[u8]>) -> Vec<u8> {
        let mut all_entries: Vec<(&[u8], &[u8])> = Vec::new();

        // Add metadata entry (empty original string)
        if let Some(meta) = metadata {
            all_entries.push((b"", meta));
        }

        all_entries.extend_from_slice(entries);

        let num_strings = all_entries.len() as u32;
        let header_size = 28u32;
        let table_size = num_strings * 8;
        let offset_originals = header_size;
        let offset_translations = header_size + table_size;
        let string_data_start = (header_size + table_size * 2) as usize;

        let mut orig_table: Vec<u8> = Vec::new();
        let mut trans_table: Vec<u8> = Vec::new();
        let mut string_data: Vec<u8> = Vec::new();

        for (orig, trans) in &all_entries {
            let orig_offset = string_data_start + string_data.len();
            orig_table.extend_from_slice(&(orig.len() as u32).to_le_bytes());
            orig_table.extend_from_slice(&(orig_offset as u32).to_le_bytes());
            string_data.extend_from_slice(orig);
            string_data.push(0);

            let trans_offset = string_data_start + string_data.len();
            trans_table.extend_from_slice(&(trans.len() as u32).to_le_bytes());
            trans_table.extend_from_slice(&(trans_offset as u32).to_le_bytes());
            string_data.extend_from_slice(trans);
            string_data.push(0);
        }

        let mut buf = Vec::new();
        buf.extend_from_slice(&MO_MAGIC_LE.to_le_bytes());
        buf.extend_from_slice(&0u32.to_le_bytes());
        buf.extend_from_slice(&num_strings.to_le_bytes());
        buf.extend_from_slice(&offset_originals.to_le_bytes());
        buf.extend_from_slice(&offset_translations.to_le_bytes());
        buf.extend_from_slice(&0u32.to_le_bytes());
        buf.extend_from_slice(&0u32.to_le_bytes());
        buf.extend_from_slice(&orig_table);
        buf.extend_from_slice(&trans_table);
        buf.extend_from_slice(&string_data);

        buf
    }

    #[test]
    fn test_basic_translation() {
        let translator = Translator::new();
        let mo_data = build_test_mo(&[(b"Hello", b"Hola"), (b"World", b"Mundo")], None);
        translator
            .load_textdomain_from_bytes("default", &mo_data)
            .unwrap();

        assert_eq!(translator.__("Hello", "default"), "Hola");
        assert_eq!(translator.__("World", "default"), "Mundo");
        assert_eq!(translator._e("Hello", "default"), "Hola");
    }

    #[test]
    fn test_missing_translation_returns_original() {
        let translator = Translator::new();
        let mo_data = build_test_mo(&[(b"Hello", b"Hola")], None);
        translator
            .load_textdomain_from_bytes("default", &mo_data)
            .unwrap();

        assert_eq!(translator.__("Nonexistent", "default"), "Nonexistent");
        assert_eq!(translator.__("Hello", "other-domain"), "Hello");
    }

    #[test]
    fn test_plural_translation() {
        let meta = b"Plural-Forms: nplurals=2; plural=(n != 1);\n";
        let mo_data = build_test_mo(
            &[
                // Plural entry: singular\0plural -> form0\0form1
                (b"%d item\x00%d items", b"%d elemento\x00%d elementos"),
            ],
            Some(meta),
        );

        let translator = Translator::new();
        translator
            .load_textdomain_from_bytes("default", &mo_data)
            .unwrap();

        assert_eq!(
            translator._n("%d item", "%d items", 1, "default"),
            "%d elemento"
        );
        assert_eq!(
            translator._n("%d item", "%d items", 0, "default"),
            "%d elementos"
        );
        assert_eq!(
            translator._n("%d item", "%d items", 5, "default"),
            "%d elementos"
        );
    }

    #[test]
    fn test_plural_fallback() {
        let translator = Translator::new();
        // No domain loaded: should fall back to English logic
        assert_eq!(translator._n("%d cat", "%d cats", 1, "none"), "%d cat");
        assert_eq!(translator._n("%d cat", "%d cats", 2, "none"), "%d cats");
    }

    #[test]
    fn test_contextual_translation() {
        // Context key in .mo: "verb\x04Read" -> "Leer"
        //                     "noun\x04Read" -> "Lectura"
        let verb_key = b"verb\x04Read";
        let noun_key = b"noun\x04Read";
        let mo_data = build_test_mo(
            &[
                (verb_key.as_slice(), b"Leer"),
                (noun_key.as_slice(), b"Lectura"),
            ],
            None,
        );

        let translator = Translator::new();
        translator
            .load_textdomain_from_bytes("default", &mo_data)
            .unwrap();

        assert_eq!(translator._x("Read", "verb", "default"), "Leer");
        assert_eq!(translator._x("Read", "noun", "default"), "Lectura");
        assert_eq!(translator._x("Read", "unknown", "default"), "Read"); // fallback
    }

    #[test]
    fn test_contextual_plural_translation() {
        let meta = b"Plural-Forms: nplurals=2; plural=(n != 1);\n";
        // Context + plural key: "post type\x04%d post\0%d posts" -> "%d entrada\0%d entradas"
        let ctx_key = b"post type\x04%d post\x00%d posts";
        let ctx_trans = b"%d entrada\x00%d entradas";
        let mo_data = build_test_mo(&[(ctx_key.as_slice(), ctx_trans.as_slice())], Some(meta));

        let translator = Translator::new();
        translator
            .load_textdomain_from_bytes("default", &mo_data)
            .unwrap();

        assert_eq!(
            translator._nx("%d post", "%d posts", 1, "post type", "default"),
            "%d entrada"
        );
        assert_eq!(
            translator._nx("%d post", "%d posts", 5, "post type", "default"),
            "%d entradas"
        );
    }

    #[test]
    fn test_unload_textdomain() {
        let translator = Translator::new();
        let mo_data = build_test_mo(&[(b"Hi", b"Hola")], None);
        translator
            .load_textdomain_from_bytes("test", &mo_data)
            .unwrap();

        assert!(translator.is_textdomain_loaded("test"));
        assert_eq!(translator.__("Hi", "test"), "Hola");

        assert!(translator.unload_textdomain("test"));
        assert!(!translator.is_textdomain_loaded("test"));
        assert_eq!(translator.__("Hi", "test"), "Hi"); // fallback
    }

    #[test]
    fn test_multiple_domains() {
        let translator = Translator::new();

        let mo1 = build_test_mo(&[(b"Save", b"Guardar")], None);
        let mo2 = build_test_mo(&[(b"Save", b"Sauvegarder")], None);

        translator
            .load_textdomain_from_bytes("spanish", &mo1)
            .unwrap();
        translator
            .load_textdomain_from_bytes("french", &mo2)
            .unwrap();

        assert_eq!(translator.__("Save", "spanish"), "Guardar");
        assert_eq!(translator.__("Save", "french"), "Sauvegarder");
    }
}
