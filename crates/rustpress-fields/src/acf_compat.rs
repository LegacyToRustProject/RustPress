//! ACF (Advanced Custom Fields) compatibility layer.
//!
//! Maps RustPress field storage to ACF's wp_postmeta format so that
//! existing ACF field data in the WordPress database can be read
//! and written transparently.
//!
//! ## ACF Storage Format in wp_postmeta
//!
//! ACF stores each field value as **two** meta rows:
//! 1. `meta_key = "field_name"`, `meta_value = "the actual value"`
//! 2. `meta_key = "_field_name"`, `meta_value = "field_abc123"` (reference to field definition)
//!
//! The underscore-prefixed key links the value to the field definition,
//! which is stored as a `post_type = 'acf-field'` post.
//!
//! ## ACF Field Groups
//!
//! Field groups are stored as `post_type = 'acf-field-group'` posts.
//! Field definitions are stored as `post_type = 'acf-field'` posts
//! with `post_parent` pointing to the field group.
//!
//! Field group settings are stored in `post_content` as a serialized array.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::field_types::FieldValue;

/// ACF post types used in wp_posts.
pub mod post_types {
    pub const FIELD_GROUP: &str = "acf-field-group";
    pub const FIELD: &str = "acf-field";
}

/// An ACF field value pair as stored in wp_postmeta.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcfMetaEntry {
    /// The field name (meta_key without underscore prefix).
    pub field_name: String,
    /// The field key (e.g., "field_abc123") stored in `_field_name`.
    pub field_key: Option<String>,
    /// The field value.
    pub value: String,
}

/// ACF-compatible field data for a single post, read from wp_postmeta.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AcfPostData {
    pub post_id: u64,
    pub fields: Vec<AcfMetaEntry>,
}

impl AcfPostData {
    /// Parse ACF field data from raw wp_postmeta key-value pairs.
    ///
    /// ACF stores each field as two meta keys:
    /// - `field_name` → value
    /// - `_field_name` → field_key reference
    ///
    /// This function pairs them together.
    pub fn from_meta(post_id: u64, meta: &HashMap<String, String>) -> Self {
        let mut fields = Vec::new();

        for (key, value) in meta {
            // Skip underscore-prefixed reference keys
            if key.starts_with('_') {
                continue;
            }
            // Skip WordPress internal meta
            if key.starts_with("_wp_") || key.starts_with("_edit_") || key.starts_with("_yoast_") {
                continue;
            }

            let reference_key = format!("_{}", key);
            let field_key = meta.get(&reference_key).cloned();

            // Only include entries that have an ACF reference key (field_xxx)
            // or that are not WordPress internal keys
            if field_key
                .as_ref()
                .map_or(false, |k| k.starts_with("field_"))
            {
                fields.push(AcfMetaEntry {
                    field_name: key.clone(),
                    field_key,
                    value: value.clone(),
                });
            }
        }

        Self { post_id, fields }
    }

    /// Convert ACF data to wp_postmeta key-value pairs for writing.
    ///
    /// Generates both the value row and the reference row for each field.
    pub fn to_meta(&self) -> Vec<(String, String)> {
        let mut pairs = Vec::new();

        for entry in &self.fields {
            // Value row: field_name → value
            pairs.push((entry.field_name.clone(), entry.value.clone()));

            // Reference row: _field_name → field_key
            if let Some(ref key) = entry.field_key {
                pairs.push((format!("_{}", entry.field_name), key.clone()));
            }
        }

        pairs
    }

    /// Get a field value by name.
    pub fn get_field(&self, field_name: &str) -> Option<&str> {
        self.fields
            .iter()
            .find(|f| f.field_name == field_name)
            .map(|f| f.value.as_str())
    }

    /// Get a field value as a FieldValue enum.
    pub fn get_field_value(&self, field_name: &str) -> Option<FieldValue> {
        let raw = self.get_field(field_name)?;
        Some(parse_acf_value(raw))
    }

