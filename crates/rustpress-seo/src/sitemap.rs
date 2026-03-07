use serde::{Deserialize, Serialize};

/// Represents a single URL entry in a sitemap.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SitemapUrl {
    /// The full URL of the page.
    pub loc: String,
    /// The date of last modification (W3C Datetime format, e.g. "2025-01-15").
    pub lastmod: Option<String>,
    /// How frequently the page is likely to change.
    pub changefreq: Option<ChangeFreq>,
    /// The priority of this URL relative to other URLs on the site (0.0 to 1.0).
    pub priority: Option<f32>,
}

/// Valid changefreq values per the sitemap protocol.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChangeFreq {
    Always,
    Hourly,
    Daily,
    Weekly,
    Monthly,
    Yearly,
    Never,
}

impl std::fmt::Display for ChangeFreq {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            ChangeFreq::Always => "always",
            ChangeFreq::Hourly => "hourly",
            ChangeFreq::Daily => "daily",
            ChangeFreq::Weekly => "weekly",
            ChangeFreq::Monthly => "monthly",
            ChangeFreq::Yearly => "yearly",
            ChangeFreq::Never => "never",
        };
        write!(f, "{s}")
    }
}

/// Represents an entry in a sitemap index file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SitemapEntry {
    /// The URL of the sitemap file.
    pub loc: String,
    /// The date of last modification.
    pub lastmod: Option<String>,
}

/// Builds XML sitemaps and sitemap index files.
#[derive(Debug, Clone, Default)]
pub struct SitemapGenerator {
    urls: Vec<SitemapUrl>,
}

impl SitemapGenerator {
    pub fn new() -> Self {
        Self { urls: Vec::new() }
    }

    /// Adds a URL entry to the sitemap.
    pub fn add_url(&mut self, url: SitemapUrl) {
        self.urls.push(url);
    }

    /// Generates a complete XML sitemap string.
    pub fn generate_xml(&self) -> String {
        let mut xml = String::new();
        xml.push_str(r#"<?xml version="1.0" encoding="UTF-8"?>"#);
        xml.push('\n');
        xml.push_str(r#"<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">"#);
        xml.push('\n');

        for url in &self.urls {
            xml.push_str("  <url>\n");
            xml.push_str(&format!("    <loc>{}</loc>\n", xml_escape(&url.loc)));

            if let Some(ref lastmod) = url.lastmod {
                xml.push_str(&format!("    <lastmod>{}</lastmod>\n", xml_escape(lastmod)));
            }
            if let Some(ref freq) = url.changefreq {
                xml.push_str(&format!("    <changefreq>{freq}</changefreq>\n"));
            }
            if let Some(priority) = url.priority {
                xml.push_str(&format!("    <priority>{priority:.1}</priority>\n"));
            }

            xml.push_str("  </url>\n");
        }

        xml.push_str("</urlset>\n");
        xml
    }

    /// Generates a sitemap index XML string from a list of sitemap entries.
    pub fn generate_sitemap_index(sitemaps: Vec<SitemapEntry>) -> String {
        let mut xml = String::new();
        xml.push_str(r#"<?xml version="1.0" encoding="UTF-8"?>"#);
        xml.push('\n');
        xml.push_str(r#"<sitemapindex xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">"#);
        xml.push('\n');

        for entry in &sitemaps {
            xml.push_str("  <sitemap>\n");
            xml.push_str(&format!("    <loc>{}</loc>\n", xml_escape(&entry.loc)));
            if let Some(ref lastmod) = entry.lastmod {
                xml.push_str(&format!("    <lastmod>{}</lastmod>\n", xml_escape(lastmod)));
            }
            xml.push_str("  </sitemap>\n");
        }

        xml.push_str("</sitemapindex>\n");
        xml
    }
}

/// Escapes special XML characters.
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_xml_basic() {
        let mut gen = SitemapGenerator::new();
        gen.add_url(SitemapUrl {
            loc: "https://example.com/".to_string(),
            lastmod: Some("2025-01-01".to_string()),
            changefreq: Some(ChangeFreq::Daily),
            priority: Some(1.0),
        });
        let xml = gen.generate_xml();
        assert!(xml.starts_with(r#"<?xml version="1.0" encoding="UTF-8"?>"#));
        assert!(xml.contains("<loc>https://example.com/</loc>"));
        assert!(xml.contains("<lastmod>2025-01-01</lastmod>"));
        assert!(xml.contains("<changefreq>daily</changefreq>"));
        assert!(xml.contains("<priority>1.0</priority>"));
        assert!(xml.contains("</urlset>"));
    }

    #[test]
    fn test_generate_xml_multiple_urls() {
        let mut gen = SitemapGenerator::new();
        gen.add_url(SitemapUrl {
            loc: "https://example.com/page1".to_string(),
            lastmod: None,
            changefreq: None,
            priority: None,
        });
        gen.add_url(SitemapUrl {
            loc: "https://example.com/page2".to_string(),
            lastmod: Some("2025-06-15".to_string()),
            changefreq: Some(ChangeFreq::Weekly),
            priority: Some(0.8),
        });
        let xml = gen.generate_xml();
        assert!(xml.contains("page1"));
        assert!(xml.contains("page2"));
        assert!(xml.contains("<priority>0.8</priority>"));
        // page1 should not have lastmod/changefreq/priority
        let page1_section = &xml[xml.find("page1").unwrap()..xml.find("page2").unwrap()];
        assert!(!page1_section.contains("<lastmod>"));
    }

    #[test]
    fn test_generate_xml_escapes_ampersand() {
        let mut gen = SitemapGenerator::new();
        gen.add_url(SitemapUrl {
            loc: "https://example.com/?a=1&b=2".to_string(),
            lastmod: None,
            changefreq: None,
            priority: None,
        });
        let xml = gen.generate_xml();
        assert!(xml.contains("?a=1&amp;b=2"));
    }

    #[test]
    fn test_generate_sitemap_index() {
        let entries = vec![
            SitemapEntry {
                loc: "https://example.com/sitemap-posts.xml".to_string(),
                lastmod: Some("2025-01-01".to_string()),
            },
            SitemapEntry {
                loc: "https://example.com/sitemap-pages.xml".to_string(),
                lastmod: None,
            },
        ];
        let xml = SitemapGenerator::generate_sitemap_index(entries);
        assert!(xml.contains("<sitemapindex"));
        assert!(xml.contains("sitemap-posts.xml"));
        assert!(xml.contains("sitemap-pages.xml"));
        assert!(xml.contains("</sitemapindex>"));
    }
}
