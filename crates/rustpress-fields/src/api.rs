//! Public API for the RustPress custom fields system.
//!
//! These functions mirror the ACF (Advanced Custom Fields) PHP API,
//! providing WordPress-compatible function names such as `get_field`,
//! `update_field`, `have_rows`, and `get_sub_field`.

use crate::field_types::FieldValue;
use crate::storage::FieldStorage;

/// Retrieves a field value for a given post.
///
/// Equivalent to ACF's `get_field($field_name, $post_id)`.
///
/// # Arguments
/// * `field_name` - The name (not key) of the field.
/// * `post_id` - The ID of the post.
/// * `storage` - The field storage backend.
///
/// # Returns
/// `Some(FieldValue)` if the field exists, `None` otherwise.
pub fn get_field(field_name: &str, post_id: i64, storage: &FieldStorage) -> Option<FieldValue> {
    storage.get_field(post_id, field_name)
}

/// Updates (or creates) a field value for a given post.
///
/// Equivalent to ACF's `update_field($field_name, $value, $post_id)`.
///
/// # Arguments
/// * `field_name` - The name of the field.
/// * `value` - The value to store.
/// * `post_id` - The ID of the post.
/// * `storage` - The mutable field storage backend.
pub fn update_field(field_name: &str, value: FieldValue, post_id: i64, storage: &mut FieldStorage) {
    storage.update_field(post_id, field_name, value);
}

/// Deletes a field value for a given post.
///
/// Equivalent to ACF's `delete_field($field_name, $post_id)`.
///
/// # Returns
/// `true` if the field existed and was removed.
pub fn delete_field(field_name: &str, post_id: i64, storage: &mut FieldStorage) -> bool {
    storage.delete_field(post_id, field_name)
}

/// Checks whether a repeater or flexible content field has any rows.
///
/// Equivalent to ACF's `have_rows($field_name, $post_id)`.
///
/// Returns `true` if the field value is a non-empty array.
pub fn have_rows(field_name: &str, post_id: i64, storage: &FieldStorage) -> bool {
    match storage.get_field(post_id, field_name) {
        Some(FieldValue::Array(rows)) => !rows.is_empty(),
        _ => false,
    }
}

/// Retrieves a sub-field value from a parent field value.
///
/// This is used when iterating over repeater or group field rows.
/// Equivalent to ACF's `get_sub_field($field_name)` within a
/// `have_rows` loop.
///
/// The `value` should be a `FieldValue::Object` (representing
/// a single row), and `field_name` is the key within that object.
///
/// # Arguments
/// * `field_name` - The sub-field name to look up.
/// * `value` - The parent value (typically a row object).
///
/// # Returns
/// A reference to the sub-field value, or `None` if not found.
pub fn get_sub_field<'a>(field_name: &str, value: &'a FieldValue) -> Option<&'a FieldValue> {
    match value {
        FieldValue::Object(map) => map.get(field_name),
        _ => None,
    }
}

/// Retrieves all rows from a repeater field, returning them as a
/// vector of references.
///
/// This provides a Rust-idiomatic way to iterate over repeater data
/// instead of the PHP `while(have_rows())` / `the_row()` pattern.
///
/// # Returns
/// A `Vec` of references to each row value, or an empty vec if the
/// field does not exist or is not an array.
pub fn get_rows(field_name: &str, post_id: i64, storage: &FieldStorage) -> Vec<FieldValue> {
    match storage.get_field(post_id, field_name) {
        Some(FieldValue::Array(rows)) => rows,
        _ => Vec::new(),
    }
}

/// Retrieves a field value as a string, or returns a default.
///
/// Convenience function that unwraps a `FieldValue::String`.
pub fn get_field_string(
    field_name: &str,
    post_id: i64,
    storage: &FieldStorage,
    default: &str,
) -> String {
    match storage.get_field(post_id, field_name) {
        Some(FieldValue::String(s)) => s,
        _ => default.to_string(),
    }
}

/// Retrieves a field value as an `f64`, or returns a default.
///
/// Convenience function that unwraps a `FieldValue::Number`.
pub fn get_field_number(
    field_name: &str,
    post_id: i64,
    storage: &FieldStorage,
    default: f64,
) -> f64 {
    match storage.get_field(post_id, field_name) {
        Some(FieldValue::Number(n)) => n,
        _ => default,
    }
}

