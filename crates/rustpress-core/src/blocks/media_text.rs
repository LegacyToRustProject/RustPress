//! core/media-text → two-column layout with media and text

use serde_json::Value;

use super::extra_classes;

pub fn render(attrs: &Value, inner_html: &str) -> String {
    let mut classes = vec!["wp-block-media-text".to_string()];

    // Media position (default: left)
    let is_stacked = attrs
        .get("isStackedOnMobile")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    if is_stacked {
        classes.push("is-stacked-on-mobile".to_string());
    }

    if attrs
        .get("mediaPosition")
        .and_then(Value::as_str)
        .unwrap_or("left")
        == "right"
    {
        classes.push("has-media-on-the-right".to_string());
    }

    if let Some(va) = attrs.get("verticalAlignment").and_then(Value::as_str) {
        classes.push(format!("is-vertically-aligned-{va}"));
    }

    if let Some(ac) = super::align_class(attrs) {
        classes.push(ac.to_string());
    }

    let extra = extra_classes(attrs);
    let class_attr = format!("{}{}", classes.join(" "), extra);

    // Media column width
    let media_width = attrs
        .get("mediaWidth")
        .and_then(Value::as_u64)
        .unwrap_or(50);
    let text_width = 100 - media_width;

    // Build media element
    let media_url = attrs.get("mediaUrl").and_then(Value::as_str).unwrap_or("");
    let media_alt = attrs.get("mediaAlt").and_then(Value::as_str).unwrap_or("");
    let media_type = attrs
        .get("mediaType")
        .and_then(Value::as_str)
        .unwrap_or("image");

    let media_html = if media_url.is_empty() {
        inner_html.to_string()
    } else if media_type == "video" {
        format!("<video src=\"{media_url}\" style=\"width:100%\" controls></video>")
    } else {
        format!("<img src=\"{media_url}\" alt=\"{media_alt}\" style=\"width:100%;height:auto\">")
    };

    // If inner_html already contains the markup, use it directly
    if inner_html.contains("wp-block-media-text__media") {
        return format!("<div class=\"{class_attr}\">{inner_html}</div>\n");
    }

    format!(
        "<div class=\"{class_attr}\">
<figure class=\"wp-block-media-text__media\" style=\"grid-column:1;flex-basis:{media_width}%\">{media_html}</figure>
<div class=\"wp-block-media-text__content\" style=\"flex-basis:{text_width}%\">{inner_html}</div>
</div>\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_media_text_basic() {
        let html = render(
            &json!({"mediaUrl": "/img.jpg", "mediaAlt": "Alt"}),
            "<p>Text</p>",
        );
        assert!(html.contains("wp-block-media-text"));
        assert!(html.contains("wp-block-media-text__media"));
        assert!(html.contains("wp-block-media-text__content"));
    }

    #[test]
    fn test_media_on_right() {
        let html = render(
            &json!({"mediaUrl": "/img.jpg", "mediaAlt": "", "mediaPosition": "right"}),
            "<p>Text</p>",
        );
        assert!(html.contains("has-media-on-the-right"));
    }

    #[test]
    fn test_video_media_type() {
        let html = render(
            &json!({"mediaUrl": "/video.mp4", "mediaAlt": "", "mediaType": "video"}),
            "",
        );
        assert!(html.contains("<video"));
    }
}
