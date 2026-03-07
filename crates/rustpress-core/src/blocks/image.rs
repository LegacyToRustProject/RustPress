use serde_json::Value;
use super::{align_class, extra_classes};

pub fn render(attrs: &Value, inner_html: &str) -> String {
    let url = attrs.get("url").and_then(|v| v.as_str()).unwrap_or("");
    let alt = attrs.get("alt").and_then(|v| v.as_str()).unwrap_or("");
    let size_slug = attrs.get("sizeSlug").and_then(|v| v.as_str()).unwrap_or("full");
    let caption = attrs.get("caption").and_then(|v| v.as_str()).unwrap_or("");

    let mut figure_classes = vec!["wp-block-image".to_string()];
    figure_classes.push(format!("size-{}", size_slug));
    if let Some(ac) = align_class(attrs) {
        figure_classes.push(ac.to_string());
    }
    let ec = extra_classes(attrs);
    let ec = ec.trim();
    if !ec.is_empty() {
        figure_classes.push(ec.to_string());
    }

    // If inner_html already has a figure, inject our classes
    if inner_html.trim_start().starts_with("<figure") {
        return inject_class(inner_html, &figure_classes.join(" "));
    }

    let mut img_attrs = format!("src=\"{}\" alt=\"{}\"", url, alt);
    if let Some(w) = attrs.get("width").and_then(|v| v.as_u64()) {
        img_attrs.push_str(&format!(" width=\"{}\"", w));
    }
    if let Some(h) = attrs.get("height").and_then(|v| v.as_u64()) {
        img_attrs.push_str(&format!(" height=\"{}\"", h));
    }

    let img = format!("<img {img_attrs} />");
    let img_wrapped = match attrs.get("linkDestination").and_then(|v| v.as_str()) {
        Some("media") if !url.is_empty() => format!("<a href=\"{}\">{}</a>", url, img),
        Some(href) if !href.is_empty() => format!("<a href=\"{}\">{}</a>", href, img),
        _ => img,
    };

    if caption.is_empty() {
        format!("<figure class=\"{}\">{}</figure>", figure_classes.join(" "), img_wrapped)
    } else {
        format!(
            "<figure class=\"{}\">{}<figcaption class=\"wp-element-caption\">{}</figcaption></figure>",
            figure_classes.join(" "),
            img_wrapped,
            caption
        )
    }
}

fn inject_class(html: &str, extra: &str) -> String {
    if let Some(pos) = html.find("<figure") {
        let after = &html[pos + 7..];
        if let Some(cls_pos) = after.find("class=\"") {
            let insert_at = pos + 7 + cls_pos + 7;
            let mut result = html.to_string();
            result.insert_str(insert_at, &format!("{} ", extra));
            return result;
        }
        // No class attr — add one
        let insert_at = pos + 7;
        let mut result = html.to_string();
        result.insert_str(insert_at, &format!(" class=\"{}\"", extra));
        return result;
    }
    html.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_basic_image() {
        let attrs = json!({ "url": "https://example.com/img.jpg", "alt": "Test" });
        let out = render(&attrs, "");
        assert!(out.contains("wp-block-image"));
        assert!(out.contains("src=\"https://example.com/img.jpg\""));
        assert!(out.contains("alt=\"Test\""));
    }

    #[test]
    fn test_size_slug() {
        let attrs = json!({ "url": "img.jpg", "sizeSlug": "medium" });
        let out = render(&attrs, "");
        assert!(out.contains("size-medium"));
    }

    #[test]
    fn test_caption() {
        let attrs = json!({ "url": "img.jpg", "caption": "My caption" });
        let out = render(&attrs, "");
        assert!(out.contains("My caption"));
        assert!(out.contains("figcaption"));
    }

    #[test]
    fn test_align() {
        let attrs = json!({ "url": "img.jpg", "align": "wide" });
        let out = render(&attrs, "");
        assert!(out.contains("alignwide"));
    }

    #[test]
    fn test_inject_class_to_existing_figure() {
        let attrs = json!({ "align": "center", "sizeSlug": "large" });
        let inner = "<figure class=\"\"><img src=\"img.jpg\" /></figure>";
        let out = render(&attrs, inner);
        assert!(out.contains("wp-block-image"));
    }
}
