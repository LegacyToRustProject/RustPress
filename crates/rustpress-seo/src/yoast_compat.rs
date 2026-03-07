//! Yoast SEO compatibility layer.
//!
//! Reads and writes SEO data using the same wp_postmeta keys and wp_options
//! that Yoast SEO uses, enabling seamless migration from WordPress+Yoast
//! to RustPress.
//!
//! ## Yoast Meta Keys (wp_postmeta)
//! - `_yoast_wpseo_title`        — Custom SEO title template
//! - `_yoast_wpseo_metadesc`     — Meta description
//! - `_yoast_wpseo_focuskw`      — Focus keyphrase
//! - `_yoast_wpseo_canonical`    — Canonical URL override
//! - `_yoast_wpseo_meta-robots-noindex`  — noindex flag (1 = noindex)
//! - `_yoast_wpseo_meta-robots-nofollow` — nofollow flag (1 = nofollow)
//! - `_yoast_wpseo_opengraph-title`      — OG title override
//! - `_yoast_wpseo_opengraph-description` — OG description override
//! - `_yoast_wpseo_opengraph-image`      — OG image URL
//! - `_yoast_wpseo_twitter-title`        — Twitter title override
//! - `_yoast_wpseo_twitter-description`  — Twitter description override
//! - `_yoast_wpseo_twitter-image`        — Twitter image URL
//!
//! ## Yoast Options (wp_options)
//! - `wpseo_titles`  — serialized title templates per post type
//! - `wpseo_social`  — social profiles (Facebook, Twitter, etc.)
//! - `wpseo`         — general Yoast settings

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::meta_tags::SeoMeta;

/// All Yoast-compatible meta keys as constants.
pub mod keys {
    pub const TITLE: &str = "_yoast_wpseo_title";
    pub const META_DESC: &str = "_yoast_wpseo_metadesc";
    pub const FOCUS_KW: &str = "_yoast_wpseo_focuskw";
    pub const CANONICAL: &str = "_yoast_wpseo_canonical";
    pub const NOINDEX: &str = "_yoast_wpseo_meta-robots-noindex";
    pub const NOFOLLOW: &str = "_yoast_wpseo_meta-robots-nofollow";
    pub const OG_TITLE: &str = "_yoast_wpseo_opengraph-title";
    pub const OG_DESC: &str = "_yoast_wpseo_opengraph-description";
    pub const OG_IMAGE: &str = "_yoast_wpseo_opengraph-image";
    pub const TWITTER_TITLE: &str = "_yoast_wpseo_twitter-title";
    pub const TWITTER_DESC: &str = "_yoast_wpseo_twitter-description";
    pub const TWITTER_IMAGE: &str = "_yoast_wpseo_twitter-image";
    pub const SCHEMA_PAGE_TYPE: &str = "_yoast_wpseo_schema_page_type";
    pub const SCHEMA_ARTICLE_TYPE: &str = "_yoast_wpseo_schema_article_type";

    /// All known meta keys for bulk queries.
    pub const ALL: &[&str] = &[
        TITLE,
        META_DESC,
        FOCUS_KW,
        CANONICAL,
        NOINDEX,
        NOFOLLOW,
        OG_TITLE,
        OG_DESC,
        OG_IMAGE,
        TWITTER_TITLE,
        TWITTER_DESC,
        TWITTER_IMAGE,
        SCHEMA_PAGE_TYPE,
        SCHEMA_ARTICLE_TYPE,
    ];
}

/// Yoast options keys in wp_options.
pub mod option_keys {
    pub const TITLES: &str = "wpseo_titles";
    pub const SOCIAL: &str = "wpseo_social";
    pub const GENERAL: &str = "wpseo";
}

/// Yoast-compatible SEO data for a single post, read from wp_postmeta.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct YoastPostSeo {
    pub post_id: u64,
    pub title: Option<String>,
    pub meta_description: Option<String>,
    pub focus_keyword: Option<String>,
    pub canonical: Option<String>,
    pub noindex: bool,
    pub nofollow: bool,
    pub og_title: Option<String>,
    pub og_description: Option<String>,
    pub og_image: Option<String>,
    pub twitter_title: Option<String>,
    pub twitter_description: Option<String>,
    pub twitter_image: Option<String>,
    pub schema_page_type: Option<String>,
    pub schema_article_type: Option<String>,
}

