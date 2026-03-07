//! Contact Form 7 compatibility layer.
//!
//! Maps RustPress form data to/from CF7's storage format in wp_posts,
//! enabling seamless migration from WordPress + Contact Form 7 to RustPress.
//!
//! ## CF7 Storage Format
//!
//! CF7 stores forms as `post_type = 'wpcf7_contact_form'` in wp_posts:
//! - `post_title`   — Form title
//! - `post_content` — Form template (CF7 shortcode-like markup)
//!
//! Additional settings are stored in wp_postmeta:
//! - `_form`     — The form body template (HTML with [field] tags)
//! - `_mail`     — PHP serialized mail settings (to, from, subject, body)
//! - `_mail_2`   — Auto-reply mail settings (optional)
//! - `_messages` — Validation/success/error messages
//! - `_additional_settings` — Line-separated key:value pairs
//!
//! ## CF7 Tag Syntax
//!
//! CF7 uses bracket tags in the form template:
//! - `[text* your-name]`     — required text field
//! - `[email* your-email]`   — required email field
//! - `[textarea your-message]` — optional textarea
//! - `[submit "Send"]`       — submit button
//!
//! The `*` suffix marks a field as required.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::builder::{FieldConfig, FormBuilder, FormConfig, FormField};
use crate::notification::NotificationConfig;

/// CF7 post type constant.
pub const POST_TYPE: &str = "wpcf7_contact_form";

/// CF7 meta keys.
pub mod meta_keys {
    pub const FORM: &str = "_form";
    pub const MAIL: &str = "_mail";
    pub const MAIL_2: &str = "_mail_2";
    pub const MESSAGES: &str = "_messages";
    pub const ADDITIONAL_SETTINGS: &str = "_additional_settings";
}

/// A parsed CF7 tag from the form template.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cf7Tag {
    pub tag_type: String,
    pub name: String,
    pub required: bool,
    pub options: Vec<String>,
    pub default_value: Option<String>,
}

/// CF7-compatible form data, read from wp_posts + wp_postmeta.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cf7FormData {
    pub post_id: u64,
    pub title: String,
    pub form_template: String,
    pub mail_to: String,
    pub mail_from: String,
    pub mail_subject: String,
    pub mail_body: String,
    pub mail_2_active: bool,
    pub mail_2_to: String,
    pub mail_2_subject: String,
    pub mail_2_body: String,
    pub messages: HashMap<String, String>,
}

impl Cf7FormData {
    /// Parse CF7 form data from wp_posts fields and wp_postmeta.
    pub fn from_post_and_meta(
        post_id: u64,
        post_title: &str,
        meta: &HashMap<String, String>,
    ) -> Self {
        let form_template = meta.get(meta_keys::FORM).cloned().unwrap_or_default();

        let (mail_to, mail_from, mail_subject, mail_body) =
            parse_cf7_mail_meta(meta.get(meta_keys::MAIL));

        let (mail_2_to, _mail_2_from, mail_2_subject, mail_2_body) =
            parse_cf7_mail_meta(meta.get(meta_keys::MAIL_2));

        let mail_2_active = meta.get(meta_keys::MAIL_2).is_some_and(|v| !v.is_empty());

        let messages = parse_cf7_messages(meta.get(meta_keys::MESSAGES));

        Self {
            post_id,
            title: post_title.to_string(),
            form_template,
            mail_to,
            mail_from,
            mail_subject,
            mail_body,
            mail_2_active,
            mail_2_to,
            mail_2_subject,
            mail_2_body,
            messages,
        }
    }

    /// Parse CF7 bracket tags from the form template.
    pub fn parse_tags(&self) -> Vec<Cf7Tag> {
        parse_cf7_tags(&self.form_template)
    }

