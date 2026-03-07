/// Standard WordPress `<head>` and footer output generation.
///
/// Equivalent to the output produced by WordPress's `wp_head()` and
/// `wp_footer()` action hooks in `wp-includes/default-filters.php`.
/// Generate standard WordPress `<head>` outputs.
///
/// All standard head elements (RSS feeds, api.w.org, EditURI, pingback,
/// shortlink, oEmbed, emoji styles, stylesheets) are rendered directly in
/// base.html to match WordPress TT25's exact HTML output order.
/// This function returns empty string; kept for plugin hook compatibility.
pub fn wp_head(_site_url: &str, _page_title: &str, _description: &str) -> String {
    String::new()
}

/// Generate standard WordPress footer outputs.
///
/// Returns empty string — WordPress TT25 does not load wp-embed.min.js
/// on the frontend by default (it is enqueued only when embeds are present).
/// Kept for plugin hook compatibility.
pub fn wp_footer(_site_url: &str) -> String {
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wp_head_returns_empty() {
        let output = wp_head("http://example.com", "My Site", "A description");
        assert!(output.is_empty());
    }

    #[test]
    fn test_wp_footer_returns_empty() {
        let output = wp_footer("http://example.com");
        assert!(output.is_empty());
    }
}
