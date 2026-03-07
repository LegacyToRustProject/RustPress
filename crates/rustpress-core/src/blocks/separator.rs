//! core/separator → `<hr class="wp-block-separator">`

use serde_json::Value;

use super::extra_classes;

pub fn render(attrs: &Value, _inner_html: &str) -> String {
    let mut classes = vec!["wp-block-separator".to_string()];

    // Style variations: wide, dots (default is plain line)
    if let Some(cn) = attrs.get("className").and_then(Value::as_str) {
        if cn.contains("is-style-wide") {
            classes.push("is-style-wide".to_string());
        } else if cn.contains("is-style-dots") {
            classes.push("is-style-dots".to_string());
        }
    }

    if let Some(ac) = super::align_class(attrs) {
        classes.push(ac.to_string());
    }

    // Colour class
    if let Some(color) = attrs.get("backgroundColor").and_then(Value::as_str) {
        classes.push(format!("has-{color}-background-color"));
        classes.push("has-background".to_string());
    }

    let extra = extra_classes(attrs);
    let class_attr = format!("{}{}", classes.join(" "), extra);

    format!("<hr class=\"{class_attr}\">\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_basic_separator() {
        let html = render(&json!({}), "");
        assert!(html.contains("<hr"));
        assert!(html.contains("wp-block-separator"));
    }

    #[test]
    fn test_separator_dots() {
        let html = render(&json!({"className": "is-style-dots"}), "");
        assert!(html.contains("is-style-dots"));
    }

    #[test]
    fn test_separator_background_color() {
        let html = render(&json!({"backgroundColor": "primary"}), "");
        assert!(html.contains("has-primary-background-color"));
    }
}
