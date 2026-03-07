use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;

// ---------------------------------------------------------------------------
// Allowed-HTML presets
// ---------------------------------------------------------------------------

/// Map from tag name to the set of allowed attribute names.
pub type AllowedHtml = HashMap<&'static str, &'static [&'static str]>;

/// Allowed HTML for post/page content (`wp_kses_post`).
///
/// Equivalent to WordPress `$allowedposttags` defined in
/// `wp-includes/kses.php`.
pub static KSES_ALLOWED_POST: LazyLock<AllowedHtml> = LazyLock::new(|| {
    let mut m = HashMap::new();
    m.insert(
        "a",
        ["href", "title", "rel", "target", "class", "id"].as_slice(),
    );
    m.insert("abbr", ["title"].as_slice());
    m.insert("b", ["class"].as_slice());
    m.insert("blockquote", ["cite", "class"].as_slice());
    m.insert("br", [].as_slice());
    m.insert("cite", ["class"].as_slice());
    m.insert("code", ["class"].as_slice());
    m.insert("del", ["datetime", "cite"].as_slice());
    m.insert("dd", ["class"].as_slice());
    m.insert("div", ["class", "id", "style"].as_slice());
    m.insert("dl", ["class"].as_slice());
    m.insert("dt", ["class"].as_slice());
    m.insert("em", ["class"].as_slice());
    m.insert("figure", ["class", "id"].as_slice());
    m.insert("figcaption", ["class"].as_slice());
    m.insert("h1", ["class", "id"].as_slice());
    m.insert("h2", ["class", "id"].as_slice());
    m.insert("h3", ["class", "id"].as_slice());
    m.insert("h4", ["class", "id"].as_slice());
    m.insert("h5", ["class", "id"].as_slice());
    m.insert("h6", ["class", "id"].as_slice());
    m.insert("hr", ["class"].as_slice());
    m.insert("i", ["class"].as_slice());
    m.insert(
        "img",
        [
            "src", "alt", "width", "height", "class", "id", "loading", "srcset", "sizes",
        ]
        .as_slice(),
    );
    m.insert("li", ["class", "id"].as_slice());
    m.insert("ol", ["class", "type", "start", "reversed"].as_slice());
    m.insert("p", ["class", "id", "style"].as_slice());
    m.insert("pre", ["class"].as_slice());
    m.insert("q", ["cite"].as_slice());
    m.insert("s", ["class"].as_slice());
    m.insert("span", ["class", "id", "style"].as_slice());
    m.insert("strong", ["class"].as_slice());
    m.insert("sub", ["class"].as_slice());
    m.insert("sup", ["class"].as_slice());
    m.insert("table", ["class", "id"].as_slice());
    m.insert(
        "thead",
        ["class", "id", "colspan", "rowspan", "scope"].as_slice(),
    );
    m.insert(
        "tbody",
        ["class", "id", "colspan", "rowspan", "scope"].as_slice(),
    );
    m.insert(
        "tfoot",
        ["class", "id", "colspan", "rowspan", "scope"].as_slice(),
    );
    m.insert(
        "tr",
        ["class", "id", "colspan", "rowspan", "scope"].as_slice(),
    );
    m.insert(
        "th",
        ["class", "id", "colspan", "rowspan", "scope"].as_slice(),
    );
    m.insert(
        "td",
        ["class", "id", "colspan", "rowspan", "scope"].as_slice(),
    );
    m.insert("u", ["class"].as_slice());
    m.insert("ul", ["class"].as_slice());
    m
});

/// Allowed HTML for comments (`wp_kses` with comment preset).
///
/// Equivalent to WordPress `$allowedtags` defined in
/// `wp-includes/kses.php`.
pub static KSES_ALLOWED_COMMENT: LazyLock<AllowedHtml> = LazyLock::new(|| {
    let mut m = HashMap::new();
    m.insert("a", ["href", "title", "rel"].as_slice());
    m.insert("abbr", ["title"].as_slice());
    m.insert("b", [].as_slice());
    m.insert("blockquote", ["cite"].as_slice());
    m.insert("br", [].as_slice());
    m.insert("cite", [].as_slice());
    m.insert("code", [].as_slice());
    m.insert("del", ["datetime"].as_slice());
    m.insert("em", [].as_slice());
    m.insert("i", [].as_slice());
    m.insert("p", [].as_slice());
    m.insert("pre", [].as_slice());
    m.insert("q", ["cite"].as_slice());
    m.insert("s", [].as_slice());
    m.insert("strong", [].as_slice());
    m
});

// ---------------------------------------------------------------------------
// Tags that are ALWAYS stripped regardless of the allow-list.
// ---------------------------------------------------------------------------

/// Tags unconditionally forbidden. Even if someone places them in an
/// allow-list they will be removed.
const ALWAYS_FORBIDDEN_TAGS: &[&str] = &[
    "script", "style", "iframe", "object", "embed", "form", "input", "textarea", "select",
    "button", "applet", "meta", "link", "base",
];

// ---------------------------------------------------------------------------
// Compiled regexes (compiled once, reused on every call)
// ---------------------------------------------------------------------------

/// Matches an HTML tag (opening, closing, or self-closing).
///
/// Captures:
///   1 – optional `/` (closing tag)
///   2 – tag name
///   3 – the rest of the tag (attributes etc.) up to the closing `>`
static RE_TAG: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?si)<(/?)([a-zA-Z][a-zA-Z0-9]*)\b([^>]*)>").unwrap());

