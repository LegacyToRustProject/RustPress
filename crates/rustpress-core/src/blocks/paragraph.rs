use serde_json::Value;
use super::{color_style, extra_classes, text_align_class};

pub fn render(attrs: &Value, inner_html: &str) -> String {
    let mut classes = vec!["wp-block-paragraph".to_string()];

    if let Some(ta) = text_align_class(attrs) {
        classes.push(ta);
    }
    if attrs.get("dropCap").and_then(|v| v.as_bool()).unwrap_or(false) {
        classes.push("has-drop-cap".to_string());
    }
    if let Some(fs) = attrs.get("fontSize").and_then(|v| v.as_str()) {
        classes.push(format!("has-{}-font-size", fs));
    }
    if let Some(bg) = attrs.get("backgroundColor").and_then(|v| v.as_str()) {
        classes.push(format!("has-{}-background-color", bg));
        classes.push("has-background".to_string());
    }
    if let Some(fg) = attrs.get("textColor").and_then(|v| v.as_str()) {
        classes.push(format!("has-{}-color", fg));
        classes.push("has-text-color".to_string());
    }
    classes.push(extra_classes(attrs).trim().to_string());
    let classes: Vec<&str> = classes.iter().map(|s| s.as_str()).filter(|s| !s.is_empty()).collect();

    let style = color_style(attrs);
    let style_attr = if style.is_empty() { String::new() } else { format!(" style=\"{}\"", style) };

    // Strip outer <p>...</p> from inner_html to avoid double-wrapping
    let content = strip_outer_tag(inner_html, "p");

    format!(
        "<p class=\"{}\"{}>{}</p>",
        classes.join(" "),
        style_attr,
        content
    )
}

fn strip_outer_tag<'a>(html: &'a str, tag: &str) -> &'a str {
    let trimmed = html.trim();
    let open = format!("<{}", tag);
    let close = format!("</{}>", tag);
    if trimmed.starts_with(&open) {
        if let Some(content_start) = trimmed.find('>') {
            let after_open = &trimmed[content_start + 1..];
            if let Some(stripped) = after_open.strip_suffix(&close) {
                return stripped;
            }
        }
    }
    html
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_basic_paragraph() {
        let attrs = json!({});
        let out = render(&attrs, "<p>Hello world</p>");
        assert!(out.contains("wp-block-paragraph"));
        assert!(out.contains("Hello world"));
    }

    #[test]
    fn test_text_align() {
        let attrs = json!({ "textAlign": "center" });
        let out = render(&attrs, "<p>Centered</p>");
        assert!(out.contains("has-text-align-center"));
    }

    #[test]
    fn test_drop_cap() {
        let attrs = json!({ "dropCap": true });
        let out = render(&attrs, "<p>Drop cap</p>");
        assert!(out.contains("has-drop-cap"));
    }

    #[test]
    fn test_font_size() {
        let attrs = json!({ "fontSize": "large" });
        let out = render(&attrs, "<p>Big text</p>");
        assert!(out.contains("has-large-font-size"));
    }

    #[test]
    fn test_background_color() {
        let attrs = json!({ "backgroundColor": "primary" });
        let out = render(&attrs, "<p>Colored</p>");
        assert!(out.contains("has-primary-background-color"));
        assert!(out.contains("has-background"));
    }

    #[test]
    fn test_text_color() {
        let attrs = json!({ "textColor": "contrast" });
        let out = render(&attrs, "<p>Text</p>");
        assert!(out.contains("has-contrast-color"));
        assert!(out.contains("has-text-color"));
    }

    #[test]
    fn test_custom_class() {
        let attrs = json!({ "className": "my-custom-class" });
        let out = render(&attrs, "<p>Custom</p>");
        assert!(out.contains("my-custom-class"));
    }

    #[test]
    fn test_no_double_p_tag() {
        let attrs = json!({});
        let out = render(&attrs, "<p>Only one p</p>");
        assert_eq!(out.matches("<p").count(), 1);
    }
}
