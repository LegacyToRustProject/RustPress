//! core/image → `<figure class="wp-block-image ..."><img ...></figure>`

use serde_json::Value;

use super::{align_class, extra_classes};

pub fn render(attrs: &Value, inner_html: &str) -> String {
    let mut classes = vec!["wp-block-image".to_string()];

    if let Some(ac) = align_class(attrs) {
        classes.push(ac.to_string());
    }

    // size-slug → size-{slug} class
    if let Some(size) = attrs.get("sizeSlug").and_then(Value::as_str) {
        classes.push(format!("size-{size}"));
    }

    // rounded corners style
    if attrs
        .get("style")
        .and_then(|s| s.get("border"))
        .and_then(|b| b.get("radius"))
        .is_some()
    {
        classes.push("has-custom-border".to_string());
    }

    let is_linked = attrs
        .get("linkDestination")
        .and_then(Value::as_str)
        .is_some()
        && attrs
            .get("linkDestination")
            .and_then(Value::as_str)
            .unwrap_or("none")
            != "none";

    let extra = extra_classes(attrs);
    let class_attr = format!("{}{}", classes.join(" "), extra);

    // If inner_html already contains the full <figure>, use it verbatim
    // but ensure our class is applied.
    let trimmed = inner_html.trim();
    if trimmed.starts_with("<figure") {
        // Inject our classes into the existing figure
        return inject_class(trimmed, &class_attr);
    }

    // Otherwise build from attrs
    let url = attrs.get("url").and_then(Value::as_str).unwrap_or("");
    let alt = attrs.get("alt").and_then(Value::as_str).unwrap_or("");
    let id = attrs.get("id").and_then(Value::as_u64);
    let id_attr = id
        .map(|i| format!(" class=\"wp-image-{i}\""))
        .unwrap_or_default();
    let width = attrs
        .get("width")
        .and_then(Value::as_u64)
        .map(|w| format!(" width=\"{w}\""))
        .unwrap_or_default();
    let height = attrs
        .get("height")
        .and_then(Value::as_u64)
        .map(|h| format!(" height=\"{h}\""))
        .unwrap_or_default();

    let img = format!("<img src=\"{url}\" alt=\"{alt}\"{id_attr}{width}{height} loading=\"lazy\">");

    let img_wrapped = if is_linked {
        let href = attrs.get("href").and_then(Value::as_str).unwrap_or(url);
        format!("<a href=\"{href}\">{img}</a>")
    } else {
        img
    };

    let caption = attrs.get("caption").and_then(Value::as_str).unwrap_or("");
    let caption_html = if !caption.is_empty() {
        format!("<figcaption class=\"wp-element-caption\">{caption}</figcaption>")
    } else {
        String::new()
    };

    format!("<figure class=\"{class_attr}\">{img_wrapped}{caption_html}</figure>\n")
}

fn inject_class(html: &str, classes: &str) -> String {
    if let Some(pos) = html.find('>') {
        let has_class = html[..pos].contains("class=");
        if has_class {
            // Append to existing class attribute
            html.replacen("class=\"", &format!("class=\"{classes} "), 1)
        } else {
            format!("{} class=\"{classes}\"{}", &html[..pos], &html[pos..])
        }
    } else {
        html.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_basic_image() {
        let html = render(&json!({"url": "/img.jpg", "alt": "Test"}), "");
        assert!(html.contains("wp-block-image"));
        assert!(html.contains("src=\"/img.jpg\""));
        assert!(html.contains("alt=\"Test\""));
    }

    #[test]
    fn test_image_size_slug() {
        let html = render(
            &json!({"url": "/img.jpg", "alt": "", "sizeSlug": "large"}),
            "",
        );
        assert!(html.contains("size-large"));
    }

    #[test]
    fn test_image_align_wide() {
        let html = render(&json!({"url": "/img.jpg", "alt": "", "align": "wide"}), "");
        assert!(html.contains("alignwide"));
    }

    #[test]
    fn test_image_with_caption() {
        let html = render(
            &json!({"url": "/img.jpg", "alt": "", "caption": "My caption"}),
            "",
        );
        assert!(html.contains("wp-element-caption"));
        assert!(html.contains("My caption"));
    }

    #[test]
    fn test_image_with_id() {
        let html = render(&json!({"url": "/img.jpg", "alt": "", "id": 42}), "");
        assert!(html.contains("wp-image-42"));
    }
}