/// Matches a single HTML attribute inside a tag.
///
/// Captures:
///   1 – attribute name
///   2 – attribute value (if double-quoted)
///   3 – attribute value (if single-quoted)
///   4 – attribute value (if unquoted)
static RE_ATTR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?si)([a-zA-Z_][\w\-.]*)(?:\s*=\s*(?:"([^"]*)"|'([^']*)'|(\S+)))?"#).unwrap()
});

/// Matches event-handler attributes (`on*`).
static RE_EVENT_HANDLER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^on[a-z]").unwrap());

/// Matches dangerous protocols in URL values.
static RE_BAD_PROTOCOL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^\s*(javascript|vbscript)\s*:").unwrap());

/// Matches any `data:` URI scheme.
static RE_DATA_URI: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^\s*data\s*:").unwrap());

/// Matches `data:image/` URIs specifically (the only safe `data:` URIs).
static RE_DATA_IMAGE_URI: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^\s*data\s*:\s*image/").unwrap());

/// Matches dangerous CSS constructs inside style attributes.
static RE_BAD_CSS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)(expression\s*\(|url\s*\(\s*(javascript|vbscript)\s*:|behavior\s*:|-moz-binding\s*:)",
    )
    .unwrap()
});

/// URL-bearing attributes that need protocol validation.
const URL_ATTRS: &[&str] = &[
    "href",
    "src",
    "action",
    "srcset",
    "formaction",
    "cite",
    "poster",
];

/// Check whether a value is a `data:` URI that is NOT `data:image/*`.
/// Returns `true` for dangerous data URIs, `false` for safe ones (or non-data URIs).
fn is_bad_data_uri(value: &str) -> bool {
    RE_DATA_URI.is_match(value) && !RE_DATA_IMAGE_URI.is_match(value)
}

// ---------------------------------------------------------------------------
// Core sanitisation function
// ---------------------------------------------------------------------------

/// Sanitise HTML content using WordPress-compatible kses rules.
///
/// Equivalent to WordPress `wp_kses($string, $allowed_html)` defined in
/// `wp-includes/kses.php`.
///
/// * `content`      – raw HTML string to sanitise.
/// * `allowed_html` – map of `tag_name -> &[allowed_attr_names]`.
///
/// Tags not present in `allowed_html` (and those in [`ALWAYS_FORBIDDEN_TAGS`])
/// are stripped entirely (both the tag and its corresponding close tag, but
/// **not** the text content between them, except for `<script>` and `<style>`
/// whose content is dangerous and therefore removed).
///
/// Attributes not present in the per-tag allow-list are removed.  Event
/// handler attributes (`on*`) and dangerous URL protocols are **always**
/// removed regardless of the allow-list.
pub fn wp_kses(content: &str, allowed_html: &AllowedHtml) -> String {
    // Phase 1: Remove content of inherently dangerous tags (<script>, <style>)
    // whose body must not leak into the output at all.
    let content = strip_dangerous_tag_content(content, "script");
    let content = strip_dangerous_tag_content(&content, "style");

    // Phase 2: Process remaining tags via regex.
    let result = RE_TAG.replace_all(&content, |caps: &regex::Captures<'_>| {
        let is_closing = !caps.get(1).map_or("", |m| m.as_str()).is_empty();
        let tag_name = caps.get(2).map_or("", |m| m.as_str()).to_ascii_lowercase();
        let attrs_raw = caps.get(3).map_or("", |m| m.as_str());

        // Always-forbidden tags are silently dropped.
        if ALWAYS_FORBIDDEN_TAGS.contains(&tag_name.as_str()) {
            return String::new();
        }

        // If the tag is not in the allow-list, drop it.
        let Some(allowed_attrs) = allowed_html.get(tag_name.as_str()) else {
            return String::new();
        };

        // Closing tags carry no attributes.
        if is_closing {
            return format!("</{}>", tag_name);
        }

        // Build the filtered attribute string.
        let filtered_attrs = filter_attributes(attrs_raw, allowed_attrs);

        // Detect self-closing markers (`/` just before `>`).
        let self_closing = attrs_raw.trim_end().ends_with('/');
        if self_closing {
            if filtered_attrs.is_empty() {
                format!("<{} />", tag_name)
            } else {
                format!("<{} {} />", tag_name, filtered_attrs)
            }
        } else if filtered_attrs.is_empty() {
            format!("<{}>", tag_name)
        } else {
            format!("<{} {}>", tag_name, filtered_attrs)
        }
    });

    result.into_owned()
}

