use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Represents a parsed Gutenberg block.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Block {
    /// Block name, e.g. "core/paragraph", "core/heading"
    pub name: String,
    /// Block attributes parsed from the JSON in the block comment
    pub attrs: Value,
    /// The inner HTML content of the block (between opening and closing comments)
    pub inner_html: String,
    /// Nested blocks (e.g. columns contain column blocks)
    pub inner_blocks: Vec<Block>,
}

impl Block {
    /// Create a new block with the given name and default empty values.
    pub fn new(name: &str) -> Self {
        Block {
            name: name.to_string(),
            attrs: Value::Object(serde_json::Map::new()),
            inner_html: String::new(),
            inner_blocks: Vec::new(),
        }
    }

    /// Check if this block is a self-closing block (no inner content).
    pub fn is_self_closing(&self) -> bool {
        self.inner_html.is_empty() && self.inner_blocks.is_empty()
    }
}

/// Parse WordPress Gutenberg block content into a vector of Block structs.
///
/// Handles:
/// - Standard blocks: `<!-- wp:name {"attr":"val"} --><p>content</p><!-- /wp:name -->`
/// - Self-closing blocks: `<!-- wp:spacer {"height":"50px"} /-->`
/// - Nested blocks (e.g. columns > column > paragraph)
/// - Freeform content outside of blocks (returned as "core/freeform" blocks)
pub fn parse_blocks(content: &str) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut pos = 0;
    let input = content;

    // Regex for block opener: <!-- wp:namespace/name {"json":"attrs"} -->
    // or self-closing: <!-- wp:namespace/name {"json":"attrs"} /-->
    let opener_re = Regex::new(
        r#"<!--\s+wp:([a-z][a-z0-9-]*/)?([a-z][a-z0-9-]*)\s*(\{[^}]*\})?\s*(/)?-->"#
    ).expect("Invalid opener regex");

    while pos < input.len() {
        // Find next block comment
        if let Some(m) = opener_re.find(&input[pos..]) {
            let match_start = pos + m.start();
            let match_end = pos + m.end();

            // If there's freeform content before this block, capture it
            if match_start > pos {
                let freeform = input[pos..match_start].trim();
                if !freeform.is_empty() {
                    blocks.push(Block {
                        name: "core/freeform".to_string(),
                        attrs: Value::Object(serde_json::Map::new()),
                        inner_html: freeform.to_string(),
                        inner_blocks: Vec::new(),
                    });
                }
            }

            // Parse the captures
            let caps = opener_re.captures(&input[pos..]).unwrap();
            let namespace = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            let block_name_short = caps.get(2).map(|m| m.as_str()).unwrap_or("");
            let attrs_str = caps.get(3).map(|m| m.as_str());
            let is_self_closing = caps.get(4).is_some();

            // Build full block name
            let full_name = if namespace.is_empty() {
                format!("core/{}", block_name_short)
            } else {
                // namespace includes trailing slash, e.g. "core/"
                format!("{}{}", namespace, block_name_short)
            };

            // Parse attributes JSON
            let attrs = if let Some(json_str) = attrs_str {
                serde_json::from_str(json_str).unwrap_or(Value::Object(serde_json::Map::new()))
            } else {
                Value::Object(serde_json::Map::new())
            };

            if is_self_closing {
                blocks.push(Block {
                    name: full_name,
                    attrs,
                    inner_html: String::new(),
                    inner_blocks: Vec::new(),
                });
                pos = match_end;
            } else {
                // Find the matching closing tag
                let closing_tag = format!("<!-- /wp:{}{} -->", namespace, block_name_short);
                if let Some(close_result) = find_matching_close(
                    &input[match_end..],
                    namespace,
                    block_name_short,
                ) {
                    let inner_content = &input[match_end..match_end + close_result.inner_end];
                    let inner_blocks = parse_blocks(inner_content);

                    // Determine inner_html: if there are inner blocks, we still store
                    // the raw HTML. For static blocks this is the content itself.
                    let inner_html = if inner_blocks.is_empty()
                        || inner_blocks.iter().all(|b| b.name == "core/freeform")
                    {
                        inner_content.to_string()
                    } else {
                        inner_content.to_string()
                    };

                    blocks.push(Block {
                        name: full_name,
                        attrs,
                        inner_html,
                        inner_blocks,
                    });

                    pos = match_end + close_result.total_end;
                } else {
                    // No closing tag found - treat as self-closing
                    tracing::warn!(
                        "No closing tag found for block '{}', treating as self-closing",
                        full_name
                    );
                    blocks.push(Block {
                        name: full_name,
                        attrs,
                        inner_html: String::new(),
                        inner_blocks: Vec::new(),
                    });
                    pos = match_end;
                }
            }
        } else {
            // No more block comments; rest is freeform
            let remaining = input[pos..].trim();
            if !remaining.is_empty() {
                blocks.push(Block {
                    name: "core/freeform".to_string(),
                    attrs: Value::Object(serde_json::Map::new()),
                    inner_html: remaining.to_string(),
                    inner_blocks: Vec::new(),
                });
            }
            break;
        }
    }

    blocks
}

/// Result of finding a matching close tag.
struct CloseResult {
    /// Position of the end of inner content (start of closing comment).
    inner_end: usize,
    /// Position past the end of the closing comment.
    total_end: usize,
}

