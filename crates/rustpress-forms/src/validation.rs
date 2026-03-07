//! Form submission validation.

use std::collections::HashMap;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::builder::{FieldConfig, FormConfig, FormField};

/// A validation rule that can be applied to a form field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidationRule {
    /// Field must not be empty.
    Required,
    /// Field must be a valid email address.
    Email,
    /// Field value must have at least this many characters.
    MinLength(usize),
    /// Field value must have at most this many characters.
    MaxLength(usize),
    /// Field value must match the given regex pattern.
    Pattern(String),
    /// Numeric field must be at least this value.
    Min(f64),
    /// Numeric field must be at most this value.
    Max(f64),
}

/// A validation error for a specific field.
#[derive(Debug, Clone, PartialEq)]
pub struct ValidationError {
    /// The name of the field that failed validation.
    pub field_name: String,
    /// Human-readable error message.
    pub message: String,
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.field_name, self.message)
    }
}

/// Validate a form submission against the form configuration.
///
/// Returns `Ok(())` if all fields pass validation, or `Err(Vec<ValidationError>)` with
/// all validation errors found.
pub fn validate_submission(
    config: &FormConfig,
    data: &HashMap<String, String>,
) -> Result<(), Vec<ValidationError>> {
    let mut errors = Vec::new();

    for field in &config.fields {
        let value = data.get(&field.name).map(|s| s.as_str()).unwrap_or("");

        // Check required (both from the `required` flag and explicit Required rule)
        if field.required || field.validation_rules.iter().any(|r| matches!(r, ValidationRule::Required)) {
            if value.trim().is_empty() {
                errors.push(ValidationError {
                    field_name: field.name.clone(),
                    message: format!("{} is required.", field.label),
                });
                // Skip further validation if required field is empty
                continue;
            }
        }

        // Skip validation for empty optional fields
        if value.trim().is_empty() {
            continue;
        }

        // Validate based on field type
        validate_field_type(field, value, &mut errors);

        // Validate explicit rules
        for rule in &field.validation_rules {
            validate_rule(field, value, rule, &mut errors);
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Validate the value against the field's inherent type constraints.
fn validate_field_type(field: &FieldConfig, value: &str, errors: &mut Vec<ValidationError>) {
    match field.field_type {
        FormField::Email => {
            if !is_valid_email(value) {
                errors.push(ValidationError {
                    field_name: field.name.clone(),
                    message: format!("{} must be a valid email address.", field.label),
                });
            }
        }
        FormField::Url => {
            if !is_valid_url(value) {
                errors.push(ValidationError {
                    field_name: field.name.clone(),
                    message: format!("{} must be a valid URL.", field.label),
                });
            }
        }
        FormField::Number => {
            if value.parse::<f64>().is_err() {
                errors.push(ValidationError {
                    field_name: field.name.clone(),
                    message: format!("{} must be a valid number.", field.label),
                });
            }
        }
        FormField::Date => {
            if chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d").is_err() {
                errors.push(ValidationError {
                    field_name: field.name.clone(),
                    message: format!("{} must be a valid date (YYYY-MM-DD).", field.label),
                });
            }
        }
        _ => {}
    }
}

/// Validate the value against a single validation rule.
fn validate_rule(
    field: &FieldConfig,
    value: &str,
    rule: &ValidationRule,
    errors: &mut Vec<ValidationError>,
) {
    match rule {
        ValidationRule::Required => {
            // Already handled above
        }
        ValidationRule::Email => {
            if !is_valid_email(value) {
                errors.push(ValidationError {
                    field_name: field.name.clone(),
                    message: format!("{} must be a valid email address.", field.label),
                });
            }
        }
        ValidationRule::MinLength(min) => {
            if value.len() < *min {
                errors.push(ValidationError {
                    field_name: field.name.clone(),
                    message: format!("{} must be at least {} characters.", field.label, min),
                });
            }
        }
        ValidationRule::MaxLength(max) => {
            if value.len() > *max {
                errors.push(ValidationError {
                    field_name: field.name.clone(),
                    message: format!("{} must be at most {} characters.", field.label, max),
                });
            }
        }
        ValidationRule::Pattern(pattern) => {
            if let Ok(re) = Regex::new(pattern) {
                if !re.is_match(value) {
                    errors.push(ValidationError {
                        field_name: field.name.clone(),
                        message: format!("{} does not match the required pattern.", field.label),
                    });
                }
            } else {
                tracing::warn!(
                    "Invalid regex pattern '{}' for field '{}'",
                    pattern,
                    field.name
                );
            }
        }
        ValidationRule::Min(min) => {
            if let Ok(num) = value.parse::<f64>() {
                if num < *min {
                    errors.push(ValidationError {
                        field_name: field.name.clone(),
                        message: format!("{} must be at least {}.", field.label, min),
                    });
                }
            }
        }
        ValidationRule::Max(max) => {
            if let Ok(num) = value.parse::<f64>() {
                if num > *max {
                    errors.push(ValidationError {
                        field_name: field.name.clone(),
                        message: format!("{} must be at most {}.", field.label, max),
                    });
                }
            }
        }
    }
}

/// Check if a string is a valid email address using a regex.
fn is_valid_email(value: &str) -> bool {
    let re = Regex::new(r"^[a-zA-Z0-9._%+\-]+@[a-zA-Z0-9.\-]+\.[a-zA-Z]{2,}$").unwrap();
    re.is_match(value)
}

/// Check if a string looks like a valid URL.
fn is_valid_url(value: &str) -> bool {
    let re = Regex::new(r"^https?://[^\s/$.?#].[^\s]*$").unwrap();
    re.is_match(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::{FormBuilder, FormField};

    fn make_contact_form() -> FormConfig {
        FormBuilder::new("test", "Test")
            .add_field(FieldConfig {
                field_type: FormField::Text,
                name: "name".into(),
                label: "Name".into(),
                required: true,
                placeholder: None,
                default_value: None,
                options: vec![],
                validation_rules: vec![
                    ValidationRule::Required,
                    ValidationRule::MinLength(2),
                    ValidationRule::MaxLength(100),
                ],
            })
            .add_field(FieldConfig {
                field_type: FormField::Email,
                name: "email".into(),
                label: "Email".into(),
                required: true,
                placeholder: None,
                default_value: None,
                options: vec![],
                validation_rules: vec![ValidationRule::Required],
            })
            .add_field(FieldConfig {
                field_type: FormField::Textarea,
                name: "message".into(),
                label: "Message".into(),
                required: false,
                placeholder: None,
                default_value: None,
                options: vec![],
                validation_rules: vec![ValidationRule::MaxLength(5000)],
            })
            .build()
    }

    #[test]
    fn test_valid_submission() {
        let form = make_contact_form();
        let mut data = HashMap::new();
        data.insert("name".into(), "Alice".into());
        data.insert("email".into(), "alice@example.com".into());
        data.insert("message".into(), "Hello!".into());

        assert!(validate_submission(&form, &data).is_ok());
    }

    #[test]
    fn test_missing_required_fields() {
        let form = make_contact_form();
        let data = HashMap::new();

        let err = validate_submission(&form, &data).unwrap_err();
        assert!(err.len() >= 2);
        assert!(err.iter().any(|e| e.field_name == "name"));
        assert!(err.iter().any(|e| e.field_name == "email"));
    }

    #[test]
    fn test_invalid_email() {
        let form = make_contact_form();
        let mut data = HashMap::new();
        data.insert("name".into(), "Alice".into());
        data.insert("email".into(), "not-an-email".into());

        let err = validate_submission(&form, &data).unwrap_err();
        assert_eq!(err.len(), 1);
        assert_eq!(err[0].field_name, "email");
        assert!(err[0].message.contains("email"));
    }

    #[test]
    fn test_min_length_violation() {
        let form = make_contact_form();
        let mut data = HashMap::new();
        data.insert("name".into(), "A".into()); // too short (min 2)
        data.insert("email".into(), "a@b.com".into());

        let err = validate_submission(&form, &data).unwrap_err();
        assert!(err.iter().any(|e| e.field_name == "name" && e.message.contains("at least 2")));
    }

    #[test]
    fn test_max_length_violation() {
        let form = make_contact_form();
        let mut data = HashMap::new();
        data.insert("name".into(), "Alice".into());
        data.insert("email".into(), "a@b.com".into());
        data.insert("message".into(), "x".repeat(5001));

        let err = validate_submission(&form, &data).unwrap_err();
        assert!(err.iter().any(|e| e.field_name == "message"));
    }

    #[test]
    fn test_number_validation() {
        let form = FormBuilder::new("num", "Num")
            .add_field(FieldConfig {
                field_type: FormField::Number,
                name: "age".into(),
                label: "Age".into(),
                required: true,
                placeholder: None,
                default_value: None,
                options: vec![],
                validation_rules: vec![
                    ValidationRule::Required,
                    ValidationRule::Min(0.0),
                    ValidationRule::Max(150.0),
                ],
            })
            .build();

        // Valid
        let mut data = HashMap::new();
        data.insert("age".into(), "25".into());
        assert!(validate_submission(&form, &data).is_ok());

        // Not a number
        data.insert("age".into(), "abc".into());
        let err = validate_submission(&form, &data).unwrap_err();
        assert!(err.iter().any(|e| e.field_name == "age"));

        // Below min
        data.insert("age".into(), "-5".into());
        let err = validate_submission(&form, &data).unwrap_err();
        assert!(err.iter().any(|e| e.message.contains("at least 0")));
    }

    #[test]
    fn test_pattern_validation() {
        let form = FormBuilder::new("pat", "Pat")
            .add_field(FieldConfig {
                field_type: FormField::Text,
                name: "code".into(),
                label: "Code".into(),
                required: true,
                placeholder: None,
                default_value: None,
                options: vec![],
                validation_rules: vec![
                    ValidationRule::Required,
                    ValidationRule::Pattern(r"^[A-Z]{3}-\d{4}$".into()),
                ],
            })
            .build();

        let mut data = HashMap::new();
        data.insert("code".into(), "ABC-1234".into());
        assert!(validate_submission(&form, &data).is_ok());

        data.insert("code".into(), "abc-1234".into());
        assert!(validate_submission(&form, &data).is_err());
    }

    #[test]
    fn test_optional_empty_field_skips_validation() {
        let form = FormBuilder::new("opt", "Opt")
            .add_field(FieldConfig {
                field_type: FormField::Email,
                name: "email".into(),
                label: "Email".into(),
                required: false,
                placeholder: None,
                default_value: None,
                options: vec![],
                validation_rules: vec![],
            })
            .build();

        // Empty optional email should be fine (no type validation on empty)
        let mut data = HashMap::new();
        data.insert("email".into(), "".into());
        assert!(validate_submission(&form, &data).is_ok());
    }
}
