//! Field type definitions for the RustPress custom fields system.
//!
//! Provides the core types used to define custom fields, including
//! the field type enum, field definitions, and field values.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// All supported field types, mirroring ACF field types.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldType {
    Text,
    Textarea,
    Number,
    Email,
    Url,
    Password,
    Wysiwyg,
    Image,
    File,
    Gallery,
    Select,
    Checkbox,
    Radio,
    TrueFalse,
    DatePicker,
    TimePicker,
    ColorPicker,
    Link,
    Relationship,
    PostObject,
    Taxonomy,
    User,
    GoogleMap,
    Repeater,
    Group,
    FlexibleContent,
}

/// A dynamic value that can be stored in a custom field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FieldValue {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<FieldValue>),
    Object(HashMap<String, FieldValue>),
}

impl FieldValue {
    /// Returns `true` if this value is `Null`.
    pub fn is_null(&self) -> bool {
        matches!(self, FieldValue::Null)
    }

    /// Attempts to extract a string reference from the value.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            FieldValue::String(s) => Some(s.as_str()),
            _ => None,
        }
    }

    /// Attempts to extract a number from the value.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            FieldValue::Number(n) => Some(*n),
            _ => None,
        }
    }

    /// Attempts to extract a boolean from the value.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            FieldValue::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Attempts to extract a slice from the value.
    pub fn as_array(&self) -> Option<&[FieldValue]> {
        match self {
            FieldValue::Array(arr) => Some(arr.as_slice()),
            _ => None,
        }
    }

    /// Attempts to extract an object (map) reference from the value.
    pub fn as_object(&self) -> Option<&HashMap<String, FieldValue>> {
        match self {
            FieldValue::Object(map) => Some(map),
            _ => None,
        }
    }
}

/// Conditional logic to control when a field is visible.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConditionalRule {
    /// The field key to evaluate.
    pub field: String,
    /// The comparison operator.
    pub operator: ConditionalOperator,
    /// The value to compare against.
    pub value: String,
}

/// Operators for conditional logic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConditionalOperator {
    Equals,
    NotEquals,
    Contains,
    IsEmpty,
    IsNotEmpty,
}

/// Defines a single custom field, including its type, label, validation
/// rules, and optional conditional display logic.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldDefinition {
    /// Unique key for this field (e.g., "field_hero_title").
    pub key: String,
    /// Human-readable label shown in the admin UI.
    pub label: String,
    /// Machine name used to store/retrieve the value.
    pub name: String,
    /// The type of field.
    pub field_type: FieldType,
    /// Instructions displayed below the field in the admin UI.
    pub instructions: String,
    /// Whether this field is required.
    pub required: bool,
    /// Default value when the field has not been set.
    pub default_value: FieldValue,
    /// Conditional logic rules (outer Vec is OR, inner Vec is AND).
    pub conditional_logic: Vec<Vec<ConditionalRule>>,
}

impl FieldDefinition {
    /// Creates a new field definition with the given key, label, name, and type.
    /// Other fields are set to sensible defaults.
    pub fn new(key: &str, label: &str, name: &str, field_type: FieldType) -> Self {
        Self {
            key: key.to_string(),
            label: label.to_string(),
            name: name.to_string(),
            field_type,
            instructions: String::new(),
            required: false,
            default_value: FieldValue::Null,
            conditional_logic: Vec::new(),
        }
    }

    /// Sets the instructions for the field.
    pub fn with_instructions(mut self, instructions: &str) -> Self {
        self.instructions = instructions.to_string();
        self
    }

