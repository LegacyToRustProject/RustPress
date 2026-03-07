//! WordPress theme.json parser and CSS variable generator.
//!
//! Parses `theme.json` (version 2/3) and generates the equivalent CSS
//! custom properties that WordPress outputs as `global-styles-inline-css`.
//!
//! Reference: <https://developer.wordpress.org/themes/global-settings-and-styles/>

use serde::Deserialize;
use serde_json::Value;
use std::path::Path;

/// Parsed theme.json data.
#[derive(Debug, Deserialize)]
pub struct ThemeJson {
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub settings: Settings,
    #[serde(default)]
    pub styles: Value,
}

#[derive(Debug, Default, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub color: ColorSettings,
    #[serde(default)]
    pub layout: LayoutSettings,
    #[serde(default)]
    pub spacing: SpacingSettings,
    #[serde(default)]
    pub typography: TypographySettings,
    #[serde(default, rename = "useRootPaddingAwareAlignments")]
    pub use_root_padding_aware_alignments: bool,
}

#[derive(Debug, Default, Deserialize)]
pub struct ColorSettings {
    #[serde(default)]
    pub palette: Vec<PaletteEntry>,
    #[serde(default)]
    pub gradients: Vec<GradientEntry>,
}

#[derive(Debug, Deserialize)]
pub struct PaletteEntry {
    pub color: String,
    pub name: String,
    pub slug: String,
}

#[derive(Debug, Deserialize)]
pub struct GradientEntry {
    pub gradient: String,
    pub name: String,
    pub slug: String,
}

#[derive(Debug, Default, Deserialize)]
pub struct LayoutSettings {
    #[serde(default, rename = "contentSize")]
    pub content_size: Option<String>,
    #[serde(default, rename = "wideSize")]
    pub wide_size: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct SpacingSettings {
    #[serde(default, rename = "spacingSizes")]
    pub spacing_sizes: Vec<SpacingSize>,
}

#[derive(Debug, Deserialize)]
pub struct SpacingSize {
    pub name: String,
    pub size: String,
    pub slug: String,
}

#[derive(Debug, Default, Deserialize)]
pub struct TypographySettings {
    #[serde(default)]
    pub fluid: Value,
    #[serde(default, rename = "fontSizes")]
    pub font_sizes: Vec<FontSize>,
    #[serde(default, rename = "fontFamilies")]
    pub font_families: Vec<FontFamily>,
}

#[derive(Debug, Deserialize)]
pub struct FontSize {
    pub name: String,
    pub size: String,
    pub slug: String,
    #[serde(default)]
    pub fluid: Value,
}

#[derive(Debug, Deserialize)]
pub struct FontFamily {
    pub name: String,
    pub slug: String,
    #[serde(default, rename = "fontFamily")]
    pub font_family: String,
    #[serde(default, rename = "fontFace")]
    pub font_face: Vec<FontFace>,
}

#[derive(Debug, Deserialize)]
pub struct FontFace {
    #[serde(default)]
    pub src: Vec<String>,
    #[serde(default, rename = "fontWeight")]
    pub font_weight: String,
    #[serde(default, rename = "fontStyle")]
    pub font_style: String,
    #[serde(default, rename = "fontFamily")]
    pub font_family: String,
}

impl ThemeJson {
    /// Load and parse theme.json from a file path.
    pub fn from_file(path: &Path) -> Result<Self, String> {
        let content =
            std::fs::read_to_string(path).map_err(|e| format!("Failed to read theme.json: {e}"))?;
        serde_json::from_str(&content).map_err(|e| format!("Failed to parse theme.json: {e}"))
    }

    /// Load from a JSON string.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(json: &str) -> Result<Self, String> {
        serde_json::from_str(json).map_err(|e| format!("Failed to parse theme.json: {e}"))
    }

