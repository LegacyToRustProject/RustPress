use chrono::NaiveDateTime;
use serde::Serialize;
use tera::Context;

use rustpress_db::entities::wp_posts;

/// Template tag helpers for rendering WordPress-like template data.
/// These functions populate Tera contexts with post/page data.

/// Format a NaiveDateTime to WordPress's default "F j, Y" format (e.g., "January 1, 2024").
fn format_date_human(dt: NaiveDateTime) -> String {
    let month = match dt.format("%m").to_string().as_str() {
        "01" => "January",
        "02" => "February",
        "03" => "March",
        "04" => "April",
        "05" => "May",
        "06" => "June",
        "07" => "July",
        "08" => "August",
        "09" => "September",
        "10" => "October",
        "11" => "November",
        "12" => "December",
        _ => "January",
    };
    let day = dt.format("%-d").to_string();
    let year = dt.format("%Y").to_string();
    format!("{} {}, {}", month, day, year)
}

/// Format a NaiveDateTime to ISO 8601 for datetime attributes.
fn format_date_iso(dt: NaiveDateTime) -> String {
    dt.format("%Y-%m-%dT%H:%M:%S+00:00").to_string()
}

/// Post data prepared for template rendering.
#[derive(Debug, Clone, Serialize)]
pub struct PostTemplateData {
    pub id: u64,
    pub title: String,
    pub content: String,
    pub excerpt: String,
    pub date: String,
    /// Human-readable date like "January 1, 2024" (WordPress "F j, Y" format)
    pub date_formatted: String,
    /// ISO 8601 date for datetime attribute: "2024-01-01T00:00:00+00:00"
    pub date_iso: String,
    pub modified: String,
    pub author_id: u64,
    pub slug: String,
    pub status: String,
    pub post_type: String,
    pub permalink: String,
    pub comment_count: i64,
    pub comment_status: String,
    pub sticky: bool,
    pub password_required: bool,
}

impl PostTemplateData {
    /// Convert a wp_posts model to template-friendly data.
    pub fn from_model(post: &wp_posts::Model, site_url: &str) -> Self {
        let permalink = format!(
            "{}/{}",
            site_url.trim_end_matches('/'),
            &post.post_name
        );

        Self {
            id: post.id,
            title: post.post_title.clone(),
            content: post.post_content.clone(),
            excerpt: if post.post_excerpt.is_empty() {
                generate_excerpt(&post.post_content, 55)
            } else {
                post.post_excerpt.clone()
            },
            date: post.post_date.format("%Y-%m-%d %H:%M:%S").to_string(),
            date_formatted: format_date_human(post.post_date),
            date_iso: format_date_iso(post.post_date),
            modified: post.post_modified.format("%Y-%m-%d %H:%M:%S").to_string(),
            author_id: post.post_author,
            slug: post.post_name.clone(),
            status: post.post_status.clone(),
            post_type: post.post_type.clone(),
            permalink,
            comment_count: post.comment_count,
            comment_status: post.comment_status.clone(),
            sticky: false,
            password_required: !post.post_password.is_empty(),
        }
    }

    /// Convert a wp_posts model with permalink generated from RewriteRules.
    pub fn from_model_with_rewrite(
        post: &wp_posts::Model,
        site_url: &str,
        rewrite: &rustpress_core::rewrite::RewriteRules,
    ) -> Self {
        let path = rewrite.build_permalink(&post.post_name, post.id, post.post_date);
        let permalink = format!("{}{}", site_url.trim_end_matches('/'), path);

        Self {
            id: post.id,
            title: post.post_title.clone(),
            content: post.post_content.clone(),
            excerpt: if post.post_excerpt.is_empty() {
                generate_excerpt(&post.post_content, 55)
            } else {
                post.post_excerpt.clone()
            },
            date: post.post_date.format("%Y-%m-%d %H:%M:%S").to_string(),
            date_formatted: format_date_human(post.post_date),
            date_iso: format_date_iso(post.post_date),
            modified: post.post_modified.format("%Y-%m-%d %H:%M:%S").to_string(),
            author_id: post.post_author,
            slug: post.post_name.clone(),
            status: post.post_status.clone(),
            post_type: post.post_type.clone(),
            permalink,
            comment_count: post.comment_count,
            comment_status: post.comment_status.clone(),
            sticky: false,
            password_required: !post.post_password.is_empty(),
        }
    }
}