    /// Convert to a RustPress FormConfig.
    pub fn to_form_config(&self) -> FormConfig {
        let tags = self.parse_tags();
        let form_id = format!("cf7_{}", self.post_id);

        let mut builder = FormBuilder::new(&form_id, &self.title);

        if !self.mail_to.is_empty() {
            builder = builder.email_to(&self.mail_to);
        }

        for tag in &tags {
            if tag.tag_type == "submit" {
                continue;
            }

            let field_type = cf7_type_to_form_field(&tag.tag_type);

            builder = builder.add_field(FieldConfig {
                field_type,
                name: tag.name.clone(),
                label: humanize_field_name(&tag.name),
                required: tag.required,
                placeholder: None,
                default_value: tag.default_value.clone(),
                options: tag.options.clone(),
                validation_rules: vec![],
            });
        }

        // Set success/error messages from CF7 messages
        if let Some(msg) = self.messages.get("mail_sent_ok") {
            builder = builder.success_message(msg);
        }
        if let Some(msg) = self.messages.get("validation_error") {
            builder = builder.error_message(msg);
        }

        builder.build()
    }

    /// Convert to a RustPress NotificationConfig.
    pub fn to_notification_config(&self) -> NotificationConfig {
        let auto_reply = if self.mail_2_active && !self.mail_2_to.is_empty() {
            // CF7 mail_2 "to" field typically uses [your-email] tag
            let email_field = extract_tag_name(&self.mail_2_to).unwrap_or("email".to_string());

            Some(crate::notification::AutoReplyConfig {
                email_field,
                subject: self.mail_2_subject.clone(),
                body_template: cf7_body_to_template(&self.mail_2_body),
                from: self.mail_from.clone(),
            })
        } else {
            None
        };

        NotificationConfig {
            to: if self.mail_to.is_empty() {
                vec![]
            } else {
                vec![self.mail_to.clone()]
            },
            from: if self.mail_from.is_empty() {
                "noreply@rustpress.local".to_string()
            } else {
                self.mail_from.clone()
            },
            subject_template: cf7_body_to_template(&self.mail_subject),
            body_template: cf7_body_to_template(&self.mail_body),
            include_all_fields: false,
            auto_reply,
        }
    }
}

/// Parse CF7 bracket tags from a form template string.
///
/// Handles tags like:
/// - `[text* your-name]`
/// - `[email* your-email placeholder "Your email"]`
/// - `[textarea your-message]`
/// - `[select menu-item "Option 1" "Option 2"]`
/// - `[submit "Send"]`
fn parse_cf7_tags(template: &str) -> Vec<Cf7Tag> {
    let mut tags = Vec::new();
    let mut pos = 0;
    let bytes = template.as_bytes();

    while pos < bytes.len() {
        if bytes[pos] == b'[' {
            if let Some(end) = template[pos..].find(']') {
                let tag_content = &template[pos + 1..pos + end];
                if let Some(tag) = parse_single_tag(tag_content) {
                    tags.push(tag);
                }
                pos += end + 1;
            } else {
                pos += 1;
            }
        } else {
            pos += 1;
        }
    }

    tags
}

fn parse_single_tag(content: &str) -> Option<Cf7Tag> {
    let content = content.trim();
    if content.is_empty() {
        return None;
    }

    let mut parts = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;

    for ch in content.chars() {
        match ch {
            '"' => {
                in_quotes = !in_quotes;
                if !in_quotes && !current.is_empty() {
                    parts.push(current.clone());
                    current.clear();
                }
            }
            ' ' if !in_quotes => {
                if !current.is_empty() {
                    parts.push(current.clone());
                    current.clear();
                }
            }
            _ => current.push(ch),
        }
    }
    if !current.is_empty() {
        parts.push(current);
    }

    if parts.is_empty() {
        return None;
    }

    let mut tag_type = parts[0].clone();
    let required = tag_type.ends_with('*');
    if required {
        tag_type = tag_type.trim_end_matches('*').to_string();
    }

    // For submit buttons, name is empty
    if tag_type == "submit" {
        return Some(Cf7Tag {
            tag_type,
            name: String::new(),
            required: false,
            options: parts[1..].to_vec(),
            default_value: None,
        });
    }

    let name = parts.get(1).cloned().unwrap_or_default();

    // Remaining parts are options (for select/radio) or attributes
    let options: Vec<String> = parts[2..].to_vec();

    Some(Cf7Tag {
        tag_type,
        name,
        required,
        options,
        default_value: None,
    })
}

