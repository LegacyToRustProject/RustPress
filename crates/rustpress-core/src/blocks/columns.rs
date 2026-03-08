//! core/columns wrapper + core/column

use serde_json::Value;

use super::extra_classes;

pub fn render_wrapper(attrs: &Value, inner_html: &str) -> String {
    let mut classes = vec!["wp-block-columns".to_string()];

    if attrs
        .get("isStackedOnMobile")
        .and_then(Value::as_bool)
        .unwrap_or(true)
    {
        classes.push("is-layout-flex".to_string());
    }

    if let Some(ac) = super::align_class(attrs) {
        classes.push(ac.to_string());
    }

    let extra = extra_classes(attrs);
    let class_attr = format!("{}{}", classes.join(" "), extra);

    format!("<div class=\"{class_attr}\">{inner_html}</div>\n")
}

pub fn render_column(attrs: &Value, inner_html: &str) -> String {
    let classes = ["wp-block-column"];

    // Width as flex-basis style
    let style = attrs
        .get("width")
        .and_then(Value::as_str)
        .map(|w| format!(" style=\"flex-basis:{w}\""))
        .unwrap_or_default();

    let extra = extra_classes(attrs);
    let class_attr = format!("{}{}", classes.join(" "), extra);

    format!("<div class=\"{class_attr}\"{style}>{inner_html}</div>\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_columns_wrapper() {
        let html = render_wrapper(&json!({}), "<div>col</div>");
        assert!(html.contains("wp-block-columns"));
    }

    #[test]
    fn test_column() {
        let html = render_column(&json!({}), "<p>Content</p>");
        assert!(html.contains("wp-block-column"));
        assert!(html.contains("Content"));
    }

    #[test]
    fn test_column_with_width() {
        let html = render_column(&json!({"width": "33.33%"}), "A");
        assert!(html.contains("flex-basis:33.33%"));
    }
}