// ---------------------------------------------------------------------------
// Convenience wrappers
// ---------------------------------------------------------------------------

/// Sanitise HTML suitable for post/page content.
///
/// Equivalent to WordPress `wp_kses_post($data)`.
pub fn wp_kses_post(content: &str) -> String {
    wp_kses(content, &KSES_ALLOWED_POST)
}

/// Sanitise HTML suitable for comments.
///
/// Equivalent to WordPress `wp_kses($data, $allowedtags)` for comments.
pub fn wp_kses_comment(content: &str) -> String {
    wp_kses(content, &KSES_ALLOWED_COMMENT)
}

/// Strip **all** HTML tags — used for option values, post meta, etc.
///
/// Equivalent to WordPress `wp_kses($data, 'strip')` / `wp_strip_all_tags()`.
pub fn wp_kses_data(content: &str) -> String {
    // Remove content of script/style first (their text is dangerous).
    let content = strip_dangerous_tag_content(content, "script");
    let content = strip_dangerous_tag_content(&content, "style");
    // Strip every remaining tag.
    RE_TAG.replace_all(&content, "").into_owned()
}

// ---------------------------------------------------------------------------
// Escaping helpers
// ---------------------------------------------------------------------------

/// HTML-entity-encode a string for safe display in HTML body text.
///
/// Equivalent to WordPress `esc_html($text)` defined in
/// `wp-includes/formatting.php`.
///
/// Encodes: `&`, `<`, `>`, `"`, `'`
pub fn esc_html(content: &str) -> String {
    let mut out = String::with_capacity(content.len());
    for ch in content.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#039;"),
            _ => out.push(ch),
        }
    }
    out
}

/// Escape a string for safe use inside an HTML attribute value.
///
/// Equivalent to WordPress `esc_attr($text)`.
///
/// This performs the same encoding as [`esc_html`] — they differ in
/// WordPress only by the filter hooks applied, which we do not yet fire.
pub fn esc_attr(content: &str) -> String {
    esc_html(content)
}

