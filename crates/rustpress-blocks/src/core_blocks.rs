use std::sync::Arc;

use crate::parser::Block;
use crate::registry::{BlockCategory, BlockRegistry, BlockType, RenderCallback};

/// Register all core WordPress block types into the given registry.
pub fn register_core_blocks(registry: &mut BlockRegistry) {
    register_text_blocks(registry);
    register_media_blocks(registry);
    register_design_blocks(registry);
    register_widget_blocks(registry);
    register_theme_blocks(registry);
    register_embed_blocks(registry);
}

// ---------------------------------------------------------------------------
// Text blocks
// ---------------------------------------------------------------------------

fn register_text_blocks(registry: &mut BlockRegistry) {
    // All text blocks are static: they render inner_html as-is.
    let static_text_blocks = [
        ("core/paragraph", "Paragraph", "editor-paragraph"),
        ("core/heading", "Heading", "heading"),
        ("core/list", "List", "editor-ul"),
        ("core/quote", "Quote", "format-quote"),
        ("core/code", "Code", "code"),
        ("core/preformatted", "Preformatted", "editor-code"),
        ("core/pullquote", "Pullquote", "format-quote"),
        ("core/verse", "Verse", "edit"),
        ("core/freeform", "Classic", "editor-kitchensink"),
    ];

    for (name, title, icon) in static_text_blocks {
        registry.register_block_type(
            BlockType::new_static(name, title, BlockCategory::Text, icon)
                .with_support("anchor", true),
        );
    }
}

// ---------------------------------------------------------------------------
// Media blocks
// ---------------------------------------------------------------------------

fn register_media_blocks(registry: &mut BlockRegistry) {
    // Static media blocks: inner_html is used as-is
    let static_media_blocks = [
        ("core/image", "Image", "format-image"),
        ("core/gallery", "Gallery", "format-gallery"),
        ("core/audio", "Audio", "format-audio"),
        ("core/video", "Video", "format-video"),
        ("core/file", "File", "media-default"),
    ];

    for (name, title, icon) in static_media_blocks {
        registry.register_block_type(
            BlockType::new_static(name, title, BlockCategory::Media, icon)
                .with_support("anchor", true),
        );
    }

    // core/cover - dynamic because it needs to assemble overlay + inner content
    registry.register_block_type(BlockType::new_dynamic(
        "core/cover",
        "Cover",
        BlockCategory::Media,
        "cover-image",
        render_cover(),
    ));

    // core/media-text - dynamic because it combines media and text sides
    registry.register_block_type(BlockType::new_dynamic(
        "core/media-text",
        "Media & Text",
        BlockCategory::Media,
        "align-pull-left",
        render_media_text(),
    ));
}

// ---------------------------------------------------------------------------
// Design blocks
// ---------------------------------------------------------------------------

fn register_design_blocks(registry: &mut BlockRegistry) {
    // Static design blocks
    registry.register_block_type(BlockType::new_static(
        "core/button",
        "Button",
        BlockCategory::Design,
        "button",
    ));

    // Dynamic/container design blocks
    registry.register_block_type(BlockType::new_dynamic(
        "core/columns",
        "Columns",
        BlockCategory::Design,
        "columns",
        render_columns(),
    ));

    registry.register_block_type(BlockType::new_dynamic(
        "core/column",
        "Column",
        BlockCategory::Design,
        "column",
        render_column(),
    ));

    registry.register_block_type(BlockType::new_dynamic(
        "core/group",
        "Group",
        BlockCategory::Design,
        "layout",
        render_group(),
    ));

    registry.register_block_type(BlockType::new_dynamic(
        "core/row",
        "Row",
        BlockCategory::Design,
        "layout",
        render_row(),
    ));

    registry.register_block_type(BlockType::new_dynamic(
        "core/stack",
        "Stack",
        BlockCategory::Design,
        "layout",
        render_stack(),
    ));

    registry.register_block_type(BlockType::new_dynamic(
        "core/spacer",
        "Spacer",
        BlockCategory::Design,
        "resize-vertical",
        render_spacer(),
    ));

    registry.register_block_type(BlockType::new_dynamic(
        "core/separator",
        "Separator",
        BlockCategory::Design,
        "minus",
        render_separator(),
    ));

    registry.register_block_type(BlockType::new_dynamic(
        "core/buttons",
        "Buttons",
        BlockCategory::Design,
        "button",
        render_buttons(),
    ));
}

// ---------------------------------------------------------------------------
// Widget blocks
// ---------------------------------------------------------------------------

