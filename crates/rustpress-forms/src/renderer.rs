//! HTML form rendering.

use crate::builder::{FieldConfig, FormConfig, FormField};
use rustpress_core::esc_attr;

/// Render a complete HTML form from a `FormConfig`.
///
/// The generated form includes:
/// - A CSRF token hidden field (`_csrf_token`)
/// - A hidden field for the form ID (`_form_id`)
/// - All configured fields with labels, placeholders, and validation attributes
/// - A submit button
pub fn render_form(config: &FormConfig, action_url: &str) -> String {
    let mut html = String::with_capacity(2048);

    html.push_str(&format!(
        "<form id=\"rustpress-form-{}\" class=\"rustpress-form\" method=\"post\" action=\"{}\" enctype=\"multipart/form-data\">\n",
        esc_attr(&config.id),
        esc_attr(action_url),
    ));

    // Hidden fields
    html.push_str(&format!(
        "  <input type=\"hidden\" name=\"_form_id\" value=\"{}\" />\n",
        esc_attr(&config.id),
    ));
    html.push_str("  <input type=\"hidden\" name=\"_csrf_token\" value=\"\" />\n");

    // Title
    html.push_str(&format!(
        "  <h3 class=\"rustpress-form-title\">{}</h3>\n",
        esc_attr(&config.title),
    ));

    // Fields
    for field in &config.fields {
        html.push_str("  <div class=\"rustpress-form-field\">\n");
        html.push_str(&render_field(field));
        html.push_str("  </div>\n");
    }

    // Submit button
    html.push_str(&format!(
        "  <div class=\"rustpress-form-submit\">\n    <button type=\"submit\">{}</button>\n  </div>\n",
        esc_attr(&config.submit_label),
    ));

    html.push_str("</form>\n");
    html
}

