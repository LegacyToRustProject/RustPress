use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};
use tracing::trace;

/// Callback type for shortcodes.
///
/// Receives attributes (key-value pairs) and inner content, returns rendered HTML.
pub type ShortcodeCallback = Arc<dyn Fn(&BTreeMap<String, String>, &str) -> String + Send + Sync>;

struct ShortcodeEntry {
    callback: ShortcodeCallback,
}

/// WordPress-compatible shortcode registry.
///
/// Provides `add_shortcode`/`do_shortcode` functionality equivalent
/// to `wp-includes/shortcodes.php`.
///
/// Shortcode syntax: `[name attr="value"]content[/name]` or `[name attr="value" /]`
#[derive(Clone, Default)]
pub struct ShortcodeRegistry {
    shortcodes: Arc<RwLock<BTreeMap<String, ShortcodeEntry>>>,
}

impl ShortcodeRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a shortcode handler.
    ///
    /// Equivalent to WordPress `add_shortcode($tag, $callback)`.
    pub fn add_shortcode(&self, tag: &str, callback: ShortcodeCallback) {
        let mut shortcodes = self.shortcodes.write().expect("shortcode lock poisoned");
        shortcodes.insert(tag.to_string(), ShortcodeEntry { callback });
        trace!(tag, "shortcode registered");
    }

    /// Remove a shortcode handler.
    ///
    /// Equivalent to WordPress `remove_shortcode($tag)`.
    pub fn remove_shortcode(&self, tag: &str) {
        let mut shortcodes = self.shortcodes.write().expect("shortcode lock poisoned");
        shortcodes.remove(tag);
    }

    /// Check if a shortcode is registered.
    ///
    /// Equivalent to WordPress `shortcode_exists($tag)`.
    pub fn shortcode_exists(&self, tag: &str) -> bool {
        let shortcodes = self.shortcodes.read().expect("shortcode lock poisoned");
        shortcodes.contains_key(tag)
    }

    /// Process all shortcodes in the given content string.
    ///
    /// Equivalent to WordPress `do_shortcode($content)`.
    pub fn do_shortcode(&self, content: &str) -> String {
        let shortcodes = self.shortcodes.read().expect("shortcode lock poisoned");
        if shortcodes.is_empty() {
            return content.to_string();
        }

        let mut result = content.to_string();

        // Process closing-tag shortcodes: [tag attr="val"]content[/tag]
        for (tag, entry) in shortcodes.iter() {
            loop {
                let open_pattern = format!("[{}", tag);
                let close_pattern = format!("[/{}]", tag);

                let Some(open_start) = result.find(&open_pattern) else {
                    break;
                };

                // Find the end of the opening tag
                let after_open = open_start + open_pattern.len();
                let Some(open_end_rel) = result[after_open..].find(']') else {
                    break;
                };
                let open_end = after_open + open_end_rel;

                // Check if self-closing: [tag /]
                let attr_str = &result[after_open..open_end];
                let is_self_closing = attr_str.trim_end().ends_with('/');

                if is_self_closing {
                    let attrs = parse_shortcode_attrs(attr_str.trim_end_matches('/').trim());
                    let replacement = (entry.callback)(&attrs, "");
                    result = format!(
                        "{}{}{}",
                        &result[..open_start],
                        replacement,
                        &result[open_end + 1..]
                    );
                } else if let Some(close_start) = result[open_end + 1..].find(&close_pattern) {
                    let close_start = open_end + 1 + close_start;
                    let close_end = close_start + close_pattern.len();

                    let attrs = parse_shortcode_attrs(attr_str.trim());
                    let inner_content = &result[open_end + 1..close_start];
                    let replacement = (entry.callback)(&attrs, inner_content);
                    result = format!(
                        "{}{}{}",
                        &result[..open_start],
                        replacement,
                        &result[close_end..]
                    );
                } else {
                    // No closing tag — treat as self-closing
                    let attrs = parse_shortcode_attrs(attr_str.trim());
                    let replacement = (entry.callback)(&attrs, "");
                    result = format!(
                        "{}{}{}",
                        &result[..open_start],
                        replacement,
                        &result[open_end + 1..]
                    );
                }
            }
        }

        result
    }
}

