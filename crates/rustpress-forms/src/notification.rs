//! Email notification system for form submissions.
//!
//! Provides a flexible notification pipeline that can format form submissions
//! into email messages, supporting customizable templates, multiple recipients,
//! and auto-reply functionality.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::builder::FormConfig;
use crate::submission::FormSubmission;

/// An email message ready to be sent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailMessage {
    pub to: Vec<String>,
    pub from: String,
    pub reply_to: Option<String>,
    pub subject: String,
    pub body_text: String,
    pub body_html: Option<String>,
    pub headers: HashMap<String, String>,
    pub created_at: DateTime<Utc>,
}

/// Configuration for notification emails.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationConfig {
    /// Recipients of the notification email.
    pub to: Vec<String>,
    /// Sender address.
    pub from: String,
    /// Subject template (supports {field_name} placeholders).
    pub subject_template: String,
    /// Body template (supports {field_name} and {all_fields} placeholders).
    pub body_template: String,
    /// Whether to include all submitted fields in the email body.
    pub include_all_fields: bool,
    /// Whether to send an auto-reply to the submitter.
    pub auto_reply: Option<AutoReplyConfig>,
}

/// Configuration for auto-reply emails sent to the submitter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoReplyConfig {
    /// The field name that contains the submitter's email.
    pub email_field: String,
    /// Subject of the auto-reply.
    pub subject: String,
    /// Body template of the auto-reply.
    pub body_template: String,
    /// Sender address for the auto-reply.
    pub from: String,
}

/// Result of sending a notification.
#[derive(Debug, Clone)]
pub struct NotificationResult {
    pub admin_email: EmailMessage,
    pub auto_reply_email: Option<EmailMessage>,
}

/// Trait for email sending backends.
pub trait EmailSender: Send + Sync {
    fn send(&self, message: &EmailMessage) -> Result<(), String>;
}

/// A no-op email sender for testing / logging only.
pub struct LogEmailSender;

impl EmailSender for LogEmailSender {
    fn send(&self, message: &EmailMessage) -> Result<(), String> {
        tracing::info!(
            to = ?message.to,
            subject = %message.subject,
            "Email would be sent (log-only mode)"
        );
        Ok(())
    }
}

/// Builds and sends notification emails for form submissions.
pub struct NotificationProcessor {
    config: NotificationConfig,
}

impl NotificationProcessor {
    pub fn new(config: NotificationConfig) -> Self {
        Self { config }
    }

    /// Build notification emails for a submission without sending.
    pub fn build_notification(
        &self,
        form: &FormConfig,
        submission: &FormSubmission,
    ) -> NotificationResult {
        let admin_email = self.build_admin_email(form, submission);
        let auto_reply_email = self.build_auto_reply(submission);

        NotificationResult {
            admin_email,
            auto_reply_email,
        }
    }

    /// Build and send notification emails.
    pub fn send_notification(
        &self,
        form: &FormConfig,
        submission: &FormSubmission,
        sender: &dyn EmailSender,
    ) -> Result<NotificationResult, String> {
        let result = self.build_notification(form, submission);

        sender.send(&result.admin_email)?;

        if let Some(ref auto_reply) = result.auto_reply_email {
            // Auto-reply failure is non-fatal
            if let Err(e) = sender.send(auto_reply) {
                tracing::warn!(error = %e, "Failed to send auto-reply email");
            }
        }

        Ok(result)
    }

    fn build_admin_email(&self, form: &FormConfig, submission: &FormSubmission) -> EmailMessage {
        let subject = self.expand_template(&self.config.subject_template, &submission.data);

        let mut body = self.expand_template(&self.config.body_template, &submission.data);

        if self.config.include_all_fields {
            let all_fields = format_all_fields(form, &submission.data);
            body = body.replace("{all_fields}", &all_fields);
        }

        // Append metadata
        body.push_str("\n\n---\n");
        body.push_str(&format!("Form: {} ({})\n", form.title, form.id));
        body.push_str(&format!(
            "Submitted: {}\n",
            submission.submitted_at.format("%Y-%m-%d %H:%M:%S UTC")
        ));
        if let Some(ref ip) = submission.ip_address {
            body.push_str(&format!("IP: {}\n", ip));
        }

        let reply_to = submission
            .data
            .values()
            .find(|v| v.contains('@') && v.contains('.'))
            .cloned();

        EmailMessage {
            to: self.config.to.clone(),
            from: self.config.from.clone(),
            reply_to,
            subject,
            body_text: body.clone(),
            body_html: Some(text_to_html(&body)),
            headers: HashMap::new(),
            created_at: Utc::now(),
        }
    }

