use regex::Regex;

/// WordPress-compatible wpautop: replaces double line breaks with paragraph elements.
///
/// This is a faithful port of WordPress's wpautop() from wp-includes/formatting.php.
/// It replaces double line breaks with HTML `<p>` tags and single line breaks with `<br />`.
pub fn wpautop(text: &str) -> String {
    if text.trim().is_empty() {
        return String::new();
    }

    let mut text = format!("{}\n", text);

    // Pre tags shouldn't be touched by autop.
    // Replace pre tags and their contents with placeholders.
    let mut pre_tags: Vec<(String, String)> = Vec::new();
    if text.contains("<pre") {
        let parts: Vec<&str> = text.split("</pre>").collect();
        let last_part = parts.last().copied().unwrap_or("");
        let mut new_text = String::new();
        let mut i = 0;

        for (idx, part) in parts.iter().enumerate() {
            if idx == parts.len() - 1 {
                // Last part (after final </pre> or the whole text if no </pre>)
                continue;
            }
            if let Some(start) = part.find("<pre") {
                let placeholder = format!("<pre wp-pre-tag-{}></pre>", i);
                let preserved = format!("{}</pre>", &part[start..]);
                pre_tags.push((placeholder.clone(), preserved));
                new_text.push_str(&part[..start]);
                new_text.push_str(&placeholder);
                i += 1;
            } else {
                new_text.push_str(part);
            }
        }
        new_text.push_str(last_part);
        text = new_text;
    }

    // Change multiple <br>s into two line breaks.
    let re_br = Regex::new(r"<br\s*/?\>\s*<br\s*/?\>").unwrap();
    text = re_br.replace_all(&text, "\n\n").to_string();

    // All block-level tags
    let allblocks = r"(?:table|thead|tfoot|caption|col|colgroup|tbody|tr|td|th|div|dl|dd|dt|ul|ol|li|pre|form|map|area|blockquote|address|math|style|p|h[1-6]|hr|fieldset|legend|section|article|aside|hgroup|header|footer|nav|figure|figcaption|details|menu|summary)";

    // Add a double line break above block-level opening tags.
    let re_open = Regex::new(&format!(r"(?i)(<{}[\s/>])", allblocks)).unwrap();
    text = re_open.replace_all(&text, "\n\n$1").to_string();

    // Add a double line break below block-level closing tags.
    let re_close = Regex::new(&format!(r"(?i)(</{}>)", allblocks)).unwrap();
    text = re_close.replace_all(&text, "$1\n\n").to_string();

    // Standardize newline characters.
    text = text.replace("\r\n", "\n").replace("\r", "\n");

    // Replace newlines inside HTML tags with a placeholder so they don't get <br>'d.
    text = wp_replace_in_html_tags(&text, "\n", " <!-- wpnl --> ");

    // Collapse line breaks around <option> elements.
    if text.contains("<option") {
        let re_opt_open = Regex::new(r"\s*<option").unwrap();
        text = re_opt_open.replace_all(&text, "<option").to_string();
        let re_opt_close = Regex::new(r"</option>\s*").unwrap();
        text = re_opt_close.replace_all(&text, "</option>").to_string();
    }

    // Collapse line breaks inside <object> elements.
    if text.contains("</object>") {
        let re_obj_open = Regex::new(r"(<object[^>]*>)\s*").unwrap();
        text = re_obj_open.replace_all(&text, "$1").to_string();
        let re_obj_close = Regex::new(r"\s*</object>").unwrap();
        text = re_obj_close.replace_all(&text, "</object>").to_string();
        let re_param = Regex::new(r"\s*(</?(?:param|embed)[^>]*>)\s*").unwrap();
        text = re_param.replace_all(&text, "$1").to_string();
    }

    // Collapse line breaks inside <audio> and <video> elements.
    if text.contains("<source") || text.contains("<track") {
        let re_av_open = Regex::new(r"([<\[](?:audio|video)[^>\]]*[>\]])\s*").unwrap();
        text = re_av_open.replace_all(&text, "$1").to_string();
        let re_av_close = Regex::new(r"\s*([<\[]/(?:audio|video)[>\]])").unwrap();
        text = re_av_close.replace_all(&text, "$1").to_string();
        let re_source = Regex::new(r"\s*(<(?:source|track)[^>]*>)\s*").unwrap();
        text = re_source.replace_all(&text, "$1").to_string();
    }

    // Collapse line breaks around <figcaption> elements.
    if text.contains("<figcaption") {
        let re_fig_open = Regex::new(r"\s*(<figcaption[^>]*>)").unwrap();
        text = re_fig_open.replace_all(&text, "$1").to_string();
        let re_fig_close = Regex::new(r"</figcaption>\s*").unwrap();
        text = re_fig_close.replace_all(&text, "</figcaption>").to_string();
    }

    // Remove more than two contiguous line breaks.
    let re_multi_nl = Regex::new(r"\n\n+").unwrap();
    text = re_multi_nl.replace_all(&text, "\n\n").to_string();

    // Split up the contents into an array of strings, separated by double line breaks.
    let paragraphs: Vec<&str> = text
        .split("\n\n")
        .filter(|s| !s.trim().is_empty())
        .collect();

    // Wrap each "paragraph" in <p> tags.
    text = paragraphs
        .iter()
        .map(|p| format!("<p>{}</p>\n", p.trim_matches('\n').trim()))
        .collect::<String>();

    // Under certain conditions, remove empty paragraphs.
    let re_empty_p = Regex::new(r"<p>\s*</p>").unwrap();
    text = re_empty_p.replace_all(&text, "").to_string();

    // If an opening or closing block element tag is wrapped in a <p>, unwrap it.
    let re_p_block = Regex::new(&format!(
        r"<p>\s*(</?\s*{}[^>]*>)\s*</p>",
        allblocks
    ))
    .unwrap();
    text = re_p_block.replace_all(&text, "$1").to_string();

    // Remove <p> that wraps around block-level opening tags.
    let re_p_before_block = Regex::new(&format!(
        r"<p>\s*(</?\s*{}[^>]*>)",
        allblocks
    ))
    .unwrap();
    text = re_p_before_block.replace_all(&text, "$1").to_string();

    // Remove </p> that follows block-level closing tags.
    let re_block_before_p = Regex::new(&format!(
        r"(</?\s*{}[^>]*>)\s*</p>",
        allblocks
    ))
    .unwrap();
    text = re_block_before_p.replace_all(&text, "$1").to_string();

    // If li is wrapped in a <p>, remove the <p>.
    let re_p_li = Regex::new(r"<p>(<li.+?)</p>").unwrap();
    text = re_p_li.replace_all(&text, "$1").to_string();

    // Blockquote fix: move <p> inside blockquote.
    let re_p_bq = Regex::new(r"(?i)<p><blockquote([^>]*)>").unwrap();
    text = re_p_bq.replace_all(&text, "<blockquote$1><p>").to_string();
    text = text.replace("</blockquote></p>", "</p></blockquote>");

    // Replace single newlines that aren't preceded by <br /> with <br />.
    // But preserve newlines inside <script>, <style>, <svg>.
    text = preserve_newlines_in_tags(&text);
    text = add_br_tags(&text);
    text = text.replace("<WPPreserveNewline />", "\n");

    // If a <br /> tag is right after a block element tag, remove it.
    let re_br_after_block = Regex::new(&format!(
        r"(</?\s*{}[^>]*>)\s*<br />",
        allblocks
    ))
    .unwrap();
    text = re_br_after_block.replace_all(&text, "$1").to_string();

    // If a <br /> tag is before certain closing block tags, remove it.
    let re_br_before_block = Regex::new(
        r"<br />\s*(</?(?:p|li|div|dl|dd|dt|th|pre|td|ul|ol)[^>]*>)",
    )
    .unwrap();
    text = re_br_before_block.replace_all(&text, "$1").to_string();

    // Remove trailing newline from last </p>.
    let re_trailing_p = Regex::new(r"\n</p>$").unwrap();
    text = re_trailing_p.replace(&text, "</p>").to_string();

    // Restore pre tags.
    for (placeholder, original) in &pre_tags {
        text = text.replace(placeholder, original);
    }

    // Restore newline placeholders.
    if text.contains("<!-- wpnl -->") {
        text = text
            .replace(" <!-- wpnl --> ", "\n")
            .replace("<!-- wpnl -->", "\n");
    }

    text.trim().to_string()
}