fn register_widget_blocks(registry: &mut BlockRegistry) {
    // core/shortcode - static, just passes through
    registry.register_block_type(BlockType::new_static(
        "core/shortcode",
        "Shortcode",
        BlockCategory::Widgets,
        "shortcode",
    ));

    // Dynamic widget blocks
    let dynamic_widgets: Vec<(&str, &str, &str, RenderCallback)> = vec![
        ("core/archives", "Archives", "list-view", render_archives()),
        (
            "core/categories",
            "Categories",
            "category",
            render_categories(),
        ),
        (
            "core/latest-posts",
            "Latest Posts",
            "list-view",
            render_latest_posts(),
        ),
        (
            "core/latest-comments",
            "Latest Comments",
            "admin-comments",
            render_latest_comments(),
        ),
        ("core/search", "Search", "search", render_search()),
        ("core/tag-cloud", "Tag Cloud", "tag", render_tag_cloud()),
        ("core/calendar", "Calendar", "calendar", render_calendar()),
        ("core/rss", "RSS", "rss", render_rss()),
    ];

    for (name, title, icon, callback) in dynamic_widgets {
        registry.register_block_type(BlockType::new_dynamic(
            name,
            title,
            BlockCategory::Widgets,
            icon,
            callback,
        ));
    }
}

// ---------------------------------------------------------------------------
// Theme blocks
// ---------------------------------------------------------------------------

fn register_theme_blocks(registry: &mut BlockRegistry) {
    let dynamic_theme_blocks: Vec<(&str, &str, &str, RenderCallback)> = vec![
        (
            "core/site-title",
            "Site Title",
            "admin-site",
            render_site_title(),
        ),
        (
            "core/site-logo",
            "Site Logo",
            "format-image",
            render_site_logo(),
        ),
        ("core/navigation", "Navigation", "menu", render_navigation()),
        (
            "core/post-title",
            "Post Title",
            "heading",
            render_post_title(),
        ),
        (
            "core/post-content",
            "Post Content",
            "editor-code",
            render_post_content(),
        ),
        (
            "core/post-excerpt",
            "Post Excerpt",
            "editor-paragraph",
            render_post_excerpt(),
        ),
        (
            "core/post-date",
            "Post Date",
            "calendar",
            render_post_date(),
        ),
        (
            "core/post-author",
            "Post Author",
            "admin-users",
            render_post_author(),
        ),
        (
            "core/post-featured-image",
            "Post Featured Image",
            "format-image",
            render_post_featured_image(),
        ),
        ("core/post-terms", "Post Terms", "tag", render_post_terms()),
        ("core/query", "Query Loop", "loop", render_query()),
        (
            "core/query-loop",
            "Query Loop (Legacy)",
            "loop",
            render_query(),
        ),
        (
            "core/template-part",
            "Template Part",
            "layout",
            render_template_part(),
        ),
    ];

    for (name, title, icon, callback) in dynamic_theme_blocks {
        registry.register_block_type(BlockType::new_dynamic(
            name,
            title,
            BlockCategory::Theme,
            icon,
            callback,
        ));
    }
}

// ---------------------------------------------------------------------------
// Embed blocks
// ---------------------------------------------------------------------------

fn register_embed_blocks(registry: &mut BlockRegistry) {
    registry.register_block_type(BlockType::new_dynamic(
        "core/embed",
        "Embed",
        BlockCategory::Embed,
        "embed-generic",
        render_embed(),
    ));
}

// ===========================================================================
// Render callbacks for dynamic blocks
// ===========================================================================

// --- Design block renderers ---

fn render_columns() -> RenderCallback {
    Arc::new(|block: &Block| {
        let class = build_class("wp-block-columns", block);
        let style = build_style(block);
        let inner = render_inner_blocks_simple(&block.inner_blocks);
        format!(
            "<div class=\"{}\"{}>{}</div>",
            class,
            style_attr(&style),
            inner
        )
    })
}

fn render_column() -> RenderCallback {
    Arc::new(|block: &Block| {
        let mut class = "wp-block-column".to_string();
        if let Some(cn) = block.attrs.get("className").and_then(|v| v.as_str()) {
            class.push(' ');
            class.push_str(cn);
        }
        let mut style_parts = Vec::new();
        if let Some(width) = block.attrs.get("width").and_then(|v| v.as_str()) {
            style_parts.push(format!("flex-basis:{width}"));
        }
        let style = build_style(block);
        let combined_style = if style_parts.is_empty() {
            style
        } else if style.is_empty() {
            style_parts.join(";")
        } else {
            format!("{};{}", style_parts.join(";"), style)
        };
        let inner = render_inner_blocks_simple(&block.inner_blocks);
        format!(
            "<div class=\"{}\"{}>{}</div>",
            class,
            style_attr(&combined_style),
            inner
        )
    })
}