    /// Set a field value (add or update).
    pub fn set_field(&mut self, field_name: &str, value: &str, field_key: Option<&str>) {
        if let Some(existing) = self.fields.iter_mut().find(|f| f.field_name == field_name) {
            existing.value = value.to_string();
            if let Some(key) = field_key {
                existing.field_key = Some(key.to_string());
            }
        } else {
            self.fields.push(AcfMetaEntry {
                field_name: field_name.to_string(),
                field_key: field_key.map(|k| k.to_string()),
                value: value.to_string(),
            });
        }
    }

    /// Remove a field by name.
    pub fn remove_field(&mut self, field_name: &str) -> bool {
        let before = self.fields.len();
        self.fields.retain(|f| f.field_name != field_name);
        self.fields.len() < before
    }
}

/// Parse a raw ACF meta value into a typed FieldValue.
///
/// ACF stores values as strings in wp_postmeta. This function
/// attempts to parse them into appropriate types.
fn parse_acf_value(raw: &str) -> FieldValue {
    // Empty
    if raw.is_empty() {
        return FieldValue::String(String::new());
    }

    // Boolean (ACF stores as "0" or "1")
    if raw == "0" || raw == "1" {
        return FieldValue::Bool(raw == "1");
    }

    // Integer
    if let Ok(n) = raw.parse::<i64>() {
        return FieldValue::Number(n as f64);
    }

    // Float
    if let Ok(n) = raw.parse::<f64>() {
        return FieldValue::Number(n);
    }

    // PHP serialized array (ACF repeater/flexible content)
    if raw.starts_with("a:") || raw.starts_with("s:") {
        return FieldValue::String(raw.to_string());
    }

    // JSON array/object (ACF sometimes uses JSON for gallery, etc.)
    if (raw.starts_with('[') && raw.ends_with(']'))
        || (raw.starts_with('{') && raw.ends_with('}'))
    {
        if let Ok(arr) = serde_json::from_str::<Vec<serde_json::Value>>(raw) {
            let items: Vec<FieldValue> = arr
                .into_iter()
                .map(|v| match v {
                    serde_json::Value::String(s) => FieldValue::String(s),
                    serde_json::Value::Number(n) => {
                        FieldValue::Number(n.as_f64().unwrap_or(0.0))
                    }
                    serde_json::Value::Bool(b) => FieldValue::Bool(b),
                    other => FieldValue::String(other.to_string()),
                })
                .collect();
            return FieldValue::Array(items);
        }
    }

    FieldValue::String(raw.to_string())
}

/// ACF field group definition, as stored in `wp_posts` with
/// `post_type = 'acf-field-group'`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcfFieldGroupPost {
    pub post_id: u64,
    pub title: String,
    pub key: String,
    pub status: String,
    pub menu_order: i32,
}