    /// Marks the field as required.
    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }

    /// Sets the default value for the field.
    pub fn with_default(mut self, default: FieldValue) -> Self {
        self.default_value = default;
        self
    }

    /// Adds a conditional logic group (AND group) to the field.
    pub fn with_condition(mut self, rules: Vec<ConditionalRule>) -> Self {
        self.conditional_logic.push(rules);
        self
    }

    /// Validates a given value against this field's type constraints.
    /// Returns `true` if the value is acceptable.
    pub fn validate(&self, value: &FieldValue) -> bool {
        if self.required && value.is_null() {
            return false;
        }
        if value.is_null() {
            return true;
        }
        match self.field_type {
            FieldType::Text
            | FieldType::Textarea
            | FieldType::Password
            | FieldType::Wysiwyg
            | FieldType::ColorPicker
            | FieldType::DatePicker
            | FieldType::TimePicker => matches!(value, FieldValue::String(_)),
            FieldType::Number => matches!(value, FieldValue::Number(_)),
            FieldType::Email => {
                if let FieldValue::String(s) = value {
                    s.contains('@') && s.contains('.')
                } else {
                    false
                }
            }
            FieldType::Url => {
                if let FieldValue::String(s) = value {
                    s.starts_with("http://") || s.starts_with("https://")
                } else {
                    false
                }
            }
            FieldType::TrueFalse => matches!(value, FieldValue::Bool(_)),
            FieldType::Image | FieldType::File | FieldType::PostObject | FieldType::User => {
                matches!(value, FieldValue::Number(_) | FieldValue::Object(_))
            }
            FieldType::Gallery | FieldType::Checkbox | FieldType::Relationship => {
                matches!(value, FieldValue::Array(_))
            }
            FieldType::Select | FieldType::Radio => {
                matches!(value, FieldValue::String(_))
            }
            FieldType::Link => matches!(value, FieldValue::Object(_)),
            FieldType::Taxonomy => {
                matches!(value, FieldValue::Number(_) | FieldValue::Array(_))
            }
            FieldType::GoogleMap => matches!(value, FieldValue::Object(_)),
            FieldType::Repeater | FieldType::FlexibleContent => {
                matches!(value, FieldValue::Array(_))
            }
            FieldType::Group => matches!(value, FieldValue::Object(_)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_field_definition_builder() {
        let field = FieldDefinition::new("field_123", "Title", "title", FieldType::Text)
            .with_instructions("Enter the page title")
            .required()
            .with_default(FieldValue::String("Untitled".to_string()));

        assert_eq!(field.key, "field_123");
        assert_eq!(field.label, "Title");
        assert_eq!(field.name, "title");
        assert_eq!(field.field_type, FieldType::Text);
        assert_eq!(field.instructions, "Enter the page title");
        assert!(field.required);
        assert_eq!(
            field.default_value,
            FieldValue::String("Untitled".to_string())
        );
    }

    #[test]
    fn test_field_value_accessors() {
        let s = FieldValue::String("hello".to_string());
        assert_eq!(s.as_str(), Some("hello"));
        assert_eq!(s.as_f64(), None);

        let n = FieldValue::Number(42.0);
        assert_eq!(n.as_f64(), Some(42.0));
        assert_eq!(n.as_str(), None);

        let b = FieldValue::Bool(true);
        assert_eq!(b.as_bool(), Some(true));

        let null = FieldValue::Null;
        assert!(null.is_null());
        assert!(!s.is_null());

        let arr = FieldValue::Array(vec![FieldValue::Number(1.0)]);
        assert_eq!(arr.as_array().unwrap().len(), 1);

        let mut map = HashMap::new();
        map.insert("key".to_string(), FieldValue::String("val".to_string()));
        let obj = FieldValue::Object(map);
        assert!(obj.as_object().is_some());
    }

    #[test]
    fn test_validate_required_field() {
        let field = FieldDefinition::new("field_1", "Name", "name", FieldType::Text).required();

        assert!(!field.validate(&FieldValue::Null));
        assert!(field.validate(&FieldValue::String("Alice".to_string())));
    }

    #[test]
    fn test_validate_email() {
        let field = FieldDefinition::new("field_email", "Email", "email", FieldType::Email);

        assert!(field.validate(&FieldValue::String("test@example.com".to_string())));
        assert!(!field.validate(&FieldValue::String("not-an-email".to_string())));
        assert!(!field.validate(&FieldValue::Number(42.0)));
        assert!(field.validate(&FieldValue::Null)); // not required
    }

    #[test]
    fn test_validate_url() {
        let field = FieldDefinition::new("field_url", "Website", "website", FieldType::Url);

        assert!(field.validate(&FieldValue::String("https://example.com".to_string())));
        assert!(!field.validate(&FieldValue::String("ftp://bad.com".to_string())));
    }

    #[test]
    fn test_field_value_serde_roundtrip() {
        let original = FieldValue::Object({
            let mut m = HashMap::new();
            m.insert("name".to_string(), FieldValue::String("test".to_string()));
            m.insert("count".to_string(), FieldValue::Number(5.0));
            m.insert("active".to_string(), FieldValue::Bool(true));
            m
        });

        let json = serde_json::to_string(&original).unwrap();
        let deserialized: FieldValue = serde_json::from_str(&json).unwrap();
        assert_eq!(original, deserialized);
    }
}
