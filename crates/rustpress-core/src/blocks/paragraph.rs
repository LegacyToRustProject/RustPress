//! core/paragraph → `<p class="wp-block-paragraph ...">...</p>`

use serde_json::Value;

use super::{color_style, extra_classes, text_align_class};

pub fn render(attrs: &Value, inner_html: &str) -> String {
    let mut classes = vec!["wp-block-paragraph".to_string()];

    if let Some(ta) = text_align_class(attrs) {
        classes.push(ta);
    }

    if attrs
        .get("dropCap")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        classes.push("has-drop-cap".to_string());
    }

    // Preset font size
    if let Some(size) = attrs.get("fontSize").and_then(Value::as_str) {
        classes.push(format!("has-{size}-font-size"));
    }

    // Preset background / text colour slugs → utility classes
    if let Some(bg) = attrs.get("backgroundColor").and_then(Value::as_str) {
        classes.push(format!("has-{bg}-background-color"));
        classes.push("has-background".to_string());
    }
    if let Some(fg) = attrs.get("textColor").and_then(Value::as_str) {
        classes.push(format!("has-{fg}-color"));
        classes.push("has-text-color".to_string());
    }

    // Custom className
    let extra = extra_classes(attrs);
    let class_attr = format!("{}{}", classes.join(" "), extra);
    let style = color_style(attrs);

    // Strip wrapping <p> from inner_html if present, we provide our own
    let content = strip_outer_p(inner_html);

    format!("<p class=\"{class_attr}\"{style}>{content}</p>\n")
}

fn strip_outer_p(html: &str) -> &str {
    let trimmed = html.trim();
    if trimmed.starts_with("<p>") && trimmed.ends_with("</p>") {
        &trimmed[3..trimmed.len() - 4]
    } else if trimmed.starts_with("<p ") {
        // Has attributes — don't strip
        trimmed
    } else {
        trimmed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_basic_paragraph() {
        let html = render(&json!({}), "Hello World");
        assert!(html.contains("wp-block-paragraph"));
        assert!(html.contains("Hello World"));
    }

    #[test]
    fn test_paragraph_with_align() {
        let html = render(&json!({"textAlign": "center"}), "Centered");
        assert!(html.contains("has-text-align-center"));
    }

    #[test]
    fn test_paragraph_drop_cap() {
        let html = render(&json!({"dropCap": true}), "Drop cap");
        assert!(html.contains("has-drop-cap"));
    }

    #[test]
    fn test_paragraph_font_size() {
        let html = render(&json!({"fontSize": "large"}), "Big text");
        assert!(html.contains("has-large-font-size"));
    }

    #[test]
    fn test_paragraph_background_color() {
        let html = render(&json!({"backgroundColor": "primary"}), "Colored");
        assert!(html.contains("has-primary-background-color"));
        assert!(html.contains("has-background"));
    }

    #[test]
    fn test_paragraph_text_color() {
        let html = render(&json!({"textColor": "contrast"}), "Contrast");
        assert!(html.contains("has-contrast-color"));
    }

    #[test]
    fn test_paragraph_extra_classname() {
        let html = render(&json!({"className": "my-custom"}), "Custom");
        assert!(html.contains("my-custom"));
    }

    #[test]
    fn test_paragraph_strips_outer_p() {
        let html = render(&json!({}), "<p>Inner content</p>");
        // Should not double-wrap
        assert!(!html.contains("<p class=\"wp-block-paragraph\"><p>"));
        assert!(html.contains("Inner content"));
    }
}