/// Render a single form field as HTML, including its label.
pub fn render_field(field: &FieldConfig) -> String {
    let mut html = String::with_capacity(512);

    let id = format!("field-{}", &field.name);
    let required_attr = if field.required { " required" } else { "" };
    let required_marker = if field.required { " <span class=\"required\">*</span>" } else { "" };

    // Label (skip for hidden fields)
    if field.field_type != FormField::Hidden {
        html.push_str(&format!(
            "    <label for=\"{}\">{}{}</label>\n",
            esc_attr(&id),
            esc_attr(&field.label),
            required_marker,
        ));
    }

    let placeholder_attr = field
        .placeholder
        .as_ref()
        .map(|p| format!(" placeholder=\"{}\"", esc_attr(p)))
        .unwrap_or_default();

    let default_val = field.default_value.as_deref().unwrap_or("");

    match field.field_type {
        FormField::Textarea => {
            html.push_str(&format!(
                "    <textarea id=\"{}\" name=\"{}\"{}{}>{}</textarea>\n",
                esc_attr(&id),
                esc_attr(&field.name),
                placeholder_attr,
                required_attr,
                esc_attr(default_val),
            ));
        }
        FormField::Select => {
            html.push_str(&format!(
                "    <select id=\"{}\" name=\"{}\"{}>\n",
                esc_attr(&id),
                esc_attr(&field.name),
                required_attr,
            ));
            html.push_str("      <option value=\"\">-- Select --</option>\n");
            for opt in &field.options {
                let selected = if Some(opt.as_str()) == field.default_value.as_deref() {
                    " selected"
                } else {
                    ""
                };
                html.push_str(&format!(
                    "      <option value=\"{}\"{}>{}</option>\n",
                    esc_attr(opt),
                    selected,
                    esc_attr(opt),
                ));
            }
            html.push_str("    </select>\n");
        }
        FormField::Radio => {
            for (i, opt) in field.options.iter().enumerate() {
                let radio_id = format!("{}-{}", id, i);
                let checked = if Some(opt.as_str()) == field.default_value.as_deref() {
                    " checked"
                } else {
                    ""
                };
                html.push_str(&format!(
                    "    <label class=\"radio-label\"><input type=\"radio\" id=\"{}\" name=\"{}\" value=\"{}\"{}{}> {}</label>\n",
                    esc_attr(&radio_id),
                    esc_attr(&field.name),
                    esc_attr(opt),
                    checked,
                    required_attr,
                    esc_attr(opt),
                ));
            }
        }
        FormField::Checkbox => {
            if field.options.is_empty() {
                // Single checkbox
                let checked = if default_val == "1" || default_val == "true" {
                    " checked"
                } else {
                    ""
                };
                html.push_str(&format!(
                    "    <input type=\"checkbox\" id=\"{}\" name=\"{}\" value=\"1\"{}{}>\n",
                    esc_attr(&id),
                    esc_attr(&field.name),
                    checked,
                    required_attr,
                ));
            } else {
                // Multiple checkboxes
                for (i, opt) in field.options.iter().enumerate() {
                    let cb_id = format!("{}-{}", id, i);
                    html.push_str(&format!(
                        "    <label class=\"checkbox-label\"><input type=\"checkbox\" id=\"{}\" name=\"{}[]\" value=\"{}\"> {}</label>\n",
                        esc_attr(&cb_id),
                        esc_attr(&field.name),
                        esc_attr(opt),
                        esc_attr(opt),
                    ));
                }
            }
        }
        _ => {
            // Standard input types: text, email, hidden, file, number, date, phone, url
            let input_type = field.field_type.html_type();
            let value_attr = if !default_val.is_empty() {
                format!(" value=\"{}\"", esc_attr(default_val))
            } else {
                String::new()
            };

            let mut extra_attrs = String::new();

            // Add pattern attribute for phone fields
            if field.field_type == FormField::Phone {
                extra_attrs.push_str(" pattern=\"[0-9+\\-\\s()]+\"");
            }

            // Add validation-derived attributes
            for rule in &field.validation_rules {
                match rule {
                    crate::validation::ValidationRule::MinLength(n) => {
                        extra_attrs.push_str(&format!(" minlength=\"{}\"", n));
                    }
                    crate::validation::ValidationRule::MaxLength(n) => {
                        extra_attrs.push_str(&format!(" maxlength=\"{}\"", n));
                    }
                    crate::validation::ValidationRule::Min(n) => {
                        extra_attrs.push_str(&format!(" min=\"{}\"", n));
                    }
                    crate::validation::ValidationRule::Max(n) => {
                        extra_attrs.push_str(&format!(" max=\"{}\"", n));
                    }
                    crate::validation::ValidationRule::Pattern(p) => {
                        extra_attrs.push_str(&format!(" pattern=\"{}\"", esc_attr(p)));
                    }
                    _ => {}
                }
            }

            html.push_str(&format!(
                "    <input type=\"{}\" id=\"{}\" name=\"{}\"{}{}{}{}>\n",
                input_type,
                esc_attr(&id),
                esc_attr(&field.name),
                value_attr,
                placeholder_attr,
                required_attr,
                extra_attrs,
            ));
        }
    }

    html
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::{FieldConfig, FormBuilder, FormField};
    use crate::validation::ValidationRule;

    #[test]
    fn test_render_form_basic_structure() {
        let form = FormBuilder::new("contact", "Contact Us")
            .add_field(FieldConfig {
                field_type: FormField::Text,
                name: "name".into(),
                label: "Name".into(),
                required: true,
                placeholder: Some("Your name".into()),
                default_value: None,
                options: vec![],
                validation_rules: vec![],
            })
            .submit_label("Send")
            .build();

        let html = render_form(&form, "/submit");
        assert!(html.contains("action=\"/submit\""));
        assert!(html.contains("name=\"_csrf_token\""));
        assert!(html.contains("name=\"_form_id\""));
        assert!(html.contains("value=\"contact\""));
        assert!(html.contains("Contact Us"));
        assert!(html.contains("type=\"submit\""));
        assert!(html.contains("Send"));
    }

    #[test]
    fn test_render_text_field_with_attributes() {
        let field_cfg = FieldConfig {
            field_type: FormField::Text,
            name: "username".into(),
            label: "Username".into(),
            required: true,
            placeholder: Some("Enter username".into()),
            default_value: None,
            options: vec![],
            validation_rules: vec![
                ValidationRule::MinLength(3),
                ValidationRule::MaxLength(20),
            ],
        };

        let html = render_field(&field_cfg);
        assert!(html.contains("type=\"text\""));
        assert!(html.contains("name=\"username\""));
        assert!(html.contains("required"));
        assert!(html.contains("placeholder=\"Enter username\""));
        assert!(html.contains("minlength=\"3\""));
        assert!(html.contains("maxlength=\"20\""));
        assert!(html.contains("<span class=\"required\">*</span>"));
    }

    #[test]
    fn test_render_select_field() {
        let field_cfg = FieldConfig {
            field_type: FormField::Select,
            name: "country".into(),
            label: "Country".into(),
            required: false,
            placeholder: None,
            default_value: Some("US".into()),
            options: vec!["US".into(), "UK".into(), "JP".into()],
            validation_rules: vec![],
        };

        let html = render_field(&field_cfg);
        assert!(html.contains("<select"));
        assert!(html.contains("name=\"country\""));
        assert!(html.contains("-- Select --"));
        assert!(html.contains("value=\"US\" selected"));
        assert!(html.contains("value=\"UK\""));
        assert!(html.contains("value=\"JP\""));
    }

    #[test]
    fn test_render_textarea() {
        let field_cfg = FieldConfig {
            field_type: FormField::Textarea,
            name: "message".into(),
            label: "Message".into(),
            required: true,
            placeholder: Some("Write here...".into()),
            default_value: Some("Hello".into()),
            options: vec![],
            validation_rules: vec![],
        };

        let html = render_field(&field_cfg);
        assert!(html.contains("<textarea"));
        assert!(html.contains("name=\"message\""));
        assert!(html.contains("required"));
        assert!(html.contains("placeholder=\"Write here...\""));
        assert!(html.contains(">Hello</textarea>"));
    }

    #[test]
    fn test_render_radio_field() {
        let field_cfg = FieldConfig {
            field_type: FormField::Radio,
            name: "color".into(),
            label: "Favorite Color".into(),
            required: false,
            placeholder: None,
            default_value: Some("Blue".into()),
            options: vec!["Red".into(), "Blue".into(), "Green".into()],
            validation_rules: vec![],
        };

        let html = render_field(&field_cfg);
        assert!(html.contains("type=\"radio\""));
        assert!(html.contains("name=\"color\""));
        assert!(html.contains("value=\"Blue\" checked"));
        assert!(html.contains("value=\"Red\""));
        assert!(!html.contains("value=\"Red\" checked"));
    }

    #[test]
    fn test_render_hidden_field_no_label() {
        let field_cfg = FieldConfig {
            field_type: FormField::Hidden,
            name: "ref_id".into(),
            label: "Reference".into(),
            required: false,
            placeholder: None,
            default_value: Some("abc123".into()),
            options: vec![],
            validation_rules: vec![],
        };

        let html = render_field(&field_cfg);
        assert!(html.contains("type=\"hidden\""));
        assert!(html.contains("value=\"abc123\""));
        // Hidden fields should not have a visible label
        assert!(!html.contains("<label"));
    }
}
