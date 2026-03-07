//! Gutenberg core block renderers.
//!
//! Each sub-module exposes a `render(attrs, inner_html) -> String` function
//! that converts parsed block data into WordPress-compatible HTML.
//!
//! Attribute keys follow the WordPress block grammar (`className`, `align`,
//! `level`, `url`, etc.).  Extra unknown attributes are silently ignored so
//! the renderer stays forward-compatible.

pub mod buttons;
pub mod code;
pub mod columns;
pub mod cover;
pub mod embed;
pub mod group;
pub mod heading;
pub mod html;
pub mod image;
pub mod list;
pub mod media_text;
pub mod paragraph;
pub mod quote;
pub mod separator;
pub mod spacer;

use serde_json::Value;

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Extract `className` from attrs and prepend a space if present.
pub(crate) fn extra_classes(attrs: &Value) -> String {
    attrs
        .get("className")
        .and_then(Value::as_str)
        .map(|c| format!(" {c}"))
        .unwrap_or_default()
}

/// Convert an `align` attr value to a WordPress alignment class.
pub(crate) fn align_class(attrs: &Value) -> Option<&'static str> {
    match attrs.get("align").and_then(Value::as_str) {
        Some("left") => Some("alignleft"),
        Some("center") => Some("aligncenter"),
        Some("right") => Some("alignright"),
        Some("wide") => Some("alignwide"),
        Some("full") => Some("alignfull"),
        _ => None,
    }
}

/// Convert text-align attr to a CSS class.
pub(crate) fn text_align_class(attrs: &Value) -> Option<String> {
    attrs
        .get("textAlign")
        .or_else(|| attrs.get("align"))
        .and_then(Value::as_str)
        .filter(|v| matches!(*v, "left" | "center" | "right"))
        .map(|v| format!("has-text-align-{v}"))
}

/// Build inline `style="..."` from optional background + text colour attrs.
pub(crate) fn color_style(attrs: &Value) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(bg) = attrs
        .get("style")
        .and_then(|s| s.get("color"))
        .and_then(|c| c.get("background"))
        .and_then(Value::as_str)
    {
        parts.push(format!("background-color:{bg}"));
    }
    if let Some(fg) = attrs
        .get("style")
        .and_then(|s| s.get("color"))
        .and_then(|c| c.get("text"))
        .and_then(Value::as_str)
    {
        parts.push(format!("color:{fg}"));
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!(" style=\"{}\"", parts.join(";"))
    }
}

// ---------------------------------------------------------------------------
// Top-level dispatcher
// ---------------------------------------------------------------------------

/// Render any supported core block by name.
///
/// Falls back to wrapping `inner_html` in a plain `<div>` for unknown blocks.
pub fn render_block(block_name: &str, attrs: &Value, inner_html: &str) -> String {
    match block_name {
        "core/paragraph" => paragraph::render(attrs, inner_html),
        "core/heading" => heading::render(attrs, inner_html),
        "core/image" => image::render(attrs, inner_html),
        "core/list" => list::render(attrs, inner_html),
        "core/list-item" => list::render_item(attrs, inner_html),
        "core/quote" => quote::render(attrs, inner_html),
        "core/code" => code::render(attrs, inner_html),
        "core/buttons" => buttons::render_wrapper(attrs, inner_html),
        "core/button" => buttons::render_button(attrs, inner_html),
        "core/columns" => columns::render_wrapper(attrs, inner_html),
        "core/column" => columns::render_column(attrs, inner_html),
        "core/separator" => separator::render(attrs, inner_html),
        "core/spacer" => spacer::render(attrs, inner_html),
        "core/media-text" => media_text::render(attrs, inner_html),
        "core/cover" => cover::render(attrs, inner_html),
        "core/group" => group::render(attrs, inner_html),
        "core/embed" => embed::render(attrs, inner_html),
        "core/html" => html::render(attrs, inner_html),
        _ => {
            // Unknown block — pass through inner HTML unchanged
            inner_html.to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_dispatcher_unknown_block_passthrough() {
        let out = render_block("core/unknown-future-block", &json!({}), "<p>content</p>");
        assert_eq!(out, "<p>content</p>");
    }

    #[test]
    fn test_extra_classes_present() {
        let attrs = json!({"className": "my-class another"});
        assert_eq!(extra_classes(&attrs), " my-class another");
    }

    #[test]
    fn test_extra_classes_absent() {
        assert_eq!(extra_classes(&json!({})), "");
    }

    #[test]
    fn test_align_class() {
        assert_eq!(align_class(&json!({"align": "wide"})), Some("alignwide"));
        assert_eq!(align_class(&json!({"align": "full"})), Some("alignfull"));
        assert_eq!(
            align_class(&json!({"align": "center"})),
            Some("aligncenter")
        );
        assert_eq!(align_class(&json!({})), None);
    }

    #[test]
    fn test_text_align_class() {
        assert_eq!(
            text_align_class(&json!({"textAlign": "center"})),
            Some("has-text-align-center".to_string())
        );
        assert_eq!(text_align_class(&json!({})), None);
    }
}
