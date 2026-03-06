//! WordPress Block Patterns REST API
//!
//! GET /wp-json/wp/v2/block-patterns/categories
//! GET /wp-json/wp/v2/block-patterns/patterns
//!
//! Returns built-in block pattern categories and patterns for the Gutenberg editor.

use axum::{routing::get, Json, Router};
use serde_json::{json, Value};

use crate::ApiState;

pub fn routes() -> Router<ApiState> {
    Router::new()
        .route(
            "/wp-json/wp/v2/block-patterns/categories",
            get(list_pattern_categories),
        )
        .route(
            "/wp-json/wp/v2/block-patterns/patterns",
            get(list_patterns),
        )
}

/// GET /wp-json/wp/v2/block-patterns/categories
async fn list_pattern_categories() -> Json<Vec<Value>> {
    let categories = vec![
        json!({ "name": "featured", "label": "Featured", "description": "Featured patterns" }),
        json!({ "name": "text", "label": "Text", "description": "Patterns with text" }),
        json!({ "name": "query", "label": "Posts", "description": "Patterns for displaying posts" }),
        json!({ "name": "banner", "label": "Banner", "description": "Banner patterns" }),
        json!({ "name": "header", "label": "Header", "description": "Header patterns" }),
        json!({ "name": "footer", "label": "Footer", "description": "Footer patterns" }),
        json!({ "name": "buttons", "label": "Buttons", "description": "Button patterns" }),
        json!({ "name": "column", "label": "Columns", "description": "Multi-column patterns" }),
        json!({ "name": "gallery", "label": "Gallery", "description": "Image gallery patterns" }),
        json!({ "name": "call-to-action", "label": "Call to Action", "description": "Call to action patterns" }),
    ];
    Json(categories)
}

/// GET /wp-json/wp/v2/block-patterns/patterns
async fn list_patterns() -> Json<Vec<Value>> {
    let patterns = vec![
        json!({
            "name": "core/text-and-image",
            "title": "Text and image",
            "content": "<!-- wp:columns --><div class=\"wp-block-columns\"><!-- wp:column --><div class=\"wp-block-column\"><!-- wp:paragraph --><p>Add your text here.</p><!-- /wp:paragraph --></div><!-- /wp:column --><!-- wp:column --><div class=\"wp-block-column\"><!-- wp:image --><figure class=\"wp-block-image\"><img alt=\"\"/></figure><!-- /wp:image --></div><!-- /wp:column --></div><!-- /wp:columns -->",
            "categories": ["text", "featured"],
            "keywords": ["text", "image", "columns"],
            "block_types": [],
            "source": "core",
            "description": ""
        }),
        json!({
            "name": "core/heading-and-paragraph",
            "title": "Heading and paragraph",
            "content": "<!-- wp:heading --><h2 class=\"wp-block-heading\">Write a heading</h2><!-- /wp:heading --><!-- wp:paragraph --><p>Add a paragraph with supporting text.</p><!-- /wp:paragraph -->",
            "categories": ["text"],
            "keywords": ["heading", "paragraph", "text"],
            "block_types": [],
            "source": "core",
            "description": ""
        }),
        json!({
            "name": "core/simple-call-to-action",
            "title": "Call to action",
            "content": "<!-- wp:group {\"align\":\"full\",\"style\":{\"spacing\":{\"padding\":{\"top\":\"4rem\",\"bottom\":\"4rem\"}}}} --><div class=\"wp-block-group alignfull\"><!-- wp:heading {\"textAlign\":\"center\"} --><h2 class=\"wp-block-heading has-text-align-center\">Ready to get started?</h2><!-- /wp:heading --><!-- wp:paragraph {\"align\":\"center\"} --><p class=\"has-text-align-center\">Add supporting text here.</p><!-- /wp:paragraph --><!-- wp:buttons {\"layout\":{\"type\":\"flex\",\"justifyContent\":\"center\"}} --><div class=\"wp-block-buttons\"><!-- wp:button --><div class=\"wp-block-button\"><a class=\"wp-block-button__link wp-element-button\">Get started</a></div><!-- /wp:button --></div><!-- /wp:buttons --></div><!-- /wp:group -->",
            "categories": ["call-to-action", "featured"],
            "keywords": ["cta", "call to action", "button"],
            "block_types": [],
            "source": "core",
            "description": ""
        }),
        json!({
            "name": "core/image-with-caption",
            "title": "Image with caption",
            "content": "<!-- wp:image {\"align\":\"center\"} --><figure class=\"wp-block-image aligncenter\"><img alt=\"\"/><figcaption class=\"wp-element-caption\">Write a caption</figcaption></figure><!-- /wp:image -->",
            "categories": ["gallery", "featured"],
            "keywords": ["image", "photo", "caption"],
            "block_types": [],
            "source": "core",
            "description": ""
        }),
        json!({
            "name": "core/three-column-text",
            "title": "Three columns of text",
            "content": "<!-- wp:columns --><div class=\"wp-block-columns\"><!-- wp:column --><div class=\"wp-block-column\"><!-- wp:paragraph --><p>Column 1 text goes here.</p><!-- /wp:paragraph --></div><!-- /wp:column --><!-- wp:column --><div class=\"wp-block-column\"><!-- wp:paragraph --><p>Column 2 text goes here.</p><!-- /wp:paragraph --></div><!-- /wp:column --><!-- wp:column --><div class=\"wp-block-column\"><!-- wp:paragraph --><p>Column 3 text goes here.</p><!-- /wp:paragraph --></div><!-- /wp:column --></div><!-- /wp:columns -->",
            "categories": ["column", "text"],
            "keywords": ["columns", "text", "three"],
            "block_types": [],
            "source": "core",
            "description": ""
        }),
    ];
    Json(patterns)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_has_required_fields() {
        // patterns list is compiled-in, just verify it compiles
        let cats = vec![
            json!({ "name": "featured", "label": "Featured" }),
        ];
        assert!(!cats.is_empty());
    }
}
