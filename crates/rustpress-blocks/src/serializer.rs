use crate::parser::Block;
use serde_json::Value;

/// Serialize a single block back to WordPress block comment format.
///
/// This produces output like:
/// - `<!-- wp:paragraph {"align":"center"} --><p>text</p><!-- /wp:paragraph -->`
/// - `<!-- wp:spacer {"height":"50px"} /-->`
pub fn serialize_block(block: &Block) -> String {
    // Freeform blocks are emitted as-is (no comment wrappers)
    if block.name == "core/freeform" {
        return block.inner_html.clone();
    }

    let comment_name = block_name_to_comment(&block.name);
    let attrs_str = serialize_attrs(&block.attrs);

    if block.is_self_closing() {
        // Self-closing block: <!-- wp:name {"attrs":"val"} /-->
        if attrs_str.is_empty() {
            format!("<!-- wp:{comment_name} /-->")
        } else {
            format!("<!-- wp:{comment_name} {attrs_str} /-->")
        }
    } else if !block.inner_blocks.is_empty() {
        // Block with inner blocks: serialize children recursively
        let inner = serialize_blocks(&block.inner_blocks);
        if attrs_str.is_empty() {
            format!("<!-- wp:{comment_name} -->{inner}<!-- /wp:{comment_name} -->")
        } else {
            format!("<!-- wp:{comment_name} {attrs_str} -->{inner}<!-- /wp:{comment_name} -->")
        }
    } else {
        // Standard block with inner_html
        if attrs_str.is_empty() {
            format!(
                "<!-- wp:{} -->{}<!-- /wp:{} -->",
                comment_name, block.inner_html, comment_name
            )
        } else {
            format!(
                "<!-- wp:{} {} -->{}<!-- /wp:{} -->",
                comment_name, attrs_str, block.inner_html, comment_name
            )
        }
    }
}

/// Serialize a slice of blocks back to WordPress block comment format.
pub fn serialize_blocks(blocks: &[Block]) -> String {
    let mut output = String::new();
    for block in blocks {
        output.push_str(&serialize_block(block));
    }
    output
}

/// Convert a full block name to the comment form.
/// "core/paragraph" -> "paragraph"
/// "my-plugin/custom-block" -> "my-plugin/custom-block"
fn block_name_to_comment(name: &str) -> &str {
    if let Some(stripped) = name.strip_prefix("core/") {
        stripped
    } else {
        name
    }
}

/// Serialize block attributes to a JSON string, or empty string if no attributes.
fn serialize_attrs(attrs: &Value) -> String {
    match attrs {
        Value::Object(map) if map.is_empty() => String::new(),
        Value::Null => String::new(),
        _ => serde_json::to_string(attrs).unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_blocks;

    #[test]
    fn test_serialize_simple_paragraph() {
        let block = Block {
            name: "core/paragraph".to_string(),
            attrs: Value::Object(serde_json::Map::new()),
            inner_html: "<p>Hello world</p>".to_string(),
            inner_blocks: vec![],
        };
        let result = serialize_block(&block);
        assert_eq!(
            result,
            "<!-- wp:paragraph --><p>Hello world</p><!-- /wp:paragraph -->"
        );
    }

    #[test]
    fn test_serialize_block_with_attributes() {
        let mut attrs = serde_json::Map::new();
        attrs.insert("align".to_string(), Value::String("center".to_string()));
        let block = Block {
            name: "core/paragraph".to_string(),
            attrs: Value::Object(attrs),
            inner_html: "<p>Centered</p>".to_string(),
            inner_blocks: vec![],
        };
        let result = serialize_block(&block);
        assert!(result.starts_with("<!-- wp:paragraph {"));
        assert!(result.contains("\"align\":\"center\""));
        assert!(result.ends_with("<p>Centered</p><!-- /wp:paragraph -->"));
    }

    #[test]
    fn test_serialize_self_closing_block() {
        let mut attrs = serde_json::Map::new();
        attrs.insert("height".to_string(), Value::String("50px".to_string()));
        let block = Block {
            name: "core/spacer".to_string(),
            attrs: Value::Object(attrs),
            inner_html: String::new(),
            inner_blocks: vec![],
        };
        let result = serialize_block(&block);
        assert!(result.starts_with("<!-- wp:spacer {"));
        assert!(result.contains("\"height\":\"50px\""));
        assert!(result.ends_with("/-->"));
    }

    #[test]
    fn test_serialize_self_closing_no_attrs() {
        let block = Block {
            name: "core/separator".to_string(),
            attrs: Value::Object(serde_json::Map::new()),
            inner_html: String::new(),
            inner_blocks: vec![],
        };
        let result = serialize_block(&block);
        assert_eq!(result, "<!-- wp:separator /-->");
    }

    #[test]
    fn test_serialize_freeform() {
        let block = Block {
            name: "core/freeform".to_string(),
            attrs: Value::Object(serde_json::Map::new()),
            inner_html: "<p>Classic content</p>".to_string(),
            inner_blocks: vec![],
        };
        let result = serialize_block(&block);
        assert_eq!(result, "<p>Classic content</p>");
    }

    #[test]
    fn test_serialize_namespaced_block() {
        let block = Block {
            name: "my-plugin/widget".to_string(),
            attrs: Value::Object(serde_json::Map::new()),
            inner_html: "<div>Widget</div>".to_string(),
            inner_blocks: vec![],
        };
        let result = serialize_block(&block);
        assert_eq!(
            result,
            "<!-- wp:my-plugin/widget --><div>Widget</div><!-- /wp:my-plugin/widget -->"
        );
    }

    #[test]
    fn test_serialize_multiple_blocks() {
        let blocks = vec![
            Block {
                name: "core/heading".to_string(),
                attrs: Value::Object(serde_json::Map::new()),
                inner_html: "<h2>Title</h2>".to_string(),
                inner_blocks: vec![],
            },
            Block {
                name: "core/paragraph".to_string(),
                attrs: Value::Object(serde_json::Map::new()),
                inner_html: "<p>Text</p>".to_string(),
                inner_blocks: vec![],
            },
        ];
        let result = serialize_blocks(&blocks);
        assert_eq!(
            result,
            "<!-- wp:heading --><h2>Title</h2><!-- /wp:heading --><!-- wp:paragraph --><p>Text</p><!-- /wp:paragraph -->"
        );
    }

    #[test]
    fn test_roundtrip_simple() {
        let original = "<!-- wp:paragraph --><p>Hello world</p><!-- /wp:paragraph -->";
        let blocks = parse_blocks(original);
        let serialized = serialize_blocks(&blocks);
        assert_eq!(serialized, original);
    }

    #[test]
    fn test_roundtrip_self_closing() {
        let original = r#"<!-- wp:separator /-->"#;
        let blocks = parse_blocks(original);
        let serialized = serialize_blocks(&blocks);
        assert_eq!(serialized, original);
    }

    #[test]
    fn test_roundtrip_multiple() {
        let original = "<!-- wp:heading --><h2>Title</h2><!-- /wp:heading --><!-- wp:paragraph --><p>Text</p><!-- /wp:paragraph -->";
        let blocks = parse_blocks(original);
        let serialized = serialize_blocks(&blocks);
        assert_eq!(serialized, original);
    }
}
