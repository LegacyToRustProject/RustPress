//! core/embed → oEmbed wrapper (YouTube, Twitter, etc.)
//!
//! RustPress does not perform live oEmbed fetching at render time.
//! Instead it wraps the provider URL in a responsive iframe or figure,
//! matching WordPress's output structure.

use serde_json::Value;

use super::{align_class, extra_classes};

pub fn render(attrs: &Value, inner_html: &str) -> String {
    let url = attrs.get("url").and_then(Value::as_str).unwrap_or("");
    let provider = attrs
        .get("providerNameSlug")
        .and_then(Value::as_str)
        .unwrap_or("embed");
    let caption = attrs.get("caption").and_then(Value::as_str).unwrap_or("");

    let mut classes = vec![
        "wp-block-embed".to_string(),
        format!("wp-block-embed-{provider}"),
    ];

    if let Some(ac) = align_class(attrs) {
        classes.push(ac.to_string());
    }

    let extra = extra_classes(attrs);
    let class_attr = format!("{}{}", classes.join(" "), extra);

    // If inner_html is already an iframe or embed markup, use it
    let embed_html = if !inner_html.trim().is_empty() {
        inner_html.to_string()
    } else if !url.is_empty() {
        build_embed_html(url, provider)
    } else {
        String::new()
    };

    let caption_html = if !caption.is_empty() {
        format!("<figcaption class=\"wp-element-caption\">{caption}</figcaption>")
    } else {
        String::new()
    };

    format!(
        "<figure class=\"{class_attr}\"><div class=\"wp-block-embed__wrapper\">{embed_html}</div>{caption_html}</figure>\n"
    )
}

fn build_embed_html(url: &str, provider: &str) -> String {
    match provider {
        "youtube" => {
            let video_id = extract_youtube_id(url).unwrap_or(url);
            format!(
                "<iframe loading=\"lazy\" src=\"https://www.youtube-nocookie.com/embed/{video_id}\" \
                frameborder=\"0\" allow=\"accelerometer;autoplay;clipboard-write;encrypted-media;gyroscope;picture-in-picture\" \
                allowfullscreen></iframe>"
            )
        }
        "vimeo" => {
            let video_id = url.rsplit('/').next().unwrap_or(url);
            format!(
                "<iframe loading=\"lazy\" src=\"https://player.vimeo.com/video/{video_id}\" \
                frameborder=\"0\" allow=\"autoplay;fullscreen;picture-in-picture\" allowfullscreen></iframe>"
            )
        }
        _ => {
            // Generic fallback: link
            format!("<a href=\"{url}\">{url}</a>")
        }
    }
}

fn extract_youtube_id(url: &str) -> Option<&str> {
    // https://youtu.be/VIDEO_ID or ?v=VIDEO_ID
    if let Some(pos) = url.find("youtu.be/") {
        return url[pos + 9..].split('?').next();
    }
    if let Some(pos) = url.find("v=") {
        return url[pos + 2..].split('&').next();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_embed_basic() {
        let html = render(
            &json!({"url": "https://www.youtube.com/watch?v=dQw4w9WgXcQ", "providerNameSlug": "youtube"}),
            "",
        );
        assert!(html.contains("wp-block-embed"));
        assert!(html.contains("wp-block-embed-youtube"));
        assert!(html.contains("iframe"));
    }

    #[test]
    fn test_embed_with_inner_html() {
        let html = render(
            &json!({"url": "https://example.com", "providerNameSlug": "example"}),
            "<iframe src=\"https://example.com\"></iframe>",
        );
        assert!(html.contains("iframe"));
    }

    #[test]
    fn test_embed_caption() {
        let html = render(
            &json!({"url": "https://youtu.be/abc", "providerNameSlug": "youtube", "caption": "My video"}),
            "",
        );
        assert!(html.contains("wp-element-caption"));
        assert!(html.contains("My video"));
    }

    #[test]
    fn test_embed_vimeo() {
        let html = render(
            &json!({"url": "https://vimeo.com/123456", "providerNameSlug": "vimeo"}),
            "",
        );
        assert!(html.contains("player.vimeo.com"));
    }
}