/// Retrieves a field value as a `bool`, or returns a default.
///
/// Convenience function that unwraps a `FieldValue::Bool`.
pub fn get_field_bool(
    field_name: &str,
    post_id: i64,
    storage: &FieldStorage,
    default: bool,
) -> bool {
    match storage.get_field(post_id, field_name) {
        Some(FieldValue::Bool(b)) => b,
        _ => default,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn setup_storage() -> FieldStorage {
        let mut storage = FieldStorage::new();
        storage.update_field(1, "title", FieldValue::String("Hello World".to_string()));
        storage.update_field(1, "count", FieldValue::Number(5.0));
        storage.update_field(1, "active", FieldValue::Bool(true));

        // Repeater field: a list of row objects
        let rows = FieldValue::Array(vec![
            FieldValue::Object({
                let mut row = HashMap::new();
                row.insert(
                    "image".to_string(),
                    FieldValue::String("/img/a.jpg".to_string()),
                );
                row.insert(
                    "caption".to_string(),
                    FieldValue::String("First".to_string()),
                );
                row
            }),
            FieldValue::Object({
                let mut row = HashMap::new();
                row.insert(
                    "image".to_string(),
                    FieldValue::String("/img/b.jpg".to_string()),
                );
                row.insert(
                    "caption".to_string(),
                    FieldValue::String("Second".to_string()),
                );
                row
            }),
        ]);
        storage.update_field(1, "slides", rows);

        storage
    }

    #[test]
    fn test_get_field() {
        let storage = setup_storage();
        let val = get_field("title", 1, &storage);
        assert_eq!(val, Some(FieldValue::String("Hello World".to_string())));

        assert!(get_field("nonexistent", 1, &storage).is_none());
        assert!(get_field("title", 999, &storage).is_none());
    }

    #[test]
    fn test_update_and_delete_field() {
        let mut storage = FieldStorage::new();

        update_field(
            "color",
            FieldValue::String("blue".to_string()),
            1,
            &mut storage,
        );
        assert_eq!(
            get_field("color", 1, &storage),
            Some(FieldValue::String("blue".to_string()))
        );

        // Update overwrites
        update_field(
            "color",
            FieldValue::String("red".to_string()),
            1,
            &mut storage,
        );
        assert_eq!(
            get_field("color", 1, &storage),
            Some(FieldValue::String("red".to_string()))
        );

        // Delete
        assert!(delete_field("color", 1, &mut storage));
        assert!(get_field("color", 1, &storage).is_none());
    }

    #[test]
    fn test_have_rows() {
        let storage = setup_storage();

        assert!(have_rows("slides", 1, &storage));
        assert!(!have_rows("title", 1, &storage)); // string, not array
        assert!(!have_rows("nonexistent", 1, &storage));
    }

    #[test]
    fn test_have_rows_empty_array() {
        let mut storage = FieldStorage::new();
        storage.update_field(1, "empty_rep", FieldValue::Array(vec![]));
        assert!(!have_rows("empty_rep", 1, &storage));
    }

    #[test]
    fn test_get_sub_field() {
        let storage = setup_storage();

        let rows = get_rows("slides", 1, &storage);
        assert_eq!(rows.len(), 2);

        let first_row = &rows[0];
        let image = get_sub_field("image", first_row);
        assert_eq!(image, Some(&FieldValue::String("/img/a.jpg".to_string())));

        let caption = get_sub_field("caption", first_row);
        assert_eq!(caption, Some(&FieldValue::String("First".to_string())));

        // Non-existent sub-field
        assert!(get_sub_field("nope", first_row).is_none());
    }

    #[test]
    fn test_get_sub_field_on_non_object() {
        let val = FieldValue::String("not an object".to_string());
        assert!(get_sub_field("anything", &val).is_none());
    }

    #[test]
    fn test_convenience_getters() {
        let storage = setup_storage();

        assert_eq!(get_field_string("title", 1, &storage, ""), "Hello World");
        assert_eq!(
            get_field_string("missing", 1, &storage, "default"),
            "default"
        );

        assert_eq!(get_field_number("count", 1, &storage, 0.0), 5.0);
        assert_eq!(get_field_number("missing", 1, &storage, 99.0), 99.0);

        assert_eq!(get_field_bool("active", 1, &storage, false), true);
        assert_eq!(get_field_bool("missing", 1, &storage, false), false);
    }

    #[test]
    fn test_get_rows_nonexistent() {
        let storage = FieldStorage::new();
        let rows = get_rows("nothing", 1, &storage);
        assert!(rows.is_empty());
    }
}
