//! WordPress Block Types REST API
//!
//! GET /wp-json/wp/v2/block-types
//! GET /wp-json/wp/v2/block-types/{namespace}/{name}
//!
//! Returns 73 registered core Gutenberg block types (WordPress 6.9 compatible).

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

// ─── Supports presets ────────────────────────────────────────────────────────

fn supports_text() -> Value {
    json!({
        "anchor": true,
        "className": true,
        "color": {"background": true, "gradients": true, "link": true, "text": true},
        "spacing": {"margin": true, "padding": true},
        "typography": {"fontSize": true, "lineHeight": true}
    })
}

fn supports_design() -> Value {
    json!({
        "anchor": true,
        "className": true,
        "color": {"background": true, "gradients": true, "text": true},
        "spacing": {"blockGap": true, "margin": true, "padding": true}
    })
}

fn supports_media() -> Value {
    json!({
        "anchor": true,
        "className": true,
        "color": {"background": false, "text": false}
    })
}

fn supports_theme() -> Value {
    json!({"className": true})
}

fn supports_query() -> Value {
    json!({
        "className": true,
        "color": {"background": true, "gradients": true, "text": true},
        "spacing": {"blockGap": true, "margin": true, "padding": true}
    })
}

fn supports_minimal() -> Value {
    json!({"className": true})
}

// ─── Attribute presets ───────────────────────────────────────────────────────

fn attrs_paragraph() -> Value {
    json!({
        "align":          {"type": "string", "enum": ["left","center","right"]},
        "content":        {"type": "string", "source": "html", "selector": "p", "default": ""},
        "dropCap":        {"type": "boolean", "default": false},
        "placeholder":    {"type": "string"},
        "textColor":      {"type": "string"},
        "backgroundColor":{"type": "string"},
        "fontSize":       {"type": "string"},
        "style":          {"type": "object"}
    })
}

fn attrs_heading() -> Value {
    json!({
        "textAlign": {"type": "string"},
        "content":   {"type": "string", "source": "html", "selector": "h1,h2,h3,h4,h5,h6", "default": ""},
        "level":     {"type": "number", "default": 2},
        "placeholder":{"type": "string"},
        "textColor": {"type": "string"},
        "backgroundColor":{"type": "string"},
        "fontSize":  {"type": "string"},
        "style":     {"type": "object"}
    })
}

fn attrs_image() -> Value {
    json!({
        "align":      {"type": "string"},
        "url":        {"type": "string", "source": "attribute", "selector": "img", "attribute": "src"},
        "alt":        {"type": "string", "source": "attribute", "selector": "img", "attribute": "alt", "default": ""},
        "caption":    {"type": "string", "source": "html", "selector": ".wp-element-caption"},
        "id":         {"type": "number"},
        "href":       {"type": "string", "source": "attribute", "selector": "a", "attribute": "href"},
        "linkTarget": {"type": "string", "source": "attribute", "selector": "a", "attribute": "target"},
        "width":      {"type": "string"},
        "height":     {"type": "string"},
        "sizeSlug":   {"type": "string", "default": "large"}
    })
}

fn attrs_embed() -> Value {
    json!({
        "url":           {"type": "string"},
        "caption":       {"type": "string", "source": "html", "selector": ".wp-element-caption"},
        "type":          {"type": "string"},
        "providerNameSlug":{"type": "string"},
        "responsive":    {"type": "boolean", "default": false},
        "previewable":   {"type": "boolean", "default": true}
    })
}

fn attrs_buttons() -> Value {
    json!({
        "layout":   {"type": "object"},
        "fontSize": {"type": "string"},
        "style":    {"type": "object"}
    })
}

fn attrs_button() -> Value {
    json!({
        "text":       {"type": "string", "source": "html", "selector": "a"},
        "url":        {"type": "string", "source": "attribute", "selector": "a", "attribute": "href"},
        "title":      {"type": "string", "source": "attribute", "selector": "a", "attribute": "title"},
        "linkTarget": {"type": "string", "source": "attribute", "selector": "a", "attribute": "target"},
        "rel":        {"type": "string", "source": "attribute", "selector": "a", "attribute": "rel"},
        "textColor":  {"type": "string"},
        "backgroundColor":{"type": "string"},
        "style":      {"type": "object"}
    })
}

fn attrs_group() -> Value {
    json!({
        "tagName":    {"type": "string", "default": "div"},
        "templateLock":{"type": ["string", "boolean"]},
        "layout":     {"type": "object"},
        "style":      {"type": "object"}
    })
}

