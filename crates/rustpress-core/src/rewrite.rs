//! WordPress-compatible permalink and rewrite rules engine.
//!
//! This module implements the WordPress rewrite API, translating between
//! human-readable permalink structures and internal query parameters.
//! It supports all standard WordPress permalink structures:
//!
//! - Plain: `/?p=123`
//! - Day and name: `/%year%/%monthnum%/%day%/%postname%/`
//! - Month and name: `/%year%/%monthnum%/%postname%/`
//! - Numeric: `/archives/%post_id%`
//! - Post name (pretty): `/%postname%/`
//!
//! In addition it resolves standard WordPress URL patterns for pages,
//! categories, tags, authors, feeds, search, date archives, and pagination.

use chrono::NaiveDateTime;
use regex::Regex;
use serde::{Deserialize, Serialize};

/// The result of resolving a URL path through the rewrite rules.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RewriteMatch {
    /// A single post matched by slug.
    Post { slug: String },
    /// A page matched by slug (may be hierarchical, e.g. `parent/child`).
    Page { slug: String },
    /// A date-based archive. Month and day are optional depending on the URL.
    DateArchive {
        year: u32,
        month: Option<u32>,
        day: Option<u32>,
    },
    /// A category archive matched by slug.
    Category { slug: String },
    /// A tag archive matched by slug.
    Tag { slug: String },
    /// An author archive matched by slug.
    Author { slug: String },
    /// An RSS/Atom feed endpoint.
    Feed,
    /// A search results page.
    Search { query: String },
    /// A pagination page (e.g. `/page/2/`).
    Pagination { page: u64 },
}

/// WordPress-compatible permalink structures.
///
/// These mirror the options available on the WordPress Settings > Permalinks page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PermalinkStructure {
    /// Plain query-string permalinks: `/?p=123`
    Plain,
    /// Day and name: `/%year%/%monthnum%/%day%/%postname%/`
    DayAndName,
    /// Month and name: `/%year%/%monthnum%/%postname%/`
    MonthAndName,
    /// Numeric: `/archives/%post_id%`
    Numeric,
    /// Post name (pretty permalinks): `/%postname%/`
    PostName,
    /// A custom structure string containing WordPress rewrite tags.
    Custom(String),
}

impl PermalinkStructure {
    /// Return the WordPress structure string for this permalink structure.
    pub fn as_structure_str(&self) -> &str {
        match self {
            PermalinkStructure::Plain => "",
            PermalinkStructure::DayAndName => "/%year%/%monthnum%/%day%/%postname%/",
            PermalinkStructure::MonthAndName => "/%year%/%monthnum%/%postname%/",
            PermalinkStructure::Numeric => "/archives/%post_id%",
            PermalinkStructure::PostName => "/%postname%/",
            PermalinkStructure::Custom(s) => s.as_str(),
        }
    }

    /// Parse a WordPress structure string into a `PermalinkStructure`.
    pub fn from_structure_str(s: &str) -> Self {
        match s {
            "" => PermalinkStructure::Plain,
            "/%year%/%monthnum%/%day%/%postname%/" => PermalinkStructure::DayAndName,
            "/%year%/%monthnum%/%postname%/" => PermalinkStructure::MonthAndName,
            "/archives/%post_id%" => PermalinkStructure::Numeric,
            "/%postname%/" => PermalinkStructure::PostName,
            other => PermalinkStructure::Custom(other.to_string()),
        }
    }
}

/// The core rewrite rules engine.
///
/// Holds the current permalink structure and compiled regex patterns for
/// resolving incoming URL paths to content types.
#[derive(Debug)]
pub struct RewriteRules {
    /// The active permalink structure.
    structure: PermalinkStructure,
    /// The raw structure string (kept for `get_structure()`).
    structure_string: String,
    /// Compiled regex patterns and their associated match builders.
    rules: Vec<CompiledRule>,
}

/// A single compiled rewrite rule.
#[derive(Debug)]
struct CompiledRule {
    /// The compiled regex.
    pattern: Regex,
    /// A function that builds a `RewriteMatch` from regex captures.
    builder: RuleBuilder,
}