/// Replace newlines within HTML tags with a placeholder.
/// This prevents newlines inside tag attributes from being converted to <br />.
fn wp_replace_in_html_tags(text: &str, search: &str, replace: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut in_tag = false;
    let mut i = 0;
    let chars: Vec<char> = text.chars().collect();

    while i < chars.len() {
        if chars[i] == '<' && !in_tag {
            in_tag = true;
            result.push('<');
            i += 1;
            continue;
        }
        if chars[i] == '>' && in_tag {
            in_tag = false;
            result.push('>');
            i += 1;
            continue;
        }
        if in_tag && chars[i] == '\n' {
            result.push_str(replace);
            i += 1;
            continue;
        }
        result.push(chars[i]);
        i += 1;
    }
    let _ = search; // used conceptually
    result
}

/// Replace single newlines with `<br />\n`, but skip if already preceded by `<br />`.
/// This avoids regex lookbehind which Rust's regex crate doesn't support.
fn add_br_tags(text: &str) -> String {
    let lines: Vec<&str> = text.split('\n').collect();
    let mut result = String::with_capacity(text.len() + text.len() / 4);

    for (i, line) in lines.iter().enumerate() {
        result.push_str(line);
        if i < lines.len() - 1 {
            // Check if this line already ends with <br /> (possibly with trailing whitespace)
            let trimmed = line.trim_end();
            if trimmed.ends_with("<br />") || trimmed.ends_with("<br/>") || trimmed.ends_with("<br>") {
                result.push('\n');
            } else {
                result.push_str("<br />\n");
            }
        }
    }
    result
}

