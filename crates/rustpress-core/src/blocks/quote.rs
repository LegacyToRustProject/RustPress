//! core/quote → `<blockquote class="wp-block-quote">...</blockquote>`

use serde_json::Value;

use super::extra_classes;

pub fn render(attrs: &Value, inner_html: &str) -> String {
    let mut classes = vec!["wp-block-quote".to_string()];

    // TT style variations: plain, large
    if let Some(style) = attrs.get("className").and_then(Value::as_str) {
        if style.contains("is-style-large") {
            classes.push("is-style-large".to_string());
        } else if style.contains("is-style-plain") {
            classes.push("is-style-plain".to_string());
        }
    }

    let extra = extra_classes(attrs);
    // avoid double-adding if already in extra_classes
    let class_attr = format!("{}{}", classes.join(" "), extra);

    // citation
    let citation = attrs.get("citation").and_then(Value::as_str).unwrap_or("");
    let cite_html = if !citation.is_empty() {
        format!("<cite>{citation}</cite>")
    } else {
        String::new()
    };

    format!("<blockquote class=\"{class_attr}\">{inner_html}{cite_html}</blockquote>\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_basic_quote() {
        let html = render(&json!({}), "<p>Great words</p>");
        assert!(html.contains("wp-block-quote"));
        assert!(html.contains("Great words"));
        assert!(html.contains("<blockquote"));
    }

    #[test]
    fn test_quote_with_citation() {
        let html = render(&json!({"citation": "Someone Famous"}), "<p>Quote</p>");
        assert!(html.contains("<cite>Someone Famous</cite>"));
    }

    #[test]
    fn test_large_style() {
        let html = render(&json!({"className": "is-style-large"}), "<p>Big</p>");
        assert!(html.contains("is-style-large"));
    }
}