/// Describes how to construct a `RewriteMatch` from regex captures.
#[derive(Debug, Clone)]
#[allow(dead_code)]
enum RuleBuilder {
    /// Match a post by slug (capture group 1 = slug).
    PostBySlug { slug_group: usize },
    /// Match a post by numeric ID (capture group 1 = id as slug).
    PostById { id_group: usize },
    /// Match a post by date+slug.
    PostByDateAndSlug {
        year_group: usize,
        month_group: usize,
        day_group: Option<usize>,
        slug_group: usize,
    },
    /// Date archive.
    DateArchive {
        year_group: usize,
        month_group: Option<usize>,
        day_group: Option<usize>,
    },
    /// Category by slug.
    Category { slug_group: usize },
    /// Tag by slug.
    Tag { slug_group: usize },
    /// Author by slug.
    Author { slug_group: usize },
    /// Feed endpoint.
    Feed,
    /// Search query.
    Search { query_group: usize },
    /// Pagination.
    Pagination { page_group: usize },
    /// Page by slug.
    Page { slug_group: usize },
}

impl RewriteRules {
    /// Create a new `RewriteRules` with the default pretty permalink structure (`/%postname%/`).
    pub fn new() -> Self {
        let mut rules = RewriteRules {
            structure: PermalinkStructure::PostName,
            structure_string: "/%postname%/".to_string(),
            rules: Vec::new(),
        };
        rules.compile_rules();
        rules
    }

    /// Set the permalink structure using a WordPress structure string.
    ///
    /// Accepted tags: `%year%`, `%monthnum%`, `%day%`, `%postname%`, `%post_id%`, `%category%`, `%author%`.
    ///
    /// # Examples
    ///
    /// ```
    /// use rustpress_core::rewrite::RewriteRules;
    ///
    /// let mut rules = RewriteRules::new();
    /// rules.set_structure("/%year%/%monthnum%/%postname%/");
    /// assert_eq!(rules.get_structure(), "/%year%/%monthnum%/%postname%/");
    /// ```
    pub fn set_structure(&mut self, structure: &str) {
        self.structure = PermalinkStructure::from_structure_str(structure);
        self.structure_string = structure.to_string();
        self.compile_rules();
    }

    /// Get the current permalink structure string.
    pub fn get_structure(&self) -> &str {
        &self.structure_string
    }

    /// Get the current `PermalinkStructure` enum variant.
    pub fn permalink_structure(&self) -> &PermalinkStructure {
        &self.structure
    }

    /// Build a permalink URL for a post given its slug, ID, and publication date.
    ///
    /// The output depends on the current permalink structure. For plain permalinks
    /// the query-string form `?p=<id>` is returned. For all other structures the
    /// appropriate path is built from the structure tags.
    ///
    /// # Examples
    ///
    /// ```
    /// use chrono::NaiveDateTime;
    /// use rustpress_core::rewrite::RewriteRules;
    ///
    /// let rules = RewriteRules::new(); // /%postname%/
    /// let date = NaiveDateTime::parse_from_str("2024-03-15 10:30:00", "%Y-%m-%d %H:%M:%S").unwrap();
    /// assert_eq!(rules.build_permalink("hello-world", 42, date), "/hello-world/");
    /// ```
    pub fn build_permalink(&self, slug: &str, post_id: u64, date: NaiveDateTime) -> String {
        let structure_str = match &self.structure {
            PermalinkStructure::Plain => {
                return format!("?p={}", post_id);
            }
            PermalinkStructure::DayAndName => "/%year%/%monthnum%/%day%/%postname%/",
            PermalinkStructure::MonthAndName => "/%year%/%monthnum%/%postname%/",
            PermalinkStructure::Numeric => "/archives/%post_id%",
            PermalinkStructure::PostName => "/%postname%/",
            PermalinkStructure::Custom(s) => s.as_str(),
        };

        let year = date.format("%Y").to_string();
        let month = date.format("%m").to_string();
        let day = date.format("%d").to_string();

        

        structure_str
            .replace("%year%", &year)
            .replace("%monthnum%", &month)
            .replace("%day%", &day)
            .replace("%postname%", slug)
            .replace("%post_id%", &post_id.to_string())
    }

