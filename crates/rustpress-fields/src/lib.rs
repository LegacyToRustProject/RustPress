//! # rustpress-fields
//!
//! Advanced Custom Fields (ACF) equivalent for RustPress.
//!
//! This crate provides a custom field management system compatible
//! with WordPress's ACF plugin patterns. It allows defining custom
//! field types, grouping them, and storing/retrieving values using
//! a WordPress-compatible API.
//!
//! ## Quick Start
//!
//! ```rust
//! use rustpress_fields::prelude::*;
//!
//! // Define a field group
//! let group = FieldGroup::new("group_hero", "Hero Section")
//!     .add_field(
//!         FieldDefinition::new("field_title", "Title", "hero_title", FieldType::Text)
//!             .required()
//!     )
//!     .add_field(
//!         FieldDefinition::new("field_bg", "Background", "hero_bg", FieldType::Image)
//!     )
//!     .add_location(vec![
//!         LocationRule::new(LocationParam::PostType, LocationOperator::Equals, "page")
//!     ]);
//!
//! // Register the group
//! let mut registry = FieldGroupRegistry::new();
//! registry.register_group(group);
//!
//! // Store and retrieve field values
//! let mut storage = FieldStorage::new();
//! update_field("hero_title", FieldValue::String("Welcome".into()), 1, &mut storage);
//!
//! let title = get_field("hero_title", 1, &storage);
//! assert_eq!(title, Some(FieldValue::String("Welcome".into())));
//! ```

pub mod api;
pub mod field_group;
pub mod field_types;
pub mod storage;

// Re-export primary types at crate root for convenience.
pub use api::{
    delete_field, get_field, get_field_bool, get_field_number, get_field_string, get_rows,
    get_sub_field, have_rows, update_field,
};
pub use field_group::{
    FieldGroup, FieldGroupRegistry, LocationOperator, LocationParam, LocationRule, Position, Style,
};
pub use field_types::{
    ConditionalOperator, ConditionalRule, FieldDefinition, FieldType, FieldValue,
};
pub use storage::FieldStorage;

/// Prelude module for convenient glob imports.
pub mod prelude {
    pub use crate::api::{
        delete_field, get_field, get_field_bool, get_field_number, get_field_string, get_rows,
        get_sub_field, have_rows, update_field,
    };
    pub use crate::field_group::{
        FieldGroup, FieldGroupRegistry, LocationOperator, LocationParam, LocationRule, Position,
        Style,
    };
    pub use crate::field_types::{
        ConditionalOperator, ConditionalRule, FieldDefinition, FieldType, FieldValue,
    };
    pub use crate::storage::FieldStorage;
}
