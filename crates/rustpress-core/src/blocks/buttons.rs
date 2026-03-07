//! core/buttons (wrapper) + core/button

use serde_json::Value;

use super::extra_classes;

/// Render the `core/buttons` wrapper div.
pub fn render_wrapper(attrs: &Value, inner_html: &str) -> String {
    let mut classes = vec!["wp-block-buttons".to_string()];

    if let Some(layout) = attrs
        .get("layout")
        .and_then(|l| l.get("justifyContent"))
        .and_then(Value::as_str)
    {
        match layout {
            "center" => classes.push("is-content-justification-center".to_string()),
            "right" => classes.push("is-content-justification-right".to_string()),
            "left" => classes.push("is-content-justification-left".to_string()),
            "space-between" => classes.push("is-content-justification-space-between".to_string()),
            _ => {}
        }
    }

    let extra = extra_classes(attrs);
    let class_attr = format!("{}{}", classes.join(" "), extra);

    format!("<div class=\"{class_attr}\">{inner_html}</div>\n")
}

/// Render a single `core/button`.
pub fn render_button(attrs: &Value, inner_html: &str) -> String {
    let mut wrapper_classes = vec!["wp-block-button".to_string()];

    // Outline style
    if let Some(cn) = attrs.get("className").and_then(Value::as_str) {
        if cn.contains("is-style-outline") {
            wrapper_classes.push("is-style-outline".to_string());
        }
    }

    let extra = extra_classes(attrs);
    let wrapper_class = format!("{}{}", wrapper_classes.join(" "), extra);

    let url = attrs.get("url").and_then(Value::as_str).unwrap_or("#");
    let text = extract_button_text(inner_html);

    // Build link classes
    let mut link_classes = vec![
        "wp-block-button__link".to_string(),
        "wp-element-button".to_string(),
    ];
    if let Some(bg) = attrs.get("backgroundColor").and_then(Value::as_str) {
        link_classes.push(format!("has-{bg}-background-color"));
        link_classes.push("has-background".to_string());
    }
    if let Some(fg) = attrs.get("textColor").and_then(Value::as_str) {
        link_classes.push(format!("has-{fg}-color"));
        link_classes.push("has-text-color".to_string());
    }

    let rel = if attrs
        .get("rel")
        .and_then(Value::as_str)
        .unwrap_or("")
        .contains("noreferrer")
    {
        " rel=\"noreferrer noopener\""
    } else {
        ""
    };

    let target = if attrs
        .get("linkTarget")
        .and_then(Value::as_str)
        .unwrap_or("")
        == "_blank"
    {
        " target=\"_blank\""
    } else {
        ""
    };

    let link_class = link_classes.join(" ");

    format!(
        "<div class=\"{wrapper_class}\"><a class=\"{link_class}\" href=\"{url}\"{target}{rel}>{text}</a></div>\n"
    )
}

fn extract_button_text(html: &str) -> &str {
    let trimmed = html.trim();
    // Try to get text inside the innermost <a> or return raw
    if let Some(start) = trimmed.rfind('>') {
        let after = &trimmed[start + 1..];
        if let Some(end) = after.find('<') {
            let text = &after[..end];
            if !text.is_empty() {
                return text;
            }
        }
    }
    trimmed
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_button_wrapper() {
        let html = render_wrapper(&json!({}), "<div class=\"wp-block-button\">...</div>");
        assert!(html.contains("wp-block-buttons"));
    }

    #[test]
    fn test_button_basic() {
        let html = render_button(&json!({"url": "/go"}), "Click me");
        assert!(html.contains("wp-block-button__link"));
        assert!(html.contains("href=\"/go\""));
        assert!(html.contains("wp-element-button"));
    }

    #[test]
    fn test_button_outline_style() {
        let html = render_button(&json!({"url": "/", "className": "is-style-outline"}), "Go");
        assert!(html.contains("is-style-outline"));
    }

    #[test]
    fn test_button_new_tab() {
        let html = render_button(&json!({"url": "/", "linkTarget": "_blank"}), "Go");
        assert!(html.contains("target=\"_blank\""));
    }
}
