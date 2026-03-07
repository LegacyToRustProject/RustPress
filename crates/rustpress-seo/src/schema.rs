use serde::Serialize;
use serde_json::json;

/// The types of Schema.org structured data supported.
#[derive(Debug, Clone)]
pub enum SchemaOrg {
    Article,
    WebSite,
    BreadcrumbList,
    Organization,
    Person,
}

/// A single breadcrumb item for BreadcrumbList schema.
#[derive(Debug, Clone, Serialize)]
pub struct BreadcrumbItem {
    /// The display name of the breadcrumb.
    pub name: String,
    /// The URL of the breadcrumb (optional for the last item).
    pub url: Option<String>,
}

/// Generates a JSON-LD `<script>` block for an Article schema.
pub fn generate_article_schema(
    title: &str,
    url: &str,
    date_published: &str,
    author_name: &str,
    image_url: Option<&str>,
) -> String {
    let mut schema = json!({
        "@context": "https://schema.org",
        "@type": "Article",
        "headline": title,
        "url": url,
        "datePublished": date_published,
        "author": {
            "@type": "Person",
            "name": author_name
        }
    });

    if let Some(img) = image_url {
        schema["image"] = json!(img);
    }

    wrap_json_ld(&schema)
}

/// Generates a JSON-LD `<script>` block for a WebSite schema.
/// If `search_url` is provided, a SearchAction potential action is included.
/// The `search_url` should contain `{search_term_string}` as a placeholder.
pub fn generate_website_schema(name: &str, url: &str, search_url: Option<&str>) -> String {
    let mut schema = json!({
        "@context": "https://schema.org",
        "@type": "WebSite",
        "name": name,
        "url": url
    });

    if let Some(search) = search_url {
        schema["potentialAction"] = json!({
            "@type": "SearchAction",
            "target": search,
            "query-input": "required name=search_term_string"
        });
    }

    wrap_json_ld(&schema)
}

/// Generates a JSON-LD `<script>` block for a BreadcrumbList schema.
pub fn generate_breadcrumb_schema(items: Vec<BreadcrumbItem>) -> String {
    let list_items: Vec<serde_json::Value> = items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let mut entry = json!({
                "@type": "ListItem",
                "position": i + 1,
                "name": item.name
            });
            if let Some(ref url) = item.url {
                entry["item"] = json!(url);
            }
            entry
        })
        .collect();

    let schema = json!({
        "@context": "https://schema.org",
        "@type": "BreadcrumbList",
        "itemListElement": list_items
    });

    wrap_json_ld(&schema)
}

/// Wraps a JSON value in a `<script type="application/ld+json">` tag.
fn wrap_json_ld(value: &serde_json::Value) -> String {
    let json_str = serde_json::to_string_pretty(value).expect("valid JSON serialization");
    format!("<script type=\"application/ld+json\">\n{json_str}\n</script>")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_article_schema() {
        let output = generate_article_schema(
            "My Great Post",
            "https://example.com/my-great-post",
            "2025-06-15",
            "John Doe",
            Some("https://example.com/image.jpg"),
        );
        assert!(output.contains(r#"<script type="application/ld+json">"#));
        assert!(output.contains(r#""@type": "Article""#));
        assert!(output.contains(r#""headline": "My Great Post""#));
        assert!(output.contains(r#""datePublished": "2025-06-15""#));
        assert!(output.contains(r#""name": "John Doe""#));
        assert!(output.contains(r#""image": "https://example.com/image.jpg""#));
        assert!(output.contains("</script>"));
    }

    #[test]
    fn test_article_schema_no_image() {
        let output = generate_article_schema(
            "No Image Post",
            "https://example.com/no-image",
            "2025-01-01",
            "Jane",
            None,
        );
        assert!(!output.contains(r#""image""#));
        assert!(output.contains(r#""headline": "No Image Post""#));
    }

    #[test]
    fn test_website_schema_with_search() {
        let output = generate_website_schema(
            "My Site",
            "https://example.com",
            Some("https://example.com/?s={search_term_string}"),
        );
        assert!(output.contains(r#""@type": "WebSite""#));
        assert!(output.contains(r#""name": "My Site""#));
        assert!(output.contains(r#""@type": "SearchAction""#));
        assert!(output.contains("{search_term_string}"));
    }

    #[test]
    fn test_website_schema_without_search() {
        let output = generate_website_schema("Simple Site", "https://example.com", None);
        assert!(output.contains(r#""@type": "WebSite""#));
        assert!(!output.contains("SearchAction"));
    }

    #[test]
    fn test_breadcrumb_schema() {
        let items = vec![
            BreadcrumbItem {
                name: "Home".to_string(),
                url: Some("https://example.com/".to_string()),
            },
            BreadcrumbItem {
                name: "Blog".to_string(),
                url: Some("https://example.com/blog/".to_string()),
            },
            BreadcrumbItem {
                name: "Current Post".to_string(),
                url: None,
            },
        ];
        let output = generate_breadcrumb_schema(items);
        assert!(output.contains(r#""@type": "BreadcrumbList""#));
        assert!(output.contains(r#""position": 1"#));
        assert!(output.contains(r#""position": 2"#));
        assert!(output.contains(r#""position": 3"#));
        assert!(output.contains(r#""name": "Home""#));
        assert!(output.contains(r#""name": "Current Post""#));
        // The last item should not have an "item" key
        // The first item should have an "item" key
        assert!(output.contains(r#""item": "https://example.com/""#));
    }
}
