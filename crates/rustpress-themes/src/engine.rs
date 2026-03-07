use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use tera::{Context, Tera};
use thiserror::Error;
use tracing::{debug, info, warn};

use crate::hierarchy::{PageType, TemplateHierarchy};

#[derive(Error, Debug)]
pub enum ThemeError {
    #[error("Template error: {0}")]
    Template(#[from] tera::Error),
    #[error("Theme not found: {0}")]
    NotFound(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Metadata for a theme, read from theme.json.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeMeta {
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    #[serde(default)]
    pub screenshot: Option<String>,
    /// URL-friendly identifier derived from the directory name.
    #[serde(skip_deserializing)]
    pub slug: String,
    /// Whether this theme is currently active.
    #[serde(skip_deserializing)]
    pub active: bool,
}

/// Theme engine that integrates Tera templates with WordPress template hierarchy.
pub struct ThemeEngine {
    tera: Tera,
    hierarchy: TemplateHierarchy,
    theme_dir: PathBuf,
    active_theme: String,
}

impl ThemeEngine {
    /// Create a new theme engine with the given themes directory and active theme name.
    pub fn new(themes_base_dir: &Path, active_theme: &str) -> Result<Self, ThemeError> {
        let theme_dir = themes_base_dir.join(active_theme);

        if !theme_dir.exists() {
            return Err(ThemeError::NotFound(active_theme.to_string()));
        }

        let glob_pattern = format!("{}/**/*.html", theme_dir.display());
        let tera = Tera::new(&glob_pattern)?;

        info!(theme = active_theme, "theme engine initialized");

        Ok(Self {
            tera,
            hierarchy: TemplateHierarchy::new(&theme_dir),
            theme_dir,
            active_theme: active_theme.to_string(),
        })
    }

    /// Create a theme engine from a single templates directory (non-theme mode).
    pub fn from_templates_dir(templates_dir: &Path) -> Result<Self, ThemeError> {
        let glob_pattern = format!("{}/**/*.html", templates_dir.display());
        let tera = Tera::new(&glob_pattern)?;

        info!("theme engine initialized from templates directory");

        Ok(Self {
            tera,
            hierarchy: TemplateHierarchy::new(templates_dir),
            theme_dir: templates_dir.to_path_buf(),
            active_theme: "default".to_string(),
        })
    }

    /// Render a page using template hierarchy resolution.
    pub fn render_page(
        &self,
        page_type: &PageType,
        context: &Context,
    ) -> Result<String, ThemeError> {
        let template_name = self.hierarchy.resolve(page_type);
        debug!(template = &template_name, "rendering template");
        let html = self.tera.render(&template_name, context)?;
        Ok(html)
    }

    /// Render a specific template by name.
    pub fn render(&self, template_name: &str, context: &Context) -> Result<String, ThemeError> {
        let html = self.tera.render(template_name, context)?;
        Ok(html)
    }

    /// Create a base context with common site-wide variables.
    pub fn base_context(&self, site_name: &str, site_description: &str, site_url: &str) -> Context {
        let mut context = Context::new();
        context.insert("site_name", site_name);
        context.insert("site_description", site_description);
        context.insert("site_url", site_url);
        context.insert("theme_name", &self.active_theme);
        context.insert("rustpress_version", env!("CARGO_PKG_VERSION"));
        context
    }

    /// Get the active theme name.
    pub fn active_theme(&self) -> &str {
        &self.active_theme
    }

    /// Get the theme directory path.
    pub fn theme_dir(&self) -> &Path {
        &self.theme_dir
    }

    /// Get a mutable reference to the inner Tera instance.
    ///
    /// This is useful for registering custom Tera functions (e.g. i18n
    /// translation helpers) from outside the themes crate.
    pub fn tera_mut(&mut self) -> &mut Tera {
        &mut self.tera
    }

    /// Reload templates from disk.
    pub fn reload(&mut self) -> Result<(), ThemeError> {
        let glob_pattern = format!("{}/**/*.html", self.theme_dir.display());
        self.tera = Tera::new(&glob_pattern)?;
        info!(theme = &self.active_theme, "templates reloaded");
        Ok(())
    }

    /// Discover all available themes by scanning the `themes/` directory.
    ///
    /// Each theme directory is expected to contain a `theme.json` file with
    /// metadata.  If the `themes/` directory does not exist, only the built-in
    /// default theme is returned.
    ///
    /// `themes_base_dir` is the path to the `themes/` directory (e.g. `./themes`).
    /// `templates_dir` is the fallback `templates/` directory that holds the
    /// built-in default theme (it should contain a `theme.json` as well).
    pub fn discover_themes(
        themes_base_dir: &Path,
        templates_dir: &Path,
        active_theme_slug: &str,
    ) -> Vec<ThemeMeta> {
        let mut themes: Vec<ThemeMeta> = Vec::new();

        // Always include the built-in "default" theme sourced from templates_dir.
        let default_meta_path = templates_dir.join("theme.json");
        let default_theme = if default_meta_path.exists() {
            match fs::read_to_string(&default_meta_path) {
                Ok(content) => match serde_json::from_str::<ThemeMeta>(&content) {
                    Ok(mut meta) => {
                        meta.slug = "default".to_string();
                        meta.active = active_theme_slug == "default";
                        Some(meta)
                    }
                    Err(e) => {
                        warn!("Failed to parse templates/theme.json: {}", e);
                        None
                    }
                },
                Err(e) => {
                    warn!("Failed to read templates/theme.json: {}", e);
                    None
                }
            }
        } else {
            // Synthesize a minimal default theme entry.
            Some(ThemeMeta {
                name: "RustPress Default".to_string(),
                version: "1.0.0".to_string(),
                author: "RustPress".to_string(),
                description: "The default RustPress theme with a clean, modern design.".to_string(),
                screenshot: None,
                slug: "default".to_string(),
                active: active_theme_slug == "default",
            })
        };

        if let Some(dt) = default_theme {
            themes.push(dt);
        }

        // Scan the themes/ directory for additional themes.
        if themes_base_dir.exists() && themes_base_dir.is_dir() {
            if let Ok(entries) = fs::read_dir(themes_base_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if !path.is_dir() {
                        continue;
                    }

                    let slug = match path.file_name().and_then(|n| n.to_str()) {
                        Some(name) => name.to_string(),
                        None => continue,
                    };

                    // Skip if this slug collides with the built-in default
                    // (the default is already added above).
                    if slug == "default" {
                        continue;
                    }

                    let meta_path = path.join("theme.json");
                    if !meta_path.exists() {
                        debug!(slug = %slug, "skipping theme directory without theme.json");
                        continue;
                    }

                    match fs::read_to_string(&meta_path) {
                        Ok(content) => match serde_json::from_str::<ThemeMeta>(&content) {
                            Ok(mut meta) => {
                                meta.slug = slug.clone();
                                meta.active = active_theme_slug == slug;
                                themes.push(meta);
                            }
                            Err(e) => {
                                warn!(slug = %slug, "failed to parse theme.json: {}", e);
                            }
                        },
                        Err(e) => {
                            warn!(slug = %slug, "failed to read theme.json: {}", e);
                        }
                    }
                }
            }
        }

        // Sort so the active theme comes first, then alphabetically.
        themes.sort_by(|a, b| b.active.cmp(&a.active).then_with(|| a.name.cmp(&b.name)));

        themes
    }
}