fn render_group() -> RenderCallback {
    Arc::new(|block: &Block| {
        let class = build_class("wp-block-group", block);
        let style = build_style(block);
        let tag_name = block
            .attrs
            .get("tagName")
            .and_then(|v| v.as_str())
            .unwrap_or("div");
        let inner = if block.inner_blocks.is_empty() {
            block.inner_html.clone()
        } else {
            render_inner_blocks_simple(&block.inner_blocks)
        };
        format!(
            "<{} class=\"{}\"{}>{}</{}>",
            tag_name,
            class,
            style_attr(&style),
            inner,
            tag_name
        )
    })
}

fn render_row() -> RenderCallback {
    Arc::new(|block: &Block| {
        let class = build_class("wp-block-group is-layout-flex", block);
        let style = build_style(block);
        let inner = render_inner_blocks_simple(&block.inner_blocks);
        format!(
            "<div class=\"{}\"{}>{}</div>",
            class,
            style_attr(&style),
            inner
        )
    })
}

fn render_stack() -> RenderCallback {
    Arc::new(|block: &Block| {
        let class = build_class("wp-block-group is-layout-flow", block);
        let style = build_style(block);
        let inner = render_inner_blocks_simple(&block.inner_blocks);
        format!(
            "<div class=\"{}\"{}>{}</div>",
            class,
            style_attr(&style),
            inner
        )
    })
}

fn render_spacer() -> RenderCallback {
    Arc::new(|block: &Block| {
        let height = block
            .attrs
            .get("height")
            .and_then(|v| v.as_str())
            .or_else(|| {
                block
                    .attrs
                    .get("height")
                    .and_then(|v| v.as_u64())
                    .map(|_| "100px")
            })
            .unwrap_or("100px");
        format!(
            "<div style=\"height:{height}\" aria-hidden=\"true\" class=\"wp-block-spacer\"></div>"
        )
    })
}

fn render_separator() -> RenderCallback {
    Arc::new(|block: &Block| {
        let class = build_class("wp-block-separator", block);
        format!("<hr class=\"{class}\" />")
    })
}

fn render_buttons() -> RenderCallback {
    Arc::new(|block: &Block| {
        let class = build_class("wp-block-buttons", block);
        let inner = render_inner_blocks_simple(&block.inner_blocks);
        format!("<div class=\"{class}\">{inner}</div>")
    })
}

// --- Media block renderers ---

fn render_cover() -> RenderCallback {
    Arc::new(|block: &Block| {
        let class = build_class("wp-block-cover", block);
        let url = block
            .attrs
            .get("url")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let dim_ratio = block
            .attrs
            .get("dimRatio")
            .and_then(|v| v.as_u64())
            .unwrap_or(50);
        let inner = if block.inner_blocks.is_empty() {
            block.inner_html.clone()
        } else {
            render_inner_blocks_simple(&block.inner_blocks)
        };

        let mut html = format!("<div class=\"{class}\">");
        if !url.is_empty() {
            html.push_str(&format!(
                "<span aria-hidden=\"true\" class=\"wp-block-cover__background has-background-dim-{}\" style=\"opacity:{}\"></span>",
                dim_ratio,
                dim_ratio as f64 / 100.0
            ));
            html.push_str(&format!(
                "<img class=\"wp-block-cover__image-background\" alt=\"\" src=\"{url}\" />"
            ));
        }
        html.push_str(&format!(
            "<div class=\"wp-block-cover__inner-container\">{inner}</div>"
        ));
        html.push_str("</div>");
        html
    })
}

fn render_media_text() -> RenderCallback {
    Arc::new(|block: &Block| {
        let class = build_class("wp-block-media-text", block);
        let media_url = block
            .attrs
            .get("mediaUrl")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let media_type = block
            .attrs
            .get("mediaType")
            .and_then(|v| v.as_str())
            .unwrap_or("image");
        let inner = if block.inner_blocks.is_empty() {
            block.inner_html.clone()
        } else {
            render_inner_blocks_simple(&block.inner_blocks)
        };

        let media_html = match media_type {
            "video" => format!("<video controls src=\"{media_url}\"></video>"),
            _ => format!(
                "<img src=\"{media_url}\" alt=\"\" class=\"wp-image-media-text\" />"
            ),
        };

        format!(
            "<div class=\"{class}\"><figure class=\"wp-block-media-text__media\">{media_html}</figure><div class=\"wp-block-media-text__content\">{inner}</div></div>"
        )
    })
}

// --- Widget block renderers ---

