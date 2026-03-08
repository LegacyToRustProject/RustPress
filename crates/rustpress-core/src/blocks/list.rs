use super::extra_classes;
use serde_json::Value;

pub fn render(attrs: &Value, inner_html: &str) -> String {
    let ordered = attrs
        .get("ordered")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let tag = if ordered { "ol" } else { "ul" };

    let mut classes = vec!["wp-block-list".to_string()];
    let ec = extra_classes(attrs);
    let ec = ec.trim();
    if !ec.is_empty() {
        classes.push(ec.to_string());
    }

    let start_attr = if ordered {
        let start = attrs.get("start").and_then(|v| v.as_u64()).unwrap_or(1);
        if start != 1 {
            format!(" start=\"{}\"", start)
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    // If inner_html already has the list element, wrap our classes around it
    let trimmed = inner_html.trim();
    let expected_open = format!("<{}", tag);
    if trimmed.starts_with(&expected_open) {
        // Inject class into existing element
        let class_str = classes.join(" ");
        return inject_class(trimmed, &class_str, tag, &start_attr);
    }

    format!(
        "<{tag} class=\"{}\"{start_attr}>{}</{tag}>",
        classes.join(" "),
        inner_html
    )
}

pub fn render_item(_attrs: &Value, inner_html: &str) -> String {
    let trimmed = inner_html.trim();
    if trimmed.starts_with("<li") {
        return trimmed.to_string();
    }
    format!("<li>{}</li>", inner_html)
}

fn inject_class(html: &str, extra: &str, tag: &str, start_attr: &str) -> String {
    let open = format!("<{}", tag);
    if let Some(pos) = html.find(&open) {
        let after = &html[pos + open.len()..];
        if let Some(cls_pos) = after.find("class=\"") {
            let insert_at = pos + open.len() + cls_pos + 7;
            let mut result = html.to_string();
            result.insert_str(insert_at, &format!("{} ", extra));
            return result;
        }
        let insert_at = pos + open.len();
        let mut result = html.to_string();
        result.insert_str(insert_at, &format!(" class=\"{}\"", extra));
        if !start_attr.is_empty() {
            result.insert_str(
                insert_at + format!(" class=\"{}\"", extra).len(),
                start_attr,
            );
        }
        return result;
    }
    html.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_unordered_list() {
        let attrs = json!({});
        let out = render(&attrs, "<li>Item 1</li><li>Item 2</li>");
        assert!(out.contains("<ul"));
        assert!(out.contains("wp-block-list"));
        assert!(out.contains("Item 1"));
    }

    #[test]
    fn test_ordered_list() {
        let attrs = json!({ "ordered": true });
        let out = render(&attrs, "<li>Item</li>");
        assert!(out.contains("<ol"));
    }

    #[test]
    fn test_ordered_with_start() {
        let attrs = json!({ "ordered": true, "start": 5 });
        let out = render(&attrs, "<li>Item</li>");
        assert!(out.contains("start=\"5\""));
    }

    #[test]
    fn test_render_item() {
        let attrs = json!({});
        let out = render_item(&attrs, "List item text");
        assert!(out.contains("<li>"));
        assert!(out.contains("List item text"));
    }
}
