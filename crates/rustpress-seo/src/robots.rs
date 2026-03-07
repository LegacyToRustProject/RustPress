/// Builds a robots.txt file with allow/disallow rules and sitemap references.
#[derive(Debug, Clone)]
pub struct RobotsGenerator {
    user_agent: String,
    allow: Vec<String>,
    disallow: Vec<String>,
    sitemap_url: Option<String>,
}

impl Default for RobotsGenerator {
    /// Creates a generator with WordPress-compatible defaults:
    /// - Disallow `/wp-admin/`
    /// - Allow `/wp-admin/admin-ajax.php`
    fn default() -> Self {
        Self {
            user_agent: "*".to_string(),
            allow: vec!["/wp-admin/admin-ajax.php".to_string()],
            disallow: vec!["/wp-admin/".to_string()],
            sitemap_url: None,
        }
    }
}

impl RobotsGenerator {
    /// Creates a new generator with default WordPress-compatible rules.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a generator with no default rules.
    pub fn empty() -> Self {
        Self {
            user_agent: "*".to_string(),
            allow: Vec::new(),
            disallow: Vec::new(),
            sitemap_url: None,
        }
    }

    /// Sets the User-agent directive.
    pub fn set_user_agent(&mut self, agent: &str) {
        self.user_agent = agent.to_string();
    }

    /// Adds an Allow rule.
    pub fn add_allow(&mut self, path: &str) {
        self.allow.push(path.to_string());
    }

    /// Adds a Disallow rule.
    pub fn add_disallow(&mut self, path: &str) {
        self.disallow.push(path.to_string());
    }

    /// Sets the Sitemap URL to include at the end of robots.txt.
    pub fn set_sitemap_url(&mut self, url: &str) {
        self.sitemap_url = Some(url.to_string());
    }

    /// Generates the robots.txt content string.
    pub fn generate(&self) -> String {
        let mut lines = Vec::new();

        lines.push(format!("User-agent: {}", self.user_agent));

        for path in &self.disallow {
            lines.push(format!("Disallow: {path}"));
        }

        for path in &self.allow {
            lines.push(format!("Allow: {path}"));
        }

        if let Some(ref url) = self.sitemap_url {
            lines.push(String::new()); // blank line before Sitemap
            lines.push(format!("Sitemap: {url}"));
        }

        let mut output = lines.join("\n");
        output.push('\n');
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_rules() {
        let gen = RobotsGenerator::new();
        let output = gen.generate();
        assert!(output.contains("User-agent: *"));
        assert!(output.contains("Disallow: /wp-admin/"));
        assert!(output.contains("Allow: /wp-admin/admin-ajax.php"));
    }

    #[test]
    fn test_custom_rules() {
        let mut gen = RobotsGenerator::empty();
        gen.add_disallow("/private/");
        gen.add_disallow("/tmp/");
        gen.add_allow("/public/");
        let output = gen.generate();
        assert!(output.contains("Disallow: /private/"));
        assert!(output.contains("Disallow: /tmp/"));
        assert!(output.contains("Allow: /public/"));
    }

    #[test]
    fn test_with_sitemap() {
        let mut gen = RobotsGenerator::new();
        gen.set_sitemap_url("https://example.com/sitemap.xml");
        let output = gen.generate();
        assert!(output.contains("Sitemap: https://example.com/sitemap.xml"));
    }

    #[test]
    fn test_custom_user_agent() {
        let mut gen = RobotsGenerator::empty();
        gen.set_user_agent("Googlebot");
        gen.add_disallow("/secret/");
        let output = gen.generate();
        assert!(output.contains("User-agent: Googlebot"));
    }
}
