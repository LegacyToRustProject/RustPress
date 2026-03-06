use std::path::Path;

use rustpress_themes::ThemeEngine;

/// Initialize the template/theme engine.
pub fn init_theme_engine(templates_dir: &str) -> Result<ThemeEngine, String> {
    let path = Path::new(templates_dir);

    if !path.exists() {
        return Err(format!("Templates directory not found: {}", templates_dir));
    }

    ThemeEngine::from_templates_dir(path).map_err(|e| e.to_string())
}

/// Initialize the admin template engine (separate from the frontend theme).
pub fn init_admin_tera(admin_templates_dir: &str) -> Result<tera::Tera, String> {
    let path = Path::new(admin_templates_dir);

    if !path.exists() {
        std::fs::create_dir_all(path)
            .map_err(|e| format!("Failed to create admin templates dir: {}", e))?;
    }

    // Load from the parent "templates/" directory so that template names
    // retain the "admin/" prefix and {% extends "admin/base.html" %} resolves.
    let parent = path.parent().unwrap_or(Path::new("templates"));
    let glob = format!("{}/**/*.html", parent.display());
    tera::Tera::new(&glob).map_err(|e| e.to_string())
}