/// Validate and sanitise a URL.
///
/// Equivalent to WordPress `esc_url($url)` defined in
/// `wp-includes/formatting.php`.
///
/// Allowed protocols: `http`, `https`, `mailto`, `tel`, `#` (fragment-only).
/// Dangerous protocols (`javascript:`, `vbscript:`, `data:` except
/// `data:image/`) are rejected and the function returns an empty string.
pub fn esc_url(url: &str) -> String {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    // Fragment-only URLs are always safe.
    if trimmed.starts_with('#') {
        return esc_attr(trimmed);
    }

    // Reject dangerous protocols.
    if RE_BAD_PROTOCOL.is_match(trimmed) {
        return String::new();
    }
    if is_bad_data_uri(trimmed) {
        return String::new();
    }

    // Allow only known-safe protocols when a scheme is present.
    if let Some(colon_pos) = trimmed.find(':') {
        let scheme = trimmed[..colon_pos].trim().to_ascii_lowercase();
        match scheme.as_str() {
            "http" | "https" | "mailto" | "tel" => {}
            // data:image/* is acceptable (already passed is_bad_data_uri check).
            "data" => {}
            _ => return String::new(),
        }
    }

    // Encode bare ampersands that are not already entities, and encode
    // quotes so the URL is safe inside an attribute.
    encode_url_entities(trimmed)
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Remove everything between `<tag ...>` and `</tag>` (inclusive) for a
/// specific tag name.  Handles nested occurrences and is case-insensitive.
fn strip_dangerous_tag_content(content: &str, tag: &str) -> String {
    let re = Regex::new(&format!(r"(?si)<{tag}\b[^>]*>.*?</{tag}\s*>")).unwrap();
    let result = re.replace_all(content, "");
    // Also strip any remaining orphaned opening/closing tags.
    let re_open = Regex::new(&format!(r"(?si)<{tag}\b[^>]*>")).unwrap();
    let result = re_open.replace_all(&result, "");
    let re_close = Regex::new(&format!(r"(?si)</{tag}\s*>")).unwrap();
    re_close.replace_all(&result, "").into_owned()
}

/// Filter the attribute string of a single opening tag, returning only
/// those attributes that are in `allowed_attrs` and that pass security
/// checks.
fn filter_attributes(attrs_raw: &str, allowed_attrs: &[&str]) -> String {
    let mut filtered = Vec::new();

    for cap in RE_ATTR.captures_iter(attrs_raw) {
        let attr_name = cap.get(1).map_or("", |m| m.as_str()).to_ascii_lowercase();

        // Skip the self-closing slash captured as an "attribute".
        if attr_name == "/" {
            continue;
        }

        // ALWAYS strip event-handler attributes.
        if RE_EVENT_HANDLER.is_match(&attr_name) {
            continue;
        }

        // Must be in the per-tag allow-list.
        if !allowed_attrs.contains(&attr_name.as_str()) {
            continue;
        }

        // Extract the value (could be from group 2, 3, or 4).
        let raw_value = cap
            .get(2)
            .or_else(|| cap.get(3))
            .or_else(|| cap.get(4))
            .map(|m| m.as_str());

        match raw_value {
            Some(value) => {
                // Validate URL attributes.
                if URL_ATTRS.contains(&attr_name.as_str())
                    && (RE_BAD_PROTOCOL.is_match(value) || is_bad_data_uri(value))
                {
                    continue;
                }
                // Validate style attributes.
                if attr_name == "style" && RE_BAD_CSS.is_match(value) {
                    continue;
                }
                // Encode the value and emit.
                let safe_value = sanitize_attr_value(value);
                filtered.push(format!("{}=\"{}\"", attr_name, safe_value));
            }
            None => {
                // Boolean attribute (e.g. `reversed`, `disabled`).
                filtered.push(attr_name);
            }
        }
    }

    filtered.join(" ")
}

/// Lightly sanitise an attribute value: make sure bare `"` and `<`/`>` are
/// entity-encoded so the value cannot break out of its quoting context.
fn sanitize_attr_value(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("&quot;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(ch),
        }
    }
    out
}