fn render_archives() -> RenderCallback {
    Arc::new(|block: &Block| {
        let display_as_dropdown = block
            .attrs
            .get("displayAsDropdown")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let class = build_class("wp-block-archives", block);

        if display_as_dropdown {
            format!(
                "<div class=\"{class}\"><label class=\"screen-reader-text\" for=\"wp-block-archives\">Archives</label><select id=\"wp-block-archives\" name=\"archive-dropdown\"><option value=\"\">Select Month</option></select></div>"
            )
        } else {
            format!(
                "<ul class=\"{class}\">\n<li>Archives will be populated dynamically</li>\n</ul>"
            )
        }
    })
}

fn render_categories() -> RenderCallback {
    Arc::new(|block: &Block| {
        let display_as_dropdown = block
            .attrs
            .get("displayAsDropdown")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let show_hierarchy = block
            .attrs
            .get("showHierarchy")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let class = build_class("wp-block-categories", block);

        let hier_class = if show_hierarchy {
            format!("{class} wp-block-categories--hierarchy")
        } else {
            class
        };

        if display_as_dropdown {
            format!(
                "<div class=\"{hier_class}\"><label class=\"screen-reader-text\" for=\"wp-block-categories\">Categories</label><select id=\"wp-block-categories\" name=\"category-dropdown\"><option value=\"\">Select Category</option></select></div>"
            )
        } else {
            format!(
                "<ul class=\"{hier_class}\">\n<li>Categories will be populated dynamically</li>\n</ul>"
            )
        }
    })
}

fn render_latest_posts() -> RenderCallback {
    Arc::new(|block: &Block| {
        let posts_to_show = block
            .attrs
            .get("postsToShow")
            .and_then(|v| v.as_u64())
            .unwrap_or(5);
        let display_post_date = block
            .attrs
            .get("displayPostDate")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let class = build_class("wp-block-latest-posts", block);

        let mut items = String::new();
        for i in 1..=posts_to_show {
            if display_post_date {
                items.push_str(&format!(
                    "<li><a href=\"#\">Latest Post {i}</a><time>January 1, 2025</time></li>\n"
                ));
            } else {
                items.push_str(&format!("<li><a href=\"#\">Latest Post {i}</a></li>\n"));
            }
        }
        format!("<ul class=\"{class}\">\n{items}</ul>")
    })
}

fn render_latest_comments() -> RenderCallback {
    Arc::new(|block: &Block| {
        let comments_to_show = block
            .attrs
            .get("commentsToShow")
            .and_then(|v| v.as_u64())
            .unwrap_or(5);
        let class = build_class("wp-block-latest-comments", block);

        let mut items = String::new();
        for i in 1..=comments_to_show {
            items.push_str(&format!(
                "<li class=\"wp-block-latest-comments__comment\"><article><footer class=\"wp-block-latest-comments__comment-meta\">Commenter on <a href=\"#\">Post {i}</a></footer><div class=\"wp-block-latest-comments__comment-excerpt\"><p>Comment placeholder...</p></div></article></li>\n"
            ));
        }
        format!(
            "<ol class=\"has-dates has-excerpts {class}\">\n{items}</ol>"
        )
    })
}

fn render_search() -> RenderCallback {
    Arc::new(|block: &Block| {
        let label = block
            .attrs
            .get("label")
            .and_then(|v| v.as_str())
            .unwrap_or("Search");
        let placeholder = block
            .attrs
            .get("placeholder")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let button_text = block
            .attrs
            .get("buttonText")
            .and_then(|v| v.as_str())
            .unwrap_or("Search");
        let class = build_class("wp-block-search", block);

        format!(
            "<form role=\"search\" method=\"get\" action=\"/\" class=\"{class}\"><label class=\"wp-block-search__label\" for=\"wp-block-search__input\">{label}</label><div class=\"wp-block-search__inside-wrapper\"><input type=\"search\" id=\"wp-block-search__input\" class=\"wp-block-search__input\" name=\"s\" value=\"\" placeholder=\"{placeholder}\" required /><button type=\"submit\" class=\"wp-block-search__button\">{button_text}</button></div></form>"
        )
    })
}

fn render_tag_cloud() -> RenderCallback {
    Arc::new(|block: &Block| {
        let show_tag_counts = block
            .attrs
            .get("showTagCounts")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let class = build_class("wp-block-tag-cloud", block);
        let count_suffix = if show_tag_counts { " (1)" } else { "" };

        format!(
            "<p class=\"{class}\"><a href=\"#\" class=\"tag-cloud-link\">Tag{count_suffix}</a></p>"
        )
    })
}