/// Pagination data for templates.
#[derive(Debug, Clone, Serialize)]
pub struct PaginationData {
    pub current_page: u64,
    pub total_pages: u64,
    pub total_posts: u64,
    pub has_previous: bool,
    pub has_next: bool,
    pub previous_page: u64,
    pub next_page: u64,
}

impl PaginationData {
    pub fn new(current_page: u64, total_pages: u64, total_posts: u64) -> Self {
        Self {
            current_page,
            total_pages,
            total_posts,
            has_previous: current_page > 1,
            has_next: current_page < total_pages,
            previous_page: current_page.saturating_sub(1).max(1),
            next_page: (current_page + 1).min(total_pages),
        }
    }
}

/// Insert single post data into a Tera context.
/// Content is run through the full WordPress content filter pipeline
/// (shortcodes, wpautop, wptexturize) before insertion.
pub fn insert_post_context(context: &mut Context, post: &PostTemplateData) {
    let processed_content = super::formatting::apply_content_filters(&post.content);
    let processed_title = super::formatting::apply_title_filters(&post.title);
    let processed_excerpt = if post.excerpt.is_empty() {
        String::new()
    } else {
        super::formatting::apply_excerpt_filters(&post.excerpt)
    };
    context.insert("post", post);
    context.insert("the_title", &processed_title);
    context.insert("the_content", &processed_content);
    context.insert("the_excerpt", &processed_excerpt);
    context.insert("the_permalink", &post.permalink);
    context.insert("the_date", &post.date);
    context.insert("the_id", &post.id);
}

/// Insert single post data with HookRegistry and ShortcodeRegistry integration.
///
/// Runs content through the full pipeline: ShortcodeRegistry, formatting, and HookRegistry filters.
pub fn insert_post_context_with_hooks(
    context: &mut Context,
    post: &PostTemplateData,
    hooks: &rustpress_core::hooks::HookRegistry,
) {
    insert_post_context_full(context, post, None, hooks);
}

/// Insert single post data with full plugin integration (shortcodes + hooks).
pub fn insert_post_context_full(
    context: &mut Context,
    post: &PostTemplateData,
    shortcodes: Option<&rustpress_core::shortcode::ShortcodeRegistry>,
    hooks: &rustpress_core::hooks::HookRegistry,
) {
    let processed_content = if let Some(sc) = shortcodes {
        super::formatting::apply_content_filters_full(&post.content, sc, hooks)
    } else {
        super::formatting::apply_content_filters_with_hooks(&post.content, hooks)
    };
    let processed_title =
        super::formatting::apply_title_filters_with_hooks(&post.title, hooks);
    let processed_excerpt = if post.excerpt.is_empty() {
        String::new()
    } else {
        super::formatting::apply_excerpt_filters_with_hooks(&post.excerpt, hooks)
    };
    context.insert("post", post);
    context.insert("the_title", &processed_title);
    context.insert("the_content", &processed_content);
    context.insert("the_excerpt", &processed_excerpt);
    context.insert("the_permalink", &post.permalink);
    context.insert("the_date", &post.date);
    context.insert("the_id", &post.id);
}

/// Process WordPress shortcodes in content.
///
/// Handles: [caption], [audio], [video], [gallery], [embed],
/// and strips unknown shortcodes gracefully.
pub fn process_shortcodes(content: &str) -> String {
    let mut result = content.to_string();

    // Process [caption] shortcodes: [caption id="x" ...]<img ...>Caption text[/caption]
    result = process_caption_shortcode(&result);

    // Process [audio] shortcodes: [audio src="url"]
    result = process_audio_shortcode(&result);

    // Process [video] shortcodes: [video src="url"]
    result = process_video_shortcode(&result);

    // Process [gallery] shortcodes: [gallery ids="1,2,3"]
    result = process_gallery_shortcode(&result);

    // Process [embed]url[/embed] shortcodes
    result = process_embed_shortcode(&result);

    // Strip remaining unknown shortcodes (preserve content between tags)
    result = strip_unknown_shortcodes(&result);

    result
}

