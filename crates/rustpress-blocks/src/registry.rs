use std::collections::HashMap;
use std::sync::Arc;

use crate::parser::Block;

/// Category for block types, matching WordPress block categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BlockCategory {
    Text,
    Media,
    Design,
    Widgets,
    Theme,
    Embed,
}

impl BlockCategory {
    /// Return the slug string for this category.
    pub fn slug(&self) -> &'static str {
        match self {
            BlockCategory::Text => "text",
            BlockCategory::Media => "media",
            BlockCategory::Design => "design",
            BlockCategory::Widgets => "widgets",
            BlockCategory::Theme => "theme",
            BlockCategory::Embed => "embed",
        }
    }

    /// Return the display title for this category.
    pub fn title(&self) -> &'static str {
        match self {
            BlockCategory::Text => "Text",
            BlockCategory::Media => "Media",
            BlockCategory::Design => "Design",
            BlockCategory::Widgets => "Widgets",
            BlockCategory::Theme => "Theme",
            BlockCategory::Embed => "Embeds",
        }
    }
}

/// A callback function that renders a dynamic block into HTML.
pub type RenderCallback = Arc<dyn Fn(&Block) -> String + Send + Sync>;

/// Represents a registered block type in the system.
pub struct BlockType {
    /// The block name, e.g. "core/paragraph".
    pub name: String,
    /// Human-readable title, e.g. "Paragraph".
    pub title: String,
    /// The category this block belongs to.
    pub category: BlockCategory,
    /// Icon identifier (dashicon name or SVG).
    pub icon: String,
    /// Map of supported features (e.g. "align" -> true).
    pub supports: HashMap<String, bool>,
    /// Optional render callback for dynamic blocks.
    /// Static blocks (paragraph, heading, etc.) typically have None here
    /// because their inner_html is used directly.
    pub render_callback: Option<RenderCallback>,
}

impl std::fmt::Debug for BlockType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BlockType")
            .field("name", &self.name)
            .field("title", &self.title)
            .field("category", &self.category)
            .field("icon", &self.icon)
            .field("supports", &self.supports)
            .field(
                "render_callback",
                &if self.render_callback.is_some() {
                    "Some(<fn>)"
                } else {
                    "None"
                },
            )
            .finish()
    }
}

impl BlockType {
    /// Create a new static block type (renders inner_html as-is).
    pub fn new_static(name: &str, title: &str, category: BlockCategory, icon: &str) -> Self {
        BlockType {
            name: name.to_string(),
            title: title.to_string(),
            category,
            icon: icon.to_string(),
            supports: HashMap::new(),
            render_callback: None,
        }
    }

    /// Create a new dynamic block type with a render callback.
    pub fn new_dynamic(
        name: &str,
        title: &str,
        category: BlockCategory,
        icon: &str,
        callback: RenderCallback,
    ) -> Self {
        BlockType {
            name: name.to_string(),
            title: title.to_string(),
            category,
            icon: icon.to_string(),
            supports: HashMap::new(),
            render_callback: Some(callback),
        }
    }

    /// Add a support flag to this block type. Returns self for chaining.
    pub fn with_support(mut self, feature: &str, enabled: bool) -> Self {
        self.supports.insert(feature.to_string(), enabled);
        self
    }

    /// Check if this block type is dynamic (has a render callback).
    pub fn is_dynamic(&self) -> bool {
        self.render_callback.is_some()
    }
}

/// Registry of all known block types.
#[derive(Default)]
pub struct BlockRegistry {
    block_types: HashMap<String, BlockType>,
}

impl BlockRegistry {
    /// Create a new empty block registry.
    pub fn new() -> Self {
        BlockRegistry {
            block_types: HashMap::new(),
        }
    }

    /// Register a block type. If a block with the same name already exists,
    /// it will be replaced.
    pub fn register_block_type(&mut self, block_type: BlockType) {
        tracing::debug!("Registering block type: {}", block_type.name);
        self.block_types
            .insert(block_type.name.clone(), block_type);
    }

