//! core/cover → background image/video overlay block

use serde_json::Value;

use super::{align_class, extra_classes};

pub fn render(attrs: &Value, inner_html: &str) -> String {
    let mut classes = vec!["wp-block-cover".to_string()];

    if let Some(ac) = align_class(attrs) {
        classes.push(ac.to_string());
    }

    // Is dark or light overlay
    let is_light = attrs
        .get("isDark")
        .and_then(Value::as_bool)
        .map(|d| !d)
        .unwrap_or(false);
    if is_light {
        classes.push("is-light".to_string());
    }

    let extra = extra_classes(attrs);
    let class_attr = format!("{}{}", classes.join(" "), extra);

    // Background
    let url = attrs.get("url").and_then(Value::as_str).unwrap_or("");
    let bg_type = attrs
        .get("backgroundType")
        .and_then(Value::as_str)
        .unwrap_or("image");
    let dim_ratio = attrs.get("dimRatio").and_then(Value::as_u64).unwrap_or(50);

    // Overlay colour
    let overlay_color = attrs
        .get("overlayColor")
        .and_then(Value::as_str)
        .unwrap_or("");
    let overlay_class = if !overlay_color.is_empty() {
        format!(" has-{overlay_color}-background-color")
    } else {
        String::new()
    };

    let style = if !url.is_empty() && bg_type == "image" {
        format!(" style=\"background-image:url('{url}')\"")
    } else {
        String::new()
    };

    let min_height = attrs
        .get("minHeight")
        .and_then(Value::as_u64)
        .map(|h| format!(" style=\"min-height:{h}px\""))
        .unwrap_or_default();

    let video_html = if bg_type == "video" && !url.is_empty() {
        format!(
            "<video class=\"wp-block-cover__video-background\" autoplay muted loop src=\"{url}\"></video>"
        )
    } else {
        String::new()
    };

    let overlay = format!(
        "<span aria-hidden=\"true\" class=\"wp-block-cover__background has-background-dim has-background-dim-{dim_ratio}{overlay_class}\"></span>"
    );

    let inner =
        format!("<div class=\"wp-block-cover__inner-container is-layout-flow\">{inner_html}</div>");

    format!("<div class=\"{class_attr}\"{style}{min_height}>{video_html}{overlay}{inner}</div>\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_cover_basic() {
        let html = render(&json!({"url": "/bg.jpg"}), "<p>Text</p>");
        assert!(html.contains("wp-block-cover"));
        assert!(html.contains("background-image:url('/bg.jpg')"));
        assert!(html.contains("wp-block-cover__background"));
    }

    #[test]
    fn test_cover_dim_ratio() {
        let html = render(&json!({"url": "/bg.jpg", "dimRatio": 70}), "");
        assert!(html.contains("has-background-dim-70"));
    }

    #[test]
    fn test_cover_video() {
        let html = render(&json!({"url": "/bg.mp4", "backgroundType": "video"}), "");
        assert!(html.contains("wp-block-cover__video-background"));
        assert!(html.contains("<video"));
    }

    #[test]
    fn test_cover_overlay_color() {
        let html = render(&json!({"url": "/bg.jpg", "overlayColor": "primary"}), "");
        assert!(html.contains("has-primary-background-color"));
    }
}