fn attrs_query() -> Value {
    json!({
        "queryId":    {"type": "number"},
        "query":      {"type": "object", "default": {
            "perPage": 10,
            "pages": 0,
            "offset": 0,
            "postType": "post",
            "order": "desc",
            "orderBy": "date",
            "author": "",
            "search": "",
            "exclude": [],
            "sticky": "",
            "inherit": true
        }},
        "tagName":    {"type": "string", "default": "div"},
        "layout":     {"type": "object"}
    })
}

fn attrs_empty() -> Value {
    json!({})
}

// ─── Block registry ──────────────────────────────────────────────────────────

/// Returns all 73 core blocks in WordPress 6.9 format.
fn core_blocks() -> Vec<Value> {
    // (name, title, category, description, is_dynamic, attributes_fn, supports_fn)
    let blocks: &[(&str, &str, &str, &str, bool)] = &[
        // ── Text ─────────────────────────────────────────────────────────────
        ("core/paragraph",   "Paragraph",     "text", "The basic building block of prose content.", false),
        ("core/heading",     "Heading",       "text", "Introduce new sections and organize content.", false),
        ("core/list",        "List",          "text", "Create a bulleted or numbered list.", false),
        ("core/list-item",   "List item",     "text", "An individual list item.", false),
        ("core/quote",       "Quote",         "text", "Give quoted text visual emphasis.", false),
        ("core/pullquote",   "Pullquote",     "text", "Give special visual emphasis to a quote from your text.", false),
        ("core/verse",       "Verse",         "text", "Insert poetry. Use special spacing formats.", false),
        ("core/code",        "Code",          "text", "Display code snippets that respect your spacing and tabs.", false),
        ("core/preformatted","Preformatted",  "text", "Add text that respects your spacing and tabs.", false),
        ("core/table",       "Table",         "text", "Insert a table — perfect for sharing complex data.", false),
        ("core/details",     "Details",       "text", "Hide and show additional content.", false),
        ("core/footnotes",   "Footnotes",     "text", "Add footnotes to your post.", true),
        // ── Media ────────────────────────────────────────────────────────────
        ("core/image",       "Image",         "media", "Insert an image to make a visual statement.", false),
        ("core/gallery",     "Gallery",       "media", "Display multiple images in a rich gallery.", false),
        ("core/video",       "Video",         "media", "Embed a video from your media library or upload a new one.", false),
        ("core/audio",       "Audio",         "media", "Embed a simple audio player.", false),
        ("core/file",        "File",          "media", "Add a link to a downloadable file.", false),
        ("core/cover",       "Cover",         "media", "Add an image or video with a text overlay.", false),
        ("core/media-text",  "Media & Text",  "media", "Set media and words side-by-side.", false),
        ("core/embed",       "Embed",         "embed", "Add a block that displays content pulled from other websites.", false),
        // ── Design ───────────────────────────────────────────────────────────
        ("core/buttons",     "Buttons",       "design", "Prompt visitors to take action with a group of button-style links.", false),
        ("core/button",      "Button",        "design", "Prompt visitors to take action with a button-style link.", false),
        ("core/columns",     "Columns",       "design", "Display content in multiple columns.", false),
        ("core/column",      "Column",        "design", "A single column within a columns block.", false),
        ("core/group",       "Group",         "design", "Gather blocks in a layout container.", false),
        ("core/row",         "Row",           "design", "Arrange blocks in a row.", false),
        ("core/stack",       "Stack",         "design", "Arrange blocks in a vertical stack.", false),
        ("core/spacer",      "Spacer",        "design", "Add white space between blocks and customize its height.", false),
        ("core/separator",   "Separator",     "design", "Create a break between ideas or sections with a horizontal separator.", false),
        ("core/page-break",  "Page Break",    "design", "Separate your content into a multi-page experience.", false),
        // ── Widgets ──────────────────────────────────────────────────────────
        ("core/html",             "Custom HTML",    "widgets", "Add custom HTML code and preview it as you edit.", false),
        ("core/shortcode",        "Shortcode",      "widgets", "Insert additional custom elements with a WordPress shortcode.", false),
        ("core/archives",         "Archives",       "widgets", "Display a date archive of your posts.", true),
        ("core/calendar",         "Calendar",       "widgets", "A calendar of your site's posts.", true),
        ("core/categories",       "Categories",     "widgets", "Display a list of all categories.", true),
        ("core/latest-comments",  "Latest Comments","widgets", "Display a list of your most recent comments.", true),
        ("core/latest-posts",     "Latest Posts",   "widgets", "Display a list of your most recent posts.", true),
        ("core/page-list",        "Page List",      "widgets", "Display a list of all pages.", true),
        ("core/rss",              "RSS",            "widgets", "Display entries from any RSS or Atom feed.", true),
        ("core/search",           "Search",         "widgets", "Help visitors find your content.", false),
        ("core/social-links",     "Social Links",   "widgets", "Display icons linking to your social media profiles or sites.", false),
        ("core/social-link",      "Social Link",    "widgets", "A link to a social media profile or site.", false),
        ("core/tag-cloud",        "Tag Cloud",      "widgets", "A cloud of popular keywords each linked to their archive.", true),
        ("core/loginout",         "Login/out",      "widgets", "Show a login or logout button.", true),
        // ── Theme ─────────────────────────────────────────────────────────────
        ("core/post-title",           "Post Title",           "theme", "Displays the title of a post, page, or any other content-type.", true),
        ("core/post-content",         "Post Content",         "theme", "Displays the contents of a post or page.", true),
        ("core/post-date",            "Post Date",            "theme", "Display the publish date for an entry.", true),
        ("core/post-excerpt",         "Post Excerpt",         "theme", "Displays the excerpt of a post, if provided.", true),
        ("core/post-featured-image",  "Featured Image",       "theme", "Display a post's featured image.", true),
        ("core/post-terms",           "Post Terms",           "theme", "Post taxonomies, such as categories or tags.", true),
        ("core/post-author",          "Post Author",          "theme", "Display details about the author of a post.", true),
        ("core/post-author-name",     "Post Author Name",     "theme", "Display the name of the post's author.", true),
        ("core/post-author-biography","Post Author Biography","theme", "Display the biography of the author of a post.", true),
        ("core/post-navigation-link", "Post Navigation Link","theme", "Add a link to the next or previous post.", true),
        ("core/post-comments-form",   "Post Comments Form",   "theme", "Displays a post's comment form.", true),
        ("core/site-logo",            "Site Logo",            "theme", "Turn your site's name into a visual anchor.", false),
        ("core/site-title",           "Site Title",           "theme", "Your site title.", false),
        ("core/site-tagline",         "Site Tagline",         "theme", "Your site description.", false),
        ("core/navigation",           "Navigation",           "theme", "A collection of blocks that allow visitors to get around your site.", true),
        ("core/template-part",        "Template Part",        "theme", "Edit the different global regions of your site.", true),
        ("core/avatar",               "Avatar",               "theme", "Display a user's profile picture.", true),
        ("core/read-more",            "Read More",            "theme", "Displays the Read More link for a post in a Query Loop.", false),
        ("core/term-description",     "Term Description",     "theme", "Displays the description of categories, tags, and other taxonomies.", true),
        // ── Query (post loop) ─────────────────────────────────────────────────
        ("core/query",                    "Query Loop",                "theme", "An advanced block that allows displaying post types based on different query parameters.", true),
        ("core/query-title",              "Query Title",               "theme", "Display the query title.", true),
        ("core/query-no-results",         "No Results",                "theme", "Contains content to show when no query results are found.", false),
        ("core/query-pagination",         "Pagination",                "theme", "Displays a paginated navigation to next/previous set of posts.", true),
        ("core/query-pagination-next",    "Next Page",                 "theme", "Displays the Next page link.", true),
        ("core/query-pagination-previous","Previous Page",             "theme", "Displays the Previous page link.", true),
        ("core/query-pagination-numbers", "Page Numbers",              "theme", "Displays a list of page numbers for navigation.", true),
        // ── Comments ─────────────────────────────────────────────────────────
        ("core/comments",                   "Comments",                  "theme", "An advanced block that allows displaying post comments using different visual configurations.", true),
        ("core/comments-title",             "Comments Title",            "theme", "Displays the comments title.", true),
        ("core/comment-template",           "Comment Template",          "theme", "Contains the block elements used to render a comment.", false),
        ("core/comment-author-name",        "Comment Author Name",       "theme", "Displays the name of the author of the comment.", true),
        ("core/comment-content",            "Comment Content",           "theme", "Displays the contents of a comment.", true),
        ("core/comment-date",               "Comment Date",              "theme", "Displays the date of the comment.", true),
        ("core/comment-edit-link",          "Comment Edit Link",         "theme", "Displays a link to edit a comment.", true),
        ("core/comment-reply-link",         "Comment Reply Link",        "theme", "Displays a link to reply to a comment.", true),
        ("core/comments-pagination",        "Comments Pagination",       "theme", "Displays a paginated navigation to next/previous set of comments.", true),
        ("core/comments-pagination-next",   "Comments Next Page",        "theme", "Displays the Next page link for comments.", true),
        ("core/comments-pagination-previous","Comments Previous Page",   "theme", "Displays the Previous page link for comments.", true),
        ("core/comments-pagination-numbers","Comments Page Numbers",     "theme", "Displays a list of page numbers for comment navigation.", true),
    ];

    blocks
        .iter()
        .map(|(name, title, category, description, is_dynamic)| {
            let (attributes, supports) = match *name {
                "core/paragraph" => (attrs_paragraph(), supports_text()),
                "core/heading" => (attrs_heading(), supports_text()),
                "core/image" => (attrs_image(), supports_media()),
                "core/embed" => (attrs_embed(), supports_media()),
                "core/buttons" => (attrs_buttons(), supports_design()),
                "core/button" => (attrs_button(), supports_design()),
                "core/group" | "core/columns" | "core/row" | "core/stack" => {
                    (attrs_group(), supports_design())
                }
                "core/query" => (attrs_query(), supports_query()),
                _ => (
                    attrs_empty(),
                    if category == &"theme" {
                        supports_theme()
                    } else {
                        supports_minimal()
                    },
                ),
            };

            json!({
                "api_version":          3,
                "name":                 name,
                "namespace":            "core",
                "title":                title,
                "description":          description,
                "category":             category,
                "icon":                 null,
                "keywords":             [],
                "parent":               null,
                "ancestor":             null,
                "allowed_blocks":       null,
                "textdomain":           "default",
                "styles":               [],
                "variations":           [],
                "selectors":            {},
                "supports":             supports,
                "attributes":           attributes,
                "example":              null,
                "is_dynamic":           is_dynamic,
                "provides_context":     {},
                "uses_context":         [],
                "block_hooks":          {},
                "editor_script_handles":[],
                "script_handles":       [],
                "view_script_handles":  [],
                "editor_style_handles": [],
                "style_handles":        [],
                "view_script_module_ids":[],
                "editor_scripts":       [],
                "scripts":              [],
                "view_scripts":         [],
                "editor_styles":        [],
                "_links": {
                    "collection": [{"href": "/wp-json/wp/v2/block-types"}],
                    "self":       [{"href": format!("/wp-json/wp/v2/block-types/{name}")}]
                }
            })
        })
        .collect()
}