    /// Resolve a URL path to a `RewriteMatch`.
    ///
    /// Returns `None` if the path does not match any rewrite rule.
    /// The path should be the request path without the query string (e.g. `/hello-world/`).
    ///
    /// # Examples
    ///
    /// ```
    /// use rustpress_core::rewrite::{RewriteRules, RewriteMatch};
    ///
    /// let rules = RewriteRules::new(); // /%postname%/
    /// let m = rules.resolve("/hello-world/").unwrap();
    /// assert_eq!(m, RewriteMatch::Post { slug: "hello-world".to_string() });
    /// ```
    pub fn resolve(&self, path: &str) -> Option<RewriteMatch> {
        let path = path.trim();
        if path.is_empty() || path == "/" {
            return None;
        }

        for rule in &self.rules {
            if let Some(caps) = rule.pattern.captures(path) {
                return Some(Self::build_match(&caps, &rule.builder));
            }
        }

        None
    }

    /// Build a `RewriteMatch` from regex captures and a `RuleBuilder`.
    fn build_match(caps: &regex::Captures, builder: &RuleBuilder) -> RewriteMatch {
        match builder {
            RuleBuilder::PostBySlug { slug_group } => RewriteMatch::Post {
                slug: caps[*slug_group].to_string(),
            },
            RuleBuilder::PostById { id_group } => RewriteMatch::Post {
                slug: caps[*id_group].to_string(),
            },
            RuleBuilder::PostByDateAndSlug { slug_group, .. } => RewriteMatch::Post {
                slug: caps[*slug_group].to_string(),
            },
            RuleBuilder::DateArchive {
                year_group,
                month_group,
                day_group,
            } => {
                let year: u32 = caps[*year_group].parse().unwrap_or(0);
                let month =
                    month_group.and_then(|g| caps.get(g).and_then(|m| m.as_str().parse().ok()));
                let day = day_group.and_then(|g| caps.get(g).and_then(|m| m.as_str().parse().ok()));
                RewriteMatch::DateArchive { year, month, day }
            }
            RuleBuilder::Category { slug_group } => RewriteMatch::Category {
                slug: caps[*slug_group].to_string(),
            },
            RuleBuilder::Tag { slug_group } => RewriteMatch::Tag {
                slug: caps[*slug_group].to_string(),
            },
            RuleBuilder::Author { slug_group } => RewriteMatch::Author {
                slug: caps[*slug_group].to_string(),
            },
            RuleBuilder::Feed => RewriteMatch::Feed,
            RuleBuilder::Search { query_group } => RewriteMatch::Search {
                query: caps[*query_group].to_string(),
            },
            RuleBuilder::Pagination { page_group } => {
                let page: u64 = caps[*page_group].parse().unwrap_or(1);
                RewriteMatch::Pagination { page }
            }
            RuleBuilder::Page { slug_group } => RewriteMatch::Page {
                slug: caps[*slug_group].trim_end_matches('/').to_string(),
            },
        }
    }

