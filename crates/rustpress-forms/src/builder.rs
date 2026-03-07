//! Form builder API for declaratively constructing form configurations.

use serde::{Deserialize, Serialize};

use crate::validation::ValidationRule;

/// The type of a form field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FormField {
    Text,
    Email,
    Textarea,
    Select,
    Checkbox,
    Radio,
    Hidden,
    File,
    Number,
    Date,
    Phone,
    Url,
}

impl FormField {
    /// Returns the HTML input type attribute value for this field type.
    pub fn html_type(&self) -> &'static str {
        match self {
            FormField::Text => "text",
            FormField::Email => "email",
            FormField::Textarea => "textarea",
            FormField::Select => "select",
            FormField::Checkbox => "checkbox",
            FormField::Radio => "radio",
            FormField::Hidden => "hidden",
            FormField::File => "file",
            FormField::Number => "number",
            FormField::Date => "date",
            FormField::Phone => "tel",
            FormField::Url => "url",
        }
    }
}

/// Configuration for a single form field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldConfig {
    /// The field type.
    pub field_type: FormField,
    /// The HTML name attribute.
    pub name: String,
    /// The human-readable label.
    pub label: String,
    /// Whether the field is required.
    pub required: bool,
    /// Placeholder text.
    pub placeholder: Option<String>,
    /// Default value.
    pub default_value: Option<String>,
    /// Options for select/radio/checkbox fields.
    pub options: Vec<String>,
    /// Validation rules applied to this field.
    pub validation_rules: Vec<ValidationRule>,
}

/// Complete form configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormConfig {
    /// Unique form identifier.
    pub id: String,
    /// Form title.
    pub title: String,
    /// Ordered list of fields.
    pub fields: Vec<FieldConfig>,
    /// Text for the submit button.
    pub submit_label: String,
    /// Message displayed on successful submission.
    pub success_message: String,
    /// Message displayed when submission fails validation.
    pub error_message: String,
    /// Email address to send submissions to (optional).
    pub email_to: Option<String>,
}

/// Builder for constructing `FormConfig` instances.
pub struct FormBuilder {
    id: String,
    title: String,
    fields: Vec<FieldConfig>,
    submit_label: String,
    success_message: String,
    error_message: String,
    email_to: Option<String>,
}

impl FormBuilder {
    /// Create a new form builder with the given ID and title.
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            fields: Vec::new(),
            submit_label: "Submit".to_string(),
            success_message: "Thank you for your submission.".to_string(),
            error_message:
                "There were errors in your submission. Please correct them and try again."
                    .to_string(),
            email_to: None,
        }
    }

    /// Add a field to the form.
    pub fn add_field(mut self, config: FieldConfig) -> Self {
        self.fields.push(config);
        self
    }

    /// Set the submit button label.
    pub fn submit_label(mut self, label: impl Into<String>) -> Self {
        self.submit_label = label.into();
        self
    }

    /// Set the success message.
    pub fn success_message(mut self, msg: impl Into<String>) -> Self {
        self.success_message = msg.into();
        self
    }

    /// Set the error message.
    pub fn error_message(mut self, msg: impl Into<String>) -> Self {
        self.error_message = msg.into();
        self
    }

    /// Set the email recipient for submissions.
    pub fn email_to(mut self, email: impl Into<String>) -> Self {
        self.email_to = Some(email.into());
        self
    }

    /// Build the final `FormConfig`.
    pub fn build(self) -> FormConfig {
        FormConfig {
            id: self.id,
            title: self.title,
            fields: self.fields,
            submit_label: self.submit_label,
            success_message: self.success_message,
            error_message: self.error_message,
            email_to: self.email_to,
        }
    }
}

/// Helper to create a `FieldConfig` with minimal boilerplate.
pub fn field(
    field_type: FormField,
    name: impl Into<String>,
    label: impl Into<String>,
) -> FieldConfig {
    FieldConfig {
        field_type,
        name: name.into(),
        label: label.into(),
        required: false,
        placeholder: None,
        default_value: None,
        options: Vec::new(),
        validation_rules: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_form_builder_basic() {
        let form = FormBuilder::new("contact", "Contact Us")
            .add_field(FieldConfig {
                field_type: FormField::Text,
                name: "name".into(),
                label: "Your Name".into(),
                required: true,
                placeholder: Some("John Doe".into()),
                default_value: None,
                options: vec![],
                validation_rules: vec![ValidationRule::Required],
            })
            .submit_label("Send Message")
            .build();

        assert_eq!(form.id, "contact");
        assert_eq!(form.title, "Contact Us");
        assert_eq!(form.fields.len(), 1);
        assert_eq!(form.submit_label, "Send Message");
        assert_eq!(form.fields[0].name, "name");
        assert!(form.fields[0].required);
    }

    #[test]
    fn test_form_builder_defaults() {
        let form = FormBuilder::new("test", "Test Form").build();

        assert_eq!(form.submit_label, "Submit");
        assert!(form.success_message.contains("Thank you"));
        assert!(form.error_message.contains("errors"));
        assert!(form.email_to.is_none());
        assert!(form.fields.is_empty());
    }

    #[test]
    fn test_form_builder_with_email() {
        let form = FormBuilder::new("feedback", "Feedback")
            .email_to("admin@example.com")
            .success_message("Thanks!")
            .error_message("Oops!")
            .build();

        assert_eq!(form.email_to, Some("admin@example.com".to_string()));
        assert_eq!(form.success_message, "Thanks!");
        assert_eq!(form.error_message, "Oops!");
    }

    #[test]
    fn test_field_helper() {
        let f = field(FormField::Email, "email", "Email Address");
        assert_eq!(f.field_type, FormField::Email);
        assert_eq!(f.name, "email");
        assert_eq!(f.label, "Email Address");
        assert!(!f.required);
        assert!(f.placeholder.is_none());
        assert!(f.options.is_empty());
    }

    #[test]
    fn test_form_field_html_type() {
        assert_eq!(FormField::Text.html_type(), "text");
        assert_eq!(FormField::Email.html_type(), "email");
        assert_eq!(FormField::Phone.html_type(), "tel");
        assert_eq!(FormField::Url.html_type(), "url");
        assert_eq!(FormField::Textarea.html_type(), "textarea");
        assert_eq!(FormField::Select.html_type(), "select");
        assert_eq!(FormField::Number.html_type(), "number");
    }
}
