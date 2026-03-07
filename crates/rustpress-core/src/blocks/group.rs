//! core/group → `<div class="wp-block-group">...</div>`

use serde_json::Value;

use super::{align_class, color_style, extra_classes};

pub fn render(attrs: &Value, inner_html: &str) -> String {
    let mut classes = vec!["wp-block-group".to_string()];

    if let Some(ac) = align_class(attrs) {
        classes.push(ac.to_string());
    }

    // Layout type → is-layout-* class
    let layout_type = attrs
        .get("layout")
        .and_then(|l| l.get("type"))
        .and_then(Value::as_str)
        .unwrap_or("default");
    match layout_type {
        "flex" => {
            classes.push("is-layout-flex".to_string());
            classes.push("wp-block-group-is-layout-flex".to_string());
        }
        "constrained" => {
            classes.push("is-layout-constrained".to_string());
            classes.push("wp-block-group-is-layout-constrained".to_string());
        }
        _ => {
            classes.push("is-layout-flow".to_string());
            classes.push("wp-block-group-is-layout-flow".to_string());
        }
    }

    // Background / text colour utility classes
    if let Some(bg) = attrs.get("backgroundColor").and_then(Value::as_str) {
        classes.push(format!("has-{bg}-background-color"));
        classes.push("has-background".to_string());
    }
    if let Some(fg) = attrs.get("textColor").and_then(Value::as_str) {
        classes.push(format!("has-{fg}-color"));
        classes.push("has-text-color".to_string());
    }

    let extra = extra_classes(attrs);
    let class_attr = format!("{}{}", classes.join(" "), extra);
    let style = color_style(attrs);

    format!("<div class=\"{class_attr}\"{style}>{inner_html}</div>\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_group_basic() {
        let html = render(&json!({}), "<p>inner</p>");
        assert!(html.contains("wp-block-group"));
        assert!(html.contains("inner"));
    }

    #[test]
    fn test_group_flex_layout() {
        let html = render(&json!({"layout": {"type": "flex"}}), "");
        assert!(html.contains("is-layout-flex"));
    }

    #[test]
    fn test_group_constrained_layout() {
        let html = render(&json!({"layout": {"type": "constrained"}}), "");
        assert!(html.contains("is-layout-constrained"));
    }

    #[test]
    fn test_group_background_color() {
        let html = render(&json!({"backgroundColor": "primary"}), "");
        assert!(html.contains("has-primary-background-color"));
        assert!(html.contains("has-background"));
    }

    #[test]
    fn test_group_align_full() {
        let html = render(&json!({"align": "full"}), "");
        assert!(html.contains("alignfull"));
    }
}
