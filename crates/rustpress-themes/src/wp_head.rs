/// Standard WordPress `<head>` and footer output generation.
///
/// Equivalent to the output produced by WordPress's `wp_head()` and
/// `wp_footer()` action hooks in `wp-includes/default-filters.php`.

/// Simple URL encoding for embedding URLs in HTML attributes.
/// Encodes only the characters that are necessary for safe embedding.
fn simple_url_encode(s: &str) -> String {
    s.replace('&', "%26")
        .replace('?', "%3F")
        .replace('=', "%3D")
        .replace(' ', "%20")
}

/// Generate standard WordPress `<head>` outputs.
///
/// Produces the meta tags and link elements that WordPress adds to every
/// page via the `wp_head` action: generator meta, REST API discovery,
/// oEmbed links, shortlink, RSS feed link, pingback endpoint, and DNS
/// prefetch hints.
pub fn wp_head(site_url: &str, page_title: &str, description: &str) -> String {
    let _ = description; // reserved for future <meta name="description"> support
    let base = site_url.trim_end_matches('/');
    let mut output = String::new();

    // Generator meta
    output.push_str("<meta name=\"generator\" content=\"RustPress\" />\n");

    // REST API discovery link
    output.push_str(&format!(
        "<link rel=\"https://api.w.org/\" href=\"{}/wp-json/\" />\n",
        base
    ));

    // REST API oEmbed links
    output.push_str(&format!(
        "<link rel=\"alternate\" type=\"application/json+oembed\" href=\"{}/wp-json/oembed/1.0/embed?url={}\" />\n",
        base,
        simple_url_encode(site_url)
    ));

    // Shortlink
    output.push_str(&format!(
        "<link rel=\"shortlink\" href=\"{}/\" />\n",
        base
    ));

    // RSS feed link
    output.push_str(&format!(
        "<link rel=\"alternate\" type=\"application/rss+xml\" title=\"{} Feed\" href=\"{}/feed/\" />\n",
        page_title,
        base
    ));

    // Pingback
    output.push_str(&format!(
        "<link rel=\"pingback\" href=\"{}/xmlrpc.php\" />\n",
        base
    ));

    // DNS prefetch for common resources
    output.push_str("<link rel=\"dns-prefetch\" href=\"//fonts.googleapis.com\" />\n");

    output
}

/// Generate standard WordPress footer outputs.
///
/// WordPress typically outputs deferred scripts here via the `wp_footer`
/// action.  For now this returns the wp-embed script that handles oEmbed
/// rendering on the front-end.
pub fn wp_footer(site_url: &str) -> String {
    format!(
        "<script type=\"text/javascript\" src=\"{}/wp-includes/js/wp-embed.min.js\"></script>\n",
        site_url.trim_end_matches('/')
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_url_encode() {
        assert_eq!(
            simple_url_encode("http://example.com?a=1&b=2"),
            "http://example.com%3Fa%3D1%26b%3D2"
        );
        assert_eq!(simple_url_encode("hello world"), "hello%20world");
    }

    #[test]
    fn test_wp_head_contains_generator() {
        let output = wp_head("http://example.com", "My Site", "A description");
        assert!(output.contains("<meta name=\"generator\" content=\"RustPress\" />"));
    }

    #[test]
    fn test_wp_head_contains_rest_api_link() {
        let output = wp_head("http://example.com", "My Site", "");
        assert!(output.contains("<link rel=\"https://api.w.org/\" href=\"http://example.com/wp-json/\" />"));
    }

    #[test]
    fn test_wp_head_contains_oembed_link() {
        let output = wp_head("http://example.com", "My Site", "");
        assert!(output.contains("application/json+oembed"));
        assert!(output.contains("/wp-json/oembed/1.0/embed?url="));
    }

    #[test]
    fn test_wp_head_contains_shortlink() {
        let output = wp_head("http://example.com", "My Site", "");
        assert!(output.contains("<link rel=\"shortlink\" href=\"http://example.com/\" />"));
    }

    #[test]
    fn test_wp_head_contains_rss_feed() {
        let output = wp_head("http://example.com", "My Blog", "");
        assert!(output.contains("application/rss+xml"));
        assert!(output.contains("title=\"My Blog Feed\""));
        assert!(output.contains("href=\"http://example.com/feed/\""));
    }

    #[test]
    fn test_wp_head_contains_pingback() {
        let output = wp_head("http://example.com", "My Site", "");
        assert!(output.contains("<link rel=\"pingback\" href=\"http://example.com/xmlrpc.php\" />"));
    }

    #[test]
    fn test_wp_head_contains_dns_prefetch() {
        let output = wp_head("http://example.com", "My Site", "");
        assert!(output.contains("<link rel=\"dns-prefetch\" href=\"//fonts.googleapis.com\" />"));
    }

    #[test]
    fn test_wp_head_trims_trailing_slash() {
        let output = wp_head("http://example.com/", "My Site", "");
        // Should not produce double slashes like "http://example.com//wp-json/"
        assert!(!output.contains("//wp-json/"));
        assert!(output.contains("http://example.com/wp-json/"));
    }

    #[test]
    fn test_wp_footer_contains_embed_script() {
        let output = wp_footer("http://example.com");
        assert!(output.contains("<script type=\"text/javascript\""));
        assert!(output.contains("wp-embed.min.js"));
        assert!(output.contains("http://example.com/wp-includes/js/wp-embed.min.js"));
    }

    #[test]
    fn test_wp_footer_trims_trailing_slash() {
        let output = wp_footer("http://example.com/");
        assert!(!output.contains("//wp-includes"));
        assert!(output.contains("http://example.com/wp-includes/js/wp-embed.min.js"));
    }
}