/// Convert CF7 field type to RustPress FormField.
fn cf7_type_to_form_field(cf7_type: &str) -> FormField {
    match cf7_type {
        "text" => FormField::Text,
        "email" => FormField::Email,
        "textarea" => FormField::Textarea,
        "select" => FormField::Select,
        "checkbox" => FormField::Checkbox,
        "radio" => FormField::Radio,
        "file" => FormField::File,
        "number" => FormField::Number,
        "date" => FormField::Date,
        "tel" => FormField::Phone,
        "url" => FormField::Url,
        "hidden" => FormField::Hidden,
        _ => FormField::Text,
    }
}

/// Convert a CF7 field name like "your-name" to a human-readable label.
fn humanize_field_name(name: &str) -> String {
    name.replace(['-', '_'], " ")
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(c) => format!("{}{}", c.to_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Convert CF7 mail body (with [tag] references) to RustPress template format ({tag}).
fn cf7_body_to_template(body: &str) -> String {
    let mut result = String::new();
    let mut pos = 0;
    let bytes = body.as_bytes();

    while pos < bytes.len() {
        if bytes[pos] == b'[' {
            if let Some(end) = body[pos..].find(']') {
                let tag_name = &body[pos + 1..pos + end];
                // Convert [your-name] → {your-name}
                result.push('{');
                result.push_str(tag_name);
                result.push('}');
                pos += end + 1;
            } else {
                result.push('[');
                pos += 1;
            }
        } else {
            result.push(body[pos..].chars().next().unwrap());
            pos += body[pos..].chars().next().unwrap().len_utf8();
        }
    }

    result
}

/// Extract a tag name from a CF7 mail field like `[your-email]`.
fn extract_tag_name(field: &str) -> Option<String> {
    let start = field.find('[')?;
    let end = field[start..].find(']')?;
    Some(field[start + 1..start + end].to_string())
}

/// Parse CF7 mail meta (simplified — handles basic key:value format).
///
/// CF7 stores mail settings as PHP serialized arrays. This function
/// provides a simplified parser that handles the most common format.
fn parse_cf7_mail_meta(raw: Option<&String>) -> (String, String, String, String) {
    let raw = match raw {
        Some(v) if !v.is_empty() => v,
        _ => return (String::new(), String::new(), String::new(), String::new()),
    };

    // Try to extract key fields from a simplified serialized format.
    // In practice, the SeaORM layer would deserialize the PHP serialized array.
    // Here we support a simple "key: value\n" format for testing.
    let mut to = String::new();
    let mut from = String::new();
    let mut subject = String::new();
    let mut body = String::new();

    for line in raw.lines() {
        if let Some(rest) = line.strip_prefix("to: ") {
            to = rest.trim().to_string();
        } else if let Some(rest) = line.strip_prefix("from: ") {
            from = rest.trim().to_string();
        } else if let Some(rest) = line.strip_prefix("subject: ") {
            subject = rest.trim().to_string();
        } else if let Some(rest) = line.strip_prefix("body: ") {
            body = rest.trim().to_string();
        }
    }

    (to, from, subject, body)
}

/// Parse CF7 messages meta.
fn parse_cf7_messages(raw: Option<&String>) -> HashMap<String, String> {
    let mut messages = HashMap::new();

    let raw = match raw {
        Some(v) if !v.is_empty() => v,
        _ => return messages,
    };

    for line in raw.lines() {
        if let Some((key, value)) = line.split_once(": ") {
            messages.insert(key.trim().to_string(), value.trim().to_string());
        }
    }

    messages
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_cf7_template() -> &'static str {
        r#"<label>Your Name
[text* your-name]</label>

<label>Your Email
[email* your-email]</label>

<label>Subject
[text your-subject]</label>

<label>Your Message
[textarea your-message]</label>

[submit "Send"]"#
    }

    fn sample_meta() -> HashMap<String, String> {
        let mut m = HashMap::new();
        m.insert(meta_keys::FORM.into(), sample_cf7_template().into());
        m.insert(
            meta_keys::MAIL.into(),
            "to: admin@example.com\nfrom: noreply@example.com\nsubject: Contact: [your-subject]\nbody: From: [your-name] <[your-email]>\n\n[your-message]".into(),
        );
        m.insert(
            meta_keys::MESSAGES.into(),
            "mail_sent_ok: Thank you for your message.\nvalidation_error: Please fill in all required fields.".into(),
        );
        m
    }

    #[test]
    fn test_parse_cf7_tags() {
        let tags = parse_cf7_tags(sample_cf7_template());

        assert_eq!(tags.len(), 5); // 4 fields + 1 submit

        assert_eq!(tags[0].tag_type, "text");
        assert_eq!(tags[0].name, "your-name");
        assert!(tags[0].required);

        assert_eq!(tags[1].tag_type, "email");
        assert_eq!(tags[1].name, "your-email");
        assert!(tags[1].required);

        assert_eq!(tags[2].tag_type, "text");
        assert_eq!(tags[2].name, "your-subject");
        assert!(!tags[2].required);

        assert_eq!(tags[3].tag_type, "textarea");
        assert_eq!(tags[3].name, "your-message");
        assert!(!tags[3].required);

        assert_eq!(tags[4].tag_type, "submit");
    }

    #[test]
    fn test_cf7_to_form_config() {
        let cf7 = Cf7FormData::from_post_and_meta(1, "Contact Form", &sample_meta());
        let form = cf7.to_form_config();

        assert_eq!(form.id, "cf7_1");
        assert_eq!(form.title, "Contact Form");
        assert_eq!(form.fields.len(), 4); // submit excluded

        assert_eq!(form.fields[0].name, "your-name");
        assert_eq!(form.fields[0].field_type, FormField::Text);
        assert!(form.fields[0].required);
        assert_eq!(form.fields[0].label, "Your Name");

        assert_eq!(form.fields[1].name, "your-email");
        assert_eq!(form.fields[1].field_type, FormField::Email);
        assert!(form.fields[1].required);

        assert_eq!(form.success_message, "Thank you for your message.");
        assert_eq!(form.error_message, "Please fill in all required fields.");
    }

    #[test]
    fn test_cf7_to_notification_config() {
        let cf7 = Cf7FormData::from_post_and_meta(1, "Contact", &sample_meta());
        let config = cf7.to_notification_config();

        assert_eq!(config.to, vec!["admin@example.com"]);
        assert_eq!(config.from, "noreply@example.com");
        // [your-subject] → {your-subject}
        assert!(config.subject_template.contains("{your-subject}"));
    }

    #[test]
    fn test_cf7_body_to_template() {
        let result = cf7_body_to_template("Hello [your-name], your email is [your-email].");
        assert_eq!(result, "Hello {your-name}, your email is {your-email}.");
    }

    #[test]
    fn test_humanize_field_name() {
        assert_eq!(humanize_field_name("your-name"), "Your Name");
        assert_eq!(humanize_field_name("your_email"), "Your Email");
        assert_eq!(humanize_field_name("message"), "Message");
    }

    #[test]
    fn test_extract_tag_name() {
        assert_eq!(extract_tag_name("[your-email]"), Some("your-email".into()));
        assert_eq!(extract_tag_name("plain text"), None);
        assert_eq!(
            extract_tag_name("Send to [admin-email]"),
            Some("admin-email".into())
        );
    }

    #[test]
    fn test_parse_cf7_tags_select() {
        let template = r#"[select menu "Option 1" "Option 2" "Option 3"]"#;
        let tags = parse_cf7_tags(template);

        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].tag_type, "select");
        assert_eq!(tags[0].name, "menu");
        assert_eq!(tags[0].options, vec!["Option 1", "Option 2", "Option 3"]);
    }

    #[test]
    fn test_cf7_type_to_form_field() {
        assert_eq!(cf7_type_to_form_field("text"), FormField::Text);
        assert_eq!(cf7_type_to_form_field("email"), FormField::Email);
        assert_eq!(cf7_type_to_form_field("textarea"), FormField::Textarea);
        assert_eq!(cf7_type_to_form_field("select"), FormField::Select);
        assert_eq!(cf7_type_to_form_field("tel"), FormField::Phone);
        assert_eq!(cf7_type_to_form_field("unknown"), FormField::Text);
    }

    #[test]
    fn test_empty_meta() {
        let cf7 = Cf7FormData::from_post_and_meta(1, "Empty", &HashMap::new());
        assert!(cf7.form_template.is_empty());
        assert!(cf7.mail_to.is_empty());

        let form = cf7.to_form_config();
        assert!(form.fields.is_empty());
    }

    #[test]
    fn test_post_type_constant() {
        assert_eq!(POST_TYPE, "wpcf7_contact_form");
    }
}