fn extract_attr(tag: &str, attr: &str) -> Option<String> {
    let pattern = format!("{}=\"", attr);
    if let Some(start) = tag.find(&pattern) {
        let val_start = start + pattern.len();
        if let Some(end) = tag[val_start..].find('"') {
            return Some(tag[val_start..val_start + end].to_string());
        }
    }
    // Also try single quotes
    let pattern_sq = format!("{}='", attr);
    if let Some(start) = tag.find(&pattern_sq) {
        let val_start = start + pattern_sq.len();
        if let Some(end) = tag[val_start..].find('\'') {
            return Some(tag[val_start..val_start + end].to_string());
        }
    }
    None
}

fn process_caption_shortcode(content: &str) -> String {
    let mut result = content.to_string();
    while let Some(start) = result.find("[caption") {
        let tag_end = match result[start..].find(']') {
            Some(i) => start + i + 1,
            None => break,
        };
        let close = match result[tag_end..].find("[/caption]") {
            Some(i) => tag_end + i,
            None => break,
        };
        let inner = &result[tag_end..close];
        let align = extract_attr(&result[start..tag_end], "align").unwrap_or_default();
        let align_class = if align.is_empty() {
            String::new()
        } else {
            format!(" class=\"{}\"", align)
        };

        // Split inner into img tag + caption text
        let caption_html = if let Some(img_end) = inner.find("/>") {
            let img = &inner[..img_end + 2];
            let caption_text = inner[img_end + 2..].trim();
            format!(
                "<figure{}>{}<figcaption>{}</figcaption></figure>",
                align_class, img, caption_text
            )
        } else {
            format!("<figure{}>{}</figure>", align_class, inner)
        };

        result = format!(
            "{}{}{}",
            &result[..start],
            caption_html,
            &result[close + 10..]
        );
    }
    result
}

fn process_audio_shortcode(content: &str) -> String {
    let mut result = content.to_string();
    while let Some(start) = result.find("[audio") {
        let end = match result[start..].find(']') {
            Some(i) => start + i + 1,
            None => break,
        };
        let tag = &result[start..end];
        let src = extract_attr(tag, "src").unwrap_or_default();
        if src.is_empty() {
            result = format!("{}{}", &result[..start], &result[end..]);
        } else {
            let html = format!(
                r#"<audio controls preload="metadata"><source src="{}">Your browser does not support audio.</audio>"#,
                src
            );
            result = format!("{}{}{}", &result[..start], html, &result[end..]);
        }
    }
    result
}

fn process_video_shortcode(content: &str) -> String {
    let mut result = content.to_string();
    while let Some(start) = result.find("[video") {
        let end = match result[start..].find(']') {
            Some(i) => start + i + 1,
            None => break,
        };
        let tag = &result[start..end];
        let src = extract_attr(tag, "src").unwrap_or_default();
        let width = extract_attr(tag, "width").unwrap_or_else(|| "100%".to_string());
        if src.is_empty() {
            result = format!("{}{}", &result[..start], &result[end..]);
        } else {
            let html = format!(
                r#"<video controls preload="metadata" style="max-width:{};height:auto"><source src="{}">Your browser does not support video.</video>"#,
                width, src
            );
            result = format!("{}{}{}", &result[..start], html, &result[end..]);
        }
    }
    result
}

fn process_gallery_shortcode(content: &str) -> String {
    let mut result = content.to_string();
    while let Some(start) = result.find("[gallery") {
        let end = match result[start..].find(']') {
            Some(i) => start + i + 1,
            None => break,
        };
        let tag = &result[start..end];
        let ids = extract_attr(tag, "ids").unwrap_or_default();
        let columns = extract_attr(tag, "columns")
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(3);

        if ids.is_empty() {
            result = format!("{}{}", &result[..start], &result[end..]);
        } else {
            let img_tags: String = ids
                .split(',')
                .map(|id| id.trim())
                .filter(|id| !id.is_empty())
                .map(|_id| {
                    // We don't have access to the DB here, so render placeholder
                    // In real WP this would resolve the attachment URL
                    "<div class=\"gallery-item\"></div>".to_string()
                })
                .collect::<Vec<_>>()
                .join("\n");

            let html = format!(
                "<div class=\"gallery gallery-columns-{}\">{}</div>",
                columns, img_tags
            );
            result = format!("{}{}{}", &result[..start], html, &result[end..]);
        }
    }
    result
}