fn render_calendar() -> RenderCallback {
    Arc::new(|_block: &Block| {
        "<div class=\"wp-block-calendar\"><table class=\"wp-calendar\"><caption>Calendar</caption><thead><tr><th>M</th><th>T</th><th>W</th><th>T</th><th>F</th><th>S</th><th>S</th></tr></thead><tbody><tr><td colspan=\"7\">Calendar will be populated dynamically</td></tr></tbody></table></div>".to_string()
    })
}

fn render_rss() -> RenderCallback {
    Arc::new(|block: &Block| {
        let feed_url = block
            .attrs
            .get("feedURL")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let class = build_class("wp-block-rss", block);

        format!(
            "<ul class=\"{class}\">\n<li>RSS feed items from {feed_url} will be populated dynamically</li>\n</ul>"
        )
    })
}

// --- Theme block renderers ---

fn render_site_title() -> RenderCallback {
    Arc::new(|block: &Block| {
        let level = block
            .attrs
            .get("level")
            .and_then(|v| v.as_u64())
            .unwrap_or(1);
        let tag = if (1..=6).contains(&level) {
            format!("h{level}")
        } else {
            "p".to_string()
        };
        let class = build_class("wp-block-site-title", block);

        format!(
            "<{tag} class=\"{class}\"><a href=\"/\">Site Title</a></{tag}>"
        )
    })
}

fn render_site_logo() -> RenderCallback {
    Arc::new(|block: &Block| {
        let width = block
            .attrs
            .get("width")
            .and_then(|v| v.as_u64())
            .unwrap_or(120);
        let class = build_class("wp-block-site-logo", block);

        format!(
            "<div class=\"{class}\"><a href=\"/\" rel=\"home\"><img width=\"{width}\" height=\"{width}\" src=\"\" class=\"custom-logo\" alt=\"Site Logo\" /></a></div>"
        )
    })
}

fn render_navigation() -> RenderCallback {
    Arc::new(|block: &Block| {
        let class = build_class("wp-block-navigation", block);
        let inner = if block.inner_blocks.is_empty() {
            if block.inner_html.is_empty() {
                "<ul class=\"wp-block-navigation__container\"><li class=\"wp-block-navigation-item\"><a href=\"/\">Home</a></li></ul>".to_string()
            } else {
                block.inner_html.clone()
            }
        } else {
            render_inner_blocks_simple(&block.inner_blocks)
        };
        format!("<nav class=\"{class}\">{inner}</nav>")
    })
}

fn render_post_title() -> RenderCallback {
    Arc::new(|block: &Block| {
        let level = block
            .attrs
            .get("level")
            .and_then(|v| v.as_u64())
            .unwrap_or(2);
        let tag = format!("h{}", level.clamp(1, 6));
        let class = build_class("wp-block-post-title", block);
        let is_link = block
            .attrs
            .get("isLink")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if is_link {
            format!(
                "<{tag} class=\"{class}\"><a href=\"#\">Post Title</a></{tag}>"
            )
        } else {
            format!("<{tag} class=\"{class}\">Post Title</{tag}>")
        }
    })
}

fn render_post_content() -> RenderCallback {
    Arc::new(|block: &Block| {
        let class = build_class("wp-block-post-content", block);
        let inner = if block.inner_blocks.is_empty() {
            block.inner_html.clone()
        } else {
            render_inner_blocks_simple(&block.inner_blocks)
        };
        format!("<div class=\"entry-content {class}\">{inner}</div>")
    })
}

fn render_post_excerpt() -> RenderCallback {
    Arc::new(|block: &Block| {
        let class = build_class("wp-block-post-excerpt", block);
        let more_text = block
            .attrs
            .get("moreText")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let mut html = format!("<div class=\"{class}\"><p class=\"wp-block-post-excerpt__excerpt\">Post excerpt placeholder...</p>");
        if !more_text.is_empty() {
            html.push_str(&format!(
                "<p class=\"wp-block-post-excerpt__more-text\"><a href=\"#\">{more_text}</a></p>"
            ));
        }
        html.push_str("</div>");
        html
    })
}

fn render_post_date() -> RenderCallback {
    Arc::new(|block: &Block| {
        let class = build_class("wp-block-post-date", block);
        let is_link = block
            .attrs
            .get("isLink")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if is_link {
            format!(
                "<div class=\"{class}\"><a href=\"#\"><time>January 1, 2025</time></a></div>"
            )
        } else {
            format!(
                "<div class=\"{class}\"><time>January 1, 2025</time></div>"
            )
        }
    })
}

