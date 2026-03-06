use std::path::PathBuf;
use tracing::debug;

/// WordPress Template Hierarchy resolver.
///
/// Determines which template file to use based on the request type,
/// following WordPress's template hierarchy rules.
pub struct TemplateHierarchy {
    theme_dir: PathBuf,
}

/// The type of page being requested.
#[derive(Debug, Clone)]
pub enum PageType {
    /// Single post: single-{post_type}-{slug}.html -> single-{post_type}.html -> single.html -> singular.html -> index.html
    Single {
        post_type: String,
        slug: String,
    },
    /// Single page: page-{slug}.html -> page-{id}.html -> page.html -> singular.html -> index.html
    Page {
        slug: String,
        id: u64,
    },
    /// Archive: archive-{post_type}.html -> archive.html -> index.html
    Archive {
        post_type: String,
    },
    /// Category: category-{slug}.html -> category-{id}.html -> category.html -> archive.html -> index.html
    Category {
        slug: String,
        id: u64,
    },
    /// Tag: tag-{slug}.html -> tag-{id}.html -> tag.html -> archive.html -> index.html
    Tag {
        slug: String,
        id: u64,
    },
    /// Author: author-{nicename}.html -> author-{id}.html -> author.html -> archive.html -> index.html
    Author {
        nicename: String,
        id: u64,
    },
    /// Date archive
    DateArchive,
    /// Search results
    Search,
    /// 404 page
    NotFound,
    /// Front page
    FrontPage,
    /// Home (blog posts index)
    Home,
    /// Attachment
    Attachment {
        mime_type: String,
    },
}

impl TemplateHierarchy {
    pub fn new(theme_dir: impl Into<PathBuf>) -> Self {
        Self {
            theme_dir: theme_dir.into(),
        }
    }

    /// Resolve the template file to use for the given page type.
    /// Returns the first matching template file, or "index.html" as fallback.
    pub fn resolve(&self, page_type: &PageType) -> String {
        let candidates = self.get_candidates(page_type);

        for candidate in &candidates {
            let path = self.theme_dir.join(candidate);
            if path.exists() {
                debug!(template = candidate, "template resolved");
                return candidate.clone();
            }
        }

        debug!("no template found, falling back to index.html");
        "index.html".to_string()
    }

    /// Get the ordered list of template candidates for a page type.
    pub fn get_candidates(&self, page_type: &PageType) -> Vec<String> {
        match page_type {
            PageType::Single { post_type, slug } => vec![
                format!("single-{}-{}.html", post_type, slug),
                format!("single-{}.html", post_type),
                "single.html".to_string(),
                "singular.html".to_string(),
                "index.html".to_string(),
            ],
            PageType::Page { slug, id } => vec![
                format!("page-{}.html", slug),
                format!("page-{}.html", id),
                "page.html".to_string(),
                "singular.html".to_string(),
                "index.html".to_string(),
            ],
            PageType::Archive { post_type } => vec![
                format!("archive-{}.html", post_type),
                "archive.html".to_string(),
                "index.html".to_string(),
            ],
            PageType::Category { slug, id } => vec![
                format!("category-{}.html", slug),
                format!("category-{}.html", id),
                "category.html".to_string(),
                "archive.html".to_string(),
                "index.html".to_string(),
            ],
            PageType::Tag { slug, id } => vec![
                format!("tag-{}.html", slug),
                format!("tag-{}.html", id),
                "tag.html".to_string(),
                "archive.html".to_string(),
                "index.html".to_string(),
            ],
            PageType::Author { nicename, id } => vec![
                format!("author-{}.html", nicename),
                format!("author-{}.html", id),
                "author.html".to_string(),
                "archive.html".to_string(),
                "index.html".to_string(),
            ],
            PageType::DateArchive => vec![
                "date.html".to_string(),
                "archive.html".to_string(),
                "index.html".to_string(),
            ],
            PageType::Search => vec!["search.html".to_string(), "index.html".to_string()],
            PageType::NotFound => vec!["404.html".to_string(), "index.html".to_string()],
            PageType::FrontPage => vec![
                "front-page.html".to_string(),
                "home.html".to_string(),
                "index.html".to_string(),
            ],
            PageType::Home => vec!["home.html".to_string(), "index.html".to_string()],
            PageType::Attachment { mime_type } => {
                let subtype = mime_type.split('/').nth(1).unwrap_or("attachment");
                vec![
                    format!("{}.html", subtype),
                    "attachment.html".to_string(),
                    "single.html".to_string(),
                    "singular.html".to_string(),
                    "index.html".to_string(),
                ]
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_single_post_candidates() {
        let hierarchy = TemplateHierarchy::new("/tmp/theme");
        let candidates = hierarchy.get_candidates(&PageType::Single {
            post_type: "post".to_string(),
            slug: "hello-world".to_string(),
        });

        assert_eq!(candidates, vec![
            "single-post-hello-world.html",
            "single-post.html",
            "single.html",
            "singular.html",
            "index.html",
        ]);
    }

    #[test]
    fn test_page_candidates() {
        let hierarchy = TemplateHierarchy::new("/tmp/theme");
        let candidates = hierarchy.get_candidates(&PageType::Page {
            slug: "about".to_string(),
            id: 42,
        });

        assert_eq!(candidates, vec![
            "page-about.html",
            "page-42.html",
            "page.html",
            "singular.html",
            "index.html",
        ]);
    }

    #[test]
    fn test_category_candidates() {
        let hierarchy = TemplateHierarchy::new("/tmp/theme");
        let candidates = hierarchy.get_candidates(&PageType::Category {
            slug: "news".to_string(),
            id: 5,
        });

        assert_eq!(candidates, vec![
            "category-news.html",
            "category-5.html",
            "category.html",
            "archive.html",
            "index.html",
        ]);
    }

    #[test]
    fn test_search_candidates() {
        let hierarchy = TemplateHierarchy::new("/tmp/theme");
        let candidates = hierarchy.get_candidates(&PageType::Search);
        assert_eq!(candidates, vec!["search.html", "index.html"]);
    }

    #[test]
    fn test_404_candidates() {
        let hierarchy = TemplateHierarchy::new("/tmp/theme");
        let candidates = hierarchy.get_candidates(&PageType::NotFound);
        assert_eq!(candidates, vec!["404.html", "index.html"]);
    }

    #[test]
    fn test_front_page_candidates() {
        let hierarchy = TemplateHierarchy::new("/tmp/theme");
        let candidates = hierarchy.get_candidates(&PageType::FrontPage);
        assert_eq!(candidates, vec![
            "front-page.html",
            "home.html",
            "index.html",
        ]);
    }

    #[test]
    fn test_attachment_candidates() {
        let hierarchy = TemplateHierarchy::new("/tmp/theme");
        let candidates = hierarchy.get_candidates(&PageType::Attachment {
            mime_type: "image/jpeg".to_string(),
        });

        assert_eq!(candidates[0], "jpeg.html");
        assert_eq!(candidates[1], "attachment.html");
    }

    #[test]
    fn test_resolve_falls_back_to_index() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("index.html"), "<html></html>").unwrap();

        let hierarchy = TemplateHierarchy::new(dir.path());
        let result = hierarchy.resolve(&PageType::Search);
        assert_eq!(result, "index.html");
    }

    #[test]
    fn test_resolve_finds_specific_template() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("index.html"), "").unwrap();
        fs::write(dir.path().join("search.html"), "").unwrap();

        let hierarchy = TemplateHierarchy::new(dir.path());
        let result = hierarchy.resolve(&PageType::Search);
        assert_eq!(result, "search.html");
    }
}