    /// Compile all rewrite rules based on the current permalink structure.
    ///
    /// This builds the ordered list of regex rules. Rules are checked in order,
    /// and the first match wins -- mirroring WordPress behavior.
    fn compile_rules(&mut self) {
        self.rules.clear();

        // --- Global rules (always present, checked first) ---

        // Feed: /feed/ or /feed/rss/ or /feed/atom/
        self.add_rule(r"^/feed(?:/(?:rss2?|atom|rdf))?/?$", RuleBuilder::Feed);

        // Search: /search/query-here/
        self.add_rule(r"^/search/(.+?)/?$", RuleBuilder::Search { query_group: 1 });

        // Pagination: /page/2/
        self.add_rule(
            r"^/page/(\d+)/?$",
            RuleBuilder::Pagination { page_group: 1 },
        );

        // Category: /category/slug/
        self.add_rule(
            r"^/category/([a-zA-Z0-9_-]+(?:/[a-zA-Z0-9_-]+)*)/?$",
            RuleBuilder::Category { slug_group: 1 },
        );

        // Tag: /tag/slug/
        self.add_rule(
            r"^/tag/([a-zA-Z0-9_-]+)/?$",
            RuleBuilder::Tag { slug_group: 1 },
        );

        // Author: /author/slug/
        self.add_rule(
            r"^/author/([a-zA-Z0-9_-]+)/?$",
            RuleBuilder::Author { slug_group: 1 },
        );

        // --- Date archive rules (checked before post rules to avoid conflicts) ---

        // Year/Month/Day archive: /2024/03/15/
        self.add_rule(
            r"^/(\d{4})/(\d{2})/(\d{2})/?$",
            RuleBuilder::DateArchive {
                year_group: 1,
                month_group: Some(2),
                day_group: Some(3),
            },
        );

        // Year/Month archive: /2024/03/
        self.add_rule(
            r"^/(\d{4})/(\d{2})/?$",
            RuleBuilder::DateArchive {
                year_group: 1,
                month_group: Some(2),
                day_group: None,
            },
        );

        // Year archive: /2024/
        self.add_rule(
            r"^/(\d{4})/?$",
            RuleBuilder::DateArchive {
                year_group: 1,
                month_group: None,
                day_group: None,
            },
        );

        // --- Structure-specific post rules ---

        match &self.structure {
            PermalinkStructure::Plain => {
                // Plain permalinks use query strings (?p=123), no path-based post rules.
            }
            PermalinkStructure::PostName => {
                // /%postname%/ -- a bare slug (must start with a letter to avoid matching years)
                self.add_rule(
                    r"^/([a-zA-Z][a-zA-Z0-9_-]*)/?$",
                    RuleBuilder::PostBySlug { slug_group: 1 },
                );
            }
            PermalinkStructure::DayAndName => {
                // /%year%/%monthnum%/%day%/%postname%/
                self.add_rule(
                    r"^/(\d{4})/(\d{2})/(\d{2})/([a-zA-Z0-9_-]+)/?$",
                    RuleBuilder::PostByDateAndSlug {
                        year_group: 1,
                        month_group: 2,
                        day_group: Some(3),
                        slug_group: 4,
                    },
                );
            }
            PermalinkStructure::MonthAndName => {
                // /%year%/%monthnum%/%postname%/
                self.add_rule(
                    r"^/(\d{4})/(\d{2})/([a-zA-Z0-9_-]+)/?$",
                    RuleBuilder::PostByDateAndSlug {
                        year_group: 1,
                        month_group: 2,
                        day_group: None,
                        slug_group: 3,
                    },
                );
            }
            PermalinkStructure::Numeric => {
                // /archives/%post_id%
                self.add_rule(
                    r"^/archives/(\d+)/?$",
                    RuleBuilder::PostById { id_group: 1 },
                );
            }
            PermalinkStructure::Custom(structure) => {
                // Build a regex from the custom structure string by replacing
                // WordPress rewrite tags with capture groups.
                let pattern = self.structure_to_regex(structure);
                let builder = self.structure_to_builder(structure);
                self.add_rule(&pattern, builder);
            }
        }

        // --- Page rule (lowest priority, catches hierarchical paths) ---
        // This is intentionally last so that specific patterns above take precedence.
        // Pages can have hierarchical slugs like /parent/child/.
        // Only for non-PostName structures (PostName's single-segment rule already covers it).
        if self.structure != PermalinkStructure::PostName {
            self.add_rule(
                r"^/([a-zA-Z0-9][a-zA-Z0-9_/-]*)/?$",
                RuleBuilder::Page { slug_group: 1 },
            );
        }
    }

    /// Add a compiled rule to the rule list.
    fn add_rule(&mut self, pattern: &str, builder: RuleBuilder) {
        if let Ok(regex) = Regex::new(pattern) {
            self.rules.push(CompiledRule {
                pattern: regex,
                builder,
            });
        } else {
            tracing::warn!("Failed to compile rewrite rule pattern: {}", pattern);
        }
    }