fn render_post_author() -> RenderCallback {
    Arc::new(|block: &Block| {
        let class = build_class("wp-block-post-author", block);
        let show_avatar = block
            .attrs
            .get("showAvatar")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let mut html = format!("<div class=\"{class}\">");
        if show_avatar {
            html.push_str("<div class=\"wp-block-post-author__avatar\"><img alt=\"\" src=\"\" class=\"avatar avatar-96 photo\" width=\"96\" height=\"96\" /></div>");
        }
        html.push_str("<div class=\"wp-block-post-author__content\"><p class=\"wp-block-post-author__name\">Author</p></div></div>");
        html
    })
}

fn render_post_featured_image() -> RenderCallback {
    Arc::new(|block: &Block| {
        let class = build_class("wp-block-post-featured-image", block);
        let is_link = block
            .attrs
            .get("isLink")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if is_link {
            format!(
                "<figure class=\"{class}\"><a href=\"#\"><img src=\"\" alt=\"\" /></a></figure>"
            )
        } else {
            format!(
                "<figure class=\"{class}\"><img src=\"\" alt=\"\" /></figure>"
            )
        }
    })
}

fn render_post_terms() -> RenderCallback {
    Arc::new(|block: &Block| {
        let term = block
            .attrs
            .get("term")
            .and_then(|v| v.as_str())
            .unwrap_or("category");
        let class = build_class("wp-block-post-terms", block);
        let separator = block
            .attrs
            .get("separator")
            .and_then(|v| v.as_str())
            .unwrap_or(", ");

        format!(
            "<div class=\"taxonomy-{term} {class}\"><span class=\"wp-block-post-terms__separator\">{separator}</span></div>"
        )
    })
}

fn render_query() -> RenderCallback {
    Arc::new(|block: &Block| {
        let class = build_class("wp-block-query", block);
        let inner = if block.inner_blocks.is_empty() {
            block.inner_html.clone()
        } else {
            render_inner_blocks_simple(&block.inner_blocks)
        };
        format!("<div class=\"{class}\">{inner}</div>")
    })
}

fn render_template_part() -> RenderCallback {
    Arc::new(|block: &Block| {
        let slug = block
            .attrs
            .get("slug")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let tag_name = block
            .attrs
            .get("tagName")
            .and_then(|v| v.as_str())
            .unwrap_or("div");
        let class = build_class("wp-block-template-part", block);
        let inner = if block.inner_blocks.is_empty() {
            block.inner_html.clone()
        } else {
            render_inner_blocks_simple(&block.inner_blocks)
        };

        format!(
            "<{tag_name} class=\"{class} {slug}\">{inner}</{tag_name}>"
        )
    })
}

// --- Embed block renderer ---

fn render_embed() -> RenderCallback {
    Arc::new(|block: &Block| {
        let url = block
            .attrs
            .get("url")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let provider = block
            .attrs
            .get("providerNameSlug")
            .and_then(|v| v.as_str())
            .unwrap_or_else(|| detect_embed_provider(url));
        let class = build_class("wp-block-embed", block);
        let _type = block
            .attrs
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("rich");

        let responsive_class = if matches!(
            provider,
            "youtube" | "vimeo" | "dailymotion" | "tiktok" | "videopress"
        ) {
            " wp-embed-aspect-16-9 wp-has-aspect-ratio"
        } else {
            ""
        };

        let inner = if block.inner_html.is_empty() {
            if url.is_empty() {
                String::new()
            } else {
                format!("<a href=\"{url}\">{url}</a>")
            }
        } else {
            block.inner_html.clone()
        };

        format!(
            "<figure class=\"{class} is-type-{_type} is-provider-{provider}{responsive_class}\"><div class=\"wp-block-embed__wrapper\">{inner}</div></figure>"
        )
    })
}

// ===========================================================================
// Helper functions
// ===========================================================================

/// Build a CSS class string from a base class and optional className attribute.
fn build_class(base: &str, block: &Block) -> String {
    let mut class = base.to_string();
    if let Some(cn) = block.attrs.get("className").and_then(|v| v.as_str()) {
        class.push(' ');
        class.push_str(cn);
    }
    if let Some(align) = block.attrs.get("align").and_then(|v| v.as_str()) {
        class.push_str(&format!(" align{align}"));
    }
    class
}

