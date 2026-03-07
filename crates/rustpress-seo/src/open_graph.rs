use serde::{Deserialize, Serialize};

/// Open Graph metadata for a page.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OpenGraphData {
    /// The type of object (e.g. "article", "website").
    pub og_type: String,
    /// The title of the page.
    pub og_title: String,
    /// A brief description.
    pub og_description: String,
    /// URL of an image to represent the page.
    pub og_image: Option<String>,
    /// The canonical URL.
    pub og_url: String,
    /// The name of the site.
    pub og_site_name: String,
}

/// Twitter Card metadata.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TwitterCardData {
    /// Card type: "summary", "summary_large_image", "app", or "player".
    pub card: String,
    /// Title of the content.
    pub title: String,
    /// Description (max 200 chars recommended).
    pub description: String,
    /// URL of an image.
    pub image: Option<String>,
}

/// Generates Open Graph meta tags as an HTML string.
pub fn generate_og_tags(data: &OpenGraphData) -> String {
    let mut tags = Vec::new();

    tags.push(format!(
        r#"<meta property="og:type" content="{}" />"#,
        escape_attr(&data.og_type)
    ));
    tags.push(format!(
        r#"<meta property="og:title" content="{}" />"#,
        escape_attr(&data.og_title)
    ));
    tags.push(format!(
        r#"<meta property="og:description" content="{}" />"#,
        escape_attr(&data.og_description)
    ));
    tags.push(format!(
        r#"<meta property="og:url" content="{}" />"#,
        escape_attr(&data.og_url)
    ));
    tags.push(format!(
        r#"<meta property="og:site_name" content="{}" />"#,
        escape_attr(&data.og_site_name)
    ));

    if let Some(ref image) = data.og_image {
        tags.push(format!(
            r#"<meta property="og:image" content="{}" />"#,
            escape_attr(image)
        ));
    }

    tags.join("\n")
}

/// Generates Twitter Card meta tags as an HTML string.
pub fn generate_twitter_tags(data: &TwitterCardData) -> String {
    let mut tags = Vec::new();

    tags.push(format!(
        r#"<meta name="twitter:card" content="{}" />"#,
        escape_attr(&data.card)
    ));
    tags.push(format!(
        r#"<meta name="twitter:title" content="{}" />"#,
        escape_attr(&data.title)
    ));
    tags.push(format!(
        r#"<meta name="twitter:description" content="{}" />"#,
        escape_attr(&data.description)
    ));

    if let Some(ref image) = data.image {
        tags.push(format!(
            r#"<meta name="twitter:image" content="{}" />"#,
            escape_attr(image)
        ));
    }

    tags.join("\n")
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
    fn test_generate_og_tags() {
        let data = OpenGraphData {
            og_type: "article".to_string(),
            og_title: "My Article".to_string(),
            og_description: "A great article".to_string(),
            og_image: Some("https://example.com/image.jpg".to_string()),
            og_url: "https://example.com/my-article".to_string(),
            og_site_name: "Example Site".to_string(),
        };
        let output = generate_og_tags(&data);
        assert!(output.contains(r#"og:type" content="article"#));
        assert!(output.contains(r#"og:title" content="My Article"#));
        assert!(output.contains(r#"og:description" content="A great article"#));
        assert!(output.contains(r#"og:image" content="https://example.com/image.jpg"#));
        assert!(output.contains(r#"og:url" content="https://example.com/my-article"#));
        assert!(output.contains(r#"og:site_name" content="Example Site"#));
    }

    #[test]
    fn test_generate_og_tags_no_image() {
        let data = OpenGraphData {
            og_type: "website".to_string(),
            og_title: "Home".to_string(),
            og_description: "Welcome".to_string(),
            og_image: None,
            og_url: "https://example.com/".to_string(),
            og_site_name: "Example".to_string(),
        };
        let output = generate_og_tags(&data);
        assert!(!output.contains("og:image"));
    }

    #[test]
    fn test_generate_twitter_tags() {
        let data = TwitterCardData {
            card: "summary_large_image".to_string(),
            title: "My Post".to_string(),
            description: "Check this out".to_string(),
            image: Some("https://example.com/photo.png".to_string()),
        };
        let output = generate_twitter_tags(&data);
        assert!(output.contains(r#"twitter:card" content="summary_large_image"#));
        assert!(output.contains(r#"twitter:title" content="My Post"#));
        assert!(output.contains(r#"twitter:description" content="Check this out"#));
        assert!(output.contains(r#"twitter:image" content="https://example.com/photo.png"#));
    }

    #[test]
    fn test_generate_twitter_tags_no_image() {
        let data = TwitterCardData {
            card: "summary".to_string(),
            title: "Title".to_string(),
            description: "Desc".to_string(),
            image: None,
        };
        let output = generate_twitter_tags(&data);
        assert!(!output.contains("twitter:image"));
    }

    #[test]
    fn test_escaping_in_og_tags() {
        let data = OpenGraphData {
            og_type: "article".to_string(),
            og_title: r#"Title with "quotes" & <symbols>"#.to_string(),
            og_description: "Normal".to_string(),
            og_image: None,
            og_url: "https://example.com/".to_string(),
            og_site_name: "Site".to_string(),
        };
        let output = generate_og_tags(&data);
        assert!(output.contains("&quot;quotes&quot;"));
        assert!(output.contains("&amp;"));
        assert!(output.contains("&lt;symbols&gt;"));
    }
}