    /// Generate CSS custom properties matching WordPress's global-styles-inline-css output.
    ///
    /// This produces the `:root` block with all `--wp--preset--*` variables
    /// that WordPress generates from theme.json settings.
    pub fn generate_css_variables(&self) -> String {
        let mut vars = Vec::new();

        // Color palette → --wp--preset--color--{slug}
        for entry in &self.settings.color.palette {
            vars.push(format!(
                "\t--wp--preset--color--{}: {};",
                entry.slug, entry.color
            ));
        }

        // Gradients → --wp--preset--gradient--{slug}
        for entry in &self.settings.color.gradients {
            vars.push(format!(
                "\t--wp--preset--gradient--{}: {};",
                entry.slug, entry.gradient
            ));
        }

        // Font sizes → --wp--preset--font-size--{slug}
        // WordPress generates clamp() for fluid font sizes
        for fs in &self.settings.typography.font_sizes {
            let value = self.compute_fluid_font_size(fs);
            vars.push(format!(
                "\t--wp--preset--font-size--{}: {};",
                fs.slug, value
            ));
        }

        // Font families → --wp--preset--font-family--{slug}
        for ff in &self.settings.typography.font_families {
            vars.push(format!(
                "\t--wp--preset--font-family--{}: {};",
                ff.slug, ff.font_family
            ));
        }

        // Spacing sizes → --wp--preset--spacing--{slug}
        for sp in &self.settings.spacing.spacing_sizes {
            vars.push(format!(
                "\t--wp--preset--spacing--{}: {};",
                sp.slug, sp.size
            ));
        }

        // Layout
        if let Some(ref cs) = self.settings.layout.content_size {
            vars.push(format!("\t--wp--style--global--content-size: {cs};"));
        }
        if let Some(ref ws) = self.settings.layout.wide_size {
            vars.push(format!("\t--wp--style--global--wide-size: {ws};"));
        }

        // Build CSS
        let mut css = String::new();

        // @font-face declarations
        css.push_str(&self.generate_font_face_css());

        // :root variables
        if !vars.is_empty() {
            css.push_str("body {\n");
            for var in &vars {
                css.push_str(var);
                css.push('\n');
            }
            css.push_str("}\n");
        }

        // Body styles from styles section
        css.push_str(&self.generate_body_styles());

        // Block-specific styles
        css.push_str(&self.generate_block_styles());

        // Element styles (links, headings, buttons, captions)
        css.push_str(&self.generate_element_styles());

        css
    }

    /// Compute fluid font size using clamp() if fluid settings are present.
    fn compute_fluid_font_size(&self, fs: &FontSize) -> String {
        match &fs.fluid {
            Value::Object(obj) => {
                let min = obj.get("min").and_then(|v| v.as_str()).unwrap_or(&fs.size);
                let max = obj.get("max").and_then(|v| v.as_str()).unwrap_or(&fs.size);
                // WordPress uses a specific formula for the viewport-relative middle value
                // clamp(min, min + (max - min) * ((100vw - 320px) / (1600 - 320)), max)
                // Simplified: clamp(min, calc(viewport-formula), max)
                let min_rem = parse_rem_value(min);
                let max_rem = parse_rem_value(max);
                if (max_rem - min_rem).abs() < 0.001 {
                    return fs.size.clone();
                }
                // WordPress formula: clamp(min, viewportFactor, max)
                // viewportFactor = min + ((max - min) * ((100vw - 320px) / (1600 - 320)))
                let diff = max_rem - min_rem;
                let factor = diff / 80.0; // (1600-320)/16 = 80rem range
                format!("clamp({min}, calc({min} + ((100vw - 20rem) * {factor:.4})), {max})")
            }
            Value::Bool(false) => fs.size.clone(),
            _ => fs.size.clone(),
        }
    }