/// Preserve newlines inside <script>, <style>, and <svg> tags.
fn preserve_newlines_in_tags(text: &str) -> String {
    let mut result = text.to_string();
    for tag in &["script", "style", "svg"] {
        let re = Regex::new(&format!(r"(?si)<{}[^>]*>.*?</{}>", tag, tag)).unwrap();
        result = re
            .replace_all(&result, |caps: &regex::Captures| {
                caps[0].replace('\n', "<WPPreserveNewline />")
            })
            .to_string();
    }
    result
}

/// WordPress-compatible wptexturize: replaces common plain characters with typographic equivalents.
///
/// Converts straight quotes to curly quotes, double hyphens to em dashes,
/// and three dots to ellipsis characters.
pub fn wptexturize(text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }

    // Don't process if the text has no characters we'd convert.
    if !text.contains('"')
        && !text.contains('\'')
        && !text.contains('-')
        && !text.contains('.')
        && !text.contains('`')
    {
        return text.to_string();
    }

    // Split text by HTML tags so we only process text nodes.
    // Tags that should not have their contents processed.
    let no_texturize_tags = [
        "pre", "code", "kbd", "style", "script", "tt", "textarea",
    ];
    let mut result = String::with_capacity(text.len());
    let mut skip_depth: Vec<String> = Vec::new();

    // Tokenize into text and HTML tags
    let tokens = tokenize_html(text);

    for token in &tokens {
        match token {
            HtmlToken::Tag(tag) => {
                // Check if this opens a no-texturize tag
                for &nt in &no_texturize_tags {
                    if tag.starts_with(&format!("<{}", nt))
                        && (tag.len() > nt.len() + 1)
                        && (tag.as_bytes()[nt.len() + 1] == b' '
                            || tag.as_bytes()[nt.len() + 1] == b'>'
                            || tag.as_bytes()[nt.len() + 1] == b'/')
                    {
                        skip_depth.push(nt.to_string());
                    } else if tag == &format!("</{}>", nt) {
                        if let Some(pos) = skip_depth.iter().rposition(|s| s == nt) {
                            skip_depth.remove(pos);
                        }
                    }
                }
                result.push_str(tag);
            }
            HtmlToken::Text(text_content) => {
                if skip_depth.is_empty() {
                    result.push_str(&texturize_text(text_content));
                } else {
                    result.push_str(text_content);
                }
            }
        }
    }

    result
}