    /// Get a registered block type by name.
    pub fn get_block_type(&self, name: &str) -> Option<&BlockType> {
        self.block_types.get(name)
    }

    /// Get all registered block types.
    pub fn get_all_block_types(&self) -> Vec<&BlockType> {
        self.block_types.values().collect()
    }

    /// Get all block types in a given category.
    pub fn get_block_types_by_category(&self, category: BlockCategory) -> Vec<&BlockType> {
        self.block_types
            .values()
            .filter(|bt| bt.category == category)
            .collect()
    }

    /// Check if a block type is registered.
    pub fn has_block_type(&self, name: &str) -> bool {
        self.block_types.contains_key(name)
    }

    /// Remove a block type from the registry.
    pub fn unregister_block_type(&mut self, name: &str) -> Option<BlockType> {
        self.block_types.remove(name)
    }

    /// Get the total number of registered block types.
    pub fn count(&self) -> usize {
        self.block_types.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_and_get_block_type() {
        let mut registry = BlockRegistry::new();
        registry.register_block_type(BlockType::new_static(
            "core/paragraph",
            "Paragraph",
            BlockCategory::Text,
            "editor-paragraph",
        ));

        let bt = registry.get_block_type("core/paragraph").unwrap();
        assert_eq!(bt.name, "core/paragraph");
        assert_eq!(bt.title, "Paragraph");
        assert_eq!(bt.category, BlockCategory::Text);
        assert!(!bt.is_dynamic());
    }

    #[test]
    fn test_register_dynamic_block() {
        let mut registry = BlockRegistry::new();
        let callback: RenderCallback =
            Arc::new(|_block: &Block| "<ul><li>Post 1</li></ul>".to_string());
        registry.register_block_type(BlockType::new_dynamic(
            "core/latest-posts",
            "Latest Posts",
            BlockCategory::Widgets,
            "list-view",
            callback,
        ));

        let bt = registry.get_block_type("core/latest-posts").unwrap();
        assert!(bt.is_dynamic());
    }

    #[test]
    fn test_get_block_types_by_category() {
        let mut registry = BlockRegistry::new();
        registry.register_block_type(BlockType::new_static(
            "core/paragraph",
            "Paragraph",
            BlockCategory::Text,
            "editor-paragraph",
        ));
        registry.register_block_type(BlockType::new_static(
            "core/heading",
            "Heading",
            BlockCategory::Text,
            "heading",
        ));
        registry.register_block_type(BlockType::new_static(
            "core/image",
            "Image",
            BlockCategory::Media,
            "format-image",
        ));

        let text_blocks = registry.get_block_types_by_category(BlockCategory::Text);
        assert_eq!(text_blocks.len(), 2);

        let media_blocks = registry.get_block_types_by_category(BlockCategory::Media);
        assert_eq!(media_blocks.len(), 1);
    }

    #[test]
    fn test_unregister_block_type() {
        let mut registry = BlockRegistry::new();
        registry.register_block_type(BlockType::new_static(
            "core/paragraph",
            "Paragraph",
            BlockCategory::Text,
            "editor-paragraph",
        ));
        assert!(registry.has_block_type("core/paragraph"));

        registry.unregister_block_type("core/paragraph");
        assert!(!registry.has_block_type("core/paragraph"));
    }

    #[test]
    fn test_block_type_with_supports() {
        let bt = BlockType::new_static(
            "core/paragraph",
            "Paragraph",
            BlockCategory::Text,
            "editor-paragraph",
        )
        .with_support("align", true)
        .with_support("anchor", true)
        .with_support("html", false);

        assert_eq!(bt.supports.get("align"), Some(&true));
        assert_eq!(bt.supports.get("anchor"), Some(&true));
        assert_eq!(bt.supports.get("html"), Some(&false));
    }
}
