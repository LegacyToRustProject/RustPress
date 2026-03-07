//! core/heading → `<h1-h6 class="wp-block-heading ...">...</h1-h6>`

use serde_json::Value;

use super::{align_class, extra_classes, text_align_class};

pub fn render(attrs: &Value, inner_html: &str) -> String {
    let level = attrs
        .get("level")
        .and_then(Value::as_u64)
        .unwrap_or(2)
        .clamp(1, 6);

    let mut classes = vec!["wp-block-heading".to_string()];

    if let Some(ac) = align_class(attrs) {
        classes.push(ac.to_string());
    }
    if let Some(ta) = text_align_class(attrs) {
        classes.push(ta);
    }
    if let Some(size) = attrs.get("fontSize").and_then(Value::as_str) {
        classes.push(format!("has-{size}-font-size"));
    }
    if let Some(fg) = attrs.get("textColor").and_then(Value::as_str) {
        classes.push(format!("has-{fg}-color"));
        classes.push("has-text-color".to_string());
    }

    let extra = extra_classes(attrs);
    let class_attr = format!("{}{}", classes.join(" "), extra);

    let content = strip_outer_heading(inner_html, level);

    format!("<h{level} class=\"{class_attr}\">{content}</h{level}>\n")
}

fn strip_outer_heading(html: &str, level: u64) -> &str {
    let trimmed = html.trim();
    let open = format!("<h{level}>");
    let close = format!("</h{level}>");
    if trimmed.starts_with(&open) && trimmed.ends_with(&close) {
        &trimmed[open.len()..trimmed.len() - close.len()]
    } else {
        trimmed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_default_h2() {
        let html = render(&json!({}), "Hello");
        assert!(html.starts_with("<h2"));
        assert!(html.contains("wp-block-heading"));
        assert!(html.contains("Hello"));
    }

    #[test]
    fn test_h1_level() {
        let html = render(&json!({"level": 1}), "Title");
        assert!(html.starts_with("<h1"));
        assert!(html.contains("</h1>"));
    }

    #[test]
    fn test_h3_with_align() {
        let html = render(&json!({"level": 3, "textAlign": "right"}), "Right");
        assert!(html.starts_with("<h3"));
        assert!(html.contains("has-text-align-right"));
    }

    #[test]
    fn test_level_clamped_to_6() {
        let html = render(&json!({"level": 9}), "Clamped");
        assert!(html.starts_with("<h6"));
    }
}
