//! core/code → `<pre class="wp-block-code"><code>...</code></pre>`

use serde_json::Value;

use super::extra_classes;

pub fn render(attrs: &Value, inner_html: &str) -> String {
    let extra = extra_classes(attrs);
    let class_attr = format!("wp-block-code{extra}");

    // Extract language for highlighting hint
    let lang = attrs.get("language").and_then(Value::as_str).unwrap_or("");
    let code_class = if !lang.is_empty() {
        format!(" class=\"language-{lang}\"")
    } else {
        String::new()
    };

    // Strip outer <pre><code> if present in inner_html
    let content = extract_code_content(inner_html);

    format!("<pre class=\"{class_attr}\"><code{code_class}>{content}</code></pre>\n")
}

fn extract_code_content(html: &str) -> &str {
    let trimmed = html.trim();
    // Strip <pre class="wp-block-code"><code> wrapper if present
    if let Some(start) = trimmed.find("<code") {
        if let Some(code_end) = trimmed[start..].find('>') {
            let after_tag = &trimmed[start + code_end + 1..];
            if let Some(close) = after_tag.rfind("</code>") {
                return &after_tag[..close];
            }
        }
    }
    // Just strip outer <pre>...</pre>
    if trimmed.starts_with("<pre") {
        if let Some(open_end) = trimmed.find('>') {
            let after_pre = &trimmed[open_end + 1..];
            if let Some(close) = after_pre.rfind("</pre>") {
                return &after_pre[..close];
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
    fn test_basic_code() {
        let html = render(&json!({}), "let x = 1;");
        assert!(html.contains("wp-block-code"));
        assert!(html.contains("<pre"));
        assert!(html.contains("<code>"));
        assert!(html.contains("let x = 1;"));
    }

    #[test]
    fn test_code_with_language() {
        let html = render(&json!({"language": "rust"}), "fn main() {}");
        assert!(html.contains("language-rust"));
    }

    #[test]
    fn test_code_extra_class() {
        let html = render(&json!({"className": "highlight"}), "code");
        assert!(html.contains("highlight"));
    }
}
