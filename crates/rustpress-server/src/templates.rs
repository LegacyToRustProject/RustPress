use std::path::Path;

use rustpress_themes::ThemeEngine;

/// Initialize the template/theme engine.
///
/// If an active theme exists under `themes_dir`, uses that theme's templates.
/// Otherwise falls back to the legacy `templates/` directory.
pub fn init_theme_engine(
    themes_dir: &str,
    templates_dir: &str,
    active_theme: &str,
) -> Result<ThemeEngine, String> {
    let themes_path = Path::new(themes_dir);

    // Try themes/{active_theme}/templates/ first
    if active_theme != "default" {
        let theme_templates = themes_path.join(active_theme).join("templates");
        if theme_templates.exists() {
            tracing::info!(theme = active_theme, "loading theme from themes directory");
            return ThemeEngine::from_templates_dir(&theme_templates).map_err(|e| e.to_string());
        }

        // Also try themes/{active_theme}/ directly (templates at root)
        let theme_dir = themes_path.join(active_theme);
        if theme_dir.exists() && theme_dir.join("base.html").exists() {
            tracing::info!(
                theme = active_theme,
                "loading theme from themes directory (flat layout)"
            );
            return ThemeEngine::from_templates_dir(&theme_dir).map_err(|e| e.to_string());
        }

        tracing::warn!(
            theme = active_theme,
            "theme not found in themes dir, falling back to templates/"
        );
    }

    // Fallback to legacy templates/ directory
    let path = Path::new(templates_dir);
    if !path.exists() {
        return Err(format!("Templates directory not found: {templates_dir}"));
    }

    ThemeEngine::from_templates_dir(path).map_err(|e| e.to_string())
}

/// Initialize the admin template engine (separate from the frontend theme).
pub fn init_admin_tera(admin_templates_dir: &str) -> Result<tera::Tera, String> {
    let path = Path::new(admin_templates_dir);

    if !path.exists() {
        std::fs::create_dir_all(path)
            .map_err(|e| format!("Failed to create admin templates dir: {e}"))?;
    }

    // Load from the parent "templates/" directory so that template names
    // retain the "admin/" prefix and {% extends "admin/base.html" %} resolves.
    let parent = path.parent().unwrap_or(Path::new("templates"));
    let glob = format!("{}/**/*.html", parent.display());
    tera::Tera::new(&glob).map_err(|e| e.to_string())
}

/// Resolve the static directory for a theme.
///
/// If the active theme has a `static/` directory under `themes/`, returns that path.
/// Otherwise returns the default `static/` directory.
pub fn resolve_theme_static_dir(
    themes_dir: &str,
    active_theme: &str,
    default_static: &str,
) -> String {
    if active_theme != "default" {
        let theme_static = Path::new(themes_dir).join(active_theme).join("static");
        if theme_static.exists() {
            return theme_static.to_string_lossy().to_string();
        }
    }
    default_static.to_string()
}

/// Resolve the theme.json path for the active theme.
///
/// Checks (in order): themes/{active}/theme.json, themes/{active}/static/theme.json,
/// then falls back to the default static dir.
pub fn resolve_theme_json_path(
    themes_dir: &str,
    active_theme: &str,
    default_static: &str,
) -> std::path::PathBuf {
    if active_theme != "default" {
        let base = Path::new(themes_dir).join(active_theme);
        // theme.json at theme root (standard for block themes)
        let root_json = base.join("theme.json");
        if root_json.exists() {
            return root_json;
        }
        // theme.json in static/ subdir
        let static_json = base.join("static").join("theme.json");
        if static_json.exists() {
            return static_json;
        }
    }
    Path::new(default_static).join("theme.json")
}