/// Build an inline style string from block attributes.
fn build_style(block: &Block) -> String {
    let mut parts = Vec::new();

    if let Some(style) = block.attrs.get("style") {
        // Handle spacing
        if let Some(spacing) = style.get("spacing") {
            if let Some(padding) = spacing.get("padding") {
                if let Some(p) = padding.as_str() {
                    parts.push(format!("padding:{p}"));
                } else {
                    if let Some(top) = padding.get("top").and_then(|v| v.as_str()) {
                        parts.push(format!("padding-top:{top}"));
                    }
                    if let Some(right) = padding.get("right").and_then(|v| v.as_str()) {
                        parts.push(format!("padding-right:{right}"));
                    }
                    if let Some(bottom) = padding.get("bottom").and_then(|v| v.as_str()) {
                        parts.push(format!("padding-bottom:{bottom}"));
                    }
                    if let Some(left) = padding.get("left").and_then(|v| v.as_str()) {
                        parts.push(format!("padding-left:{left}"));
                    }
                }
            }
            if let Some(margin) = spacing.get("margin") {
                if let Some(m) = margin.as_str() {
                    parts.push(format!("margin:{m}"));
                } else {
                    if let Some(top) = margin.get("top").and_then(|v| v.as_str()) {
                        parts.push(format!("margin-top:{top}"));
                    }
                    if let Some(bottom) = margin.get("bottom").and_then(|v| v.as_str()) {
                        parts.push(format!("margin-bottom:{bottom}"));
                    }
                }
            }
            if let Some(gap) = spacing.get("blockGap").and_then(|v| v.as_str()) {
                parts.push(format!("gap:{gap}"));
            }
        }

        // Handle color
        if let Some(color) = style.get("color") {
            if let Some(bg) = color.get("background").and_then(|v| v.as_str()) {
                parts.push(format!("background-color:{bg}"));
            }
            if let Some(text) = color.get("text").and_then(|v| v.as_str()) {
                parts.push(format!("color:{text}"));
            }
        }

        // Handle typography
        if let Some(typography) = style.get("typography") {
            if let Some(font_size) = typography.get("fontSize").and_then(|v| v.as_str()) {
                parts.push(format!("font-size:{font_size}"));
            }
            if let Some(line_height) = typography.get("lineHeight").and_then(|v| v.as_str()) {
                parts.push(format!("line-height:{line_height}"));
            }
        }
    }

    // Handle direct backgroundColor/textColor preset references
    if let Some(bg) = block.attrs.get("backgroundColor").and_then(|v| v.as_str()) {
        parts.push(format!("background-color:var(--wp--preset--color--{bg})"));
    }

    parts.join(";")
}

/// Create a style attribute string, or empty string if no styles.
fn style_attr(style: &str) -> String {
    if style.is_empty() {
        String::new()
    } else {
        format!(" style=\"{style}\"")
    }
}

/// Detect the embed provider from a URL.
fn detect_embed_provider(url: &str) -> &'static str {
    if url.contains("youtube.com") || url.contains("youtu.be") {
        "youtube"
    } else if url.contains("vimeo.com") {
        "vimeo"
    } else if url.contains("twitter.com") || url.contains("x.com") {
        "twitter"
    } else if url.contains("facebook.com") || url.contains("fb.watch") {
        "facebook"
    } else if url.contains("instagram.com") {
        "instagram"
    } else if url.contains("tiktok.com") {
        "tiktok"
    } else if url.contains("spotify.com") {
        "spotify"
    } else if url.contains("soundcloud.com") {
        "soundcloud"
    } else if url.contains("flickr.com") {
        "flickr"
    } else if url.contains("dailymotion.com") {
        "dailymotion"
    } else if url.contains("reddit.com") {
        "reddit"
    } else if url.contains("tumblr.com") {
        "tumblr"
    } else if url.contains("wordpress.com") || url.contains("wp.com") {
        "wordpress"
    } else {
        "generic"
    }
}