/// Apply typographic replacements to a text node (not HTML).
fn texturize_text(text: &str) -> String {
    let mut s = text.to_string();

    // Em dash: --- → —
    s = s.replace("---", "\u{2014}");

    // En dash: -- → –
    s = s.replace("--", "\u{2013}");

    // Ellipsis: ... → …
    s = s.replace("...", "\u{2026}");

    // Backtick quotes: `` → " and '' → "
    s = s.replace("``", "\u{201c}");
    // Only replace '' when it's likely a closing quote (after non-space)
    // WordPress handles this with context, simplified here

    // Double quotes
    s = convert_double_quotes(&s);

    // Single quotes / apostrophes
    s = convert_single_quotes(&s);

    s
}

/// Convert straight double quotes to curly double quotes.
fn convert_double_quotes(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    let mut in_opening = false;

    while i < chars.len() {
        if chars[i] == '"' {
            // Determine if opening or closing quote
            if i == 0 || is_opening_context(chars[i.saturating_sub(1)]) {
                // Opening double quote
                result.push('\u{201c}'); // "
                in_opening = true;
            } else {
                // Closing double quote
                result.push('\u{201d}'); // "
                in_opening = false;
            }
        } else {
            result.push(chars[i]);
        }
        i += 1;
    }

    // If we ended with an unclosed opening quote, that's fine
    let _ = in_opening;
    result
}

/// Convert straight single quotes to curly single quotes / apostrophes.
fn convert_single_quotes(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        if chars[i] == '\'' {
            if i > 0 && i + 1 < chars.len() && chars[i - 1].is_alphanumeric() && chars[i + 1].is_alphanumeric() {
                // Apostrophe (e.g., it's, don't)
                result.push('\u{2019}'); // '
            } else if i == 0 || is_opening_context(chars[i.saturating_sub(1)]) {
                // Opening single quote
                result.push('\u{2018}'); // '
            } else {
                // Closing single quote
                result.push('\u{2019}'); // '
            }
        } else {
            result.push(chars[i]);
        }
        i += 1;
    }

    result
}

/// Check if the previous character suggests the next quote should be an opening quote.
fn is_opening_context(prev: char) -> bool {
    prev == ' '
        || prev == '\n'
        || prev == '\t'
        || prev == '\r'
        || prev == '('
        || prev == '['
        || prev == '{'
        || prev == '>'
        || prev == '\u{00a0}' // nbsp
}

/// Tokenize HTML into text nodes and tag nodes.
enum HtmlToken {
    Text(String),
    Tag(String),
}

fn tokenize_html(html: &str) -> Vec<HtmlToken> {
    let mut tokens = Vec::new();
    let mut current_text = String::new();
    let chars: Vec<char> = html.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        if chars[i] == '<' {
            // Check if this is a real tag (not just a lone <)
            if let Some(end) = find_tag_end(&chars, i) {
                // Push accumulated text
                if !current_text.is_empty() {
                    tokens.push(HtmlToken::Text(std::mem::take(&mut current_text)));
                }
                let tag: String = chars[i..=end].iter().collect();
                tokens.push(HtmlToken::Tag(tag));
                i = end + 1;
                continue;
            }
        }
        current_text.push(chars[i]);
        i += 1;
    }

    if !current_text.is_empty() {
        tokens.push(HtmlToken::Text(current_text));
    }

    tokens
}