/// Encode characters in a URL that need escaping for safe embedding
/// inside an HTML attribute value.
fn encode_url_entities(url: &str) -> String {
    let mut out = String::with_capacity(url.len());
    let bytes = url.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        match bytes[i] {
            b'&' => {
                // Keep existing `&amp;`, `&#nnn;`, `&#xHH;` entities as-is.
                if (i + 1 < len && bytes[i + 1] == b'#')
                    || (i + 3 < len && &bytes[i + 1..i + 4] == b"amp")
                {
                    out.push('&');
                } else {
                    out.push_str("&amp;");
                }
            }
            b'"' => out.push_str("&quot;"),
            b'\'' => out.push_str("&#039;"),
            b'<' => out.push_str("&lt;"),
            b'>' => out.push_str("&gt;"),
            _ => out.push(bytes[i] as char),
        }
        i += 1;
    }
    out
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -- wp_kses: basic tag stripping --------------------------------------

    #[test]
    fn test_strip_script_tag() {
        let input = "<p>Hello</p><script>alert('xss')</script><p>World</p>";
        let result = wp_kses_post(input);
        assert!(!result.contains("<script"));
        assert!(!result.contains("alert"));
        assert!(result.contains("<p>Hello</p>"));
        assert!(result.contains("<p>World</p>"));
    }

    #[test]
    fn test_strip_iframe_tag() {
        let input = "<p>Safe</p><iframe src=\"evil.com\"></iframe>";
        let result = wp_kses_post(input);
        assert!(!result.contains("<iframe"));
        assert!(!result.contains("evil.com"));
        assert!(result.contains("<p>Safe</p>"));
    }

    #[test]
    fn test_strip_style_tag_with_content() {
        let input = "<p>text</p><style>body{display:none}</style><p>more</p>";
        let result = wp_kses_post(input);
        assert!(!result.contains("<style"));
        assert!(!result.contains("display:none"));
        assert!(result.contains("<p>text</p>"));
    }

    #[test]
    fn test_strip_object_embed_form_tags() {
        let input =
            "<object data=\"x\"></object><embed src=\"y\"><form action=\"z\"><input></form>";
        let result = wp_kses_post(input);
        assert!(!result.contains("<object"));
        assert!(!result.contains("<embed"));
        assert!(!result.contains("<form"));
        assert!(!result.contains("<input"));
    }

    // -- Event handler removal ---------------------------------------------

    #[test]
    fn test_strip_onclick_handler() {
        let input = "<a href=\"safe.html\" onclick=\"evil()\">link</a>";
        let result = wp_kses_post(input);
        assert!(result.contains("href=\"safe.html\""));
        assert!(!result.contains("onclick"));
        assert!(!result.contains("evil()"));
    }

    #[test]
    fn test_strip_onerror_handler() {
        let input = "<img src=\"pic.jpg\" onerror=\"alert(1)\">";
        let result = wp_kses_post(input);
        assert!(result.contains("src=\"pic.jpg\""));
        assert!(!result.contains("onerror"));
    }

    #[test]
    fn test_strip_onload_handler() {
        let input = "<img src=\"pic.jpg\" onload=\"hack()\">";
        let result = wp_kses_post(input);
        assert!(!result.contains("onload"));
    }

    #[test]
    fn test_strip_onmouseover_handler() {
        let input = "<div onmouseover=\"steal()\">hover me</div>";
        let result = wp_kses_post(input);
        assert!(!result.contains("onmouseover"));
    }

    #[test]
    fn test_strip_onfocus_handler() {
        let input = "<a href=\"safe.html\" onfocus=\"evil()\">link</a>";
        let result = wp_kses_post(input);
        assert!(!result.contains("onfocus"));
    }

    // -- javascript: URL stripping -----------------------------------------

    #[test]
    fn test_strip_javascript_href() {
        let input = "<a href=\"javascript:alert(1)\">click</a>";
        let result = wp_kses_post(input);
        assert!(!result.contains("javascript:"));
    }

    #[test]
    fn test_strip_javascript_img_src() {
        let input = "<img src=\"javascript:evil()\">";
        let result = wp_kses_post(input);
        assert!(!result.contains("javascript:"));
    }

    #[test]
    fn test_strip_vbscript_href() {
        let input = "<a href=\"vbscript:MsgBox\">click</a>";
        let result = wp_kses_post(input);
        assert!(!result.contains("vbscript:"));
    }

    // -- data: URI handling ------------------------------------------------

    #[test]
    fn test_strip_data_uri_non_image() {
        let input = "<a href=\"data:text/html,<script>alert(1)</script>\">click</a>";
        let result = wp_kses_post(input);
        assert!(!result.contains("data:text"));
    }

    #[test]
    fn test_allow_data_image_uri() {
        let input = "<img src=\"data:image/png;base64,abc123\">";
        let result = wp_kses_post(input);
        assert!(result.contains("data:image/png"));
    }

    // -- Allowed tags/attrs preserved, disallowed stripped ------------------

    #[test]
    fn test_allowed_tags_preserved() {
        let input = "<p class=\"intro\">Hello <strong>world</strong></p>";
        let result = wp_kses_post(input);
        assert_eq!(
            result,
            "<p class=\"intro\">Hello <strong>world</strong></p>"
        );
    }

    #[test]
    fn test_allowed_img_attributes() {
        let input =
            "<img src=\"photo.jpg\" alt=\"photo\" width=\"100\" height=\"50\" loading=\"lazy\">";
        let result = wp_kses_post(input);
        assert!(result.contains("src=\"photo.jpg\""));
        assert!(result.contains("alt=\"photo\""));
        assert!(result.contains("width=\"100\""));
        assert!(result.contains("height=\"50\""));
        assert!(result.contains("loading=\"lazy\""));
    }

    #[test]
    fn test_disallowed_attributes_stripped() {
        // `data-custom` is not in the allow-list for <p>
        let input = "<p class=\"ok\" data-custom=\"bad\">text</p>";
        let result = wp_kses_post(input);
        assert!(result.contains("class=\"ok\""));
        assert!(!result.contains("data-custom"));
    }

    #[test]
    fn test_unknown_tag_stripped() {
        let input = "<p>safe</p><blink>bad</blink><p>ok</p>";
        let result = wp_kses_post(input);
        assert!(!result.contains("<blink"));
        assert!(result.contains("bad")); // text content preserved
        assert!(result.contains("<p>safe</p>"));
    }

    // -- Nested dangerous content ------------------------------------------

    #[test]
    fn test_nested_script_in_div() {
        let input = "<div class=\"wrapper\"><script>alert('xss')</script></div>";
        let result = wp_kses_post(input);
        assert!(!result.contains("<script"));
        assert!(!result.contains("alert"));
        assert!(result.contains("<div class=\"wrapper\">"));
    }

    #[test]
    fn test_nested_event_handler_in_nested_tags() {
        let input = "<div><p><a href=\"ok.html\" onclick=\"bad()\">link</a></p></div>";
        let result = wp_kses_post(input);
        assert!(!result.contains("onclick"));
        assert!(result.contains("<a href=\"ok.html\">"));
    }

    // -- wp_kses_post allows standard HTML but strips scripts --------------

    #[test]
    fn test_kses_post_allows_headings() {
        let input = "<h1 class=\"title\">Title</h1><h2 id=\"sub\">Subtitle</h2>";
        let result = wp_kses_post(input);
        assert!(result.contains("<h1 class=\"title\">"));
        assert!(result.contains("<h2 id=\"sub\">"));
    }

    #[test]
    fn test_kses_post_allows_table() {
        let input = "<table class=\"data\"><thead><tr><th>A</th></tr></thead><tbody><tr><td>1</td></tr></tbody></table>";
        let result = wp_kses_post(input);
        assert!(result.contains("<table"));
        assert!(result.contains("<th>"));
        assert!(result.contains("<td>"));
    }

    #[test]
    fn test_kses_post_allows_lists() {
        let input = "<ul class=\"items\"><li>one</li><li>two</li></ul>";
        let result = wp_kses_post(input);
        assert!(result.contains("<ul class=\"items\">"));
        assert!(result.contains("<li>one</li>"));
    }

    #[test]
    fn test_kses_post_strips_script_preserves_rest() {
        let input = "<p>Hello</p><script>document.write('hack')</script><p>World</p>";
        let result = wp_kses_post(input);
        assert_eq!(result, "<p>Hello</p><p>World</p>");
    }

    // -- wp_kses_comment is more restrictive --------------------------------

    #[test]
    fn test_kses_comment_strips_img() {
        let input = "<p>text</p><img src=\"pic.jpg\">";
        let result = wp_kses_comment(input);
        assert!(!result.contains("<img"));
        assert!(result.contains("<p>text</p>"));
    }

    #[test]
    fn test_kses_comment_strips_div() {
        let input = "<div class=\"evil\">content</div>";
        let result = wp_kses_comment(input);
        assert!(!result.contains("<div"));
        assert!(result.contains("content"));
    }

    #[test]
    fn test_kses_comment_allows_basic_formatting() {
        let input = "<p>Hello <strong>world</strong> and <em>universe</em></p>";
        let result = wp_kses_comment(input);
        assert!(result.contains("<p>"));
        assert!(result.contains("<strong>"));
        assert!(result.contains("<em>"));
    }

    #[test]
    fn test_kses_comment_strips_class_from_p() {
        // In comment mode, <p> has no allowed attributes.
        let input = "<p class=\"fancy\">text</p>";
        let result = wp_kses_comment(input);
        assert!(result.contains("<p>"));
        assert!(!result.contains("class"));
    }

    #[test]
    fn test_kses_comment_allows_a_href() {
        let input = "<a href=\"https://example.com\" title=\"Example\" rel=\"nofollow\">link</a>";
        let result = wp_kses_comment(input);
        assert!(result.contains("href=\"https://example.com\""));
        assert!(result.contains("title=\"Example\""));
        assert!(result.contains("rel=\"nofollow\""));
    }

    #[test]
    fn test_kses_comment_strips_a_target() {
        // target is not allowed in comment mode for <a>.
        let input = "<a href=\"https://example.com\" target=\"_blank\">link</a>";
        let result = wp_kses_comment(input);
        assert!(result.contains("href"));
        assert!(!result.contains("target"));
    }

    // -- wp_kses_data strips ALL HTML --------------------------------------

    #[test]
    fn test_kses_data_strips_all() {
        let input = "<p>Hello <strong>world</strong></p>";
        let result = wp_kses_data(input);
        assert_eq!(result, "Hello world");
    }

    #[test]
    fn test_kses_data_strips_script_content() {
        let input = "safe<script>alert('xss')</script>text";
        let result = wp_kses_data(input);
        assert!(!result.contains("alert"));
        assert!(result.contains("safe"));
        assert!(result.contains("text"));
    }

    // -- esc_html encodes entities -----------------------------------------

    #[test]
    fn test_esc_html_basic() {
        assert_eq!(
            esc_html("<p>Hello & \"World\"</p>"),
            "&lt;p&gt;Hello &amp; &quot;World&quot;&lt;/p&gt;"
        );
    }

    #[test]
    fn test_esc_html_single_quotes() {
        assert_eq!(esc_html("it's"), "it&#039;s");
    }

    #[test]
    fn test_esc_html_no_special_chars() {
        assert_eq!(esc_html("plain text"), "plain text");
    }

    // -- esc_attr ----------------------------------------------------------

    #[test]
    fn test_esc_attr_encodes_quotes() {
        assert_eq!(esc_attr("hello \"world\""), "hello &quot;world&quot;");
    }

    // -- esc_url -----------------------------------------------------------

    #[test]
    fn test_esc_url_allows_http() {
        let result = esc_url("http://example.com/page?a=1&b=2");
        assert!(result.starts_with("http://example.com/page"));
    }

    #[test]
    fn test_esc_url_allows_https() {
        let result = esc_url("https://example.com");
        assert!(result.contains("https://example.com"));
    }

    #[test]
    fn test_esc_url_allows_mailto() {
        let result = esc_url("mailto:user@example.com");
        assert!(result.contains("mailto:user@example.com"));
    }

    #[test]
    fn test_esc_url_allows_tel() {
        let result = esc_url("tel:+1234567890");
        assert!(result.contains("tel:+1234567890"));
    }

    #[test]
    fn test_esc_url_allows_fragment() {
        let result = esc_url("#section");
        assert_eq!(result, "#section");
    }

    #[test]
    fn test_esc_url_strips_javascript() {
        let result = esc_url("javascript:alert(1)");
        assert_eq!(result, "");
    }

    #[test]
    fn test_esc_url_strips_vbscript() {
        let result = esc_url("vbscript:MsgBox");
        assert_eq!(result, "");
    }

    #[test]
    fn test_esc_url_strips_data_non_image() {
        let result = esc_url("data:text/html,<script>alert(1)</script>");
        assert_eq!(result, "");
    }

    #[test]
    fn test_esc_url_allows_data_image() {
        let result = esc_url("data:image/png;base64,abc");
        assert!(!result.is_empty());
        assert!(result.contains("data:image/png"));
    }

    #[test]
    fn test_esc_url_rejects_unknown_scheme() {
        let result = esc_url("ftp://example.com");
        assert_eq!(result, "");
    }

    #[test]
    fn test_esc_url_empty_input() {
        assert_eq!(esc_url(""), "");
        assert_eq!(esc_url("   "), "");
    }

    // -- Style attribute with dangerous CSS ---------------------------------

    #[test]
    fn test_strip_expression_in_style() {
        let input = "<div style=\"width:expression(alert(1))\">content</div>";
        let result = wp_kses_post(input);
        assert!(!result.contains("expression"));
        // The tag is preserved but the dangerous style attribute is stripped.
        assert!(result.contains("<div>"));
    }

    #[test]
    fn test_strip_javascript_url_in_style() {
        let input = "<div style=\"background:url(javascript:alert(1))\">content</div>";
        let result = wp_kses_post(input);
        assert!(!result.contains("javascript"));
    }

    #[test]
    fn test_allow_safe_style() {
        let input = "<div style=\"color: red; margin: 10px;\">content</div>";
        let result = wp_kses_post(input);
        assert!(result.contains("style=\"color: red; margin: 10px;\""));
    }

    // -- Case insensitivity -------------------------------------------------

    #[test]
    fn test_case_insensitive_script_tag() {
        let input = "<SCRIPT>alert('xss')</SCRIPT>";
        let result = wp_kses_post(input);
        assert!(!result.contains("alert"));
        assert!(!result.to_lowercase().contains("<script"));
    }

    #[test]
    fn test_case_insensitive_event_handler() {
        let input = "<a href=\"safe.html\" ONCLICK=\"evil()\">link</a>";
        let result = wp_kses_post(input);
        assert!(!result.to_lowercase().contains("onclick"));
    }

    #[test]
    fn test_case_insensitive_javascript_protocol() {
        let input = "<a href=\"JavaScript:alert(1)\">click</a>";
        let result = wp_kses_post(input);
        assert!(!result.to_lowercase().contains("javascript"));
    }

    // -- Self-closing tags --------------------------------------------------

    #[test]
    fn test_self_closing_br() {
        let input = "<p>line1<br/>line2</p>";
        let result = wp_kses_post(input);
        assert!(result.contains("<br"));
        assert!(result.contains("<p>"));
    }

    #[test]
    fn test_self_closing_img() {
        let input = "<img src=\"photo.jpg\" alt=\"pic\" />";
        let result = wp_kses_post(input);
        assert!(result.contains("src=\"photo.jpg\""));
        assert!(result.contains("alt=\"pic\""));
    }

    // -- Custom allowed_html map -------------------------------------------

    #[test]
    fn test_custom_allowed_html() {
        let mut allowed: AllowedHtml = HashMap::new();
        allowed.insert("b", [].as_slice());
        allowed.insert("i", [].as_slice());

        let input = "<b>bold</b> <i>italic</i> <u>underline</u> <em>emphasis</em>";
        let result = wp_kses(input, &allowed);
        assert!(result.contains("<b>bold</b>"));
        assert!(result.contains("<i>italic</i>"));
        assert!(!result.contains("<u>"));
        assert!(!result.contains("<em>"));
        // Text content of stripped tags is preserved.
        assert!(result.contains("underline"));
        assert!(result.contains("emphasis"));
    }

    #[test]
    fn test_empty_allowed_html() {
        let allowed: AllowedHtml = HashMap::new();
        let input = "<p>Hello <b>World</b></p>";
        let result = wp_kses(input, &allowed);
        assert_eq!(result, "Hello World");
    }

    // -- Edge cases --------------------------------------------------------

    #[test]
    fn test_plain_text_unchanged() {
        let input = "Just plain text with no HTML at all.";
        let result = wp_kses_post(input);
        assert_eq!(result, input);
    }

    #[test]
    fn test_entities_in_text_preserved() {
        let input = "<p>5 &gt; 3 &amp; 2 &lt; 4</p>";
        let result = wp_kses_post(input);
        assert!(result.contains("&gt;"));
        assert!(result.contains("&amp;"));
        assert!(result.contains("&lt;"));
    }

    #[test]
    fn test_multiple_scripts_stripped() {
        let input = "<script>a()</script>safe<script>b()</script>";
        let result = wp_kses_post(input);
        assert!(!result.contains("<script"));
        assert!(!result.contains("a()"));
        assert!(!result.contains("b()"));
        assert!(result.contains("safe"));
    }

    #[test]
    fn test_deeply_nested_structure() {
        let input = "<div><p><strong><em>deep</em></strong></p></div>";
        let result = wp_kses_post(input);
        assert!(result.contains("<div>"));
        assert!(result.contains("<p>"));
        assert!(result.contains("<strong>"));
        assert!(result.contains("<em>"));
        assert!(result.contains("deep"));
    }

    #[test]
    fn test_mixed_safe_and_dangerous() {
        let input = concat!(
            "<h1>Title</h1>",
            "<script>alert('xss')</script>",
            "<p>Paragraph with <a href=\"https://safe.com\">link</a></p>",
            "<iframe src=\"evil.com\"></iframe>",
            "<img src=\"photo.jpg\" onerror=\"hack()\">",
        );
        let result = wp_kses_post(input);
        assert!(result.contains("<h1>Title</h1>"));
        assert!(!result.contains("<script"));
        assert!(!result.contains("alert"));
        assert!(result.contains("<a href=\"https://safe.com\">link</a>"));
        assert!(!result.contains("<iframe"));
        assert!(result.contains("src=\"photo.jpg\""));
        assert!(!result.contains("onerror"));
    }
}