fn process_embed_shortcode(content: &str) -> String {
    let mut result = content.to_string();
    while let Some(start) = result.find("[embed") {
        let tag_end = match result[start..].find(']') {
            Some(i) => start + i + 1,
            None => break,
        };
        let close = match result[tag_end..].find("[/embed]") {
            Some(i) => tag_end + i,
            None => break,
        };
        let url = result[tag_end..close].trim();

        // Basic oEmbed-like handling for common providers
        let html = if url.contains("youtube.com") || url.contains("youtu.be") {
            let video_id = extract_youtube_id(url);
            if let Some(vid) = video_id {
                format!(
                    r#"<div class="wp-embed"><iframe width="560" height="315" src="https://www.youtube.com/embed/{}" frameborder="0" allowfullscreen></iframe></div>"#,
                    vid
                )
            } else {
                format!("<a href=\"{}\">{}</a>", url, url)
            }
        } else {
            format!("<a href=\"{}\">{}</a>", url, url)
        };

        result = format!("{}{}{}", &result[..start], html, &result[close + 8..]);
    }
    result
}

fn extract_youtube_id(url: &str) -> Option<String> {
    // Handle youtu.be/ID
    if let Some(pos) = url.find("youtu.be/") {
        let id = &url[pos + 9..];
        let id = id.split(&['?', '&', '#'][..]).next()?;
        return Some(id.to_string());
    }
    // Handle youtube.com/watch?v=ID
    if let Some(pos) = url.find("v=") {
        let id = &url[pos + 2..];
        let id = id.split(&['&', '#'][..]).next()?;
        return Some(id.to_string());
    }
    None
}

fn strip_unknown_shortcodes(content: &str) -> String {
    let mut result = String::with_capacity(content.len());
    let chars: Vec<char> = content.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        if chars[i] == '[' && i + 1 < len && chars[i + 1] != ' ' {
            // Check if this looks like a shortcode [tag ...] or [/tag]
            let remaining: String = chars[i..].iter().collect();
            if let Some(close) = remaining.find(']') {
                let tag_content = &remaining[1..close];
                let tag_name = tag_content
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .trim_start_matches('/');

                // Only strip if it looks like a shortcode tag name (alphanumeric + hyphens)
                if !tag_name.is_empty()
                    && tag_name
                        .chars()
                        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
                {
                    // If it's a closing tag [/name], skip it
                    if tag_content.starts_with('/') {
                        i += close + 1;
                        continue;
                    }
                    // If it's a self-closing shortcode, skip it
                    i += close + 1;
                    continue;
                }
            }
        }
        result.push(chars[i]);
        i += 1;
    }
    result
}

/// Insert multiple posts (archive/index) data into a Tera context.
/// Content is run through the WordPress content filter pipeline before insertion.
pub fn insert_posts_context(
    context: &mut Context,
    posts: &[PostTemplateData],
    pagination: &PaginationData,
) {
    // Apply content filters to each post (strip block comments, add layout classes, etc.)
    let processed_posts: Vec<PostTemplateData> = posts
        .iter()
        .map(|post| {
            let mut p = post.clone();
            p.content = super::formatting::apply_content_filters(&p.content);
            p.title = super::formatting::apply_title_filters(&p.title);
            p
        })
        .collect();
    context.insert("posts", &processed_posts);
    context.insert("pagination", pagination);
    context.insert("have_posts", &!posts.is_empty());
}

/// Generate an excerpt from content by stripping HTML and truncating.
/// Uses WordPress-compatible `[&hellip;]` suffix.
fn generate_excerpt(content: &str, word_count: usize) -> String {
    // Strip HTML tags (simple approach)
    let text = strip_html_tags(content);

    // Take first N words
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.len() <= word_count {
        words.join(" ")
    } else {
        format!("{} [&hellip;]", words[..word_count].join(" "))
    }
}

