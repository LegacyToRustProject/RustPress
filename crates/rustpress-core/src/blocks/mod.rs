pub mod buttons;
pub mod code;
pub mod columns;
pub mod cover;
pub mod embed;
pub mod group;
pub mod heading;
pub mod html;
pub mod image;
pub mod list;
pub mod media_text;
pub mod paragraph;
pub mod quote;
pub mod separator;
pub mod spacer;

use serde_json::Value;

pub(crate) fn extra_classes(attrs: &Value) -> String {
    attrs
        .get("className")
        .and_then(|v| v.as_str())
        .map(|s| format!(" {}", s))
        .unwrap_or_default()
}

pub(crate) fn align_class(attrs: &Value) -> Option<&'static str> {
    match attrs.get("align").and_then(|v| v.as_str()) {
        Some("left") => Some("alignleft"),
        Some("center") => Some("aligncenter"),
        Some("right") => Some("alignright"),
        Some("wide") => Some("alignwide"),
        Some("full") => Some("alignfull"),
        _ => None,
    }
}

pub(crate) fn text_align_class(attrs: &Value) -> Option<String> {
    attrs
        .get("textAlign")
        .and_then(|v| v.as_str())
        .map(|a| format!("has-text-align-{}", a))
}

pub(crate) fn color_style(attrs: &Value) -> String {
    let mut parts = Vec::new();
    if let Some(bg) = attrs.get("backgroundColor").and_then(|v| v.as_str()) {
        parts.push(format!("background-color: var(--wp--preset--color--{})", bg));
    }
    if let Some(style) = attrs.get("style").and_then(|v| v.get("color")) {
        if let Some(bg) = style.get("background").and_then(|v| v.as_str()) {
            parts.push(format!("background-color: {}", bg));
        }
        if let Some(fg) = style.get("text").and_then(|v| v.as_str()) {
            parts.push(format!("color: {}", fg));
        }
    }
    if let Some(fg) = attrs.get("textColor").and_then(|v| v.as_str()) {
        parts.push(format!("color: var(--wp--preset--color--{})", fg));
    }
    parts.join("; ")
}

pub fn render_block(block_name: &str, attrs: &Value, inner_html: &str) -> String {
    match block_name {
        "core/paragraph" => paragraph::render(attrs, inner_html),
        "core/heading" => heading::render(attrs, inner_html),
        "core/image" => image::render(attrs, inner_html),
        "core/list" => list::render(attrs, inner_html),
        "core/list-item" => list::render_item(attrs, inner_html),
        "core/quote" => quote::render(attrs, inner_html),
        "core/code" => code::render(attrs, inner_html),
        "core/buttons" => buttons::render_wrapper(attrs, inner_html),
        "core/button" => buttons::render_button(attrs, inner_html),
        "core/columns" => columns::render_wrapper(attrs, inner_html),
        "core/column" => columns::render_column(attrs, inner_html),
        "core/separator" => separator::render(attrs, inner_html),
        "core/spacer" => spacer::render(attrs, inner_html),
        "core/media-text" => media_text::render(attrs, inner_html),
        "core/cover" => cover::render(attrs, inner_html),
        "core/group" => group::render(attrs, inner_html),
        "core/embed" => embed::render(attrs, inner_html),
        "core/html" => html::render(attrs, inner_html),
        _ => inner_html.to_string(),
    }
}