/// Find the closing > for a tag starting at position `start`.
fn find_tag_end(chars: &[char], start: usize) -> Option<usize> {
    let mut i = start + 1;
    let mut in_quote = false;
    let mut quote_char = '"';

    while i < chars.len() {
        if in_quote {
            if chars[i] == quote_char {
                in_quote = false;
            }
        } else {
            match chars[i] {
                '>' => return Some(i),
                '"' | '\'' => {
                    in_quote = true;
                    quote_char = chars[i];
                }
                _ => {}
            }
        }
        i += 1;
    }
    None
}

/// Apply the full WordPress content filter pipeline.
///
/// This is equivalent to WordPress's `apply_filters('the_content', $content)`.
/// It processes shortcodes, applies wpautop, and wptexturize.
pub fn apply_content_filters(content: &str) -> String {
    use super::tags::process_shortcodes;

    let mut result = content.to_string();

    // Check if content uses the block editor before stripping comments
    let is_block_content = has_blocks(&result);

    // 0. Strip WordPress block editor comments (<!-- wp:xxx --> / <!-- /wp:xxx -->)
    result = strip_block_comments(&result);

    // 0.5. Add WordPress layout classes to block elements
    if is_block_content {
        result = add_block_layout_classes(&result);
    }

    // 1. Process shortcodes ([caption], [audio], [video], etc.)
    result = process_shortcodes(&result);

    // 2. Apply wpautop (paragraph wrapping) — skip for block editor content
    //    Block content already has proper HTML structure; wpautop would break it
    if !is_block_content {
        result = wpautop(&result);
    }

    // 3. Apply wptexturize (smart typography)
    result = wptexturize(&result);

    result
}

/// Check if content contains Gutenberg block markers.
///
/// WordPress uses this to skip wpautop for block editor content,
/// since blocks already contain properly structured HTML.
pub fn has_blocks(content: &str) -> bool {
    content.contains("<!-- wp:")
}

/// Strip WordPress Gutenberg block comments from content.
///
/// Removes `<!-- wp:blockname -->`, `<!-- wp:blockname {"attrs":...} -->`,
/// and `<!-- /wp:blockname -->` comments that the block editor stores in post_content.
/// WordPress's block parser processes these; we strip them since we render raw HTML.
fn strip_block_comments(content: &str) -> String {
    // Match block comments including nested JSON braces like {"layout":{"type":"constrained"}}
    let re = Regex::new(r"<!-- /?wp:\S+?(?:\s+\{.*?\})?\s*/?-->").unwrap();
    re.replace_all(content, "").to_string()
}

