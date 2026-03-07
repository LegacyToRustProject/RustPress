//! core/list → `<ul/ol class="wp-block-list">` + core/list-item → `<li>`

use serde_json::Value;

use super::extra_classes;

pub fn render(attrs: &Value, inner_html: &str) -> String {
    let ordered = attrs
        .get("ordered")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let tag = if ordered { "ol" } else { "ul" };

    let classes = ["wp-block-list"];
    let extra = extra_classes(attrs);
    let class_attr = format!("{}{}", classes.join(" "), extra);

    // start attribute for ordered lists
    let start_attr = if ordered {
        attrs
            .get("start")
            .and_then(Value::as_u64)
            .filter(|&s| s != 1)
            .map(|s| format!(" start=\"{s}\""))
            .unwrap_or_default()
    } else {
        String::new()
    };

    format!("<{tag} class=\"{class_attr}\"{start_attr}>{inner_html}</{tag}>\n")
}

pub fn render_item(_attrs: &Value, inner_html: &str) -> String {
    format!("<li>{inner_html}</li>\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_unordered_list() {
        let html = render(&json!({}), "<li>A</li><li>B</li>");
        assert!(html.contains("<ul"));
        assert!(html.contains("wp-block-list"));
        assert!(html.contains("<li>A</li>"));
    }

    #[test]
    fn test_ordered_list() {
        let html = render(&json!({"ordered": true}), "<li>A</li>");
        assert!(html.contains("<ol"));
        assert!(!html.contains("<ul"));
    }

    #[test]
    fn test_ordered_list_with_start() {
        let html = render(&json!({"ordered": true, "start": 5}), "<li>A</li>");
        assert!(html.contains("start=\"5\""));
    }

    #[test]
    fn test_list_item() {
        let html = render_item(&json!({}), "Hello");
        assert_eq!(html.trim(), "<li>Hello</li>");
    }
}