    fn build_auto_reply(&self, submission: &FormSubmission) -> Option<EmailMessage> {
        let auto_reply = self.config.auto_reply.as_ref()?;

        let recipient_email = submission.data.get(&auto_reply.email_field)?;

        if recipient_email.is_empty() || !recipient_email.contains('@') {
            return None;
        }

        let body = self.expand_template(&auto_reply.body_template, &submission.data);

        Some(EmailMessage {
            to: vec![recipient_email.clone()],
            from: auto_reply.from.clone(),
            reply_to: None,
            subject: auto_reply.subject.clone(),
            body_text: body.clone(),
            body_html: Some(text_to_html(&body)),
            headers: HashMap::new(),
            created_at: Utc::now(),
        })
    }

    fn expand_template(&self, template: &str, data: &HashMap<String, String>) -> String {
        let mut result = template.to_string();
        for (key, value) in data {
            result = result.replace(&format!("{{{}}}", key), value);
        }
        result
    }
}

/// Format all submitted fields into a readable text block.
fn format_all_fields(form: &FormConfig, data: &HashMap<String, String>) -> String {
    let mut lines = Vec::new();

    // Use form field order
    for field in &form.fields {
        if let Some(value) = data.get(&field.name) {
            if !value.is_empty() {
                lines.push(format!("{}: {}", field.label, value));
            }
        }
    }

    // Include any extra fields not in the form config
    for (key, value) in data {
        let in_config = form.fields.iter().any(|f| f.name == *key);
        if !in_config && !value.is_empty() && !key.starts_with('_') {
            lines.push(format!("{}: {}", key, value));
        }
    }

    lines.join("\n")
}

/// Convert plain text to simple HTML.
fn text_to_html(text: &str) -> String {
    let escaped = text
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");

    let paragraphs: Vec<String> = escaped
        .split("\n\n")
        .map(|p| {
            let lines = p.replace('\n', "<br>\n");
            format!("<p>{}</p>", lines)
        })
        .collect();

    format!(
        "<!DOCTYPE html><html><body style=\"font-family: sans-serif; line-height: 1.6;\">{}</body></html>",
        paragraphs.join("\n")
    )
}

