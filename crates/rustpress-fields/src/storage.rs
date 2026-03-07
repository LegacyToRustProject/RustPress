//! Field value storage and retrieval.
//!
//! Provides an in-memory storage backend that mirrors the WordPress
//! `wp_postmeta` table pattern, where each field value is stored
//! as a (post_id, meta_key) pair.

use std::collections::HashMap;
use tracing::{debug, trace};

use crate::field_types::FieldValue;

/// In-memory storage for custom field values.
///
/// This mirrors the WordPress `wp_postmeta` pattern where each
/// (post_id, field_name) pair maps to a single serialized value.
///
/// In production, this would be backed by the database; this
/// implementation serves as a working default and test double.
#[derive(Debug, Default, Clone)]
pub struct FieldStorage {
    /// Maps post_id -> (field_name -> value).
    data: HashMap<i64, HashMap<String, FieldValue>>,
}

impl FieldStorage {
    /// Creates a new, empty field storage.
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }

    /// Retrieves a field value for a given post and field name.
    ///
    /// Returns `None` if the post has no value for this field.
    pub fn get_field(&self, post_id: i64, field_name: &str) -> Option<FieldValue> {
        trace!(post_id, field_name, "Getting field value");
        self.data
            .get(&post_id)
            .and_then(|fields| fields.get(field_name))
            .cloned()
    }

    /// Stores or updates a field value for a given post.
    pub fn update_field(&mut self, post_id: i64, field_name: &str, value: FieldValue) {
        debug!(post_id, field_name, "Updating field value");
        self.data
            .entry(post_id)
            .or_default()
            .insert(field_name.to_string(), value);
    }

    /// Removes a field value for a given post.
    ///
    /// Returns `true` if the field existed and was removed.
    pub fn delete_field(&mut self, post_id: i64, field_name: &str) -> bool {
        debug!(post_id, field_name, "Deleting field value");
        if let Some(fields) = self.data.get_mut(&post_id) {
            let removed = fields.remove(field_name).is_some();
            if fields.is_empty() {
                self.data.remove(&post_id);
            }
            removed
        } else {
            false
        }
    }

    /// Retrieves all field values for a given post.
    ///
    /// Returns an empty map if the post has no custom fields.
    pub fn get_fields(&self, post_id: i64) -> HashMap<String, FieldValue> {
        trace!(post_id, "Getting all fields for post");
        self.data.get(&post_id).cloned().unwrap_or_default()
    }

    /// Returns `true` if any fields are stored for the given post.
    pub fn has_fields(&self, post_id: i64) -> bool {
        self.data
            .get(&post_id)
            .is_some_and(|fields| !fields.is_empty())
    }

    /// Returns the number of posts that have stored field values.
    pub fn post_count(&self) -> usize {
        self.data.len()
    }

    /// Returns the total number of stored field values across all posts.
    pub fn total_field_count(&self) -> usize {
        self.data.values().map(|fields| fields.len()).sum()
    }

    /// Removes all field values for a given post.
    ///
    /// Returns `true` if the post had any fields.
    pub fn delete_post_fields(&mut self, post_id: i64) -> bool {
        self.data.remove(&post_id).is_some()
    }

    /// Serializes a field value to a JSON string, matching how WordPress
    /// stores serialized meta values.
    pub fn serialize_value(value: &FieldValue) -> Result<String, serde_json::Error> {
        serde_json::to_string(value)
    }

    /// Deserializes a JSON string back into a field value.
    pub fn deserialize_value(json: &str) -> Result<FieldValue, serde_json::Error> {
        serde_json::from_str(json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_and_update_field() {
        let mut storage = FieldStorage::new();

        assert!(storage.get_field(1, "title").is_none());

        storage.update_field(1, "title", FieldValue::String("Hello".to_string()));
        let val = storage.get_field(1, "title");
        assert_eq!(val, Some(FieldValue::String("Hello".to_string())));
    }

    #[test]
    fn test_delete_field() {
        let mut storage = FieldStorage::new();
        storage.update_field(1, "color", FieldValue::String("red".to_string()));
        storage.update_field(1, "size", FieldValue::Number(42.0));

        assert!(storage.delete_field(1, "color"));
        assert!(storage.get_field(1, "color").is_none());
        // Other field still exists
        assert!(storage.get_field(1, "size").is_some());

        // Delete last field removes the post entry
        assert!(storage.delete_field(1, "size"));
        assert!(!storage.has_fields(1));
    }

    #[test]
    fn test_delete_nonexistent_field() {
        let mut storage = FieldStorage::new();
        assert!(!storage.delete_field(999, "nope"));
    }

    #[test]
    fn test_get_all_fields() {
        let mut storage = FieldStorage::new();
        storage.update_field(1, "a", FieldValue::String("x".to_string()));
        storage.update_field(1, "b", FieldValue::Number(1.0));
        storage.update_field(2, "c", FieldValue::Bool(true));

        let fields = storage.get_fields(1);
        assert_eq!(fields.len(), 2);
        assert_eq!(fields.get("a"), Some(&FieldValue::String("x".to_string())));
        assert_eq!(fields.get("b"), Some(&FieldValue::Number(1.0)));

        // Non-existent post returns empty map
        assert!(storage.get_fields(999).is_empty());
    }

    #[test]
    fn test_post_and_field_counts() {
        let mut storage = FieldStorage::new();
        assert_eq!(storage.post_count(), 0);
        assert_eq!(storage.total_field_count(), 0);

        storage.update_field(1, "a", FieldValue::Null);
        storage.update_field(1, "b", FieldValue::Null);
        storage.update_field(2, "a", FieldValue::Null);

        assert_eq!(storage.post_count(), 2);
        assert_eq!(storage.total_field_count(), 3);
    }

    #[test]
    fn test_delete_post_fields() {
        let mut storage = FieldStorage::new();
        storage.update_field(1, "x", FieldValue::Null);
        storage.update_field(1, "y", FieldValue::Null);

        assert!(storage.delete_post_fields(1));
        assert!(!storage.has_fields(1));
        assert!(!storage.delete_post_fields(1)); // already gone
    }

    #[test]
    fn test_serialize_deserialize_roundtrip() {
        let value = FieldValue::Object({
            let mut m = HashMap::new();
            m.insert("name".to_string(), FieldValue::String("test".to_string()));
            m.insert(
                "items".to_string(),
                FieldValue::Array(vec![FieldValue::Number(1.0), FieldValue::Number(2.0)]),
            );
            m
        });

        let json = FieldStorage::serialize_value(&value).unwrap();
        let restored = FieldStorage::deserialize_value(&json).unwrap();
        assert_eq!(value, restored);
    }
}
