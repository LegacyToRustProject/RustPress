use super::{extra_classes, text_align_class};
use serde_json::Value;

pub fn render(attrs: &Value, inner_html: &str) -> String {
    let level = attrs
        .get("level")
        .and_then(|v| v.as_u64())
        .unwrap_or(2)
        .clamp(1, 6);

    let mut classes = vec![format!("wp-block-heading")];
    if let Some(ta) = text_align_class(attrs) {
        classes.push(ta);
    }
    if let Some(fs) = attrs.get("fontSize").and_then(|v| v.as_str()) {
        classes.push(format!("has-{}-font-size", fs));
    }
    if let Some(fg) = attrs.get("textColor").and_then(|v| v.as_str()) {
        classes.push(format!("has-{}-color", fg));
        classes.push("has-text-color".to_string());
    }
    let ec = extra_classes(attrs);
    let ec = ec.trim();
    if !ec.is_empty() {
        classes.push(ec.to_string());
    }

    let tag = format!("h{}", level);
    let content = strip_outer_heading(inner_html, level);

    format!("<{tag} class=\"{}\">{}</{tag}>", classes.join(" "), content)
}

fn strip_outer_heading(html: &str, level: u64) -> &str {
    let trimmed = html.trim();
    let open = format!("<h{}", level);
    let close = format!("</h{}>", level);
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
    fn test_default_h2() {
        let attrs = json!({});
        let out = render(&attrs, "<h2>Hello</h2>");
        assert!(out.starts_with("<h2"));
        assert!(out.contains("wp-block-heading"));
        assert!(out.contains("Hello"));
    }

    #[test]
    fn test_custom_level() {
        let attrs = json!({ "level": 3 });
        let out = render(&attrs, "<h3>Section</h3>");
        assert!(out.starts_with("<h3"));
        assert!(out.ends_with("</h3>"));
    }

    #[test]
    fn test_level_clamped() {
        let attrs = json!({ "level": 9 });
        let out = render(&attrs, "Bad level");
        assert!(out.starts_with("<h6"));
    }

    #[test]
    fn test_text_align() {
        let attrs = json!({ "level": 1, "textAlign": "center" });
        let out = render(&attrs, "<h1>Centered</h1>");
        assert!(out.contains("has-text-align-center"));
    }
}