    /// Generate @font-face CSS declarations.
    fn generate_font_face_css(&self) -> String {
        let mut css = String::new();
        for ff in &self.settings.typography.font_families {
            for face in &ff.font_face {
                css.push_str("@font-face {\n");
                css.push_str(&format!("\tfont-family: {};\n", face.font_family));
                if !face.font_weight.is_empty() {
                    css.push_str(&format!("\tfont-weight: {};\n", face.font_weight));
                }
                if !face.font_style.is_empty() {
                    css.push_str(&format!("\tfont-style: {};\n", face.font_style));
                }
                // Convert file:./assets/fonts/... to /static/fonts/...
                let srcs: Vec<String> = face
                    .src
                    .iter()
                    .map(|s| {
                        let path = s
                            .replace("file:./assets/fonts/", "/static/fonts/")
                            .replace("file:./", "/static/");
                        // Determine format from extension
                        let format = if path.ends_with(".woff2") {
                            "woff2"
                        } else if path.ends_with(".woff") {
                            "woff"
                        } else if path.ends_with(".ttf") {
                            "truetype"
                        } else {
                            "woff2"
                        };
                        format!("url(\"{path}\") format(\"{format}\")")
                    })
                    .collect();
                css.push_str(&format!("\tsrc: {};\n", srcs.join(", ")));
                css.push_str("\tfont-display: fallback;\n");
                css.push_str("}\n");
            }
        }
        css
    }

    /// Generate body-level styles from the styles section.
    fn generate_body_styles(&self) -> String {
        let styles = &self.styles;
        if styles.is_null() {
            return String::new();
        }

        let mut css = String::new();
        let mut body_props = Vec::new();

        // Background and text color
        if let Some(bg) = styles.pointer("/color/background").and_then(|v| v.as_str()) {
            body_props.push(format!("\tbackground-color: {};", resolve_var_ref(bg)));
        }
        if let Some(text) = styles.pointer("/color/text").and_then(|v| v.as_str()) {
            body_props.push(format!("\tcolor: {};", resolve_var_ref(text)));
        }

        // Typography
        if let Some(ff) = styles
            .pointer("/typography/fontFamily")
            .and_then(|v| v.as_str())
        {
            body_props.push(format!("\tfont-family: {};", resolve_var_ref(ff)));
        }
        if let Some(fs) = styles
            .pointer("/typography/fontSize")
            .and_then(|v| v.as_str())
        {
            body_props.push(format!("\tfont-size: {};", resolve_var_ref(fs)));
        }
        if let Some(fw) = styles
            .pointer("/typography/fontWeight")
            .and_then(|v| v.as_str())
        {
            body_props.push(format!("\tfont-weight: {fw};"));
        }
        if let Some(lh) = styles
            .pointer("/typography/lineHeight")
            .and_then(|v| v.as_str())
        {
            body_props.push(format!("\tline-height: {lh};"));
        }
        if let Some(ls) = styles
            .pointer("/typography/letterSpacing")
            .and_then(|v| v.as_str())
        {
            body_props.push(format!("\tletter-spacing: {ls};"));
        }

        // Spacing
        if let Some(gap) = styles.pointer("/spacing/blockGap").and_then(|v| v.as_str()) {
            body_props.push(format!(
                "\t--wp--style--block-gap: {};",
                resolve_var_ref(gap)
            ));
        }
        // WordPress converts body padding to --wp--style--root--padding-* CSS variables,
        // NOT direct padding properties. These variables are consumed by .wp-site-blocks
        // and .has-global-padding selectors.
        if let Some(pt) = styles
            .pointer("/spacing/padding/top")
            .and_then(|v| v.as_str())
        {
            body_props.push(format!(
                "\t--wp--style--root--padding-top: {};",
                resolve_var_ref(pt)
            ));
        }
        if let Some(pr) = styles
            .pointer("/spacing/padding/right")
            .and_then(|v| v.as_str())
        {
            body_props.push(format!(
                "\t--wp--style--root--padding-right: {};",
                resolve_var_ref(pr)
            ));
        }
        if let Some(pb) = styles
            .pointer("/spacing/padding/bottom")
            .and_then(|v| v.as_str())
        {
            body_props.push(format!(
                "\t--wp--style--root--padding-bottom: {};",
                resolve_var_ref(pb)
            ));
        }
        if let Some(pl) = styles
            .pointer("/spacing/padding/left")
            .and_then(|v| v.as_str())
        {
            body_props.push(format!(
                "\t--wp--style--root--padding-left: {};",
                resolve_var_ref(pl)
            ));
        }

        if !body_props.is_empty() {
            css.push_str("body {\n");
            for prop in &body_props {
                css.push_str(prop);
                css.push('\n');
            }
            css.push_str("}\n");
        }

        css
    }