/// Simple HTML tag stripper.
fn strip_html_tags(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let mut in_tag = false;

    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => result.push(ch),
            _ => {}
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_html_tags() {
        assert_eq!(strip_html_tags("<p>Hello</p>"), "Hello");
        assert_eq!(strip_html_tags("<b>Bold</b> and <i>italic</i>"), "Bold and italic");
        assert_eq!(strip_html_tags("No tags"), "No tags");
        assert_eq!(strip_html_tags(""), "");
    }

    #[test]
    fn test_generate_excerpt_short_content() {
        let content = "Short content here";
        let excerpt = generate_excerpt(content, 55);
        assert_eq!(excerpt, "Short content here");
    }

    #[test]
    fn test_generate_excerpt_long_content() {
        let words: Vec<String> = (0..100).map(|i| format!("word{}", i)).collect();
        let content = words.join(" ");
        let excerpt = generate_excerpt(&content, 10);
        assert!(excerpt.ends_with("[&hellip;]"));
        // Count words before "[&hellip;]"
        let excerpt_words: Vec<&str> = excerpt.trim_end_matches(" [&hellip;]").split_whitespace().collect();
        assert_eq!(excerpt_words.len(), 10);
    }

    #[test]
    fn test_generate_excerpt_strips_html() {
        let content = "<p>Hello <b>world</b> this is content</p>";
        let excerpt = generate_excerpt(content, 55);
        assert_eq!(excerpt, "Hello world this is content");
    }

    #[test]
    fn test_pagination_data_first_page() {
        let pg = PaginationData::new(1, 5, 50);
        assert!(!pg.has_previous);
        assert!(pg.has_next);
        assert_eq!(pg.previous_page, 1);
        assert_eq!(pg.next_page, 2);
    }

    #[test]
    fn test_pagination_data_last_page() {
        let pg = PaginationData::new(5, 5, 50);
        assert!(pg.has_previous);
        assert!(!pg.has_next);
        assert_eq!(pg.previous_page, 4);
        assert_eq!(pg.next_page, 5);
    }

    #[test]
    fn test_pagination_data_middle_page() {
        let pg = PaginationData::new(3, 5, 50);
        assert!(pg.has_previous);
        assert!(pg.has_next);
        assert_eq!(pg.previous_page, 2);
        assert_eq!(pg.next_page, 4);
    }

    #[test]
    fn test_pagination_single_page() {
        let pg = PaginationData::new(1, 1, 5);
        assert!(!pg.has_previous);
        assert!(!pg.has_next);
    }

    #[test]
    fn test_shortcode_caption() {
        let input = r#"[caption id="attachment_1" align="aligncenter"]<img src="test.jpg" />My caption[/caption]"#;
        let result = process_shortcodes(input);
        assert!(result.contains("<figure"));
        assert!(result.contains("<figcaption>My caption</figcaption>"));
        assert!(result.contains("test.jpg"));
    }

    #[test]
    fn test_shortcode_audio() {
        let input = r#"[audio src="song.mp3"]"#;
        let result = process_shortcodes(input);
        assert!(result.contains("<audio controls"));
        assert!(result.contains("song.mp3"));
    }

    #[test]
    fn test_shortcode_video() {
        let input = r#"[video src="clip.mp4" width="640"]"#;
        let result = process_shortcodes(input);
        assert!(result.contains("<video controls"));
        assert!(result.contains("clip.mp4"));
    }

    #[test]
    fn test_shortcode_embed_youtube() {
        let input = "[embed]https://www.youtube.com/watch?v=dQw4w9WgXcQ[/embed]";
        let result = process_shortcodes(input);
        assert!(result.contains("youtube.com/embed/dQw4w9WgXcQ"));
    }

    #[test]
    fn test_shortcode_strip_unknown() {
        let input = "Hello [unknown_tag]World[/unknown_tag] Test";
        let result = process_shortcodes(input);
        assert_eq!(result, "Hello World Test");
    }

    #[test]
    fn test_shortcode_no_shortcodes() {
        let input = "Normal content without shortcodes";
        let result = process_shortcodes(input);
        assert_eq!(result, input);
    }

    #[test]
    fn test_insert_posts_context() {
        let mut ctx = Context::new();
        let posts = vec![];
        let pg = PaginationData::new(1, 1, 0);
        insert_posts_context(&mut ctx, &posts, &pg);

        // have_posts should be false for empty list
        let json = ctx.into_json();
        assert_eq!(json["have_posts"], false);
    }
}