/// Find the matching closing block comment, handling nested blocks of the same type.
fn find_matching_close(content: &str, namespace: &str, block_name: &str) -> Option<CloseResult> {
    let open_pattern = format!(
        r#"<!--\s+wp:{}{}\s*(?:\{{[^}}]*\}})?\s*-->"#,
        regex::escape(namespace),
        regex::escape(block_name)
    );
    let close_pattern = format!(
        r#"<!--\s+/wp:{}{}\s*-->"#,
        regex::escape(namespace),
        regex::escape(block_name)
    );

    let open_re = Regex::new(&open_pattern).ok()?;
    let close_re = Regex::new(&close_pattern).ok()?;

    let mut depth = 1;
    let mut search_pos = 0;

    while search_pos < content.len() {
        // Find next open or close
        let next_open = open_re.find(&content[search_pos..]);
        let next_close = close_re.find(&content[search_pos..]);

        match (next_open, next_close) {
            (Some(o), Some(c)) => {
                if o.start() < c.start() {
                    // Found nested open before close
                    depth += 1;
                    search_pos += o.end();
                } else {
                    // Found close
                    depth -= 1;
                    if depth == 0 {
                        let inner_end = search_pos + c.start();
                        let total_end = search_pos + c.end();
                        return Some(CloseResult {
                            inner_end,
                            total_end,
                        });
                    }
                    search_pos += c.end();
                }
            }
            (None, Some(c)) => {
                depth -= 1;
                if depth == 0 {
                    let inner_end = search_pos + c.start();
                    let total_end = search_pos + c.end();
                    return Some(CloseResult {
                        inner_end,
                        total_end,
                    });
                }
                search_pos += c.end();
            }
            _ => break,
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_paragraph() {
        let content = r#"<!-- wp:paragraph --><p>Hello world</p><!-- /wp:paragraph -->"#;
        let blocks = parse_blocks(content);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].name, "core/paragraph");
        assert_eq!(blocks[0].inner_html, "<p>Hello world</p>");
        assert!(blocks[0].inner_blocks.is_empty());
    }

    #[test]
    fn test_parse_block_with_attributes() {
        let content =
            r#"<!-- wp:paragraph {"align":"center"} --><p class="has-text-align-center">Centered</p><!-- /wp:paragraph -->"#;
        let blocks = parse_blocks(content);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].name, "core/paragraph");
        assert_eq!(blocks[0].attrs["align"], "center");
        assert!(blocks[0].inner_html.contains("Centered"));
    }

    #[test]
    fn test_parse_self_closing_block() {
        let content = r#"<!-- wp:spacer {"height":"50px"} /-->"#;
        let blocks = parse_blocks(content);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].name, "core/spacer");
        assert_eq!(blocks[0].attrs["height"], "50px");
        assert!(blocks[0].inner_html.is_empty());
        assert!(blocks[0].is_self_closing());
    }

    #[test]
    fn test_parse_multiple_blocks() {
        let content = r#"<!-- wp:heading --><h2>Title</h2><!-- /wp:heading --><!-- wp:paragraph --><p>Text</p><!-- /wp:paragraph -->"#;
        let blocks = parse_blocks(content);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].name, "core/heading");
        assert_eq!(blocks[1].name, "core/paragraph");
    }

    #[test]
    fn test_parse_nested_blocks() {
        let content = r#"<!-- wp:columns --><!-- wp:column --><p>Col 1</p><!-- /wp:column --><!-- wp:column --><p>Col 2</p><!-- /wp:column --><!-- /wp:columns -->"#;
        let blocks = parse_blocks(content);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].name, "core/columns");
        assert_eq!(blocks[0].inner_blocks.len(), 2);
        assert_eq!(blocks[0].inner_blocks[0].name, "core/column");
        assert_eq!(blocks[0].inner_blocks[1].name, "core/column");
    }

    #[test]
    fn test_parse_freeform_content() {
        let content = "<p>Just some plain HTML</p>";
        let blocks = parse_blocks(content);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].name, "core/freeform");
        assert_eq!(blocks[0].inner_html, "<p>Just some plain HTML</p>");
    }

    #[test]
    fn test_parse_mixed_content() {
        let content = r#"<p>Before</p><!-- wp:paragraph --><p>Inside</p><!-- /wp:paragraph -->"#;
        let blocks = parse_blocks(content);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].name, "core/freeform");
        assert_eq!(blocks[1].name, "core/paragraph");
    }

    #[test]
    fn test_parse_namespaced_block() {
        let content = r#"<!-- wp:my-plugin/custom-block {"foo":"bar"} --><div>Custom</div><!-- /wp:my-plugin/custom-block -->"#;
        let blocks = parse_blocks(content);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].name, "my-plugin/custom-block");
        assert_eq!(blocks[0].attrs["foo"], "bar");
    }

    #[test]
    fn test_parse_separator_self_closing() {
        let content = r#"<!-- wp:separator /-->"#;
        let blocks = parse_blocks(content);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].name, "core/separator");
        assert!(blocks[0].is_self_closing());
    }

    #[test]
    fn test_deeply_nested_blocks() {
        let content = r#"<!-- wp:columns --><!-- wp:column --><!-- wp:paragraph --><p>Deep</p><!-- /wp:paragraph --><!-- /wp:column --><!-- /wp:columns -->"#;
        let blocks = parse_blocks(content);
        assert_eq!(blocks.len(), 1);
        let col = &blocks[0].inner_blocks[0];
        assert_eq!(col.name, "core/column");
        assert_eq!(col.inner_blocks.len(), 1);
        assert_eq!(col.inner_blocks[0].name, "core/paragraph");
        assert!(col.inner_blocks[0].inner_html.contains("Deep"));
    }
}