    /// Generate element-level styles (links, headings, buttons, captions).
    fn generate_element_styles(&self) -> String {
        let styles = &self.styles;
        if styles.is_null() {
            return String::new();
        }

        let mut css = String::new();

        // Elements
        if let Some(elements) = styles.get("elements").and_then(|v| v.as_object()) {
            for (element, props) in elements {
                let selector = match element.as_str() {
                    "link" => "a",
                    "heading" => "h1, h2, h3, h4, h5, h6",
                    "h1" => "h1",
                    "h2" => "h2",
                    "h3" => "h3",
                    "h4" => "h4",
                    "h5" => "h5",
                    "h6" => "h6",
                    "button" => ".wp-element-button, .wp-block-button__link",
                    "caption" => ".wp-element-caption, .wp-block-image figcaption",
                    _ => continue,
                };
                let declarations = extract_css_declarations(props);
                if !declarations.is_empty() {
                    css.push_str(&format!("{selector} {{\n"));
                    for decl in &declarations {
                        css.push_str(&format!("\t{decl};\n"));
                    }
                    css.push_str("}\n");
                }

                // :hover state
                if let Some(hover) = props.get(":hover") {
                    let hover_decls = extract_css_declarations(hover);
                    if !hover_decls.is_empty() {
                        let hover_sel = match element.as_str() {
                            "link" => "a:hover",
                            "button" => ".wp-element-button:hover, .wp-block-button__link:hover",
                            _ => continue,
                        };
                        css.push_str(&format!("{hover_sel} {{\n"));
                        for decl in &hover_decls {
                            css.push_str(&format!("\t{decl};\n"));
                        }
                        css.push_str("}\n");
                    }
                }
            }
        }

        css
    }

    /// Generate block-specific styles from styles.blocks section.
    fn generate_block_styles(&self) -> String {
        let styles = &self.styles;
        if styles.is_null() {
            return String::new();
        }

        let mut css = String::new();

        if let Some(blocks) = styles.get("blocks").and_then(|v| v.as_object()) {
            for (block_name, props) in blocks {
                // Convert core/heading → .wp-block-heading
                let selector = block_name_to_selector(block_name);
                if selector.is_empty() {
                    continue;
                }

                let declarations = extract_css_declarations(props);
                if !declarations.is_empty() {
                    css.push_str(&format!("{selector} {{\n"));
                    for decl in &declarations {
                        css.push_str(&format!("\t{decl};\n"));
                    }
                    css.push_str("}\n");
                }

                // Block elements (links inside blocks, etc.)
                if let Some(elements) = props.get("elements").and_then(|v| v.as_object()) {
                    for (element, elem_props) in elements {
                        let elem_sel = match element.as_str() {
                            "link" => "a",
                            "heading" => "h1, h2, h3, h4, h5, h6",
                            _ => continue,
                        };
                        let decls = extract_css_declarations(elem_props);
                        if !decls.is_empty() {
                            css.push_str(&format!("{selector} {elem_sel} {{\n"));
                            for decl in &decls {
                                css.push_str(&format!("\t{decl};\n"));
                            }
                            css.push_str("}\n");
                        }

                        // :hover
                        if let Some(hover) = elem_props.get(":hover") {
                            let hover_decls = extract_css_declarations(hover);
                            if !hover_decls.is_empty() {
                                css.push_str(&format!("{selector} {elem_sel}:hover {{\n"));
                                for decl in &hover_decls {
                                    css.push_str(&format!("\t{decl};\n"));
                                }
                                css.push_str("}\n");
                            }
                        }
                    }
                }

                // Inline CSS property (raw CSS)
                if let Some(raw_css) = props.get("css").and_then(|v| v.as_str()) {
                    // WordPress prefixes with the block selector
                    css.push_str(&format!("{selector} {{ {raw_css} }}\n"));
                }
            }
        }

        css
    }
}