/// Simple recursive rendering for inner blocks (used inside render callbacks
/// that don't have access to the full BlockRenderer).
fn render_inner_blocks_simple(blocks: &[Block]) -> String {
    let mut output = String::new();
    for block in blocks {
        if block.name == "core/freeform" {
            output.push_str(&block.inner_html);
        } else if !block.inner_blocks.is_empty() {
            // Recursively render container blocks with simple wrappers
            output.push_str(&render_inner_blocks_simple(&block.inner_blocks));
        } else {
            output.push_str(&block.inner_html);
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_blocks;

    #[test]
    fn test_register_core_blocks_count() {
        let mut registry = BlockRegistry::new();
        register_core_blocks(&mut registry);
        // We should have a substantial number of blocks registered
        assert!(
            registry.count() >= 40,
            "Expected at least 40 core blocks, got {}",
            registry.count()
        );
    }

    #[test]
    fn test_all_text_blocks_registered() {
        let mut registry = BlockRegistry::new();
        register_core_blocks(&mut registry);

        let text_blocks = [
            "core/paragraph",
            "core/heading",
            "core/list",
            "core/quote",
            "core/code",
            "core/preformatted",
            "core/pullquote",
            "core/verse",
            "core/freeform",
        ];
        for name in text_blocks {
            assert!(
                registry.has_block_type(name),
                "Missing text block: {name}"
            );
        }
    }

    #[test]
    fn test_all_media_blocks_registered() {
        let mut registry = BlockRegistry::new();
        register_core_blocks(&mut registry);

        let media_blocks = [
            "core/image",
            "core/gallery",
            "core/audio",
            "core/video",
            "core/cover",
            "core/file",
            "core/media-text",
        ];
        for name in media_blocks {
            assert!(
                registry.has_block_type(name),
                "Missing media block: {name}"
            );
        }
    }

    #[test]
    fn test_all_design_blocks_registered() {
        let mut registry = BlockRegistry::new();
        register_core_blocks(&mut registry);

        let design_blocks = [
            "core/columns",
            "core/column",
            "core/group",
            "core/row",
            "core/stack",
            "core/spacer",
            "core/separator",
            "core/buttons",
            "core/button",
        ];
        for name in design_blocks {
            assert!(
                registry.has_block_type(name),
                "Missing design block: {name}"
            );
        }
    }

    #[test]
    fn test_all_widget_blocks_registered() {
        let mut registry = BlockRegistry::new();
        register_core_blocks(&mut registry);

        let widget_blocks = [
            "core/shortcode",
            "core/archives",
            "core/categories",
            "core/latest-posts",
            "core/latest-comments",
            "core/search",
            "core/tag-cloud",
            "core/calendar",
            "core/rss",
        ];
        for name in widget_blocks {
            assert!(
                registry.has_block_type(name),
                "Missing widget block: {name}"
            );
        }
    }

    #[test]
    fn test_all_theme_blocks_registered() {
        let mut registry = BlockRegistry::new();
        register_core_blocks(&mut registry);

        let theme_blocks = [
            "core/site-title",
            "core/site-logo",
            "core/navigation",
            "core/post-title",
            "core/post-content",
            "core/post-excerpt",
            "core/post-date",
            "core/post-author",
            "core/post-featured-image",
            "core/post-terms",
            "core/query",
            "core/query-loop",
            "core/template-part",
        ];
        for name in theme_blocks {
            assert!(
                registry.has_block_type(name),
                "Missing theme block: {name}"
            );
        }
    }

    #[test]
    fn test_embed_block_registered() {
        let mut registry = BlockRegistry::new();
        register_core_blocks(&mut registry);
        assert!(registry.has_block_type("core/embed"));
    }

    #[test]
    fn test_spacer_render() {
        let mut registry = BlockRegistry::new();
        register_core_blocks(&mut registry);

        let blocks = parse_blocks(r#"<!-- wp:spacer {"height":"50px"} /-->"#);
        let bt = registry.get_block_type("core/spacer").unwrap();
        let callback = bt.render_callback.as_ref().unwrap();
        let html = callback(&blocks[0]);
        assert!(html.contains("50px"));
        assert!(html.contains("wp-block-spacer"));
    }

    #[test]
    fn test_separator_render() {
        let mut registry = BlockRegistry::new();
        register_core_blocks(&mut registry);

        let block = Block::new("core/separator");
        let bt = registry.get_block_type("core/separator").unwrap();
        let callback = bt.render_callback.as_ref().unwrap();
        let html = callback(&block);
        assert!(html.contains("<hr"));
        assert!(html.contains("wp-block-separator"));
    }

    #[test]
    fn test_embed_provider_detection() {
        assert_eq!(
            detect_embed_provider("https://www.youtube.com/watch?v=abc"),
            "youtube"
        );
        assert_eq!(detect_embed_provider("https://vimeo.com/123"), "vimeo");
        assert_eq!(
            detect_embed_provider("https://twitter.com/user/status/1"),
            "twitter"
        );
        assert_eq!(
            detect_embed_provider("https://x.com/user/status/1"),
            "twitter"
        );
        assert_eq!(detect_embed_provider("https://example.com/page"), "generic");
    }

    #[test]
    fn test_search_block_render() {
        let mut registry = BlockRegistry::new();
        register_core_blocks(&mut registry);

        let mut block = Block::new("core/search");
        block.attrs = serde_json::json!({"label": "Find", "buttonText": "Go"});
        let bt = registry.get_block_type("core/search").unwrap();
        let callback = bt.render_callback.as_ref().unwrap();
        let html = callback(&block);
        assert!(html.contains("Find"));
        assert!(html.contains("Go"));
        assert!(html.contains("wp-block-search"));
    }
}
