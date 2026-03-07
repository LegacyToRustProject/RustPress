//! WordPress Block Types REST API
//!
//! GET /wp-json/wp/v2/block-types
//! GET /wp-json/wp/v2/block-types/{namespace}/{name}
//!
//! Returns registered Gutenberg block types (core blocks).

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use serde_json::{json, Value};

use crate::common::WpError;
use crate::ApiState;

pub fn routes() -> Router<ApiState> {
    Router::new()
        .route("/wp-json/wp/v2/block-types", get(list_block_types))
        .route(
            "/wp-json/wp/v2/block-types/{namespace}/{name}",
            get(get_block_type),
        )
}

/// Core block definitions (minimal WordPress-compatible subset).
fn core_blocks() -> Vec<Value> {
    let core = [
        (
            "core/paragraph",
            "Paragraph",
            "text",
            "The basic building block of prose content.",
        ),
        (
            "core/heading",
            "Heading",
            "text",
            "Introduce new sections and organize content to help visitors.",
        ),
        (
            "core/image",
            "Image",
            "media",
            "Insert an image to make a visual statement.",
        ),
        (
            "core/list",
            "List",
            "text",
            "Create a bulleted or numbered list.",
        ),
        (
            "core/quote",
            "Quote",
            "text",
            "Give quoted text visual emphasis.",
        ),
        (
            "core/code",
            "Code",
            "text",
            "Display code snippets that respect your spacing and tabs.",
        ),
        (
            "core/preformatted",
            "Preformatted",
            "text",
            "Add text that respects your spacing and tabs.",
        ),
        (
            "core/pullquote",
            "Pullquote",
            "text",
            "Give special visual emphasis to a quote from your text.",
        ),
        (
            "core/table",
            "Table",
            "text",
            "Insert a table — perfect for sharing complex data.",
        ),
        (
            "core/verse",
            "Verse",
            "text",
            "Insert poetry. Use special spacing formats.",
        ),
        (
            "core/file",
            "File",
            "media",
            "Add a link to a downloadable file.",
        ),
        (
            "core/video",
            "Video",
            "media",
            "Embed a video from your media library or upload a new one.",
        ),
        (
            "core/audio",
            "Audio",
            "media",
            "Embed a simple audio player.",
        ),
        (
            "core/cover",
            "Cover",
            "media",
            "Add an image or video with a text overlay.",
        ),
        (
            "core/gallery",
            "Gallery",
            "media",
            "Display multiple images in a rich gallery.",
        ),
        (
            "core/media-text",
            "Media & Text",
            "media",
            "Set media and words side-by-side.",
        ),
        (
            "core/buttons",
            "Buttons",
            "design",
            "Prompt visitors to take action with a group of button-style links.",
        ),
        (
            "core/button",
            "Button",
            "design",
            "Prompt visitors to take action with a button-style link.",
        ),
        (
            "core/columns",
            "Columns",
            "design",
            "Display content in multiple columns.",
        ),
        (
            "core/column",
            "Column",
            "design",
            "A single column within a columns block.",
        ),
        (
            "core/group",
            "Group",
            "design",
            "Gather blocks in a layout container.",
        ),
        (
            "core/spacer",
            "Spacer",
            "design",
            "Add white space between blocks and customize its height.",
        ),
        (
            "core/separator",
            "Separator",
            "design",
            "Create a break between ideas or sections with a horizontal separator.",
        ),
        (
            "core/html",
            "Custom HTML",
            "widgets",
            "Add custom HTML code and preview it as you edit.",
        ),
        (
            "core/shortcode",
            "Shortcode",
            "widgets",
            "Insert additional custom elements with a WordPress shortcode.",
        ),
        (
            "core/archives",
            "Archives",
            "widgets",
            "Display a date archive of your posts.",
        ),
        (
            "core/calendar",
            "Calendar",
            "widgets",
            "A calendar of your site's posts.",
        ),
        (
            "core/categories",
            "Categories",
            "widgets",
            "Display a list of all categories.",
        ),
        (
            "core/latest-comments",
            "Latest Comments",
            "widgets",
            "Display a list of your most recent comments.",
        ),
        (
            "core/latest-posts",
            "Latest Posts",
            "widgets",
            "Display a list of your most recent posts.",
        ),
        (
            "core/page-list",
            "Page List",
            "widgets",
            "Display a list of all pages.",
        ),
        (
            "core/rss",
            "RSS",
            "widgets",
            "Display entries from any RSS or Atom feed.",
        ),
        (
            "core/search",
            "Search",
            "widgets",
            "Help visitors find your content.",
        ),
        (
            "core/social-links",
            "Social Links",
            "widgets",
            "Display icons linking to your social media profiles or sites.",
        ),
        (
            "core/tag-cloud",
            "Tag Cloud",
            "widgets",
            "A cloud of popular keywords each linked to their archive.",
        ),
        (
            "core/post-title",
            "Post Title",
            "theme",
            "Displays the title of a post, page, or any other content-type.",
        ),
        (
            "core/post-content",
            "Post Content",
            "theme",
            "Displays the contents of a post or page.",
        ),
        (
            "core/post-date",
            "Post Date",
            "theme",
            "Display the publish date for an entry.",
        ),
        (
            "core/post-excerpt",
            "Post Excerpt",
            "theme",
            "Displays the excerpt of a post, if provided.",
        ),
        (
            "core/post-featured-image",
            "Featured Image",
            "theme",
            "Display a post's featured image.",
        ),
        ("core/post-terms", "Post Terms", "theme", "Post terms."),
        (
            "core/site-logo",
            "Site Logo",
            "theme",
            "Turn your site's name into a visual anchor.",
        ),
        ("core/site-title", "Site Title", "theme", "Your site title."),
        (
            "core/site-tagline",
            "Site Tagline",
            "theme",
            "Your site description.",
        ),
        (
            "core/navigation",
            "Navigation",
            "theme",
            "A collection of blocks that allow visitors to get around your site.",
        ),
        (
            "core/template-part",
            "Template Part",
            "theme",
            "Edit the different global regions of your site.",
        ),
    ];

    core.iter()
        .map(|(name, title, category, description)| {
            json!({
                "api_version": 3,
                "name": name,
                "namespace": "core",
                "title": title,
                "description": description,
                "category": category,
                "icon": null,
                "keywords": [],
                "parent": null,
                "ancestor": null,
                "allowed_blocks": null,
                "textdomain": "default",
                "styles": [],
                "variations": [],
                "selectors": {},
                "supports": {},
                "example": null,
                "editor_script_handles": [],
                "script_handles": [],
                "view_script_handles": [],
                "editor_style_handles": [],
                "style_handles": [],
                "view_script_module_ids": [],
                "is_dynamic": false,
                "editor_scripts": [],
                "scripts": [],
                "view_scripts": [],
                "editor_styles": [],
                "block_hooks": {},
                "attributes": {},
                "provides_context": {},
                "uses_context": [],
                "_links": {
                    "collection": [{"href": "/wp-json/wp/v2/block-types"}],
                    "self": [{"href": format!("/wp-json/wp/v2/block-types/{}", name)}]
                }
            })
        })
        .collect()
}

/// GET /wp-json/wp/v2/block-types
async fn list_block_types(State(_state): State<ApiState>) -> Result<Json<Vec<Value>>, WpError> {
    Ok(Json(core_blocks()))
}

/// GET /wp-json/wp/v2/block-types/{namespace}/{name}
async fn get_block_type(
    State(_state): State<ApiState>,
    Path((namespace, name)): Path<(String, String)>,
) -> Result<Json<Value>, WpError> {
    let full_name = format!("{}/{}", namespace, name);
    let blocks = core_blocks();
    blocks
        .into_iter()
        .find(|b| b.get("name").and_then(|v| v.as_str()) == Some(&full_name))
        .map(Json)
        .ok_or_else(|| {
            WpError::new(
                StatusCode::NOT_FOUND,
                "rest_block_type_invalid",
                "Invalid block type.",
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_core_blocks_not_empty() {
        let blocks = core_blocks();
        assert!(!blocks.is_empty());
        assert!(blocks.iter().any(|b| b["name"] == "core/paragraph"));
        assert!(blocks.iter().any(|b| b["name"] == "core/image"));
    }
}