impl YoastPostSeo {
    /// Parse Yoast SEO data from raw wp_postmeta key-value pairs.
    pub fn from_meta(post_id: u64, meta: &HashMap<String, String>) -> Self {
        Self {
            post_id,
            title: non_empty(meta.get(keys::TITLE)),
            meta_description: non_empty(meta.get(keys::META_DESC)),
            focus_keyword: non_empty(meta.get(keys::FOCUS_KW)),
            canonical: non_empty(meta.get(keys::CANONICAL)),
            noindex: meta.get(keys::NOINDEX).is_some_and(|v| v == "1"),
            nofollow: meta.get(keys::NOFOLLOW).is_some_and(|v| v == "1"),
            og_title: non_empty(meta.get(keys::OG_TITLE)),
            og_description: non_empty(meta.get(keys::OG_DESC)),
            og_image: non_empty(meta.get(keys::OG_IMAGE)),
            twitter_title: non_empty(meta.get(keys::TWITTER_TITLE)),
            twitter_description: non_empty(meta.get(keys::TWITTER_DESC)),
            twitter_image: non_empty(meta.get(keys::TWITTER_IMAGE)),
            schema_page_type: non_empty(meta.get(keys::SCHEMA_PAGE_TYPE)),
            schema_article_type: non_empty(meta.get(keys::SCHEMA_ARTICLE_TYPE)),
        }
    }

    /// Convert to key-value pairs for writing back to wp_postmeta.
    pub fn to_meta(&self) -> Vec<(String, String)> {
        let mut pairs = Vec::new();

        if let Some(ref v) = self.title {
            pairs.push((keys::TITLE.to_string(), v.clone()));
        }
        if let Some(ref v) = self.meta_description {
            pairs.push((keys::META_DESC.to_string(), v.clone()));
        }
        if let Some(ref v) = self.focus_keyword {
            pairs.push((keys::FOCUS_KW.to_string(), v.clone()));
        }
        if let Some(ref v) = self.canonical {
            pairs.push((keys::CANONICAL.to_string(), v.clone()));
        }
        if self.noindex {
            pairs.push((keys::NOINDEX.to_string(), "1".to_string()));
        }
        if self.nofollow {
            pairs.push((keys::NOFOLLOW.to_string(), "1".to_string()));
        }
        if let Some(ref v) = self.og_title {
            pairs.push((keys::OG_TITLE.to_string(), v.clone()));
        }
        if let Some(ref v) = self.og_description {
            pairs.push((keys::OG_DESC.to_string(), v.clone()));
        }
        if let Some(ref v) = self.og_image {
            pairs.push((keys::OG_IMAGE.to_string(), v.clone()));
        }
        if let Some(ref v) = self.twitter_title {
            pairs.push((keys::TWITTER_TITLE.to_string(), v.clone()));
        }
        if let Some(ref v) = self.twitter_description {
            pairs.push((keys::TWITTER_DESC.to_string(), v.clone()));
        }
        if let Some(ref v) = self.twitter_image {
            pairs.push((keys::TWITTER_IMAGE.to_string(), v.clone()));
        }
        if let Some(ref v) = self.schema_page_type {
            pairs.push((keys::SCHEMA_PAGE_TYPE.to_string(), v.clone()));
        }
        if let Some(ref v) = self.schema_article_type {
            pairs.push((keys::SCHEMA_ARTICLE_TYPE.to_string(), v.clone()));
        }

        pairs
    }

    /// Convert Yoast data to the generic SeoMeta struct used by the rendering layer.
    ///
    /// `post_title` and `site_name` are used for fallback values when Yoast
    /// fields are empty, and for Yoast title template expansion.
    pub fn to_seo_meta(&self, post_title: &str, site_name: &str, post_url: &str) -> SeoMeta {
        let title = self
            .title
            .as_ref()
            .map(|t| expand_yoast_template(t, post_title, site_name));

        let robots = build_robots_directive(self.noindex, self.nofollow);

        SeoMeta {
            title,
            description: self.meta_description.clone(),
            canonical: self
                .canonical
                .clone()
                .or_else(|| Some(post_url.to_string())),
            robots,
            og_title: self
                .og_title
                .clone()
                .or_else(|| Some(post_title.to_string())),
            og_description: self
                .og_description
                .clone()
                .or(self.meta_description.clone()),
            og_image: self.og_image.clone(),
            og_url: Some(post_url.to_string()),
            og_type: Some("article".to_string()),
            og_site_name: Some(site_name.to_string()),
            twitter_card: Some("summary_large_image".to_string()),
            twitter_title: self
                .twitter_title
                .clone()
                .or_else(|| Some(post_title.to_string())),
            twitter_description: self
                .twitter_description
                .clone()
                .or(self.meta_description.clone()),
            twitter_image: self.twitter_image.clone().or(self.og_image.clone()),
        }
    }
}

