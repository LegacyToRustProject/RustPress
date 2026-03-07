pub mod core_blocks;
pub mod parser;
pub mod registry;
pub mod renderer;
pub mod serializer;

pub use parser::{parse_blocks, Block};
pub use registry::{BlockCategory, BlockRegistry, BlockType, RenderCallback};
pub use renderer::BlockRenderer;
pub use serializer::{serialize_block, serialize_blocks};

/// Create a fully initialized BlockRenderer with all core WordPress blocks registered.
pub fn create_default_renderer() -> BlockRenderer {
    let mut registry = BlockRegistry::new();
    core_blocks::register_core_blocks(&mut registry);
    BlockRenderer::new(registry)
}
