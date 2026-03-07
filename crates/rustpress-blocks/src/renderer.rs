use crate::parser::Block;
use crate::registry::BlockRegistry;

/// Block renderer that uses a registry to determine how to render each block.
pub struct BlockRenderer {
    registry: BlockRegistry,
}

impl BlockRenderer {
    /// Create a new BlockRenderer with the given registry.
    pub fn new(registry: BlockRegistry) -> Self {
        BlockRenderer { registry }
    }

    /// Get a reference to the underlying block registry.
    pub fn registry(&self) -> &BlockRegistry {
        &self.registry
    }

    /// Render a single block to HTML.
    ///
    /// For static blocks (no render_callback), returns the inner_html as-is.
    /// For dynamic blocks, invokes the render_callback.
    /// For blocks with inner_blocks, renders them recursively.
    /// Unknown blocks are rendered using their inner_html.
    pub fn render_block(&self, block: &Block) -> String {
        // Freeform blocks just return their content
        if block.name == "core/freeform" {
            return block.inner_html.clone();
        }

        // Look up the block type in the registry
        if let Some(block_type) = self.registry.get_block_type(&block.name) {
            if let Some(ref callback) = block_type.render_callback {
                // Dynamic block: use render callback
                // For blocks with inner_blocks, the callback is responsible
                // for rendering children if needed
                return callback(block);
            }
        }

        // Static block or unknown block: check if it has inner blocks
        if !block.inner_blocks.is_empty() {
            // For container blocks (columns, group, etc.), render inner blocks
            // but preserve any wrapper HTML from the block itself
            return self.render_container_block(block);
        }

        // Simple static block: return inner_html as-is
        block.inner_html.clone()
    }

    /// Render all blocks in a slice, concatenating the results.
    pub fn render_blocks(&self, blocks: &[Block]) -> String {
        let mut output = String::new();
        for block in blocks {
            output.push_str(&self.render_block(block));
        }
        output
    }

    /// Render a container block that has inner blocks.
    ///
    /// This extracts any wrapper HTML and renders inner blocks between them.
    /// For example, a columns block wraps its column children in a div.
    fn render_container_block(&self, block: &Block) -> String {
        // Render inner blocks
        let inner_rendered = self.render_blocks(&block.inner_blocks);

        // For container blocks, we try to use the inner_html structure
        // but replace the inner block content with rendered versions.
        // As a fallback, just return rendered inner blocks.
        //
        // WordPress container blocks typically have wrapper elements.
        // We detect them by looking for common patterns.
        match block.name.as_str() {
            "core/columns" => {
                let class = extract_class_attr(block, "wp-block-columns");
                format!("<div class=\"{class}\">{inner_rendered}</div>")
            }
            "core/column" => {
                let class = extract_class_attr(block, "wp-block-column");
                format!("<div class=\"{class}\">{inner_rendered}</div>")
            }
            "core/group" => {
                let class = extract_class_attr(block, "wp-block-group");
                format!("<div class=\"{class}\">{inner_rendered}</div>")
            }
            "core/row" => {
                let class = extract_class_attr(block, "is-layout-flex");
                format!(
                    "<div class=\"wp-block-group {class}\">{inner_rendered}</div>"
                )
            }
            "core/stack" => {
                let class = extract_class_attr(block, "is-layout-flex");
                format!(
                    "<div class=\"wp-block-group {class}\">{inner_rendered}</div>"
                )
            }
            "core/buttons" => {
                format!("<div class=\"wp-block-buttons\">{inner_rendered}</div>")
            }
            "core/query" | "core/query-loop" => inner_rendered,
            _ => {
                // Unknown container: just render inner blocks
                inner_rendered
            }
        }
    }
}

/// Extract a CSS class from block attributes, with a default base class.
fn extract_class_attr(block: &Block, default_class: &str) -> String {
    if let Some(class_name) = block.attrs.get("className").and_then(|v| v.as_str()) {
        format!("{default_class} {class_name}")
    } else {
        default_class.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core_blocks::register_core_blocks;
    use crate::parser::parse_blocks;

    fn make_renderer() -> BlockRenderer {
        let mut registry = BlockRegistry::new();
        register_core_blocks(&mut registry);
        BlockRenderer::new(registry)
    }

    #[test]
    fn test_render_simple_paragraph() {
        let renderer = make_renderer();
        let blocks =
            parse_blocks(r#"<!-- wp:paragraph --><p>Hello world</p><!-- /wp:paragraph -->"#);
        let html = renderer.render_blocks(&blocks);
        assert_eq!(html, "<p>Hello world</p>");
    }

    #[test]
    fn test_render_multiple_blocks() {
        let renderer = make_renderer();
        let blocks = parse_blocks(
            r#"<!-- wp:heading --><h2>Title</h2><!-- /wp:heading --><!-- wp:paragraph --><p>Content</p><!-- /wp:paragraph -->"#,
        );
        let html = renderer.render_blocks(&blocks);
        assert_eq!(html, "<h2>Title</h2><p>Content</p>");
    }

    #[test]
    fn test_render_unknown_block() {
        let renderer = make_renderer();
        let block = Block {
            name: "unknown-plugin/custom-block".to_string(),
            attrs: serde_json::Value::Object(serde_json::Map::new()),
            inner_html: "<div>Custom content</div>".to_string(),
            inner_blocks: vec![],
        };
        let html = renderer.render_block(&block);
        assert_eq!(html, "<div>Custom content</div>");
    }

    #[test]
    fn test_render_freeform_block() {
        let renderer = make_renderer();
        let block = Block {
            name: "core/freeform".to_string(),
            attrs: serde_json::Value::Object(serde_json::Map::new()),
            inner_html: "<p>Classic editor content</p>".to_string(),
            inner_blocks: vec![],
        };
        let html = renderer.render_block(&block);
        assert_eq!(html, "<p>Classic editor content</p>");
    }

    #[test]
    fn test_render_dynamic_block() {
        let renderer = make_renderer();
        let block = Block {
            name: "core/archives".to_string(),
            attrs: serde_json::Value::Object(serde_json::Map::new()),
            inner_html: String::new(),
            inner_blocks: vec![],
        };
        let html = renderer.render_block(&block);
        // Dynamic blocks should produce some HTML via their callback
        assert!(html.contains("wp-block-archives"));
    }

    #[test]
    fn test_render_self_closing_spacer() {
        let renderer = make_renderer();
        let blocks = parse_blocks(r#"<!-- wp:spacer {"height":"50px"} /-->"#);
        let html = renderer.render_blocks(&blocks);
        assert!(html.contains("50px"));
    }
}