/// Create a default notification config for a form.
pub fn default_notification_config(form: &FormConfig) -> NotificationConfig {
    NotificationConfig {
        to: form
            .email_to
            .as_ref()
            .map(|e| vec![e.clone()])
            .unwrap_or_default(),
        from: "noreply@rustpress.local".into(),
        subject_template: format!("New submission: {}", form.title),
        body_template: "A new form submission has been received.\n\n{all_fields}".into(),
        include_all_fields: true,
        auto_reply: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::{FieldConfig, FormBuilder, FormField};

    fn make_form() -> FormConfig {
        FormBuilder::new("contact", "Contact Us")
            .add_field(FieldConfig {
                field_type: FormField::Text,
                name: "name".into(),
                label: "Name".into(),
                required: true,
                placeholder: None,
                default_value: None,
                options: vec![],
                validation_rules: vec![],
            })
            .add_field(FieldConfig {
                field_type: FormField::Email,
                name: "email".into(),
                label: "Email".into(),
                required: true,
                placeholder: None,
                default_value: None,
                options: vec![],
                validation_rules: vec![],
            })
            .add_field(FieldConfig {
                field_type: FormField::Textarea,
                name: "message".into(),
                label: "Message".into(),
                required: false,
                placeholder: None,
                default_value: None,
                options: vec![],
                validation_rules: vec![],
            })
            .email_to("admin@example.com")
            .build()
    }

    fn make_submission() -> FormSubmission {
        let mut data = HashMap::new();
        data.insert("name".into(), "Alice".into());
        data.insert("email".into(), "alice@example.com".into());
        data.insert("message".into(), "Hello, I have a question.".into());
        FormSubmission::new("contact", data, Some("127.0.0.1".into()), None)
    }

    fn make_config() -> NotificationConfig {
        NotificationConfig {
            to: vec!["admin@example.com".into()],
            from: "noreply@example.com".into(),
            subject_template: "New message from {name}".into(),
            body_template: "You have a new message:\n\n{all_fields}".into(),
            include_all_fields: true,
            auto_reply: Some(AutoReplyConfig {
                email_field: "email".into(),
                subject: "Thank you for contacting us".into(),
                body_template:
                    "Hi {name},\n\nThank you for your message. We will get back to you soon.".into(),
                from: "noreply@example.com".into(),
            }),
        }
    }

    #[test]
    fn test_build_admin_email() {
        let form = make_form();
        let submission = make_submission();
        let processor = NotificationProcessor::new(make_config());

        let result = processor.build_notification(&form, &submission);

        assert_eq!(result.admin_email.to, vec!["admin@example.com"]);
        assert_eq!(result.admin_email.subject, "New message from Alice");
        assert!(result.admin_email.body_text.contains("Name: Alice"));
        assert!(result
            .admin_email
            .body_text
            .contains("Email: alice@example.com"));
        assert!(result
            .admin_email
            .body_text
            .contains("Message: Hello, I have a question."));
        assert!(result.admin_email.body_text.contains("Form: Contact Us"));
        assert!(result.admin_email.body_text.contains("IP: 127.0.0.1"));
        assert!(result.admin_email.body_html.is_some());
    }

    #[test]
    fn test_auto_reply() {
        let form = make_form();
        let submission = make_submission();
        let processor = NotificationProcessor::new(make_config());

        let result = processor.build_notification(&form, &submission);
        let auto_reply = result.auto_reply_email.unwrap();

        assert_eq!(auto_reply.to, vec!["alice@example.com"]);
        assert_eq!(auto_reply.subject, "Thank you for contacting us");
        assert!(auto_reply.body_text.contains("Hi Alice"));
        assert!(auto_reply
            .body_text
            .contains("We will get back to you soon"));
    }

    #[test]
    fn test_no_auto_reply_when_disabled() {
        let form = make_form();
        let submission = make_submission();
        let mut config = make_config();
        config.auto_reply = None;

        let processor = NotificationProcessor::new(config);
        let result = processor.build_notification(&form, &submission);

        assert!(result.auto_reply_email.is_none());
    }

    #[test]
    fn test_no_auto_reply_when_email_missing() {
        let form = make_form();
        let mut data = HashMap::new();
        data.insert("name".into(), "Bob".into());
        // No email field
        let submission = FormSubmission::new("contact", data, None, None);

        let processor = NotificationProcessor::new(make_config());
        let result = processor.build_notification(&form, &submission);

        assert!(result.auto_reply_email.is_none());
    }

    #[test]
    fn test_reply_to_set_from_submission() {
        let form = make_form();
        let submission = make_submission();
        let processor = NotificationProcessor::new(make_config());

        let result = processor.build_notification(&form, &submission);
        assert_eq!(
            result.admin_email.reply_to,
            Some("alice@example.com".into())
        );
    }

    #[test]
    fn test_default_notification_config() {
        let form = make_form();
        let config = default_notification_config(&form);

        assert_eq!(config.to, vec!["admin@example.com"]);
        assert!(config.subject_template.contains("Contact Us"));
        assert!(config.include_all_fields);
        assert!(config.auto_reply.is_none());
    }

    #[test]
    fn test_log_email_sender() {
        let sender = LogEmailSender;
        let msg = EmailMessage {
            to: vec!["test@example.com".into()],
            from: "noreply@example.com".into(),
            reply_to: None,
            subject: "Test".into(),
            body_text: "Hello".into(),
            body_html: None,
            headers: HashMap::new(),
            created_at: Utc::now(),
        };
        assert!(sender.send(&msg).is_ok());
    }

    #[test]
    fn test_send_notification_with_log_sender() {
        let form = make_form();
        let submission = make_submission();
        let processor = NotificationProcessor::new(make_config());
        let sender = LogEmailSender;

        let result = processor.send_notification(&form, &submission, &sender);
        assert!(result.is_ok());
    }

    #[test]
    fn test_text_to_html() {
        let html = text_to_html("Hello\nWorld\n\nNew paragraph");
        assert!(html.contains("<p>Hello<br>"));
        assert!(html.contains("<p>New paragraph</p>"));
        assert!(html.contains("<!DOCTYPE html>"));
    }

    #[test]
    fn test_format_all_fields() {
        let form = make_form();
        let mut data = HashMap::new();
        data.insert("name".into(), "Alice".into());
        data.insert("email".into(), "a@b.com".into());
        data.insert("message".into(), "Hi".into());

        let formatted = format_all_fields(&form, &data);
        assert!(formatted.contains("Name: Alice"));
        assert!(formatted.contains("Email: a@b.com"));
        assert!(formatted.contains("Message: Hi"));
    }
}
