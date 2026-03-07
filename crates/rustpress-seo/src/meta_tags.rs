use regex::Regex;
use serde::{Deserialize, Serialize};

/// Holds all SEO meta information for a page.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SeoMeta {
    pub title: Option<String>,
    pub description: Option<String>,
    pub canonical: Option<String>,
    pub robots: Option<String>,
    pub og_title: Option<String>,
    pub og_description: Option<String>,
    pub og_image: Option<String>,
    pub og_url: Option<String>,
    pub og_type: Option<String>,
    pub og_site_name: Option<String>,
    pub twitter_card: Option<String>,
    pub twitter_title: Option<String>,
    pub twitter_description: Option<String>,
    pub twitter_image: Option<String>,
}

/// Generates an HTML string of meta tags from an `SeoMeta` struct.
pub fn generate_meta_tags(meta: &SeoMeta) -> String {
    let mut tags = Vec::new();

    if let Some(ref desc) = meta.description {
        tags.push(format!(
            r#"<meta name="description" content="{}" />"#,
            escape_attr(desc)
        ));
    }

    if let Some(ref canonical) = meta.canonical {
        tags.push(format!(r#"<link rel="canonical" href="{}" />"#, escape_attr(canonical)));
    }

    if let Some(ref robots) = meta.robots {
        tags.push(format!(
            r#"<meta name="robots" content="{}" />"#,
            escape_attr(robots)
        ));
    }

    // Open Graph tags
    if let Some(ref v) = meta.og_title {
        tags.push(format!(r#"<meta property="og:title" content="{}" />"#, escape_attr(v)));
    }
    if let Some(ref v) = meta.og_description {
        tags.push(format!(r#"<meta property="og:description" content="{}" />"#, escape_attr(v)));
    }
    if let Some(ref v) = meta.og_image {
        tags.push(format!(r#"<meta property="og:image" content="{}" />"#, escape_attr(v)));
    }
    if let Some(ref v) = meta.og_url {
        tags.push(format!(r#"<meta property="og:url" content="{}" />"#, escape_attr(v)));
    }
    if let Some(ref v) = meta.og_type {
        tags.push(format!(r#"<meta property="og:type" content="{}" />"#, escape_attr(v)));
    }
    if let Some(ref v) = meta.og_site_name {
        tags.push(format!(r#"<meta property="og:site_name" content="{}" />"#, escape_attr(v)));
    }

    // Twitter Card tags
    if let Some(ref v) = meta.twitter_card {
        tags.push(format!(r#"<meta name="twitter:card" content="{}" />"#, escape_attr(v)));
    }
    if let Some(ref v) = meta.twitter_title {
        tags.push(format!(r#"<meta name="twitter:title" content="{}" />"#, escape_attr(v)));
    }
    if let Some(ref v) = meta.twitter_description {
        tags.push(format!(r#"<meta name="twitter:description" content="{}" />"#, escape_attr(v)));
    }
    if let Some(ref v) = meta.twitter_image {
        tags.push(format!(r#"<meta name="twitter:image" content="{}" />"#, escape_attr(v)));
    }

    tags.join("\n")
}

/// Strips HTML tags from content and truncates to `max_len` characters,
/// breaking at a word boundary when possible. Appends "..." if truncated.
pub fn auto_generate_description(content: &str, max_len: usize) -> String {
    // Strip HTML tags
    let tag_re = Regex::new(r"<[^>]+>").expect("valid regex");
    let stripped = tag_re.replace_all(content, "");

    // Collapse whitespace
    let ws_re = Regex::new(r"\s+").expect("valid regex");
    let cleaned = ws_re.replace_all(&stripped, " ");
    let cleaned = cleaned.trim();

    if cleaned.len() <= max_len {
        return cleaned.to_string();
    }

    // Truncate at a word boundary
    let truncated = &cleaned[..max_len];
    match truncated.rfind(' ') {
        Some(pos) => format!("{}...", &truncated[..pos]),
        None => format!("{}...", truncated),
    }
}

/// Generates an SEO title in the format "Post Title {separator} Site Name".
pub fn generate_title(post_title: &str, site_name: &str, separator: &str) -> String {
    if post_title.is_empty() {
        return site_name.to_string();
    }
    format!("{} {} {}", post_title, separator, site_name)
}

/// Escapes characters for use in HTML attributes.
fn escape_attr(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_meta_tags_basic() {
        let meta = SeoMeta {
            description: Some("A test page".to_string()),
            canonical: Some("https://example.com/test".to_string()),
            robots: Some("index,follow".to_string()),
            ..Default::default()
        };
        let output = generate_meta_tags(&meta);
        assert!(output.contains(r#"<meta name="description" content="A test page" />"#));
        assert!(output.contains(r#"<link rel="canonical" href="https://example.com/test" />"#));
        assert!(output.contains(r#"<meta name="robots" content="index,follow" />"#));
    }

    #[test]
    fn test_generate_meta_tags_og_and_twitter() {
        let meta = SeoMeta {
            og_title: Some("OG Title".to_string()),
            og_type: Some("article".to_string()),
            twitter_card: Some("summary_large_image".to_string()),
            twitter_title: Some("Twitter Title".to_string()),
            ..Default::default()
        };
        let output = generate_meta_tags(&meta);
        assert!(output.contains(r#"og:title" content="OG Title"#));
        assert!(output.contains(r#"og:type" content="article"#));
        assert!(output.contains(r#"twitter:card" content="summary_large_image"#));
        assert!(output.contains(r#"twitter:title" content="Twitter Title"#));
    }

    #[test]
    fn test_generate_meta_tags_escapes_special_chars() {
        let meta = SeoMeta {
            description: Some(r#"He said "hello" & <goodbye>"#.to_string()),
            ..Default::default()
        };
        let output = generate_meta_tags(&meta);
        assert!(output.contains("He said &quot;hello&quot; &amp; &lt;goodbye&gt;"));
    }

    #[test]
    fn test_auto_generate_description_strips_html() {
        let content = "<p>Hello <strong>world</strong>, this is a <a href=\"#\">test</a>.</p>";
        let desc = auto_generate_description(content, 200);
        assert_eq!(desc, "Hello world, this is a test.");
    }

    #[test]
    fn test_auto_generate_description_truncates() {
        let content = "This is a long sentence that should be truncated at a word boundary.";
        let desc = auto_generate_description(content, 30);
        assert!(desc.ends_with("..."));
        assert!(desc.len() <= 40); // truncated + "..."
        // Should break at word boundary
        assert!(!desc.contains("truncat"));
    }

    #[test]
    fn test_auto_generate_description_short_content() {
        let desc = auto_generate_description("Short text", 200);
        assert_eq!(desc, "Short text");
    }

    #[test]
    fn test_generate_title() {
        assert_eq!(
            generate_title("My Post", "My Site", "-"),
            "My Post - My Site"
        );
        assert_eq!(
            generate_title("My Post", "My Site", "|"),
            "My Post | My Site"
        );
    }

    #[test]
    fn test_generate_title_empty_post() {
        assert_eq!(generate_title("", "My Site", "-"), "My Site");
    }
}