    /// Convert a WordPress structure string to a regex pattern.
    ///
    /// Replaces rewrite tags (`%year%`, `%monthnum%`, etc.) with named capture groups.
    fn structure_to_regex(&self, structure: &str) -> String {
        let mut pattern = regex::escape(structure);

        // Replace escaped tag markers back to build capture groups.
        // regex::escape will have escaped the % signs, so we work with the escaped form.
        pattern = pattern.replace(r"%year%", r"(\d{4})");
        pattern = pattern.replace(r"%monthnum%", r"(\d{2})");
        pattern = pattern.replace(r"%day%", r"(\d{2})");
        pattern = pattern.replace(r"%postname%", r"([a-zA-Z0-9_-]+)");
        pattern = pattern.replace(r"%post_id%", r"(\d+)");
        pattern = pattern.replace(r"%category%", r"([a-zA-Z0-9_-]+)");
        pattern = pattern.replace(r"%author%", r"([a-zA-Z0-9_-]+)");

        format!("^{}/?$", pattern)
    }

    /// Determine the appropriate `RuleBuilder` for a custom structure string.
    ///
    /// This inspects which tags are present and assigns capture group indices
    /// based on their order of appearance.
    fn structure_to_builder(&self, structure: &str) -> RuleBuilder {
        // Collect tags in order of appearance to determine capture group indices.
        let tags: Vec<&str> = {
            let tag_re = Regex::new(r"%(\w+)%").unwrap();
            tag_re
                .captures_iter(structure)
                .map(|c| c.get(0).unwrap().as_str())
                .collect()
        };

        let mut has_postname = false;
        let mut has_post_id = false;
        let mut has_year = false;
        let mut has_month = false;
        let mut has_day = false;

        let mut postname_idx = 0usize;
        let mut post_id_idx = 0usize;
        let mut year_idx = 0usize;
        let mut month_idx = 0usize;
        let mut day_idx = 0usize;

        for (i, tag) in tags.iter().enumerate() {
            let group = i + 1; // regex capture groups are 1-indexed
            match *tag {
                "%year%" => {
                    has_year = true;
                    year_idx = group;
                }
                "%monthnum%" => {
                    has_month = true;
                    month_idx = group;
                }
                "%day%" => {
                    has_day = true;
                    day_idx = group;
                }
                "%postname%" => {
                    has_postname = true;
                    postname_idx = group;
                }
                "%post_id%" => {
                    has_post_id = true;
                    post_id_idx = group;
                }
                _ => {}
            }
        }

        if has_postname && has_year {
            RuleBuilder::PostByDateAndSlug {
                year_group: year_idx,
                month_group: month_idx,
                day_group: if has_day { Some(day_idx) } else { None },
                slug_group: postname_idx,
            }
        } else if has_postname {
            RuleBuilder::PostBySlug {
                slug_group: postname_idx,
            }
        } else if has_post_id {
            RuleBuilder::PostById {
                id_group: post_id_idx,
            }
        } else if has_year {
            RuleBuilder::DateArchive {
                year_group: year_idx,
                month_group: if has_month { Some(month_idx) } else { None },
                day_group: if has_day { Some(day_idx) } else { None },
            }
        } else {
            // Fallback: treat as page slug.
            RuleBuilder::Page { slug_group: 1 }
        }
    }
}

impl Default for RewriteRules {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDateTime;

    fn make_date(s: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S").unwrap()
    }