/// ACF field definition, as stored in `wp_posts` with
/// `post_type = 'acf-field'` and `post_parent` = field group ID.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcfFieldPost {
    pub post_id: u64,
    pub field_group_id: u64,
    pub key: String,
    pub label: String,
    pub name: String,
    pub field_type: String,
    pub menu_order: i32,
    pub required: bool,
    pub instructions: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_meta() -> HashMap<String, String> {
        let mut m = HashMap::new();
        // ACF field: hero_title
        m.insert("hero_title".into(), "Welcome to RustPress".into());
        m.insert("_hero_title".into(), "field_abc123".into());
        // ACF field: hero_image
        m.insert("hero_image".into(), "42".into());
        m.insert("_hero_image".into(), "field_def456".into());
        // WordPress internal meta (should be skipped)
        m.insert("_wp_attached_file".into(), "image.jpg".into());
        m.insert("_edit_lock".into(), "1234567890:1".into());
        // Non-ACF field (no reference key)
        m.insert("random_field".into(), "some value".into());
        m
    }

    #[test]
    fn test_from_meta_pairs_acf_fields() {
        let data = AcfPostData::from_meta(1, &sample_meta());

        assert_eq!(data.post_id, 1);
        assert_eq!(data.fields.len(), 2); // Only ACF fields with field_xxx references

        let hero_title = data.get_field("hero_title");
        assert_eq!(hero_title, Some("Welcome to RustPress"));

        let hero_image = data.get_field("hero_image");
        assert_eq!(hero_image, Some("42"));
    }

    #[test]
    fn test_field_key_reference() {
        let data = AcfPostData::from_meta(1, &sample_meta());

        let entry = data.fields.iter().find(|f| f.field_name == "hero_title").unwrap();
        assert_eq!(entry.field_key.as_deref(), Some("field_abc123"));
    }

    #[test]
    fn test_to_meta_roundtrip() {
        let original = AcfPostData::from_meta(1, &sample_meta());
        let pairs = original.to_meta();

        // Should produce 4 pairs: 2 values + 2 references
        assert_eq!(pairs.len(), 4);

        let meta: HashMap<String, String> = pairs.into_iter().collect();
        assert_eq!(meta.get("hero_title").unwrap(), "Welcome to RustPress");
        assert_eq!(meta.get("_hero_title").unwrap(), "field_abc123");
        assert_eq!(meta.get("hero_image").unwrap(), "42");
        assert_eq!(meta.get("_hero_image").unwrap(), "field_def456");
    }

    #[test]
    fn test_get_field_value_types() {
        let data = AcfPostData {
            post_id: 1,
            fields: vec![
                AcfMetaEntry { field_name: "count".into(), field_key: Some("field_1".into()), value: "42".into() },
                AcfMetaEntry { field_name: "name".into(), field_key: Some("field_2".into()), value: "hello".into() },
                AcfMetaEntry { field_name: "flag".into(), field_key: Some("field_3".into()), value: "1".into() },
                AcfMetaEntry { field_name: "price".into(), field_key: Some("field_4".into()), value: "19.99".into() },
            ],
        };

        // "42" → parsed as Number (but since it could also be "1"/"0" boolean, "42" is Number)
        // Actually "42" will try i64 parse first → Number(42.0)
        assert!(matches!(data.get_field_value("count"), Some(FieldValue::Number(_))));

        // "hello" → String
        assert!(matches!(data.get_field_value("name"), Some(FieldValue::String(_))));

        // "1" → Boolean(true) (ACF convention)
        assert!(matches!(data.get_field_value("flag"), Some(FieldValue::Bool(true))));

        // "19.99" → Number
        assert!(matches!(data.get_field_value("price"), Some(FieldValue::Number(_))));
    }

    #[test]
    fn test_set_field() {
        let mut data = AcfPostData::from_meta(1, &sample_meta());

        // Update existing
        data.set_field("hero_title", "Updated Title", None);
        assert_eq!(data.get_field("hero_title"), Some("Updated Title"));

        // Add new
        data.set_field("new_field", "new value", Some("field_new123"));
        assert_eq!(data.get_field("new_field"), Some("new value"));
        assert_eq!(data.fields.len(), 3);
    }

    #[test]
    fn test_remove_field() {
        let mut data = AcfPostData::from_meta(1, &sample_meta());
        let count_before = data.fields.len();

        assert!(data.remove_field("hero_title"));
        assert_eq!(data.fields.len(), count_before - 1);
        assert!(data.get_field("hero_title").is_none());

        // Remove non-existent
        assert!(!data.remove_field("nonexistent"));
    }

    #[test]
    fn test_parse_acf_value_json_array() {
        let val = parse_acf_value("[\"image1.jpg\",\"image2.jpg\"]");
        match val {
            FieldValue::Array(items) => {
                assert_eq!(items.len(), 2);
            }
            _ => panic!("Expected Array"),
        }
    }

    #[test]
    fn test_empty_meta_produces_empty_data() {
        let data = AcfPostData::from_meta(1, &HashMap::new());
        assert!(data.fields.is_empty());
    }
}