/// Convert `var:preset|type|slug` references to `var(--wp--preset--type--slug)`.
fn resolve_var_ref(value: &str) -> String {
    if let Some(rest) = value.strip_prefix("var:") {
        let parts: Vec<&str> = rest.split('|').collect();
        if parts.len() >= 3 {
            format!("var(--wp--{}--{}--{})", parts[0], parts[1], parts[2])
        } else if parts.len() == 2 {
            format!("var(--wp--{}--{})", parts[0], parts[1])
        } else {
            value.to_string()
        }
    } else {
        value.to_string()
    }
}

/// Parse a rem/px value to f64 rem.
fn parse_rem_value(s: &str) -> f64 {
    let s = s.trim();
    if let Some(v) = s.strip_suffix("rem") {
        v.parse().unwrap_or(1.0)
    } else if let Some(v) = s.strip_suffix("px") {
        v.parse::<f64>().unwrap_or(16.0) / 16.0
    } else if let Some(v) = s.strip_suffix("em") {
        v.parse().unwrap_or(1.0)
    } else {
        s.parse().unwrap_or(1.0)
    }
}

/// Convert block name (e.g. "core/heading") to CSS selector (e.g. ".wp-block-heading").
fn block_name_to_selector(name: &str) -> String {
    match name {
        "core/paragraph" => "p".to_string(),
        "core/heading" => ".wp-block-heading".to_string(),
        "core/image" => ".wp-block-image".to_string(),
        "core/quote" => ".wp-block-quote".to_string(),
        "core/code" => ".wp-block-code".to_string(),
        "core/preformatted" => ".wp-block-preformatted".to_string(),
        "core/pullquote" => ".wp-block-pullquote".to_string(),
        "core/table" => ".wp-block-table".to_string(),
        "core/list" => ".wp-block-list".to_string(),
        "core/separator" => ".wp-block-separator".to_string(),
        "core/columns" => ".wp-block-columns".to_string(),
        "core/column" => ".wp-block-column".to_string(),
        "core/group" => ".wp-block-group".to_string(),
        "core/buttons" => ".wp-block-buttons".to_string(),
        "core/button" => ".wp-block-button".to_string(),
        "core/cover" => ".wp-block-cover".to_string(),
        "core/gallery" => ".wp-block-gallery".to_string(),
        "core/spacer" => ".wp-block-spacer".to_string(),
        "core/site-title" => ".wp-block-site-title".to_string(),
        "core/navigation" => ".wp-block-navigation".to_string(),
        "core/post-title" => ".wp-block-post-title".to_string(),
        "core/post-date" => ".wp-block-post-date".to_string(),
        "core/post-content" => ".wp-block-post-content".to_string(),
        "core/post-template" => ".wp-block-post-template".to_string(),
        "core/avatar" => ".wp-block-avatar".to_string(),
        "core/comment-author-name" => ".wp-block-comment-author-name".to_string(),
        "core/comment-content" => ".wp-block-comment-content".to_string(),
        "core/comment-date" => ".wp-block-comment-date".to_string(),
        "core/comment-edit-link" => ".wp-block-comment-edit-link".to_string(),
        "core/comment-reply-link" => ".wp-block-comment-reply-link".to_string(),
        "core/post-comments-form" => ".wp-block-post-comments-form".to_string(),
        _ => {
            // Generic: core/foo-bar → .wp-block-foo-bar
            if let Some(name) = name.strip_prefix("core/") {
                format!(".wp-block-{name}")
            } else {
                String::new()
            }
        }
    }
}