    // -----------------------------------------------------------------------
    // build_permalink tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_build_permalink_postname() {
        let rules = RewriteRules::new();
        let date = make_date("2024-03-15 10:30:00");
        assert_eq!(
            rules.build_permalink("hello-world", 42, date),
            "/hello-world/"
        );
    }

    #[test]
    fn test_build_permalink_day_and_name() {
        let mut rules = RewriteRules::new();
        rules.set_structure("/%year%/%monthnum%/%day%/%postname%/");
        let date = make_date("2024-03-15 10:30:00");
        assert_eq!(
            rules.build_permalink("hello-world", 42, date),
            "/2024/03/15/hello-world/"
        );
    }

    #[test]
    fn test_build_permalink_month_and_name() {
        let mut rules = RewriteRules::new();
        rules.set_structure("/%year%/%monthnum%/%postname%/");
        let date = make_date("2024-11-05 08:00:00");
        assert_eq!(
            rules.build_permalink("my-post", 99, date),
            "/2024/11/my-post/"
        );
    }

    #[test]
    fn test_build_permalink_numeric() {
        let mut rules = RewriteRules::new();
        rules.set_structure("/archives/%post_id%");
        let date = make_date("2024-01-01 00:00:00");
        assert_eq!(
            rules.build_permalink("anything", 123, date),
            "/archives/123"
        );
    }

    #[test]
    fn test_build_permalink_plain() {
        let mut rules = RewriteRules::new();
        rules.set_structure("");
        let date = make_date("2024-06-20 12:00:00");
        assert_eq!(rules.build_permalink("slug", 55, date), "?p=55");
    }

    #[test]
    fn test_build_permalink_custom() {
        let mut rules = RewriteRules::new();
        rules.set_structure("/blog/%year%/%postname%/");
        let date = make_date("2025-07-04 09:15:00");
        assert_eq!(
            rules.build_permalink("independence", 1776, date),
            "/blog/2025/independence/"
        );
    }

    // -----------------------------------------------------------------------
    // resolve tests -- PostName structure (default)
    // -----------------------------------------------------------------------

    #[test]
    fn test_resolve_postname_basic() {
        let rules = RewriteRules::new();
        assert_eq!(
            rules.resolve("/hello-world/"),
            Some(RewriteMatch::Post {
                slug: "hello-world".to_string()
            })
        );
    }

    #[test]
    fn test_resolve_postname_no_trailing_slash() {
        let rules = RewriteRules::new();
        assert_eq!(
            rules.resolve("/hello-world"),
            Some(RewriteMatch::Post {
                slug: "hello-world".to_string()
            })
        );
    }

    #[test]
    fn test_resolve_root_returns_none() {
        let rules = RewriteRules::new();
        assert_eq!(rules.resolve("/"), None);
        assert_eq!(rules.resolve(""), None);
    }

    // -----------------------------------------------------------------------
    // resolve tests -- DayAndName structure
    // -----------------------------------------------------------------------

    #[test]
    fn test_resolve_day_and_name() {
        let mut rules = RewriteRules::new();
        rules.set_structure("/%year%/%monthnum%/%day%/%postname%/");
        assert_eq!(
            rules.resolve("/2024/03/15/hello-world/"),
            Some(RewriteMatch::Post {
                slug: "hello-world".to_string()
            })
        );
    }

    #[test]
    fn test_resolve_day_and_name_no_trailing_slash() {
        let mut rules = RewriteRules::new();
        rules.set_structure("/%year%/%monthnum%/%day%/%postname%/");
        assert_eq!(
            rules.resolve("/2024/03/15/hello-world"),
            Some(RewriteMatch::Post {
                slug: "hello-world".to_string()
            })
        );
    }

    // -----------------------------------------------------------------------
    // resolve tests -- MonthAndName structure
    // -----------------------------------------------------------------------

    #[test]
    fn test_resolve_month_and_name() {
        let mut rules = RewriteRules::new();
        rules.set_structure("/%year%/%monthnum%/%postname%/");
        assert_eq!(
            rules.resolve("/2024/11/my-post/"),
            Some(RewriteMatch::Post {
                slug: "my-post".to_string()
            })
        );
    }

    // -----------------------------------------------------------------------
    // resolve tests -- Numeric structure
    // -----------------------------------------------------------------------

    #[test]
    fn test_resolve_numeric() {
        let mut rules = RewriteRules::new();
        rules.set_structure("/archives/%post_id%");
        assert_eq!(
            rules.resolve("/archives/123/"),
            Some(RewriteMatch::Post {
                slug: "123".to_string()
            })
        );
    }

    #[test]
    fn test_resolve_numeric_no_trailing_slash() {
        let mut rules = RewriteRules::new();
        rules.set_structure("/archives/%post_id%");
        assert_eq!(
            rules.resolve("/archives/456"),
            Some(RewriteMatch::Post {
                slug: "456".to_string()
            })
        );
    }

    // -----------------------------------------------------------------------
    // resolve tests -- global patterns (feed, search, pagination, etc.)
    // -----------------------------------------------------------------------

    #[test]
    fn test_resolve_feed() {
        let rules = RewriteRules::new();
        assert_eq!(rules.resolve("/feed/"), Some(RewriteMatch::Feed));
        assert_eq!(rules.resolve("/feed/rss2/"), Some(RewriteMatch::Feed));
        assert_eq!(rules.resolve("/feed/atom/"), Some(RewriteMatch::Feed));
        assert_eq!(rules.resolve("/feed"), Some(RewriteMatch::Feed));
    }

    #[test]
    fn test_resolve_search() {
        let rules = RewriteRules::new();
        assert_eq!(
            rules.resolve("/search/hello+world/"),
            Some(RewriteMatch::Search {
                query: "hello+world".to_string()
            })
        );
    }

    #[test]
    fn test_resolve_pagination() {
        let rules = RewriteRules::new();
        assert_eq!(
            rules.resolve("/page/3/"),
            Some(RewriteMatch::Pagination { page: 3 })
        );
    }

    #[test]
    fn test_resolve_category() {
        let rules = RewriteRules::new();
        assert_eq!(
            rules.resolve("/category/tech/"),
            Some(RewriteMatch::Category {
                slug: "tech".to_string()
            })
        );
    }

    #[test]
    fn test_resolve_category_nested() {
        let rules = RewriteRules::new();
        assert_eq!(
            rules.resolve("/category/tech/rust/"),
            Some(RewriteMatch::Category {
                slug: "tech/rust".to_string()
            })
        );
    }

    #[test]
    fn test_resolve_tag() {
        let rules = RewriteRules::new();
        assert_eq!(
            rules.resolve("/tag/rust/"),
            Some(RewriteMatch::Tag {
                slug: "rust".to_string()
            })
        );
    }

    #[test]
    fn test_resolve_author() {
        let rules = RewriteRules::new();
        assert_eq!(
            rules.resolve("/author/admin/"),
            Some(RewriteMatch::Author {
                slug: "admin".to_string()
            })
        );
    }

    // -----------------------------------------------------------------------
    // resolve tests -- date archives
    // -----------------------------------------------------------------------

    #[test]
    fn test_resolve_year_archive() {
        let rules = RewriteRules::new();
        assert_eq!(
            rules.resolve("/2024/"),
            Some(RewriteMatch::DateArchive {
                year: 2024,
                month: None,
                day: None,
            })
        );
    }

    #[test]
    fn test_resolve_month_archive() {
        let rules = RewriteRules::new();
        assert_eq!(
            rules.resolve("/2024/03/"),
            Some(RewriteMatch::DateArchive {
                year: 2024,
                month: Some(3),
                day: None,
            })
        );
    }

    #[test]
    fn test_resolve_day_archive() {
        let rules = RewriteRules::new();
        assert_eq!(
            rules.resolve("/2024/03/15/"),
            Some(RewriteMatch::DateArchive {
                year: 2024,
                month: Some(3),
                day: Some(15),
            })
        );
    }

    // -----------------------------------------------------------------------
    // resolve tests -- page fallback (non-PostName structures)
    // -----------------------------------------------------------------------

    #[test]
    fn test_resolve_page_slug() {
        let mut rules = RewriteRules::new();
        rules.set_structure("/%year%/%monthnum%/%day%/%postname%/");
        // A slug that doesn't match year/month/day/postname should fall through to page
        assert_eq!(
            rules.resolve("/about/"),
            Some(RewriteMatch::Page {
                slug: "about".to_string()
            })
        );
    }

    #[test]
    fn test_resolve_hierarchical_page() {
        let mut rules = RewriteRules::new();
        rules.set_structure("/%year%/%monthnum%/%day%/%postname%/");
        assert_eq!(
            rules.resolve("/about/team/"),
            Some(RewriteMatch::Page {
                slug: "about/team".to_string()
            })
        );
    }

    // -----------------------------------------------------------------------
    // set_structure / get_structure
    // -----------------------------------------------------------------------

    #[test]
    fn test_get_set_structure() {
        let mut rules = RewriteRules::new();
        assert_eq!(rules.get_structure(), "/%postname%/");

        rules.set_structure("/%year%/%monthnum%/%postname%/");
        assert_eq!(rules.get_structure(), "/%year%/%monthnum%/%postname%/");

        rules.set_structure("");
        assert_eq!(rules.get_structure(), "");
        assert_eq!(rules.permalink_structure(), &PermalinkStructure::Plain);
    }

    // -----------------------------------------------------------------------
    // PermalinkStructure round-trip
    // -----------------------------------------------------------------------

    #[test]
    fn test_permalink_structure_round_trip() {
        let structures = vec![
            "",
            "/%postname%/",
            "/%year%/%monthnum%/%postname%/",
            "/%year%/%monthnum%/%day%/%postname%/",
            "/archives/%post_id%",
        ];

        for s in structures {
            let parsed = PermalinkStructure::from_structure_str(s);
            assert_eq!(parsed.as_structure_str(), s, "Round-trip failed for: {}", s);
        }
    }

    #[test]
    fn test_custom_structure_resolve() {
        let mut rules = RewriteRules::new();
        rules.set_structure("/blog/%year%/%postname%/");

        assert_eq!(
            rules.resolve("/blog/2025/my-article/"),
            Some(RewriteMatch::Post {
                slug: "my-article".to_string()
            })
        );
    }

    #[test]
    fn test_custom_structure_build_permalink() {
        let mut rules = RewriteRules::new();
        rules.set_structure("/blog/%year%/%postname%/");
        let date = make_date("2025-07-04 09:15:00");
        assert_eq!(
            rules.build_permalink("test-post", 1, date),
            "/blog/2025/test-post/"
        );
    }

    // -----------------------------------------------------------------------
    // Edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn test_resolve_no_match_for_deep_path_in_postname() {
        // In PostName mode, multi-segment paths don't match the post rule
        // (they would need to be pages, but page fallback is only for non-PostName).
        let rules = RewriteRules::new();
        let result = rules.resolve("/some/deep/path/");
        // Should not match Post (single-segment only)
        assert_ne!(
            result,
            Some(RewriteMatch::Post {
                slug: "some/deep/path".to_string()
            })
        );
    }

    #[test]
    fn test_resolve_feed_rss() {
        let rules = RewriteRules::new();
        assert_eq!(rules.resolve("/feed/rss/"), Some(RewriteMatch::Feed));
    }

    #[test]
    fn test_resolve_search_encoded_spaces() {
        let rules = RewriteRules::new();
        assert_eq!(
            rules.resolve("/search/rust%20programming/"),
            Some(RewriteMatch::Search {
                query: "rust%20programming".to_string()
            })
        );
    }

    #[test]
    fn test_default_impl() {
        let rules = RewriteRules::default();
        assert_eq!(rules.get_structure(), "/%postname%/");
    }

    #[test]
    fn test_resolve_pagination_page_1() {
        let rules = RewriteRules::new();
        assert_eq!(
            rules.resolve("/page/1/"),
            Some(RewriteMatch::Pagination { page: 1 })
        );
    }

    #[test]
    fn test_numeric_post_id_large() {
        let mut rules = RewriteRules::new();
        rules.set_structure("/archives/%post_id%");
        assert_eq!(
            rules.resolve("/archives/999999/"),
            Some(RewriteMatch::Post {
                slug: "999999".to_string()
            })
        );
    }

    #[test]
    fn test_build_permalink_preserves_slug_characters() {
        let rules = RewriteRules::new();
        let date = make_date("2024-01-01 00:00:00");
        // Slugs with numbers and hyphens should be preserved as-is
        assert_eq!(
            rules.build_permalink("my-post-2024", 1, date),
            "/my-post-2024/"
        );
    }
}