/// Parse shortcode attributes from a string like `key="value" key2="value2"`.
///
/// Equivalent to WordPress `shortcode_atts()` parsing.
fn parse_shortcode_attrs(attr_str: &str) -> BTreeMap<String, String> {
    let mut attrs = BTreeMap::new();
    let mut remaining = attr_str.trim();

    while !remaining.is_empty() {
        // Skip whitespace
        remaining = remaining.trim_start();
        if remaining.is_empty() {
            break;
        }

        // Find key
        if let Some(eq_pos) = remaining.find('=') {
            let key = remaining[..eq_pos].trim();
            remaining = remaining[eq_pos + 1..].trim_start();

            // Parse value (quoted or unquoted)
            if remaining.starts_with('"') {
                remaining = &remaining[1..];
                if let Some(end_quote) = remaining.find('"') {
                    let value = &remaining[..end_quote];
                    attrs.insert(key.to_string(), value.to_string());
                    remaining = &remaining[end_quote + 1..];
                } else {
                    break;
                }
            } else if remaining.starts_with('\'') {
                remaining = &remaining[1..];
                if let Some(end_quote) = remaining.find('\'') {
                    let value = &remaining[..end_quote];
                    attrs.insert(key.to_string(), value.to_string());
                    remaining = &remaining[end_quote + 1..];
                } else {
                    break;
                }
            } else {
                // Unquoted value — take until next space
                let end = remaining.find(' ').unwrap_or(remaining.len());
                let value = &remaining[..end];
                attrs.insert(key.to_string(), value.to_string());
                remaining = &remaining[end..];
            }
        } else {
            // Positional argument (no key=value)
            break;
        }
    }

    attrs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_shortcode() {
        let registry = ShortcodeRegistry::new();
        registry.add_shortcode("hello", Arc::new(|_, _| "Hello, World!".to_string()));
        let result = registry.do_shortcode("Before [hello] After");
        assert_eq!(result, "Before Hello, World! After");
    }

    #[test]
    fn test_shortcode_with_content() {
        let registry = ShortcodeRegistry::new();
        registry.add_shortcode(
            "bold",
            Arc::new(|_, content| format!("<strong>{}</strong>", content)),
        );
        let result = registry.do_shortcode("Normal [bold]important[/bold] text");
        assert_eq!(result, "Normal <strong>important</strong> text");
    }

    #[test]
    fn test_shortcode_with_attributes() {
        let registry = ShortcodeRegistry::new();
        registry.add_shortcode(
            "gallery",
            Arc::new(|attrs, _| {
                let ids = attrs.get("ids").cloned().unwrap_or_default();
                format!("<div class=\"gallery\" data-ids=\"{}\"></div>", ids)
            }),
        );
        let result = registry.do_shortcode("[gallery ids=\"1,2,3\"]");
        assert_eq!(result, "<div class=\"gallery\" data-ids=\"1,2,3\"></div>");
    }

    #[test]
    fn test_self_closing_shortcode() {
        let registry = ShortcodeRegistry::new();
        registry.add_shortcode("hr", Arc::new(|_, _| "<hr>".to_string()));
        let result = registry.do_shortcode("Above [hr /] Below");
        assert_eq!(result, "Above <hr> Below");
    }

    #[test]
    fn test_no_shortcodes() {
        let registry = ShortcodeRegistry::new();
        let result = registry.do_shortcode("Plain text with no shortcodes");
        assert_eq!(result, "Plain text with no shortcodes");
    }

    #[test]
    fn test_shortcode_exists() {
        let registry = ShortcodeRegistry::new();
        assert!(!registry.shortcode_exists("test"));
        registry.add_shortcode("test", Arc::new(|_, _| String::new()));
        assert!(registry.shortcode_exists("test"));
    }

    #[test]
    fn test_remove_shortcode() {
        let registry = ShortcodeRegistry::new();
        registry.add_shortcode("test", Arc::new(|_, _| "replaced".to_string()));
        assert!(registry.shortcode_exists("test"));
        registry.remove_shortcode("test");
        assert!(!registry.shortcode_exists("test"));
    }

    #[test]
    fn test_parse_attrs() {
        let attrs = parse_shortcode_attrs("id=\"42\" class=\"wide\" name='test'");
        assert_eq!(attrs.get("id"), Some(&"42".to_string()));
        assert_eq!(attrs.get("class"), Some(&"wide".to_string()));
        assert_eq!(attrs.get("name"), Some(&"test".to_string()));
    }
}