/// Add WordPress layout classes to block elements in post content.
///
/// WordPress adds `is-layout-flex`/`is-layout-flow` and corresponding
/// `wp-block-*-is-layout-*` classes at render time. Since RustPress serves
/// raw post_content from the database, we need to add these classes ourselves.
fn add_block_layout_classes(content: &str) -> String {
    let mut result = content.to_string();

    // Block name → (layout type, compound class suffix)
    let flex_blocks = [
        ("wp-block-columns", "wp-block-columns-is-layout-flex"),
        ("wp-block-buttons", "wp-block-buttons-is-layout-flex"),
        ("wp-block-gallery", "wp-block-gallery-is-layout-flex"),
    ];

    let flow_blocks = [
        ("wp-block-quote", "wp-block-quote-is-layout-flow"),
        ("wp-block-cover__inner-container", "wp-block-cover-is-layout-flow"),
    ];

    for (block_class, compound_class) in &flex_blocks {
        let pattern = format!(
            r#"class="([^"]*\b{}\b(?:(?!\bis-layout-)[^"])*)"#,
            regex::escape(block_class)
        );
        if let Ok(re) = Regex::new(&pattern) {
            let bc = *block_class;
            let cc = *compound_class;
            result = re
                .replace_all(&result, |caps: &regex::Captures| {
                    let classes = &caps[1];
                    if classes.contains("is-layout-") {
                        return caps[0].to_string();
                    }
                    format!("class=\"{} is-layout-flex {}\"", classes, cc)
                })
                .to_string();
            let _ = bc; // block_class used via pattern
        }
    }

    for (block_class, compound_class) in &flow_blocks {
        let pattern = format!(
            r#"class="([^"]*\b{}\b(?:(?!\bis-layout-)[^"])*)"#,
            regex::escape(block_class)
        );
        if let Ok(re) = Regex::new(&pattern) {
            let bc = *block_class;
            let cc = *compound_class;
            result = re
                .replace_all(&result, |caps: &regex::Captures| {
                    let classes = &caps[1];
                    if classes.contains("is-layout-") {
                        return caps[0].to_string();
                    }
                    format!("class=\"{} is-layout-flow {}\"", classes, cc)
                })
                .to_string();
            let _ = bc;
        }
    }

    // wp-block-column (without matching wp-block-columns)
    // Use word boundary: "wp-block-column" not followed by "s"
    if let Ok(re) = Regex::new(r#"class="([^"]*\bwp-block-column\b(?!s)(?:(?!\bis-layout-)[^"])*)"#) {
        result = re
            .replace_all(&result, |caps: &regex::Captures| {
                let classes = &caps[1];
                if classes.contains("is-layout-") {
                    return caps[0].to_string();
                }
                format!(
                    "class=\"{} is-layout-flow wp-block-column-is-layout-flow\"",
                    classes
                )
            })
            .to_string();
    }

    result
}

/// Apply content filters suitable for titles (no wpautop, just wptexturize).
pub fn apply_title_filters(title: &str) -> String {
    wptexturize(title)
}

/// Apply the full WordPress content filter pipeline with HookRegistry integration.
///
/// Runs the standard formatting pipeline, then passes the result through
/// `apply_filters("the_content", ...)` so plugins can modify post content.
pub fn apply_content_filters_with_hooks(
    content: &str,
    hooks: &rustpress_core::hooks::HookRegistry,
) -> String {
    let mut result = apply_content_filters(content);

    // Pass through HookRegistry so plugins can modify the_content
    let filtered = hooks.apply_filters("the_content", serde_json::json!(result));
    if let serde_json::Value::String(s) = filtered {
        result = s;
    }

    result
}

/// Apply the full content filter pipeline using both ShortcodeRegistry and HookRegistry.
///
/// This is the most complete content processing path:
/// 1. Process shortcodes via ShortcodeRegistry (plugin-extensible)
/// 2. Run built-in formatting (wpautop, wptexturize)
/// 3. Apply HookRegistry filters (`the_content`)
pub fn apply_content_filters_full(
    content: &str,
    shortcodes: &rustpress_core::shortcode::ShortcodeRegistry,
    hooks: &rustpress_core::hooks::HookRegistry,
) -> String {
    let mut result = content.to_string();

    // Check if content uses the block editor before stripping comments
    let is_block_content = has_blocks(&result);

    // 0. Strip WordPress block editor comments
    result = strip_block_comments(&result);

    // 0.5. Add WordPress layout classes to block elements
    if is_block_content {
        result = add_block_layout_classes(&result);
    }

    // 1. Process shortcodes via registry (plugins can add shortcodes here)
    result = shortcodes.do_shortcode(&result);

    // 2. Also process built-in hardcoded shortcodes as fallback
    result = super::tags::process_shortcodes(&result);

    // 3. Apply wpautop (paragraph wrapping) — skip for block editor content
    if !is_block_content {
        result = wpautop(&result);
    }

    // 4. Apply wptexturize (smart typography)
    result = wptexturize(&result);

    // 5. Pass through HookRegistry so plugins can modify the_content
    let filtered = hooks.apply_filters("the_content", serde_json::json!(result));
    if let serde_json::Value::String(s) = filtered {
        result = s;
    }

    result
}

