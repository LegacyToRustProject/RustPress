use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Standard WordPress `<head>` and footer output generation.
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

/// A registered style or script asset.
#[derive(Debug, Clone)]
pub struct EnqueuedAsset {
    pub handle: String,
    pub src: String,
    pub deps: Vec<String>,
    pub version: String,
    /// For styles: media attribute. For scripts: "defer" or "async" or "".
    pub extra: String,
    /// Whether loaded in footer (scripts only).
    pub in_footer: bool,
}

/// WordPress-compatible style/script enqueue manager.
///
/// Mirrors `wp_enqueue_style()` / `wp_enqueue_script()` / `wp_register_style()` etc.
/// Registered assets are collected and rendered as `<link>` or `<script>` tags.
#[derive(Debug, Clone, Default)]
pub struct AssetManager {
    styles: Arc<RwLock<HashMap<String, EnqueuedAsset>>>,
    scripts: Arc<RwLock<HashMap<String, EnqueuedAsset>>>,
    enqueued_styles: Arc<RwLock<Vec<String>>>,
    enqueued_scripts: Arc<RwLock<Vec<String>>>,
}

impl AssetManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a stylesheet (equivalent to `wp_register_style()`).
    pub fn register_style(&self, handle: &str, src: &str, deps: &[&str], ver: &str, media: &str) {
        let asset = EnqueuedAsset {
            handle: handle.to_string(),
            src: src.to_string(),
            deps: deps.iter().map(|s| s.to_string()).collect(),
            version: ver.to_string(),
            extra: if media.is_empty() {
                "all".to_string()
            } else {
                media.to_string()
            },
            in_footer: false,
        };
        self.styles
            .write()
            .unwrap()
            .insert(handle.to_string(), asset);
    }

    /// Register and enqueue a stylesheet (equivalent to `wp_enqueue_style()`).
    pub fn enqueue_style(&self, handle: &str, src: &str, deps: &[&str], ver: &str, media: &str) {
        self.register_style(handle, src, deps, ver, media);
        let mut enqueued = self.enqueued_styles.write().unwrap();
        if !enqueued.contains(&handle.to_string()) {
            enqueued.push(handle.to_string());
        }
    }

    /// Register a script (equivalent to `wp_register_script()`).
    pub fn register_script(
        &self,
        handle: &str,
        src: &str,
        deps: &[&str],
        ver: &str,
        in_footer: bool,
    ) {
        let asset = EnqueuedAsset {
            handle: handle.to_string(),
            src: src.to_string(),
            deps: deps.iter().map(|s| s.to_string()).collect(),
            version: ver.to_string(),
            extra: String::new(),
            in_footer,
        };
        self.scripts
            .write()
            .unwrap()
            .insert(handle.to_string(), asset);
    }

    /// Register and enqueue a script (equivalent to `wp_enqueue_script()`).
    pub fn enqueue_script(
        &self,
        handle: &str,
        src: &str,
        deps: &[&str],
        ver: &str,
        in_footer: bool,
    ) {
        self.register_script(handle, src, deps, ver, in_footer);
        let mut enqueued = self.enqueued_scripts.write().unwrap();
        if !enqueued.contains(&handle.to_string()) {
            enqueued.push(handle.to_string());
        }
    }

    /// Render enqueued stylesheets as `<link>` tags for `<head>`.
    pub fn render_head_styles(&self) -> String {
        let enqueued = self.enqueued_styles.read().unwrap();
        let styles = self.styles.read().unwrap();
        let mut html = String::new();

        for handle in enqueued.iter() {
            if let Some(asset) = styles.get(handle) {
                let versioned_src = if asset.version.is_empty() {
                    asset.src.clone()
                } else {
                    format!("{}?ver={}", asset.src, asset.version)
                };
                html.push_str(&format!(
                    "<link rel=\"stylesheet\" id=\"{}-css\" href=\"{}\" media=\"{}\" />\n",
                    asset.handle, versioned_src, asset.extra
                ));
            }
        }

        html
    }

    /// Render enqueued header scripts as `<script>` tags.
    pub fn render_head_scripts(&self) -> String {
        let enqueued = self.enqueued_scripts.read().unwrap();
        let scripts = self.scripts.read().unwrap();
        let mut html = String::new();

        for handle in enqueued.iter() {
            if let Some(asset) = scripts.get(handle) {
                if !asset.in_footer {
                    let versioned_src = if asset.version.is_empty() {
                        asset.src.clone()
                    } else {
                        format!("{}?ver={}", asset.src, asset.version)
                    };
                    html.push_str(&format!(
                        "<script id=\"{}-js\" src=\"{}\"></script>\n",
                        asset.handle, versioned_src
                    ));
                }
            }
        }

        html
    }

    /// Render enqueued footer scripts as `<script>` tags.
    pub fn render_footer_scripts(&self) -> String {
        let enqueued = self.enqueued_scripts.read().unwrap();
        let scripts = self.scripts.read().unwrap();
        let mut html = String::new();

        for handle in enqueued.iter() {
            if let Some(asset) = scripts.get(handle) {
                if asset.in_footer {
                    let versioned_src = if asset.version.is_empty() {
                        asset.src.clone()
                    } else {
                        format!("{}?ver={}", asset.src, asset.version)
                    };
                    html.push_str(&format!(
                        "<script id=\"{}-js\" src=\"{}\"></script>\n",
                        asset.handle, versioned_src
                    ));
                }
            }
        }

        html
    }
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

    #[test]
    fn test_enqueue_style() {
        let mgr = AssetManager::new();
        mgr.enqueue_style("theme-style", "/style.css", &[], "1.0", "all");
        let html = mgr.render_head_styles();
        assert!(html.contains("theme-style-css"));
        assert!(html.contains("/style.css?ver=1.0"));
        assert!(html.contains("media=\"all\""));
    }

    #[test]
    fn test_enqueue_script_footer() {
        let mgr = AssetManager::new();
        mgr.enqueue_script("theme-js", "/app.js", &[], "2.0", true);
        assert!(mgr.render_head_scripts().is_empty());
        let footer = mgr.render_footer_scripts();
        assert!(footer.contains("theme-js-js"));
        assert!(footer.contains("/app.js?ver=2.0"));
    }
}
