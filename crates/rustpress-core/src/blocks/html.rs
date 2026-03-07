//! core/html → raw custom HTML passthrough
//!
//! # Security note
//!
//! This renderer passes raw HTML through unchanged, matching WordPress
//! behaviour (`[html]` block is editor-only, trusted author input).
//!
//! In RustPress, content is stored in the DB and written by authenticated
//! authors/admins.  Untrusted user-supplied HTML (e.g. comment fields) goes
//! through `wp_kses` separately.  We do NOT sanitise here for the same
//! reason WordPress does not — it would break intentional JS/SVG embeds.
//!
//! If you need to display user-generated HTML through this block, run
//! `rustpress_core::wp_kses_post()` on the content before calling `render`.

use serde_json::Value;

/// Pass through raw HTML unchanged (WordPress core/html block semantics).
pub fn render(_attrs: &Value, inner_html: &str) -> String {
    inner_html.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_html_passthrough() {
        let raw = "<script>alert('hi')</script>";
        let out = render(&json!({}), raw);
        assert_eq!(out, raw);
    }

    #[test]
    fn test_html_empty() {
        assert_eq!(render(&json!({}), ""), "");
    }
}