/// Apply title filters with HookRegistry integration.
pub fn apply_title_filters_with_hooks(
    title: &str,
    hooks: &rustpress_core::hooks::HookRegistry,
) -> String {
    let mut result = apply_title_filters(title);

    let filtered = hooks.apply_filters("the_title", serde_json::json!(result));
    if let serde_json::Value::String(s) = filtered {
        result = s;
    }

    result
}

/// Apply content filters for excerpts.
pub fn apply_excerpt_filters(excerpt: &str) -> String {
    if excerpt.is_empty() {
        return String::new();
    }
    let mut result = wpautop(excerpt);
    result = wptexturize(&result);
    result
}

/// Apply excerpt filters with HookRegistry integration.
pub fn apply_excerpt_filters_with_hooks(
    excerpt: &str,
    hooks: &rustpress_core::hooks::HookRegistry,
) -> String {
    let mut result = apply_excerpt_filters(excerpt);

    let filtered = hooks.apply_filters("get_the_excerpt", serde_json::json!(result));
    if let serde_json::Value::String(s) = filtered {
        result = s;
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    // ======================== wpautop tests ========================

    #[test]
    fn test_wpautop_empty() {
        assert_eq!(wpautop(""), "");
        assert_eq!(wpautop("   "), "");
    }

    #[test]
    fn test_wpautop_single_line() {
        assert_eq!(wpautop("Hello world"), "<p>Hello world</p>");
    }

    #[test]
    fn test_wpautop_double_newline_creates_paragraphs() {
        let input = "First paragraph.\n\nSecond paragraph.";
        let result = wpautop(input);
        assert!(result.contains("<p>First paragraph.</p>"));
        assert!(result.contains("<p>Second paragraph.</p>"));
    }

    #[test]
    fn test_wpautop_single_newline_creates_br() {
        let input = "Line one.\nLine two.";
        let result = wpautop(input);
        assert!(result.contains("Line one.<br />"));
        assert!(result.contains("Line two."));
    }

    #[test]
    fn test_wpautop_preserves_existing_p_tags() {
        let input = "<p>Already wrapped.</p>";
        let result = wpautop(input);
        assert!(result.contains("<p>Already wrapped.</p>"));
        // Should not double-wrap
        assert!(!result.contains("<p><p>"));
    }

    #[test]
    fn test_wpautop_preserves_block_elements() {
        let input = "<div>Block content</div>";
        let result = wpautop(input);
        assert!(result.contains("<div>Block content</div>"));
        // Should not wrap div in <p>
        assert!(!result.contains("<p><div>"));
    }

    #[test]
    fn test_wpautop_preserves_pre_tags() {
        let input = "Before.\n\n<pre>Line 1\nLine 2\nLine 3</pre>\n\nAfter.";
        let result = wpautop(input);
        assert!(result.contains("<pre>Line 1\nLine 2\nLine 3</pre>"));
        assert!(result.contains("<p>Before.</p>"));
        assert!(result.contains("<p>After.</p>"));
    }

    #[test]
    fn test_wpautop_handles_multiple_paragraphs() {
        let input = "Para one.\n\nPara two.\n\nPara three.";
        let result = wpautop(input);
        assert!(result.contains("<p>Para one.</p>"));
        assert!(result.contains("<p>Para two.</p>"));
        assert!(result.contains("<p>Para three.</p>"));
    }

    #[test]
    fn test_wpautop_list_elements() {
        let input = "<ul>\n<li>Item 1</li>\n<li>Item 2</li>\n</ul>";
        let result = wpautop(input);
        assert!(result.contains("<li>Item 1</li>"));
        assert!(result.contains("<li>Item 2</li>"));
        // Should not add <br> inside list
    }

    #[test]
    fn test_wpautop_heading() {
        let input = "<h2>Title</h2>\n\nContent here.";
        let result = wpautop(input);
        assert!(result.contains("<h2>Title</h2>"));
        assert!(result.contains("<p>Content here.</p>"));
    }

    #[test]
    fn test_wpautop_mixed_content() {
        let input = "Paragraph text.\n\n<blockquote>A quote.</blockquote>\n\nMore text.";
        let result = wpautop(input);
        assert!(result.contains("<p>Paragraph text.</p>"));
        assert!(result.contains("<blockquote>"));
        assert!(result.contains("<p>More text.</p>"));
    }

    // ======================== wptexturize tests ========================

    #[test]
    fn test_wptexturize_empty() {
        assert_eq!(wptexturize(""), "");
    }

    #[test]
    fn test_wptexturize_no_changes_needed() {
        assert_eq!(wptexturize("Hello world"), "Hello world");
    }

    #[test]
    fn test_wptexturize_em_dash() {
        let result = wptexturize("Hello --- world");
        assert!(result.contains('\u{2014}')); // em dash
    }

    #[test]
    fn test_wptexturize_en_dash() {
        let result = wptexturize("Hello -- world");
        assert!(result.contains('\u{2013}')); // en dash
    }

    #[test]
    fn test_wptexturize_ellipsis() {
        let result = wptexturize("Hello...");
        assert!(result.contains('\u{2026}')); // ellipsis
    }

    #[test]
    fn test_wptexturize_double_quotes() {
        let result = wptexturize(r#"He said "hello" to her."#);
        assert!(result.contains('\u{201c}')); // opening "
        assert!(result.contains('\u{201d}')); // closing "
        assert!(!result.contains('"')); // no straight quotes left
    }

    #[test]
    fn test_wptexturize_single_quotes_apostrophe() {
        let result = wptexturize("It's a test.");
        assert!(result.contains('\u{2019}')); // curly apostrophe
        assert!(!result.contains('\'')); // no straight quote
    }

    #[test]
    fn test_wptexturize_preserves_pre_tag_content() {
        let result = wptexturize(r#"<pre>He said "hello"</pre>"#);
        assert!(result.contains(r#""hello""#)); // unchanged inside <pre>
    }

    #[test]
    fn test_wptexturize_preserves_code_tag_content() {
        let result = wptexturize(r#"<code>it's code</code>"#);
        assert!(result.contains("it's code")); // unchanged inside <code>
    }

    #[test]
    fn test_wptexturize_processes_outside_tags() {
        let result = wptexturize(r#"Before <code>inside</code> "after""#);
        // "after" should be texturized
        assert!(result.contains('\u{201c}'));
        assert!(result.contains('\u{201d}'));
        // content inside <code> unchanged
        assert!(result.contains("inside"));
    }

    #[test]
    fn test_wptexturize_html_attributes_preserved() {
        let result = wptexturize(r#"<a href="http://example.com">Link</a>"#);
        assert!(result.contains(r#"href="http://example.com""#)); // attributes unchanged
    }

    // ======================== apply_content_filters tests ========================

    #[test]
    fn test_apply_content_filters_basic() {
        let input = "Hello world.\n\nSecond paragraph.";
        let result = apply_content_filters(input);
        assert!(result.contains("<p>Hello world.</p>"));
        assert!(result.contains("<p>Second paragraph.</p>"));
    }

    #[test]
    fn test_apply_content_filters_with_shortcode() {
        let input = "Text before.\n\n[audio src=\"song.mp3\"]\n\nText after.";
        let result = apply_content_filters(input);
        assert!(result.contains("<audio controls"));
        assert!(result.contains("song.mp3"));
    }

    #[test]
    fn test_apply_content_filters_typography() {
        let input = "He said \"hello\" and she said \"goodbye\"...";
        let result = apply_content_filters(input);
        assert!(result.contains('\u{201c}')); // curly quotes
        assert!(result.contains('\u{2026}')); // ellipsis
    }

    #[test]
    fn test_apply_title_filters() {
        let result = apply_title_filters("It's a \"great\" day...");
        assert!(result.contains('\u{2019}')); // apostrophe
        assert!(result.contains('\u{201c}')); // opening quote
        assert!(result.contains('\u{2026}')); // ellipsis
        // Should NOT contain <p> tags
        assert!(!result.contains("<p>"));
    }
}