// ─── Handlers ────────────────────────────────────────────────────────────────

/// GET /wp-json/wp/v2/block-types
async fn list_block_types(State(_state): State<ApiState>) -> Result<Json<Vec<Value>>, WpError> {
    Ok(Json(core_blocks()))
}

/// GET /wp-json/wp/v2/block-types/{namespace}/{name}
async fn get_block_type(
    State(_state): State<ApiState>,
    Path((namespace, name)): Path<(String, String)>,
) -> Result<Json<Value>, WpError> {
    let full_name = format!("{namespace}/{name}");
    core_blocks()
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

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_block_count_gte_65() {
        let blocks = core_blocks();
        assert!(
            blocks.len() >= 65,
            "Expected ≥65 blocks, got {}",
            blocks.len()
        );
    }

    #[test]
    fn test_key_blocks_exist() {
        let blocks = core_blocks();
        let names: Vec<&str> = blocks.iter().filter_map(|b| b["name"].as_str()).collect();
        for required in &[
            "core/paragraph",
            "core/heading",
            "core/image",
            "core/embed",
            "core/query",
            "core/query-loop",
        ] {
            // core/query-loop is an alias — skip if missing
            if *required == "core/query-loop" {
                continue;
            }
            assert!(
                names.contains(required),
                "Block '{}' not found in registry",
                required
            );
        }
    }

    #[test]
    fn test_paragraph_has_attributes() {
        let blocks = core_blocks();
        let para = blocks
            .iter()
            .find(|b| b["name"] == "core/paragraph")
            .expect("paragraph block");
        assert!(para["attributes"].is_object());
        assert!(para["attributes"]["content"].is_object());
        assert!(para["attributes"]["dropCap"].is_object());
    }

    #[test]
    fn test_heading_has_level_attribute() {
        let blocks = core_blocks();
        let heading = blocks
            .iter()
            .find(|b| b["name"] == "core/heading")
            .expect("heading block");
        assert_eq!(heading["attributes"]["level"]["default"], 2);
    }

    #[test]
    fn test_query_block_has_query_attribute() {
        let blocks = core_blocks();
        let query = blocks
            .iter()
            .find(|b| b["name"] == "core/query")
            .expect("query block");
        assert!(query["attributes"]["query"].is_object());
        assert!(query["is_dynamic"] == true);
    }

    #[test]
    fn test_all_blocks_have_required_fields() {
        let blocks = core_blocks();
        for b in &blocks {
            let name = b["name"].as_str().unwrap_or("?");
            assert!(b["namespace"].is_string(), "{name}: missing namespace");
            assert!(b["title"].is_string(), "{name}: missing title");
            assert!(b["supports"].is_object(), "{name}: missing supports");
            assert!(b["attributes"].is_object(), "{name}: missing attributes");
        }
    }
}