/// Extract CSS property declarations from a theme.json style object.
fn extract_css_declarations(props: &Value) -> Vec<String> {
    let mut decls = Vec::new();

    // Color
    if let Some(bg) = props.pointer("/color/background").and_then(|v| v.as_str()) {
        decls.push(format!("background-color: {}", resolve_var_ref(bg)));
    }
    if let Some(text) = props.pointer("/color/text").and_then(|v| v.as_str()) {
        decls.push(format!("color: {}", resolve_var_ref(text)));
    }

    // Typography
    if let Some(ff) = props
        .pointer("/typography/fontFamily")
        .and_then(|v| v.as_str())
    {
        decls.push(format!("font-family: {}", resolve_var_ref(ff)));
    }
    if let Some(fs) = props
        .pointer("/typography/fontSize")
        .and_then(|v| v.as_str())
    {
        decls.push(format!("font-size: {}", resolve_var_ref(fs)));
    }
    if let Some(fw) = props
        .pointer("/typography/fontWeight")
        .and_then(|v| v.as_str())
    {
        decls.push(format!("font-weight: {fw}"));
    }
    if let Some(lh) = props
        .pointer("/typography/lineHeight")
        .and_then(|v| v.as_str())
    {
        decls.push(format!("line-height: {lh}"));
    }
    if let Some(ls) = props
        .pointer("/typography/letterSpacing")
        .and_then(|v| v.as_str())
    {
        decls.push(format!("letter-spacing: {ls}"));
    }
    if let Some(td) = props
        .pointer("/typography/textDecoration")
        .and_then(|v| v.as_str())
    {
        decls.push(format!("text-decoration: {td}"));
    }
    if let Some(tt) = props
        .pointer("/typography/textTransform")
        .and_then(|v| v.as_str())
    {
        decls.push(format!("text-transform: {tt}"));
    }

    // Spacing
    if let Some(mt) = props
        .pointer("/spacing/margin/top")
        .and_then(|v| v.as_str())
    {
        decls.push(format!("margin-top: {}", resolve_var_ref(mt)));
    }
    if let Some(mb) = props
        .pointer("/spacing/margin/bottom")
        .and_then(|v| v.as_str())
    {
        decls.push(format!("margin-bottom: {}", resolve_var_ref(mb)));
    }
    if let Some(pt) = props
        .pointer("/spacing/padding/top")
        .and_then(|v| v.as_str())
    {
        decls.push(format!("padding-top: {}", resolve_var_ref(pt)));
    }
    if let Some(pr) = props
        .pointer("/spacing/padding/right")
        .and_then(|v| v.as_str())
    {
        decls.push(format!("padding-right: {}", resolve_var_ref(pr)));
    }
    if let Some(pb) = props
        .pointer("/spacing/padding/bottom")
        .and_then(|v| v.as_str())
    {
        decls.push(format!("padding-bottom: {}", resolve_var_ref(pb)));
    }
    if let Some(pl) = props
        .pointer("/spacing/padding/left")
        .and_then(|v| v.as_str())
    {
        decls.push(format!("padding-left: {}", resolve_var_ref(pl)));
    }
    if let Some(gap) = props.pointer("/spacing/blockGap").and_then(|v| v.as_str()) {
        decls.push(format!("gap: {}", resolve_var_ref(gap)));
    }

    // Border
    if let Some(br) = props.pointer("/border/radius").and_then(|v| v.as_str()) {
        decls.push(format!("border-radius: {br}"));
    }
    if let Some(bc) = props.pointer("/border/color").and_then(|v| v.as_str()) {
        decls.push(format!("border-color: {}", resolve_var_ref(bc)));
    }
    if let Some(bw) = props.pointer("/border/width").and_then(|v| v.as_str()) {
        decls.push(format!("border-width: {bw}"));
    }
    if let Some(bs) = props.pointer("/border/style").and_then(|v| v.as_str()) {
        decls.push(format!("border-style: {bs}"));
    }
    // Individual side borders
    for side in &["top", "right", "bottom", "left"] {
        if let Some(border_side) = props.pointer(&format!("/border/{side}")) {
            if let Some(obj) = border_side.as_object() {
                let mut parts = Vec::new();
                if let Some(w) = obj.get("width").and_then(|v| v.as_str()) {
                    parts.push(w.to_string());
                }
                if let Some(s) = obj.get("style").and_then(|v| v.as_str()) {
                    parts.push(s.to_string());
                }
                if let Some(c) = obj.get("color").and_then(|v| v.as_str()) {
                    parts.push(resolve_var_ref(c));
                }
                if !parts.is_empty() {
                    decls.push(format!("border-{}: {}", side, parts.join(" ")));
                }
            }
        }
    }

    // Margin left/right
    if let Some(ml) = props
        .pointer("/spacing/margin/left")
        .and_then(|v| v.as_str())
    {
        decls.push(format!("margin-left: {}", resolve_var_ref(ml)));
    }
    if let Some(mr) = props
        .pointer("/spacing/margin/right")
        .and_then(|v| v.as_str())
    {
        decls.push(format!("margin-right: {}", resolve_var_ref(mr)));
    }

    // Outline
    if let Some(oc) = props.pointer("/outline/color").and_then(|v| v.as_str()) {
        decls.push(format!("outline-color: {}", resolve_var_ref(oc)));
    }
    if let Some(ow) = props.pointer("/outline/width").and_then(|v| v.as_str()) {
        decls.push(format!("outline-width: {ow}"));
    }
    if let Some(os) = props.pointer("/outline/style").and_then(|v| v.as_str()) {
        decls.push(format!("outline-style: {os}"));
    }
    if let Some(oo) = props.pointer("/outline/offset").and_then(|v| v.as_str()) {
        decls.push(format!("outline-offset: {oo}"));
    }

    // Dimensions
    if let Some(mh) = props
        .pointer("/dimensions/minHeight")
        .and_then(|v| v.as_str())
    {
        decls.push(format!("min-height: {}", resolve_var_ref(mh)));
    }

    // Shadow
    if let Some(shadow) = props.pointer("/shadow").and_then(|v| v.as_str()) {
        decls.push(format!("box-shadow: {}", resolve_var_ref(shadow)));
    }

    decls
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_var_ref() {
        assert_eq!(
            resolve_var_ref("var:preset|color|base"),
            "var(--wp--preset--color--base)"
        );
        assert_eq!(
            resolve_var_ref("var:preset|font-size|large"),
            "var(--wp--preset--font-size--large)"
        );
        assert_eq!(resolve_var_ref("#FFFFFF"), "#FFFFFF");
    }

    #[test]
    fn test_block_name_to_selector() {
        assert_eq!(block_name_to_selector("core/heading"), ".wp-block-heading");
        assert_eq!(block_name_to_selector("core/button"), ".wp-block-button");
        assert_eq!(
            block_name_to_selector("core/unknown-block"),
            ".wp-block-unknown-block"
        );
    }

    #[test]
    fn test_parse_minimal_theme_json() {
        let json = r##"{
            "version": 3,
            "settings": {
                "color": {
                    "palette": [
                        {"color": "#FFFFFF", "name": "Base", "slug": "base"},
                        {"color": "#111111", "name": "Contrast", "slug": "contrast"}
                    ]
                },
                "spacing": {
                    "spacingSizes": [
                        {"name": "Small", "size": "30px", "slug": "40"}
                    ]
                },
                "typography": {
                    "fontSizes": [
                        {"name": "Small", "size": "0.875rem", "slug": "small", "fluid": false}
                    ],
                    "fontFamilies": []
                }
            },
            "styles": {}
        }"##;

        let theme = ThemeJson::from_str(json).unwrap();
        let css = theme.generate_css_variables();

        assert!(css.contains("--wp--preset--color--base: #FFFFFF"));
        assert!(css.contains("--wp--preset--color--contrast: #111111"));
        assert!(css.contains("--wp--preset--spacing--40: 30px"));
        assert!(css.contains("--wp--preset--font-size--small: 0.875rem"));
    }

    #[test]
    fn test_fluid_font_size() {
        let json = r##"{
            "version": 3,
            "settings": {
                "typography": {
                    "fluid": true,
                    "fontSizes": [
                        {
                            "name": "Large",
                            "size": "1.38rem",
                            "slug": "large",
                            "fluid": {"min": "1.125rem", "max": "1.375rem"}
                        }
                    ],
                    "fontFamilies": []
                }
            },
            "styles": {}
        }"##;

        let theme = ThemeJson::from_str(json).unwrap();
        let css = theme.generate_css_variables();

        assert!(css.contains("--wp--preset--font-size--large: clamp(1.125rem,"));
        assert!(css.contains("1.375rem)"));
    }
}
