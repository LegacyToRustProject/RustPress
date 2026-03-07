//! core/spacer → `<div style="height:Xpx" class="wp-block-spacer">`

use serde_json::Value;

pub fn render(attrs: &Value, _inner_html: &str) -> String {
    let height = attrs
        .get("height")
        .and_then(|h| {
            if let Some(s) = h.as_str() {
                // Already has unit (e.g. "50px" or "2em")
                return Some(s.to_string());
            }
            h.as_u64().map(|n| format!("{n}px"))
        })
        .unwrap_or_else(|| "100px".to_string());

    let width = attrs
        .get("width")
        .and_then(|w| {
            if let Some(s) = w.as_str() {
                return Some(format!("width:{s};"));
            }
            w.as_u64().map(|n| format!("width:{n}px;"))
        })
        .unwrap_or_default();

    format!(
        "<div style=\"{width}height:{height}\" aria-hidden=\"true\" class=\"wp-block-spacer\"></div>\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_spacer_default() {
        let html = render(&json!({}), "");
        assert!(html.contains("wp-block-spacer"));
        assert!(html.contains("height:100px"));
    }

    #[test]
    fn test_spacer_with_numeric_height() {
        let html = render(&json!({"height": 50}), "");
        assert!(html.contains("height:50px"));
    }

    #[test]
    fn test_spacer_with_string_height() {
        let html = render(&json!({"height": "3rem"}), "");
        assert!(html.contains("height:3rem"));
    }
}