/// Yoast social settings stored in wp_options `wpseo_social`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct YoastSocialSettings {
    pub og_default_image: Option<String>,
    pub og_frontpage_title: Option<String>,
    pub og_frontpage_desc: Option<String>,
    pub facebook_site: Option<String>,
    pub twitter_site: Option<String>,
    pub twitter_card_type: Option<String>,
}

/// Expand Yoast title template variables.
///
/// Yoast uses `%%title%%`, `%%sitename%%`, `%%sep%%`, `%%page%%` etc.
fn expand_yoast_template(template: &str, post_title: &str, site_name: &str) -> String {
    template
        .replace("%%title%%", post_title)
        .replace("%%sitename%%", site_name)
        .replace("%%sep%%", "-")
        .replace("%%page%%", "")
        .replace("%%primary_category%%", "")
        .trim()
        .to_string()
}

fn build_robots_directive(noindex: bool, nofollow: bool) -> Option<String> {
    match (noindex, nofollow) {
        (false, false) => None, // default: index,follow (no tag needed)
        (true, false) => Some("noindex".to_string()),
        (false, true) => Some("nofollow".to_string()),
        (true, true) => Some("noindex,nofollow".to_string()),
    }
}

fn non_empty(val: Option<&String>) -> Option<String> {
    val.filter(|s| !s.is_empty()).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_meta() -> HashMap<String, String> {
        let mut m = HashMap::new();
        m.insert(keys::TITLE.into(), "%%title%% %%sep%% %%sitename%%".into());
        m.insert(keys::META_DESC.into(), "A great article about Rust.".into());
        m.insert(keys::FOCUS_KW.into(), "rust programming".into());
        m.insert(
            keys::CANONICAL.into(),
            "https://example.com/rust-article".into(),
        );
        m.insert(keys::NOINDEX.into(), "0".into());
        m.insert(keys::OG_TITLE.into(), "Rust Article - OG".into());
        m.insert(keys::OG_IMAGE.into(), "https://example.com/og.jpg".into());
        m.insert(keys::TWITTER_TITLE.into(), "".into()); // empty = fallback
        m
    }

    #[test]
    fn test_from_meta() {
        let yoast = YoastPostSeo::from_meta(42, &sample_meta());

        assert_eq!(yoast.post_id, 42);
        assert_eq!(
            yoast.title.as_deref(),
            Some("%%title%% %%sep%% %%sitename%%")
        );
        assert_eq!(
            yoast.meta_description.as_deref(),
            Some("A great article about Rust.")
        );
        assert_eq!(yoast.focus_keyword.as_deref(), Some("rust programming"));
        assert_eq!(
            yoast.canonical.as_deref(),
            Some("https://example.com/rust-article")
        );
        assert!(!yoast.noindex);
        assert!(!yoast.nofollow);
        assert_eq!(yoast.og_title.as_deref(), Some("Rust Article - OG"));
        assert_eq!(
            yoast.og_image.as_deref(),
            Some("https://example.com/og.jpg")
        );
        assert!(yoast.twitter_title.is_none()); // empty string → None
    }

    #[test]
    fn test_noindex_nofollow_parsing() {
        let mut m = HashMap::new();
        m.insert(keys::NOINDEX.into(), "1".into());
        m.insert(keys::NOFOLLOW.into(), "1".into());

        let yoast = YoastPostSeo::from_meta(1, &m);
        assert!(yoast.noindex);
        assert!(yoast.nofollow);
    }

    #[test]
    fn test_to_seo_meta_with_template_expansion() {
        let yoast = YoastPostSeo::from_meta(1, &sample_meta());
        let seo = yoast.to_seo_meta("My Post", "My Site", "https://example.com/my-post");

        // Title template expanded: "%%title%% %%sep%% %%sitename%%" → "My Post - My Site"
        assert_eq!(seo.title.as_deref(), Some("My Post - My Site"));

        // Meta description direct
        assert_eq!(
            seo.description.as_deref(),
            Some("A great article about Rust.")
        );

        // Canonical from Yoast data
        assert_eq!(
            seo.canonical.as_deref(),
            Some("https://example.com/rust-article")
        );

        // OG from Yoast override
        assert_eq!(seo.og_title.as_deref(), Some("Rust Article - OG"));
        assert_eq!(seo.og_image.as_deref(), Some("https://example.com/og.jpg"));
        assert_eq!(seo.og_site_name.as_deref(), Some("My Site"));

        // Twitter fallback to post title (twitter_title was empty)
        assert_eq!(seo.twitter_title.as_deref(), Some("My Post"));

        // robots: noindex=0 → None (default index,follow)
        assert!(seo.robots.is_none());
    }

    #[test]
    fn test_to_seo_meta_noindex() {
        let mut m = HashMap::new();
        m.insert(keys::NOINDEX.into(), "1".into());

        let yoast = YoastPostSeo::from_meta(1, &m);
        let seo = yoast.to_seo_meta("Post", "Site", "https://example.com/post");

        assert_eq!(seo.robots.as_deref(), Some("noindex"));
    }

    #[test]
    fn test_to_meta_roundtrip() {
        let original = YoastPostSeo {
            post_id: 1,
            title: Some("Custom Title".into()),
            meta_description: Some("Description".into()),
            focus_keyword: Some("rust".into()),
            canonical: Some("https://example.com/page".into()),
            noindex: true,
            nofollow: false,
            og_title: Some("OG Title".into()),
            og_description: None,
            og_image: Some("https://example.com/img.jpg".into()),
            twitter_title: None,
            twitter_description: None,
            twitter_image: None,
            schema_page_type: Some("WebPage".into()),
            schema_article_type: None,
        };

        let pairs = original.to_meta();

        // Rebuild from pairs
        let meta: HashMap<String, String> = pairs.into_iter().collect();
        let restored = YoastPostSeo::from_meta(1, &meta);

        assert_eq!(restored.title, original.title);
        assert_eq!(restored.meta_description, original.meta_description);
        assert_eq!(restored.focus_keyword, original.focus_keyword);
        assert_eq!(restored.canonical, original.canonical);
        assert_eq!(restored.noindex, original.noindex);
        assert_eq!(restored.nofollow, original.nofollow);
        assert_eq!(restored.og_title, original.og_title);
        assert_eq!(restored.og_image, original.og_image);
        assert_eq!(restored.schema_page_type, original.schema_page_type);
    }

    #[test]
    fn test_expand_yoast_template() {
        assert_eq!(
            expand_yoast_template("%%title%% %%sep%% %%sitename%%", "Hello", "MySite"),
            "Hello - MySite"
        );
        assert_eq!(
            expand_yoast_template("%%title%%", "Post Title", "Site"),
            "Post Title"
        );
        assert_eq!(
            expand_yoast_template("Static Title", "Post", "Site"),
            "Static Title"
        );
    }

    #[test]
    fn test_build_robots_directive() {
        assert_eq!(build_robots_directive(false, false), None);
        assert_eq!(build_robots_directive(true, false), Some("noindex".into()));
        assert_eq!(build_robots_directive(false, true), Some("nofollow".into()));
        assert_eq!(
            build_robots_directive(true, true),
            Some("noindex,nofollow".into())
        );
    }

    #[test]
    fn test_empty_meta_produces_defaults() {
        let yoast = YoastPostSeo::from_meta(1, &HashMap::new());
        assert!(yoast.title.is_none());
        assert!(yoast.meta_description.is_none());
        assert!(!yoast.noindex);
        assert!(!yoast.nofollow);

        let seo = yoast.to_seo_meta("Fallback Title", "Site", "https://example.com/page");
        // OG title falls back to post title
        assert_eq!(seo.og_title.as_deref(), Some("Fallback Title"));
        // Canonical falls back to post URL
        assert_eq!(seo.canonical.as_deref(), Some("https://example.com/page"));
    }

    #[test]
    fn test_all_keys_constant() {
        assert_eq!(keys::ALL.len(), 14);
        assert!(keys::ALL.contains(&keys::TITLE));
        assert!(keys::ALL.contains(&keys::SCHEMA_ARTICLE_TYPE));
    }
}
